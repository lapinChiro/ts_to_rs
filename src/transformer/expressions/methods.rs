//! Method call mapping from TypeScript to Rust equivalents.

use crate::ir::fold::{walk_expr, IrFolder};
use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType};
use std::collections::HashSet;

/// Methods where `map_method_call` transforms to Rust APIs that expect `&str` / `impl Pattern`.
///
/// For these methods, TypeResolver's expected type `RustType::String` (from TS `string` param)
/// would cause `.to_string()` to be added to string literal arguments, producing `String`
/// instead of `&str`. This conflicts with Rust's `Pattern` trait requirements (stable Rust
/// does not implement `Pattern` for `String`).
///
/// When the method name is in this list, `convert_call_args_with_types` suppresses
/// the `.to_string()` coercion on string literal arguments.
/// Methods where `map_method_call` transforms to Rust APIs that expect `&str` / `impl Pattern`,
/// regardless of the object type. For these, string literal `.to_string()` is always suppressed.
pub(super) const PATTERN_ARG_METHODS: &[&str] = &[
    "includes",
    "startsWith",
    "endsWith",
    "split",
    "replace",
    "replaceAll",
    "join",
];

/// Methods that only need `.to_string()` suppression when called on a Regex object.
/// (e.g., `regex.test("str")` → `Regex::is_match(&"str")`)
/// Non-Regex calls to `test`/`exec` should NOT suppress `.to_string()`.
pub(super) const REGEX_PATTERN_ARG_METHODS: &[&str] = &["test", "exec"];

/// Method names whose calls are **unconditionally** intercepted and rewritten
/// by [`map_method_call`] — arms without a `match` guard.
///
/// **Single source of truth**. This array MUST list exactly the method names
/// that [`map_method_call`] handles with **unguarded** named match arms. The
/// unit test [`tests::test_remapped_methods_match_map_method_call_arms`]
/// enforces the invariant bidirectionally — the build fails if either side
/// drifts.
///
/// These names dispatch to Rust APIs with different signatures (different arity,
/// different types) than the TypeScript counterparts. Because the rewrite replaces
/// the callee entirely, the TS signature's parameter types (e.g., `Option<T>` for
/// optional params, trailing `thisArg`) must NOT drive either:
///
/// - expected-type propagation to arguments (would produce spurious `Some(arg)` wraps)
/// - fill-in of `None` for missing optional params (would add trailing args that the
///   target Rust API does not accept, e.g. `s.starts_with("hello", None)`)
///
/// Both the transformer and TypeResolver consult this list so that remapped calls
/// preserve the TS call's arity and expression types verbatim.
///
/// **Conditionally remapped names are INTENTIONALLY excluded**. `test` / `exec`
/// have guarded arms (`if matches!(&object, Expr::Regex { .. })`) that only
/// fire for `Expr::Regex` receivers — for any other receiver they fall through
/// to the passthrough arm, and the user-defined method's TS signature MUST
/// drive normal arg conversion. Listing them here would silently strip
/// legitimate `Option<T>` expected types from user-defined `.test()` /
/// `.exec()` calls, causing type mismatches at call sites with optional args.
/// `replace` / `match` are still listed because they have an **unguarded**
/// arm in addition to guarded Regex-receiver arms, so their remap always fires.
pub(crate) const REMAPPED_METHODS: &[&str] = &[
    "includes",
    "startsWith",
    "endsWith",
    "toLowerCase",
    "toUpperCase",
    "toString",
    "trim",
    "split",
    "map",
    "filter",
    "find",
    "some",
    "every",
    "substring",
    "slice",
    "splice",
    "sort",
    "indexOf",
    "reduce",
    "forEach",
    "join",
    "replace",
    "replaceAll",
    "match",
];

/// Returns whether `method` is **unconditionally** rewritten by
/// [`map_method_call`]. See [`REMAPPED_METHODS`].
pub(crate) fn is_remapped_method(method: &str) -> bool {
    REMAPPED_METHODS.contains(&method)
}

/// Builds an iterator method chain: `object.iter().cloned().method(args)`.
///
/// Strips closure type annotations from `args` (iterator yields `&T`, so explicit
/// annotations would conflict). When `collect` is true, appends `.collect::<Vec<_>>()`.
fn build_iter_method_call(object: Expr, rust_method: &str, args: Vec<Expr>, collect: bool) -> Expr {
    let iter_call = Expr::MethodCall {
        object: Box::new(Expr::MethodCall {
            object: Box::new(object),
            method: "iter".to_string(),
            args: vec![],
        }),
        method: "cloned".to_string(),
        args: vec![],
    };
    let args = args
        .into_iter()
        .map(strip_closure_type_annotations)
        .collect();
    let method_call = Expr::MethodCall {
        object: Box::new(iter_call),
        method: rust_method.to_string(),
        args,
    };
    if collect {
        Expr::MethodCall {
            object: Box::new(method_call),
            method: "collect::<Vec<_>>".to_string(),
            args: vec![],
        }
    } else {
        method_call
    }
}

