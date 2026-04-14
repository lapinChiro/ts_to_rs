use super::*;

#[test]
fn test_convert_expr_nullish_coalescing_basic() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `a ?? b` → `a.unwrap_or_else(|| b)`
    let expr = parse_expr("a ?? b;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("a".to_string())),
            method: "unwrap_or_else".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::Ident("b".to_string()))),
            }],
        }
    );
}

#[test]
fn test_convert_opt_chain_length_returns_len_as_f64() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x?.length;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // x?.length → x.as_ref().map(|_v| _v.len() as f64)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "as_ref".to_string(),
                args: vec![],
            }),
            method: "map".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "_v".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::Cast {
                    expr: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("_v".to_string())),
                        method: "len".to_string(),
                        args: vec![],
                    }),
                    target: RustType::F64,
                })),
            }],
        }
    );
}

#[test]
fn test_convert_opt_chain_normal_field_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x?.y;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // x?.y → x.as_ref().map(|_v| _v.y) — 既存動作が壊れないこと
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "as_ref".to_string(),
                args: vec![],
            }),
            method: "map".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "_v".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("_v".to_string())),
                    field: "y".to_string(),
                })),
            }],
        }
    );
}

#[test]
fn test_convert_opt_chain_non_option_type_returns_plain_access() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![("y".to_string(), RustType::String).into()],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("function f(x: Foo) { x?.y; }", reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("x".to_string())),
            field: "y".to_string(),
        }
    );
}

#[test]
fn test_convert_opt_chain_option_type_returns_map_pattern() {
    let f = TctxFixture::from_source("function f(x: Foo | null) { x?.y; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "map"
    ));
}

#[test]
fn test_convert_opt_chain_unknown_type_returns_map_pattern() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x?.y;");

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
        used_marker_names: std::collections::HashSet::new(),
    }
    .convert_expr(&expr)
    .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "map"
    ));
}

#[test]
fn test_opt_chain_method_call_maps_to_rust_name() {
    // s?.toUpperCase() → s.as_ref().map(|_v| _v.to_uppercase())
    let f = TctxFixture::from_source("function f(s: string | null) { s?.toUpperCase(); }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Dig into the map closure body and verify method name is to_uppercase
    if let Expr::MethodCall {
        method: outer_method,
        args,
        ..
    } = &result
    {
        assert_eq!(outer_method, "map");
        if let Some(Expr::Closure {
            body: ClosureBody::Expr(body_expr),
            ..
        }) = args.first()
        {
            if let Expr::MethodCall { method, .. } = body_expr.as_ref() {
                assert_eq!(
                    method, "to_uppercase",
                    "expected to_uppercase, got {method}"
                );
                return;
            }
        }
    }
    panic!("unexpected IR structure: {result:?}");
}

#[test]
fn test_convert_nullish_coalescing_non_option_statically_short_circuits() {
    // `function f(x: string, y: string) { x ?? y; }` — expression-statement context.
    // Both operands have TS static type `string` (never null in strict mode).
    //
    // Expected behavior (I-022 design):
    // - `resolve_bin_expr` NC arm propagates `Option<String>` expected to LHS span.
    // - `convert_expr(x)` with `Option<String>` expected wraps the bare `Ident("x")`
    //   in `Some(_)` via the Option-wrap branch of `convert_expr_with_expected`.
    // - `convert_bin_expr` NC arm then sees `produces_option_result(Some(x)) = true`
    //   → `is_option_left = true` → skips the short-circuit.
    // - RHS `y` is resolved with expected `String` (non-Option) → bare `Ident("y")`.
    // - Result: `Some(x).unwrap_or_else(|| y)`.
    //
    // Semantically equivalent to `x` (Some always unwraps to x), just verbose.
    // The `or_else` closure form is used because `y` is an Ident (non-copy literal).
    let f = TctxFixture::from_source("function f(x: string, y: string) { x ?? y; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::MethodCall {
            object,
            method,
            args,
        } => {
            assert_eq!(method, "unwrap_or_else");
            assert!(
                matches!(
                    object.as_ref(),
                    Expr::FnCall {
                        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                        args: some_args,
                    } if some_args.len() == 1 && matches!(&some_args[0], Expr::Ident(n) if n == "x")
                ),
                "expected Some(x) receiver, got {object:?}"
            );
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::Closure { .. }));
        }
        _ => panic!("expected MethodCall with unwrap_or_else, got {result:?}"),
    }
}

