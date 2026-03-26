use super::*;

#[test]
fn test_convert_fn_decl_object_destructuring_param_generates_expansion() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ x, y }: Point): void { console.log(x); }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();

    match item {
        Item::Fn { params, body, .. } => {
            // Parameter should be renamed to snake_case of the type
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "point");
            assert_eq!(
                params[0].ty,
                Some(RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
            // Body should start with expansion statements
            assert!(body.len() >= 2);
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "x".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident("point".to_string())),
                        field: "x".to_string(),
                    }),
                }
            );
            assert_eq!(
                body[1],
                Stmt::Let {
                    mutable: false,
                    name: "y".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident("point".to_string())),
                        field: "y".to_string(),
                    }),
                }
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_object_destructuring_rename() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ x: newX, y: newY }: Point): void {}");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();

    match item {
        Item::Fn { body, .. } => {
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "new_x".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident("point".to_string())),
                        field: "x".to_string(),
                    }),
                }
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_destructuring_with_normal_params() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(name: string, { x, y }: Point): void {}");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();

    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "name");
            assert_eq!(params[1].name, "point");
            // Expansion statements in body
            assert_eq!(body.len(), 2);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_destructuring_no_type_annotation_falls_back_to_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ x, y }): void {}");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_fn_decl(
        &fn_decl,
        Visibility::Public,
        false,
    );
    assert!(
        result.is_ok(),
        "object destructuring without type annotation should fallback to serde_json::Value"
    );
}

#[test]
fn test_object_destructuring_param_default_number_generates_unwrap_or() {
    let fn_decl = parse_fn_decl("function f({ x = 0 }: { x?: number }): void { console.log(x); }");
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Private, false)
        .unwrap();
    match &items[0] {
        Item::Fn { body, .. } => {
            // First statement should be the destructuring expansion with unwrap_or
            assert!(
                !body.is_empty(),
                "expected at least 1 body statement, got {}",
                body.len()
            );
            match &body[0] {
                Stmt::Let {
                    name,
                    init: Some(expr),
                    ..
                } => {
                    assert_eq!(name, "x");
                    assert!(
                        matches!(expr, Expr::MethodCall { method, .. } if method == "unwrap_or"),
                        "expected unwrap_or call, got: {:?}",
                        expr
                    );
                }
                other => panic!("expected Let with unwrap_or, got: {:?}", other),
            }
        }
        other => panic!("expected Fn item, got: {:?}", other),
    }
}

// --- Nested destructuring rest parameter (I-244) ---

/// Helper: build TypeRegistry with Outer { inner: Inner } and Inner { a: String, b: f64, c: bool }.
fn reg_with_outer_inner() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Inner".to_string(),
        TypeDef::new_struct(
            vec![
                ("a".to_string(), RustType::String),
                ("b".to_string(), RustType::F64),
                ("c".to_string(), RustType::Bool),
            ],
            HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Outer".to_string(),
        TypeDef::new_struct(
            vec![(
                "inner".to_string(),
                RustType::Named {
                    name: "Inner".to_string(),
                    type_args: vec![],
                },
            )],
            HashMap::new(),
            vec![],
        ),
    );
    reg
}

#[test]
fn test_nested_destructuring_rest_with_type_info_generates_struct_init() {
    let reg = reg_with_outer_inner();
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ inner: { a, ...rest } }: Outer): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let (items, _) = Transformer::for_module(&tctx, &mut synthetic)
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();

    match items.last().unwrap() {
        Item::Fn { body, params, .. } => {
            // Parameter should be outer: Outer
            assert_eq!(params[0].name, "outer");

            // body[0] = let a = outer.inner.a;
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "a".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("outer".to_string())),
                            field: "inner".to_string(),
                        }),
                        field: "a".to_string(),
                    }),
                }
            );

            // body[1] = let rest = _TypeLitN { b: outer.inner.b, c: outer.inner.c };
            match &body[1] {
                Stmt::Let {
                    name,
                    init:
                        Some(Expr::StructInit {
                            name: struct_name,
                            fields,
                            base,
                        }),
                    ..
                } => {
                    assert_eq!(name, "rest");
                    assert!(
                        struct_name.starts_with("_TypeLit"),
                        "expected synthetic struct name, got {struct_name}"
                    );
                    assert!(base.is_none());
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].0, "b");
                    assert_eq!(fields[1].0, "c");
                }
                other => panic!("expected Let with StructInit, got {other:?}"),
            }
        }
        other => panic!("expected Item::Fn, got {other:?}"),
    }

    // Synthetic rest struct should be registered
    let rest_types: Vec<_> = synthetic
        .all_items()
        .into_iter()
        .filter(|item| matches!(item, Item::Struct { .. }))
        .collect();
    assert!(
        !rest_types.is_empty(),
        "expected synthetic rest struct to be registered"
    );
}

#[test]
fn test_nested_destructuring_rest_all_fields_explicit_generates_empty_struct() {
    let reg = reg_with_outer_inner();
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ inner: { a, b, c, ...rest } }: Outer): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let (items, _) = Transformer::for_module(&tctx, &mut synthetic)
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();

    match items.last().unwrap() {
        Item::Fn { body, .. } => {
            // body should have 3 field expansions (a, b, c) + 1 empty rest struct
            let rest_stmt = body.iter().find(|s| match s {
                Stmt::Let { name, .. } => name == "rest",
                _ => false,
            });
            match rest_stmt {
                Some(Stmt::Let {
                    init: Some(Expr::StructInit { fields, .. }),
                    ..
                }) => {
                    assert_eq!(
                        fields.len(),
                        0,
                        "rest should be empty struct when all fields are explicit"
                    );
                }
                other => panic!("expected Let with empty StructInit, got {other:?}"),
            }
        }
        other => panic!("expected Item::Fn, got {other:?}"),
    }
}

#[test]
fn test_nested_destructuring_rest_without_type_info_returns_error() {
    // Outer type not registered → field type unknown → UnsupportedSyntaxError
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ inner: { a, ...rest } }: Outer): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let result = Transformer::for_module(&tctx, &mut synthetic).convert_fn_decl(
        &fn_decl,
        Visibility::Public,
        false,
    );

    assert!(
        result.is_err(),
        "should fail when type info is unavailable for nested rest"
    );
}