/// Maps TypeScript method names to Rust equivalents.
///
/// Handles simple renames, methods that need wrapping (e.g., `trim` → `trim().to_string()`),
/// methods that need chaining (e.g., `split` → `split(s).map(…).collect::<Vec<String>>()`),
/// string methods (e.g., `substring` → `[a..b].to_string()`),
/// and array methods (e.g., `reduce` → `iter().fold()`, `indexOf` → `iter().position()`,
/// `slice` → `[a..b].to_vec()`, `splice` → `drain().collect()`).
pub(super) fn map_method_call(object: Expr, method: &str, args: Vec<Expr>) -> Expr {
    match method {
        // Simple name mappings
        "includes" => Expr::MethodCall {
            object: Box::new(object),
            method: "contains".to_string(),
            args: args.into_iter().map(|a| Expr::Ref(Box::new(a))).collect(),
        },
        "startsWith" => Expr::MethodCall {
            object: Box::new(object),
            method: "starts_with".to_string(),
            args,
        },
        "endsWith" => Expr::MethodCall {
            object: Box::new(object),
            method: "ends_with".to_string(),
            args,
        },
        "toLowerCase" => Expr::MethodCall {
            object: Box::new(object),
            method: "to_lowercase".to_string(),
            args,
        },
        "toUpperCase" => Expr::MethodCall {
            object: Box::new(object),
            method: "to_uppercase".to_string(),
            args,
        },
        "toString" => Expr::MethodCall {
            object: Box::new(object),
            method: "to_string".to_string(),
            args,
        },
        // trim() returns &str, wrap with .to_string()
        "trim" => Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(object),
                method: "trim".to_string(),
                args,
            }),
            method: "to_string".to_string(),
            args: vec![],
        },
        // split() → .split(sep).map(|s| s.to_string()).collect::<Vec<String>>()
        // TS split() returns string[], Rust split() returns Iterator<Item=&str>
        "split" => Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "split".to_string(),
                    args,
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "s".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("s".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    })),
                }],
            }),
            method: "collect::<Vec<String>>".to_string(),
            args: vec![],
        },
        // Iterator methods that collect.
        // TypeScript's .map(fn) passes Self::Item by value, so no body rewrite is needed.
        // TODO: clone 削減 — Copy 型には .copied()、不要な clone は所有権解析で除去
        "map" => build_iter_method_call(object, "map", args, true),
        // filter's predicate receives &Self::Item in Rust, so body param refs must be dereffed.
        "filter" => {
            let args = args.into_iter().map(deref_closure_params).collect();
            build_iter_method_call(object, "filter", args, true)
        }
        // find's predicate also receives &Self::Item; no collect (returns Option<T>).
        "find" => {
            let args = args.into_iter().map(deref_closure_params).collect();
            build_iter_method_call(object, "find", args, false)
        }
        // some/every pass Self::Item by value to their predicates — no deref.
        "some" => build_iter_method_call(object, "any", args, false),
        "every" => build_iter_method_call(object, "all", args, false),
        // substring(start, end) → [start..end].to_string()
        // substring(start) → [start..].to_string()
        "substring" => {
            let mut iter = args.into_iter();
            let start = iter.next();
            let end = iter.next();
            Expr::MethodCall {
                object: Box::new(Expr::Index {
                    object: Box::new(object),
                    index: Box::new(Expr::Range {
                        start: start.map(Box::new),
                        end: end.map(Box::new),
                    }),
                }),
                method: "to_string".to_string(),
                args: vec![],
            }
        }
        // slice(start, end) → [start..end].to_vec()
        // slice(start) → [start..].to_vec()
        "slice" => {
            let mut iter = args.into_iter();
            let start = iter.next();
            let end = iter.next();
            Expr::MethodCall {
                object: Box::new(Expr::Index {
                    object: Box::new(object),
                    index: Box::new(Expr::Range {
                        start: start.map(Box::new),
                        end: end.map(Box::new),
                    }),
                }),
                method: "to_vec".to_string(),
                args: vec![],
            }
        }
        // splice(start, count) → .drain(start..start+count).collect::<Vec<_>>()
        // Pre-compute end when both are numeric literals to avoid float range issues
        "splice" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let start = iter.next().unwrap();
            let count = iter.next().unwrap();
            // Pre-compute end = start + count when both are numeric literals
            let end = match (&start, &count) {
                (Expr::NumberLit(s), Expr::NumberLit(c)) => Expr::NumberLit(s + c),
                _ => Expr::BinaryOp {
                    left: Box::new(start.clone()),
                    op: BinOp::Add,
                    right: Box::new(count),
                },
            };
            let drain_call = Expr::MethodCall {
                object: Box::new(object),
                method: "drain".to_string(),
                args: vec![Expr::Range {
                    start: Some(Box::new(start)),
                    end: Some(Box::new(end)),
                }],
            };
            Expr::MethodCall {
                object: Box::new(drain_call),
                method: "collect::<Vec<_>>".to_string(),
                args: vec![],
            }
        }
        // sort() → .sort_by(|a, b| a.partial_cmp(b).unwrap())
        // sort(fn) → .sort_by(fn) with type annotations stripped
        "sort" => {
            if args.is_empty() {
                // f64 doesn't implement Ord, so use partial_cmp
                let cmp_closure = Expr::Closure {
                    params: vec![
                        Param {
                            name: "a".to_string(),
                            ty: None,
                        },
                        Param {
                            name: "b".to_string(),
                            ty: None,
                        },
                    ],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                        object: Box::new(Expr::MethodCall {
                            object: Box::new(Expr::Ident("a".to_string())),
                            method: "partial_cmp".to_string(),
                            args: vec![Expr::Ident("b".to_string())],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    })),
                };
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: "sort_by".to_string(),
                    args: vec![cmp_closure],
                };
            }
            // With comparator: strip type annotations and wrap body in partial_cmp
            // TS comparator returns number (negative/zero/positive), Rust needs Ordering
            let args = args
                .into_iter()
                .map(|arg| {
                    let stripped = strip_closure_type_annotations(arg);
                    wrap_sort_comparator_body(stripped)
                })
                .collect();
            Expr::MethodCall {
                object: Box::new(object),
                method: "sort_by".to_string(),
                args,
            }
        }
        // indexOf(x) → .iter().position(|item| *item == x).map(|i| i as f64).unwrap_or(-1.0)
        "indexOf" => {
            if args.len() != 1 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let search_value = args.into_iter().next().unwrap();
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            let position_call = Expr::MethodCall {
                object: Box::new(iter_call),
                method: "position".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "item".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Deref(Box::new(Expr::Ident("item".to_string())))),
                        op: BinOp::Eq,
                        right: Box::new(search_value),
                    })),
                }],
            };
            // .map(|i| i as f64).unwrap_or(-1.0)
            let map_call = Expr::MethodCall {
                object: Box::new(position_call),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "i".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    })),
                }],
            };
            Expr::MethodCall {
                object: Box::new(map_call),
                method: "unwrap_or".to_string(),
                args: vec![Expr::NumberLit(-1.0)],
            }
        }
        // reduce(fn, init) → .iter().fold(init, fn)
        // Strip closure param type annotations (iter() yields &T, Rust infers correctly)
        "reduce" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let callback = strip_closure_type_annotations(iter.next().unwrap());
            let init = iter.next().unwrap();
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "fold".to_string(),
                args: vec![init, callback],
            }
        }
        // forEach → .iter().cloned().for_each(fn)
        "forEach" => build_iter_method_call(object, "for_each", args, false),
        // join(sep) → join(&sep) — Rust's join takes &str, not String
        "join" => {
            let args = args
                .into_iter()
                .map(|arg| match arg {
                    // Variable: &sep
                    Expr::Ident(name) => Expr::Ref(Box::new(Expr::Ident(name))),
                    // String literal: already &str in Rust, pass through
                    lit @ Expr::StringLit(_) => lit,
                    // Other expressions: call .as_str()
                    other => Expr::MethodCall {
                        object: Box::new(other),
                        method: "as_str".to_string(),
                        args: vec![],
                    },
                })
                .collect();
            Expr::MethodCall {
                object: Box::new(object),
                method: "join".to_string(),
                args,
            }
        }
        // Regex-aware replace: str.replace(/p/g, "r") → regex.replace_all(&str, "r")
        "replace" if matches!(args.first(), Some(Expr::Regex { .. })) => {
            let mut args_iter = args.into_iter();
            let regex_expr = args_iter.next().unwrap();
            let remaining_args: Vec<Expr> = args_iter.collect();
            if let Expr::Regex {
                pattern, global, ..
            } = regex_expr
            {
                let regex_obj = build_regex_new(pattern);
                let method_name = if global { "replace_all" } else { "replace" };
                let mut call_args = vec![Expr::Ref(Box::new(object))];
                call_args.extend(remaining_args);
                // Regex::replace/replace_all returns Cow<str>, convert to String
                Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(regex_obj),
                        method: method_name.to_string(),
                        args: call_args,
                    }),
                    method: "to_string".to_string(),
                    args: vec![],
                }
            } else {
                unreachable!()
            }
        }
        // str.match(regex) → regex.find(&str) or regex.find_iter(&str) depending on g flag
        "match" if matches!(args.first(), Some(Expr::Regex { .. })) => {
            let regex_expr = args.into_iter().next().unwrap();
            if let Expr::Regex {
                pattern, global, ..
            } = regex_expr
            {
                let regex_obj = build_regex_new(pattern);
                let method_name = if global { "find_iter" } else { "find" };
                Expr::MethodCall {
                    object: Box::new(regex_obj),
                    method: method_name.to_string(),
                    args: vec![Expr::Ref(Box::new(object))],
                }
            } else {
                unreachable!()
            }
        }
        // regex.test(str) → regex.is_match(&str)
        "test" if matches!(&object, Expr::Regex { .. }) => {
            if let Expr::Regex { pattern, .. } = object {
                let regex_obj = build_regex_new(pattern);
                Expr::MethodCall {
                    object: Box::new(regex_obj),
                    method: "is_match".to_string(),
                    args: args.into_iter().map(|a| Expr::Ref(Box::new(a))).collect(),
                }
            } else {
                unreachable!()
            }
        }
        // regex.exec(str) → regex.captures(&str)
        "exec" if matches!(&object, Expr::Regex { .. }) => {
            if let Expr::Regex { pattern, .. } = object {
                let regex_obj = build_regex_new(pattern);
                Expr::MethodCall {
                    object: Box::new(regex_obj),
                    method: "captures".to_string(),
                    args: args.into_iter().map(|a| Expr::Ref(Box::new(a))).collect(),
                }
            } else {
                unreachable!()
            }
        }
        // str.replaceAll("a", "b") → str.replace("a", "b") (Rust replace replaces all)
        "replaceAll" => Expr::MethodCall {
            object: Box::new(object),
            method: "replace".to_string(),
            args,
        },
        // String replace: str.replace("a", "b") → str.replacen("a", "b", 1)
        // TS replaces only the first occurrence; Rust's replace() replaces all.
        "replace" => {
            let mut new_args = args;
            new_args.push(Expr::IntLit(1));
            Expr::MethodCall {
                object: Box::new(object),
                method: "replacen".to_string(),
                args: new_args,
            }
        }
        // No mapping needed — pass through unchanged
        _ => Expr::MethodCall {
            object: Box::new(object),
            method: method.to_string(),
            args,
        },
    }
}