#[test]
fn test_convert_nullish_coalescing_option_returns_unwrap_or_else() {
    let f = TctxFixture::from_source("function f(x: string | null, y: string) { x ?? y; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "unwrap_or_else"
    ));
}

#[test]
fn test_convert_nullish_coalescing_unknown_type_returns_unwrap_or_else() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x ?? y;");

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
        used_marker_names: std::collections::HashSet::new(),
    }
    .convert_expr(&expr)
    .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "unwrap_or_else"
    ));
}

#[test]
fn test_convert_opt_chain_computed_uses_safe_index_helper() {
    // x?.[0] → x.as_ref().and_then(|_v| _v.get(0).cloned())
    // Verifies that optional chaining computed access uses build_safe_index_expr
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x?.[0];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Outer: x.as_ref().and_then(closure)
    if let Expr::MethodCall {
        method: outer_method,
        args,
        ..
    } = &result
    {
        assert_eq!(outer_method, "and_then");
        if let Some(Expr::Closure {
            body: ClosureBody::Expr(body_expr),
            ..
        }) = args.first()
        {
            // Inner: _v.get(0).cloned()
            if let Expr::MethodCall {
                object,
                method: cloned_method,
                ..
            } = body_expr.as_ref()
            {
                assert_eq!(cloned_method, "cloned");
                if let Expr::MethodCall {
                    object: inner_obj,
                    method: get_method,
                    args: get_args,
                } = object.as_ref()
                {
                    assert_eq!(get_method, "get");
                    assert_eq!(*inner_obj, Box::new(Expr::Ident("_v".to_string())));
                    assert_eq!(get_args.len(), 1);
                    assert_eq!(get_args[0], Expr::IntLit(0));
                    return;
                }
            }
        }
    }
    panic!("unexpected IR structure for x?.[0]: {result:?}");
}

