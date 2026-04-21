//! Unit tests for [`super::map_method_call`],
//! [`super::closures::deref_closure_params`] and the
//! [`REMAPPED_METHODS`] ↔ [`map_method_call`] consistency invariant.

use super::closures::deref_closure_params;
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