/// Builds a `Regex::new(r"pattern").unwrap()` expression from a pattern string.
///
/// Returns `Expr::Regex` which the generator renders as `Regex::new(r"pattern").unwrap()`.
fn build_regex_new(pattern: String) -> Expr {
    Expr::Regex {
        pattern,
        global: false,
        sticky: false,
    }
}

/// Wraps closure parameter identifier references in `Expr::Deref`.
///
/// `Iterator::filter` / `Iterator::find` pass `&Self::Item` to their predicate,
/// whereas TypeScript passes the value by value. `deref_closure_params` inserts
/// a `*` so that every reference to the closure's parameters inside the body
/// operates on the dereferenced value, matching TypeScript semantics.
///
/// Scope handling:
/// - Only the outermost closure's parameter names are eligible for rewriting.
/// - Nested closures temporarily shadow matching names while their body is
///   folded, so that `|x| xs.find(|x| *x == 0)` (inner `x` shadows outer) is
///   not doubly dereffed.
/// - `let x = ...` within the body is not treated as shadowing (out of scope
///   for iterator predicates, which rarely declare locals).
///
/// Applied only to `filter`/`find` — other iterator methods (`map`, `any`,
/// `all`, `for_each`, `fold`) pass `Self::Item` by value, so no deref is
/// required. Non-closure arguments pass through unchanged.
fn deref_closure_params(expr: Expr) -> Expr {
    let Expr::Closure {
        params,
        return_type,
        body,
    } = expr
    else {
        return expr;
    };

    let param_names: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    let mut folder = DerefParams {
        params: param_names,
    };
    let new_body = match body {
        ClosureBody::Expr(inner) => ClosureBody::Expr(Box::new(folder.fold_expr(*inner))),
        ClosureBody::Block(stmts) => {
            ClosureBody::Block(stmts.into_iter().map(|s| folder.fold_stmt(s)).collect())
        }
    };
    Expr::Closure {
        params,
        return_type,
        body: new_body,
    }
}