#[test]
fn test_convert_opt_chain_nested_option_uses_and_then() {
    // x?.y?.z where x: Foo | null, Foo.y: Bar | null, Bar.z: String
    // Should use .and_then() for the inner chain to avoid Option<Option<T>>
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![(
                "y".to_string(),
                RustType::Option(Box::new(RustType::Named {
                    name: "Bar".to_string(),
                    type_args: vec![],
                })),
            )
                .into()],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Bar".to_string(),
        TypeDef::new_struct(
            vec![("z".to_string(), RustType::String).into()],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("function f(x: Foo | null) { x?.y?.z; }", reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // The outermost should use and_then (not map) to avoid Option<Option<T>>
    let result_str = format!("{result:?}");
    assert!(
        result_str.contains("and_then"),
        "nested optional chaining should use and_then, got: {result:?}"
    );
}

#[test]
fn test_convert_nullish_coalescing_rhs_string_gets_to_string_when_lhs_is_option_string() {
    let source = r#"
        const s: string | undefined = undefined;
        const r = s ?? "default";
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();

    let swc_expr = extract_var_init_at(f.module(), 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Should be s.unwrap_or_else(|| "default".to_string())
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "unwrap_or_else");
        if let Expr::Closure { body, .. } = &args[0] {
            if let ClosureBody::Expr(expr) = body {
                assert!(
                    matches!(
                        expr.as_ref(),
                        Expr::MethodCall { method, .. } if method == "to_string"
                    ),
                    "expected .to_string() on rhs, got: {expr:?}"
                );
            } else {
                panic!("expected ClosureBody::Expr");
            }
        } else {
            panic!("expected Closure");
        }
    } else {
        panic!("expected MethodCall, got: {result:?}");
    }
}

// ── I-022: convert_bin_expr NC arm — Vec index LHS + chain ──

#[test]
fn test_convert_nc_vec_index_lhs_emits_unwrap_or_else_with_string_default() {
    // `function f(arr: string[], i: number): string { return arr[i] ?? "m"; }`
    // Expected IR: `arr.get(i as usize).cloned().unwrap_or_else(|| "m".to_string())`
    // - LHS `arr[i]` gets `Option<String>` expected via resolve_bin_expr NC arm
    //   → convert_member_expr emits `.get().cloned()` (no unwrap).
    // - is_option_left = true (produces_option_result).
    // - RHS "m" with String expected → ".to_string()" (non-copy → unwrap_or_else).
    let f = TctxFixture::from_source(
        "function f(arr: string[], i: number): string { return arr[i] ?? \"m\"; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Outer: unwrap_or_else with closure returning "m".to_string()
    match &result {
        Expr::MethodCall {
            object,
            method,
            args,
        } => {
            assert_eq!(method, "unwrap_or_else");
            assert_eq!(args.len(), 1);
            // RHS: `|| "m".to_string()`
            match &args[0] {
                Expr::Closure {
                    body: ClosureBody::Expr(body_expr),
                    ..
                } => {
                    assert!(
                        matches!(body_expr.as_ref(), Expr::MethodCall { method: m, .. } if m == "to_string"),
                        "expected .to_string() closure body, got {body_expr:?}"
                    );
                }
                other => panic!("expected Closure RHS, got {other:?}"),
            }
            // LHS: `arr.get(i as usize).cloned()` — Vec index form
            match object.as_ref() {
                Expr::MethodCall {
                    object: get_recv,
                    method: m,
                    args: _,
                } if m == "cloned" => match get_recv.as_ref() {
                    Expr::MethodCall {
                        method: inner_m,
                        args: inner_args,
                        ..
                    } => {
                        assert_eq!(inner_m, "get");
                        assert_eq!(inner_args.len(), 1);
                    }
                    other => panic!("expected .get() inner, got {other:?}"),
                },
                other => panic!("expected .cloned() on .get(), got {other:?}"),
            }
        }
        other => panic!("expected MethodCall unwrap_or_else, got {other:?}"),
    }
}

#[test]
fn test_convert_nc_vec_index_lhs_with_ident_default_lazy() {
    // `function f(arr: string[], i: number, d: string): string { return arr[i] ?? d; }`
    // RHS is Ident (non-copy), so unwrap_or_else lazy form.
    let f = TctxFixture::from_source(
        "function f(arr: string[], i: number, d: string): string { return arr[i] ?? d; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::MethodCall { method, args, .. } => {
            assert_eq!(method, "unwrap_or_else");
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Closure {
                    body: ClosureBody::Expr(body_expr),
                    ..
                } => {
                    assert_eq!(body_expr.as_ref(), &Expr::Ident("d".to_string()));
                }
                other => panic!("expected Closure wrapping Ident(d), got {other:?}"),
            }
        }
        other => panic!("expected MethodCall unwrap_or_else, got {other:?}"),
    }
}

#[test]
fn test_convert_nc_chain_option_option_string_emits_or_else_chain() {
    // `function f(a: string | null, b: string | null, c: string): string {
    //      return a ?? b ?? c;
    //  }`
    // Parsed as `(a ?? b) ?? c`. Expected IR:
    //   a.or_else(|| b).unwrap_or_else(|| c)
    // - Inner NC: LHS a (Option), RHS b (Option) → .or_else (chain preserves Option).
    // - Outer NC: LHS `a.or_else(|| b)` (Option), RHS c (non-Option) → .unwrap_or_else.
    let f = TctxFixture::from_source(
        "function f(a: string | null, b: string | null, c: string): string { \
         return a ?? b ?? c; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Outer: unwrap_or_else
    match &result {
        Expr::MethodCall {
            object,
            method,
            args,
        } => {
            assert_eq!(method, "unwrap_or_else", "outer NC must terminate Option");
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Closure {
                    body: ClosureBody::Expr(body_expr),
                    ..
                } => {
                    assert_eq!(body_expr.as_ref(), &Expr::Ident("c".to_string()));
                }
                other => panic!("expected outer closure wrapping Ident(c), got {other:?}"),
            }
            // Inner: or_else on `a`
            match object.as_ref() {
                Expr::MethodCall {
                    object: inner_recv,
                    method: inner_m,
                    args: inner_args,
                } => {
                    assert_eq!(inner_m, "or_else", "inner NC must preserve Option");
                    assert_eq!(inner_args.len(), 1);
                    assert_eq!(inner_recv.as_ref(), &Expr::Ident("a".to_string()));
                    match &inner_args[0] {
                        Expr::Closure {
                            body: ClosureBody::Expr(inner_body),
                            ..
                        } => {
                            assert_eq!(inner_body.as_ref(), &Expr::Ident("b".to_string()));
                        }
                        other => {
                            panic!("expected inner closure wrapping Ident(b), got {other:?}")
                        }
                    }
                }
                other => panic!("expected inner .or_else(|| b), got {other:?}"),
            }
        }
        other => panic!("expected outer MethodCall, got {other:?}"),
    }
}

