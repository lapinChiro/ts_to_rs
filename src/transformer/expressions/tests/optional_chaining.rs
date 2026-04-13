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
fn test_convert_nullish_coalescing_non_option_returns_left() {
    let f = TctxFixture::from_source("function f(x: string, y: string) { x ?? y; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
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