struct DerefParams {
    params: HashSet<String>,
}

impl IrFolder for DerefParams {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Ident(name) if self.params.contains(&name) => {
                Expr::Deref(Box::new(Expr::Ident(name)))
            }
            Expr::Closure {
                params,
                return_type,
                body,
            } => {
                // Inner closure: its params shadow outer params of the same name
                // while its body is folded.
                let shadowed: Vec<String> = params
                    .iter()
                    .filter(|p| self.params.contains(&p.name))
                    .map(|p| p.name.clone())
                    .collect();
                for n in &shadowed {
                    self.params.remove(n);
                }
                let new_body = match body {
                    ClosureBody::Expr(inner) => ClosureBody::Expr(Box::new(self.fold_expr(*inner))),
                    ClosureBody::Block(stmts) => {
                        ClosureBody::Block(stmts.into_iter().map(|s| self.fold_stmt(s)).collect())
                    }
                };
                for n in shadowed {
                    self.params.insert(n);
                }
                Expr::Closure {
                    params,
                    return_type,
                    body: new_body,
                }
            }
            other => walk_expr(self, other),
        }
    }
}

/// Strips type annotations from closure parameters and return type.
///
/// Used for iterator method closures (`fold`, `sort_by`, etc.) where Rust's type
/// inference handles `&T` references correctly without explicit annotations.
fn strip_closure_type_annotations(expr: Expr) -> Expr {
    match expr {
        Expr::Closure {
            params,
            return_type: _,
            body,
        } => Expr::Closure {
            params: params
                .into_iter()
                .map(|p| Param {
                    name: p.name,
                    ty: None,
                })
                .collect(),
            return_type: None,
            body,
        },
        other => other,
    }
}

