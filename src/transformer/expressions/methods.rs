//! Method call mapping from TypeScript to Rust equivalents.

use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType};

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
        // Iterator methods that collect: .map(fn) / .filter(fn) → .iter().cloned().method(fn).collect()
        // .cloned() converts &T → T, giving closures value semantics matching TypeScript behavior.
        // TODO: clone 削減 — Copy 型には .copied()、不要な clone は所有権解析で除去
        "map" | "filter" => build_iter_method_call(object, method, args, true),
        // Iterator methods without collect: .find(fn), .some(fn), .every(fn)
        "find" => build_iter_method_call(object, "find", args, false),
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