#[test]
fn test_convert_nc_chain_all_option_emits_or_else_only() {
    // `function f(a: string | null, b: string | null): string | null {
    //      return a ?? b;
    //  }`
    // Both sides Option<String>, outer return also Option<String>.
    // Expected: `a.or_else(|| b)` (Option preserved, no unwrap).
    let f = TctxFixture::from_source(
        "function f(a: string | null, b: string | null): string | null { return a ?? b; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(
                method, "or_else",
                "chain with both Option and Option return should emit .or_else, got {method}"
            );
        }
        other => panic!("expected MethodCall .or_else, got {other:?}"),
    }
}

#[test]
fn test_convert_nc_option_lhs_with_non_option_rhs_emits_unwrap_or_else() {
    // Baseline: `name ?? "anonymous"` where name: Option<String>. Existing behavior
    // must be preserved (pre-I-022 snapshot unchanged for this case).
    let f = TctxFixture::from_source(
        "function withFallback(name: string | undefined): string { return name ?? \"anonymous\"; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::MethodCall { object, method, .. } => {
            assert_eq!(method, "unwrap_or_else");
            assert_eq!(object.as_ref(), &Expr::Ident("name".to_string()));
        }
        other => panic!("expected name.unwrap_or_else, got {other:?}"),
    }
}

#[test]
fn test_convert_nc_vec_of_option_lhs_uses_flatten_and_unwrap_or_else() {
    // `function f(items: (string | null)[], i: number): string {
    //      return items[i] ?? "default";
    //  }`
    // items: Vec<Option<String>>. items[i]: TS static type is `string | null`
    // (Option<String>) with element nullability. Runtime: `items.get(i).cloned()`
    // yields `Option<Option<String>>` which must be `.flatten()` to `Option<String>`.
    //
    // Pre-I-022 emitted `.get(i).cloned().unwrap()` (panic on empty array) followed
    // by `.unwrap_or_else(|| "default".to_string())` on the unwrapped `Option<String>`
    // — the `.unwrap()` panics on empty array BEFORE reaching the fallback, violating
    // TS semantics (`items[i] ?? "default"` → "default" on empty). I-022 unified NC
    // arm propagates `Option<String>` to LHS span so convert_member_expr emits
    // `.flatten()` form, preserving Option through the chain.
    let f = TctxFixture::from_source(
        "function f(items: (string | null)[], i: number): string { \
         return items[i] ?? \"default\"; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Outer: unwrap_or_else (closure returning "default".to_string())
    match &result {
        Expr::MethodCall {
            object,
            method,
            args,
        } => {
            assert_eq!(method, "unwrap_or_else");
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::Closure { .. }));
            // Inner receiver must be `.flatten()` preserving Option (NOT `.unwrap()`)
            match object.as_ref() {
                Expr::MethodCall {
                    object: flatten_recv,
                    method: flatten_m,
                    args: flatten_args,
                } => {
                    assert_eq!(
                        flatten_m, "flatten",
                        "expected .flatten() to collapse Option<Option<T>>, got {flatten_m}"
                    );
                    assert!(flatten_args.is_empty());
                    // Receiver of flatten must be `.cloned()` on `.get(i)`
                    match flatten_recv.as_ref() {
                        Expr::MethodCall {
                            object: cloned_recv,
                            method: cloned_m,
                            ..
                        } if cloned_m == "cloned" => match cloned_recv.as_ref() {
                            Expr::MethodCall {
                                method: get_m,
                                args: get_args,
                                ..
                            } => {
                                assert_eq!(get_m, "get");
                                assert_eq!(get_args.len(), 1);
                            }
                            other => panic!("expected .get() inner, got {other:?}"),
                        },
                        other => panic!("expected .cloned() before .flatten(), got {other:?}"),
                    }
                }
                other => panic!(
                    "expected .flatten() receiver (NOT .unwrap() which would panic on empty array), got {other:?}"
                ),
            }
        }
        other => panic!("expected outer MethodCall unwrap_or_else, got {other:?}"),
    }
}