/// Wraps a TS sort comparator closure body with `partial_cmp(&0.0).unwrap()`.
///
/// TS comparators return a number (negative/zero/positive), but Rust's `sort_by`
/// expects `Ordering`. This wraps the body expression: `body` → `body.partial_cmp(&0.0).unwrap()`.
fn wrap_sort_comparator_body(expr: Expr) -> Expr {
    match expr {
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            let new_body = match body {
                ClosureBody::Expr(inner) => {
                    let wrapped = Expr::MethodCall {
                        object: Box::new(Expr::MethodCall {
                            object: inner,
                            method: "partial_cmp".to_string(),
                            args: vec![Expr::Ref(Box::new(Expr::NumberLit(0.0)))],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    };
                    ClosureBody::Expr(Box::new(wrapped))
                }
                other => other, // Block bodies — don't attempt to wrap
            };
            Expr::Closure {
                params,
                return_type,
                body: new_body,
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── REMAPPED_METHODS ↔ map_method_call consistency ─────────────

    /// Supplies argument lists appropriate for each remapped method's arity
    /// contract in `map_method_call`. Methods that require exactly N args
    /// (e.g., `splice` requires 2) fall through to passthrough otherwise,
    /// which would make the consistency test produce false negatives.
    fn args_for_remap_probe(method: &str) -> Vec<Expr> {
        fn dummy_closure() -> Expr {
            Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::BoolLit(true))),
            }
        }
        match method {
            // Zero-arg methods
            "toString" | "toLowerCase" | "toUpperCase" | "trim" | "sort" => vec![],
            // Closure-taking methods
            "map" | "filter" | "find" | "some" | "every" | "forEach" => vec![dummy_closure()],
            // reduce requires 2 args (callback, init)
            "reduce" => vec![dummy_closure(), Expr::NumberLit(0.0)],
            // splice requires exactly 2 args
            "splice" => vec![Expr::NumberLit(0.0), Expr::NumberLit(1.0)],
            // indexOf requires exactly 1 arg
            "indexOf" => vec![Expr::NumberLit(0.0)],
            // replace/replaceAll take 2 string args. The unguarded `replace` arm
            // always fires, renaming to `replacen` with an appended `1`.
            "replace" | "replaceAll" => vec![
                Expr::StringLit("a".to_string()),
                Expr::StringLit("b".to_string()),
            ],
            // `match` with a Regex arg hits the guarded arm that always fires
            // because it's the only `match` handler in map_method_call.
            "match" => vec![Expr::Regex {
                pattern: "x".to_string(),
                global: false,
                sticky: false,
            }],
            // join: StringLit input is a no-op in map_method_call (Rust's `join`
            // accepts &str directly); probe with an Ident to force the `&sep`
            // transformation so the test can observe the remap.
            "join" => vec![Expr::Ident("sep".to_string())],
            // Everything else: single string/number arg
            _ => vec![Expr::StringLit("arg".to_string())],
        }
    }

    /// Builds the passthrough form `obj.<method>(<args>)` that `map_method_call`
    /// produces via its catch-all arm.
    fn passthrough(method: &str, obj: Expr, args: Vec<Expr>) -> Expr {
        Expr::MethodCall {
            object: Box::new(obj),
            method: method.to_string(),
            args,
        }
    }

    #[test]
    fn test_remapped_methods_match_map_method_call_arms() {
        // Forward: every name in REMAPPED_METHODS must produce a transformed
        // output from map_method_call (not the default passthrough). Missing
        // arms drift silently, causing TS signature types to leak into the
        // transformer's arg conversion — re-introducing the Some() wrap and
        // None-fill regressions that Step 2 resolved. REMAPPED_METHODS only
        // lists *unconditionally* remapped names, so an Ident receiver is
        // sufficient to exercise each arm.
        for &name in REMAPPED_METHODS {
            let obj = Expr::Ident("obj".to_string());
            let args = args_for_remap_probe(name);
            let output = map_method_call(obj.clone(), name, args.clone());
            assert_ne!(
                output,
                passthrough(name, obj, args),
                "REMAPPED_METHODS contains {name:?} but map_method_call returns the passthrough form \
                 for it. Either remove it from REMAPPED_METHODS or add a named arm to map_method_call.",
            );
        }
    }

    #[test]
    fn test_non_remapped_common_methods_passthrough() {
        // Reverse direction (probe form): names that are known NOT to be remapped
        // must actually fall through to the passthrough arm. If map_method_call
        // ever grows a silent arm for these, REMAPPED_METHODS must be updated.
        for name in [
            "push",
            "pop",
            "length",
            "charCodeAt",
            "shift",
            "unshift",
            "concat",
        ] {
            assert!(
                !is_remapped_method(name),
                "{name} unexpectedly reported as remapped",
            );
            let obj = Expr::Ident("obj".to_string());
            let args = vec![Expr::NumberLit(1.0)];
            let output = map_method_call(obj.clone(), name, args.clone());
            assert_eq!(
                output,
                passthrough(name, obj, args),
                "{name} should pass through unchanged but map_method_call transformed it; \
                 add {name:?} to REMAPPED_METHODS.",
            );
        }
    }

    #[test]
    fn test_conditionally_remapped_methods_excluded_from_remapped_list() {
        // `test` / `exec` are remapped ONLY when the receiver is `Expr::Regex`
        // (see the guarded arms in `map_method_call`). For ANY other receiver
        // they fall through to the passthrough arm, and user-defined
        // `.test(...)` / `.exec(...)` methods must use their TS signature for
        // normal arg conversion (Option<T> expected-type propagation etc.).
        //
        // Listing them in REMAPPED_METHODS would silently strip legitimate
        // `Option<T>` expected types from user-defined calls at non-Regex
        // sites, causing type mismatches for optional params.
        for name in ["test", "exec"] {
            assert!(
                !is_remapped_method(name),
                "{name} is conditionally remapped (Regex receiver only) and MUST NOT \
                 appear in REMAPPED_METHODS — otherwise user-defined .{name}() calls \
                 with optional params lose their Option<T> expected types.",
            );
            // Non-Regex receiver: passthrough.
            let obj = Expr::Ident("obj".to_string());
            let args = vec![Expr::StringLit("s".to_string())];
            let output = map_method_call(obj.clone(), name, args.clone());
            assert_eq!(
                output,
                passthrough(name, obj, args),
                "{name} with non-Regex receiver should pass through",
            );
            // Regex receiver: remap via guarded arm.
            let regex_obj = Expr::Regex {
                pattern: "r".to_string(),
                global: false,
                sticky: false,
            };
            let args = vec![Expr::StringLit("s".to_string())];
            let output = map_method_call(regex_obj.clone(), name, args.clone());
            assert_ne!(
                output,
                passthrough(name, regex_obj, args),
                "{name} with Regex receiver must hit its guarded remap arm",
            );
        }
    }

    #[test]
    fn test_map_method_call_to_string() {
        let object = Expr::Ident("x".to_string());
        let result = map_method_call(object, "toString", vec![]);
        assert_eq!(
            result,
            Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            }
        );
    }

    #[test]
    fn test_map_method_call_to_string_preserves_args() {
        // toString(radix) — args must be preserved so Rust compiler catches the error
        let object = Expr::Ident("x".to_string());
        let args = vec![Expr::NumberLit(16.0)];
        let result = map_method_call(object, "toString", args);
        assert_eq!(
            result,
            Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "to_string".to_string(),
                args: vec![Expr::NumberLit(16.0)],
            }
        );
    }

    // ── deref_closure_params ───────────────────────────────────────

    fn closure1(name: &str, body: Expr) -> Expr {
        Expr::Closure {
            params: vec![Param {
                name: name.to_string(),
                ty: None,
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(body)),
        }
    }

    fn ident(name: &str) -> Expr {
        Expr::Ident(name.to_string())
    }

    fn deref_ident(name: &str) -> Expr {
        Expr::Deref(Box::new(ident(name)))
    }

    #[test]
    fn test_deref_closure_params_simple_comparison() {
        // |x| x > 4.0  →  |x| *x > 4.0
        let input = closure1(
            "x",
            Expr::BinaryOp {
                left: Box::new(ident("x")),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(4.0)),
            },
        );
        let expected = closure1(
            "x",
            Expr::BinaryOp {
                left: Box::new(deref_ident("x")),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(4.0)),
            },
        );
        assert_eq!(deref_closure_params(input), expected);
    }

    #[test]
    fn test_deref_closure_params_nested_binary() {
        // |x| x > 4.0 && x < 10.0  →  |x| *x > 4.0 && *x < 10.0
        let gt = Expr::BinaryOp {
            left: Box::new(ident("x")),
            op: BinOp::Gt,
            right: Box::new(Expr::NumberLit(4.0)),
        };
        let lt = Expr::BinaryOp {
            left: Box::new(ident("x")),
            op: BinOp::Lt,
            right: Box::new(Expr::NumberLit(10.0)),
        };
        let input = closure1(
            "x",
            Expr::BinaryOp {
                left: Box::new(gt.clone()),
                op: BinOp::LogicalAnd,
                right: Box::new(lt.clone()),
            },
        );

        let expected_gt = Expr::BinaryOp {
            left: Box::new(deref_ident("x")),
            op: BinOp::Gt,
            right: Box::new(Expr::NumberLit(4.0)),
        };
        let expected_lt = Expr::BinaryOp {
            left: Box::new(deref_ident("x")),
            op: BinOp::Lt,
            right: Box::new(Expr::NumberLit(10.0)),
        };
        let expected = closure1(
            "x",
            Expr::BinaryOp {
                left: Box::new(expected_gt),
                op: BinOp::LogicalAnd,
                right: Box::new(expected_lt),
            },
        );
        let _ = (gt, lt);
        assert_eq!(deref_closure_params(input), expected);
    }

    #[test]
    fn test_deref_closure_params_field_access() {
        // |x| x.field  →  |x| (*x).field  (represented as FieldAccess { object: Deref(Ident), ...})
        let input = closure1(
            "x",
            Expr::FieldAccess {
                object: Box::new(ident("x")),
                field: "field".to_string(),
            },
        );
        let expected = closure1(
            "x",
            Expr::FieldAccess {
                object: Box::new(deref_ident("x")),
                field: "field".to_string(),
            },
        );
        assert_eq!(deref_closure_params(input), expected);
    }

    #[test]
    fn test_deref_closure_params_non_closure_passthrough() {
        // Non-closure expressions pass through unchanged
        let input = ident("x");
        assert_eq!(deref_closure_params(input.clone()), input);

        let num = Expr::NumberLit(1.0);
        assert_eq!(deref_closure_params(num.clone()), num);
    }

    #[test]
    fn test_deref_closure_params_does_not_touch_non_param_ident() {
        // |x| x > threshold  →  |x| *x > threshold  (threshold is captured, not a param)
        let input = closure1(
            "x",
            Expr::BinaryOp {
                left: Box::new(ident("x")),
                op: BinOp::Gt,
                right: Box::new(ident("threshold")),
            },
        );
        let expected = closure1(
            "x",
            Expr::BinaryOp {
                left: Box::new(deref_ident("x")),
                op: BinOp::Gt,
                right: Box::new(ident("threshold")),
            },
        );
        assert_eq!(deref_closure_params(input), expected);
    }

    #[test]
    fn test_deref_closure_params_nested_closure_shadowing() {
        // |x| (|x| x)(y)  →  |x| (|x| x)(y)
        // Inner `x` shadows outer `x`; neither should be dereffed.
        // The outer body references no non-shadowed params, so no change.
        let inner = closure1("x", ident("x"));
        let outer_body = Expr::FnCall {
            target: crate::ir::CallTarget::Free("dummy".to_string()),
            args: vec![inner],
        };
        let input = closure1("x", outer_body.clone());
        let expected = closure1("x", outer_body);
        assert_eq!(deref_closure_params(input), expected);
    }

    // ── map_method_call: iterator chain methods ───────────────────

    fn predicate_closure(name: &str) -> Expr {
        // |x| x > 0.0
        closure1(
            name,
            Expr::BinaryOp {
                left: Box::new(ident(name)),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            },
        )
    }

    /// Extracts `.iter().cloned()` chain root and its terminal method from
    /// `object.iter().cloned().method(args)` or the collected form.
    fn expect_iter_chain(result: &Expr) -> (&str, &[Expr]) {
        match result {
            Expr::MethodCall {
                object,
                method,
                args,
            } if method == "collect::<Vec<_>>" => {
                if let Expr::MethodCall {
                    object: inner_obj,
                    method: inner_method,
                    args: inner_args,
                } = object.as_ref()
                {
                    assert!(is_iter_cloned(inner_obj), "expected .iter().cloned() chain");
                    return (inner_method.as_str(), inner_args.as_slice());
                }
                panic!("expected nested method call for collect form");
            }
            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                assert!(is_iter_cloned(object), "expected .iter().cloned() chain");
                (method.as_str(), args.as_slice())
            }
            other => panic!("expected MethodCall, got {:?}", other),
        }
    }

    fn is_iter_cloned(expr: &Expr) -> bool {
        if let Expr::MethodCall { object, method, .. } = expr {
            if method == "cloned" {
                if let Expr::MethodCall { method: inner, .. } = object.as_ref() {
                    return inner == "iter";
                }
            }
        }
        false
    }

    #[test]
    fn test_map_method_call_filter_generates_iter_chain() {
        let obj = ident("nums");
        let result = map_method_call(obj, "filter", vec![predicate_closure("x")]);
        let (method, args) = expect_iter_chain(&result);
        assert_eq!(method, "filter");
        // filter closure body should have Deref(x) on the left
        let Expr::Closure { body, .. } = &args[0] else {
            panic!("expected closure arg");
        };
        let ClosureBody::Expr(inner) = body else {
            panic!("expected Expr body");
        };
        assert!(
            matches!(
                inner.as_ref(),
                Expr::BinaryOp { left, .. } if matches!(left.as_ref(), Expr::Deref(_))
            ),
            "filter body must deref param: {:?}",
            inner
        );
    }

    #[test]
    fn test_map_method_call_find_generates_iter_chain() {
        let obj = ident("nums");
        let result = map_method_call(obj, "find", vec![predicate_closure("x")]);
        // find does NOT collect — top-level is the .find() call directly
        assert!(
            !matches!(&result, Expr::MethodCall { method, .. } if method == "collect::<Vec<_>>"),
            "find should not collect: {:?}",
            result
        );
        let (method, args) = expect_iter_chain(&result);
        assert_eq!(method, "find");
        let Expr::Closure { body, .. } = &args[0] else {
            panic!("expected closure arg");
        };
        let ClosureBody::Expr(inner) = body else {
            panic!("expected Expr body");
        };
        assert!(
            matches!(
                inner.as_ref(),
                Expr::BinaryOp { left, .. } if matches!(left.as_ref(), Expr::Deref(_))
            ),
            "find body must deref param: {:?}",
            inner
        );
    }

    #[test]
    fn test_map_method_call_map_generates_iter_chain() {
        let obj = ident("nums");
        let map_body = closure1(
            "x",
            Expr::BinaryOp {
                left: Box::new(ident("x")),
                op: BinOp::Mul,
                right: Box::new(Expr::NumberLit(2.0)),
            },
        );
        let result = map_method_call(obj, "map", vec![map_body]);
        let (method, args) = expect_iter_chain(&result);
        assert_eq!(method, "map");
        // map body must NOT have Deref (pass by value)
        let Expr::Closure { body, .. } = &args[0] else {
            panic!("expected closure arg");
        };
        let ClosureBody::Expr(inner) = body else {
            panic!("expected Expr body");
        };
        assert!(
            matches!(
                inner.as_ref(),
                Expr::BinaryOp { left, .. } if matches!(left.as_ref(), Expr::Ident(_))
            ),
            "map body must NOT deref param: {:?}",
            inner
        );
    }

    #[test]
    fn test_map_method_call_some_maps_to_any() {
        let obj = ident("nums");
        let result = map_method_call(obj, "some", vec![predicate_closure("x")]);
        let (method, args) = expect_iter_chain(&result);
        assert_eq!(method, "any");
        let Expr::Closure { body, .. } = &args[0] else {
            panic!("expected closure arg");
        };
        let ClosureBody::Expr(inner) = body else {
            panic!("expected Expr body");
        };
        // some (→any) passes Self::Item by value — no deref
        assert!(
            matches!(
                inner.as_ref(),
                Expr::BinaryOp { left, .. } if matches!(left.as_ref(), Expr::Ident(_))
            ),
            "some body must NOT deref param: {:?}",
            inner
        );
    }

    #[test]
    fn test_map_method_call_every_maps_to_all() {
        let obj = ident("nums");
        let result = map_method_call(obj, "every", vec![predicate_closure("x")]);
        let (method, _) = expect_iter_chain(&result);
        assert_eq!(method, "all");
    }

    #[test]
    fn test_map_method_call_unknown_method_passthrough() {
        let obj = ident("x");
        let args = vec![Expr::NumberLit(1.0)];
        let result = map_method_call(obj.clone(), "unknownMethod", args.clone());
        assert_eq!(
            result,
            Expr::MethodCall {
                object: Box::new(obj),
                method: "unknownMethod".to_string(),
                args,
            }
        );
    }

    #[test]
    fn test_deref_closure_params_nested_closure_captures_outer() {
        // |x| (|y| x + y)   →  |x| (|y| *x + y)
        // Inner closure captures outer `x`; y is not in our params, left alone.
        let inner_body = Expr::BinaryOp {
            left: Box::new(ident("x")),
            op: BinOp::Add,
            right: Box::new(ident("y")),
        };
        let inner = Expr::Closure {
            params: vec![Param {
                name: "y".to_string(),
                ty: None,
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(inner_body)),
        };
        let input = closure1("x", inner);

        let expected_inner_body = Expr::BinaryOp {
            left: Box::new(deref_ident("x")),
            op: BinOp::Add,
            right: Box::new(ident("y")),
        };
        let expected_inner = Expr::Closure {
            params: vec![Param {
                name: "y".to_string(),
                ty: None,
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(expected_inner_body)),
        };
        let expected = closure1("x", expected_inner);
        assert_eq!(deref_closure_params(input), expected);
    }
}