#[test]
fn test_convert_nc_chain_vec_of_option_inner_rhs_also_flattens() {
    // `function f(items: (string|null)[], i: number, j: number): string {
    //      return items[i] ?? items[j] ?? "default";
    //  }`
    // Regression test for /check_job deep review finding: inner NC RHS
    // (`items[j]`) must emit `.flatten()` (Option-preserving) under chain
    // context, NOT `.unwrap()` (which would panic in `.or_else()` closure
    // before reaching terminal "default").
    //
    // Required propagate_expected Bin(NC) arm addition + resolve_bin_expr
    // preserve-chain-Option check to make inner RHS span receive Option<String>
    // expected instead of String (inner_val) in chain context.
    //
    // Expected output:
    //   items.get(i).cloned().flatten()
    //        .or_else(|| items.get(j).cloned().flatten())
    //        .unwrap_or_else(|| "default".to_string())
    let f = TctxFixture::from_source(
        "function f(items: (string | null)[], i: number, j: number): string { \
         return items[i] ?? items[j] ?? \"default\"; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Outer: unwrap_or_else (terminal, converts Option<String> → String).
    let (outer_recv, outer_method) = match &result {
        Expr::MethodCall { object, method, .. } => (object.as_ref(), method.as_str()),
        other => panic!("expected outer MethodCall, got {other:?}"),
    };
    assert_eq!(outer_method, "unwrap_or_else");

    // Inner: or_else (chain-preserving).
    let (inner_recv, inner_method, inner_args) = match outer_recv {
        Expr::MethodCall {
            object,
            method,
            args,
        } => (object.as_ref(), method.as_str(), args),
        other => panic!("expected inner MethodCall (.or_else), got {other:?}"),
    };
    assert_eq!(
        inner_method, "or_else",
        "chain inner NC must emit .or_else (Option-preserving), got {inner_method}"
    );

    // Inner LHS: items[i] → .flatten() (Vec<Option<T>> access).
    match inner_recv {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "flatten", "inner LHS must use .flatten()");
        }
        other => panic!("expected inner LHS MethodCall, got {other:?}"),
    }

    // Inner RHS closure: items[j] → .flatten() (NOT .unwrap(), which would panic).
    match &inner_args[0] {
        Expr::Closure {
            body: ClosureBody::Expr(body_expr),
            ..
        } => match body_expr.as_ref() {
            Expr::MethodCall { method, .. } => {
                assert_eq!(
                    method, "flatten",
                    "inner RHS in chain must use .flatten() (not .unwrap() which panics on empty array), got {method}"
                );
            }
            other => panic!("expected inner RHS to be MethodCall, got {other:?}"),
        },
        other => panic!("expected inner RHS Closure, got {other:?}"),
    }
}

#[test]
fn test_convert_nc_option_lhs_with_copy_lit_rhs_emits_unwrap_or() {
    // Baseline: `x ?? 0` where x: Option<f64>. Number literal is copy → eager
    // unwrap_or. Preserves pre-I-022 snapshot for `withDefault`.
    let f = TctxFixture::from_source(
        "function withDefault(x: number | null): number { return x ?? 0; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::MethodCall { method, args, .. } => {
            assert_eq!(method, "unwrap_or");
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::NumberLit(v) if *v == 0.0));
        }
        other => panic!("expected x.unwrap_or(0.0), got {other:?}"),
    }
}
