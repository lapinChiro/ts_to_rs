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

// --- Default value 3-way split tests ---

#[test]
fn test_object_destructuring_param_default_string_lit_generates_unwrap_or_else() {
    let fn_decl =
        parse_fn_decl("function f({ x = \"hello\" }: { x?: string }): void { console.log(x); }");
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Private, false)
        .unwrap();

    match &items[0] {
        Item::Fn { body, .. } => {
            assert!(!body.is_empty(), "expected at least 1 body statement");
            match &body[0] {
                Stmt::Let {
                    name,
                    init: Some(expr),
                    ..
                } => {
                    assert_eq!(name, "x");
                    assert!(
                        matches!(expr, Expr::MethodCall { method, .. } if method == "unwrap_or_else"),
                        "string literal default should use unwrap_or_else, got: {:?}",
                        expr
                    );
                    // Verify the closure wraps a StringLit
                    if let Expr::MethodCall { args, .. } = expr {
                        assert_eq!(args.len(), 1);
                        assert!(
                            matches!(&args[0], Expr::Closure { body: crate::ir::ClosureBody::Expr(inner), .. } if matches!(inner.as_ref(), Expr::StringLit(_))),
                            "closure body should contain StringLit"
                        );
                    }
                }
                other => panic!("expected Let with unwrap_or_else, got: {:?}", other),
            }
        }
        other => panic!("expected Fn item, got: {:?}", other),
    }
}

#[test]
fn test_object_destructuring_param_default_to_string_generates_unwrap_or_else() {
    // The 3-way split matches `Expr::MethodCall { method: "to_string", .. }`.
    // TS `.trim()` converts to IR `.trim().to_string()`, so the outermost IR
    // MethodCall has method == "to_string", triggering the unwrap_or_else branch.
    let fn_decl = parse_fn_decl("function f({ x = val.trim() }: { x?: string }): void {}");
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Private, false)
        .unwrap();

    match &items[0] {
        Item::Fn { body, .. } => {
            assert!(!body.is_empty(), "expected at least 1 body statement");
            match &body[0] {
                Stmt::Let {
                    name,
                    init: Some(expr),
                    ..
                } => {
                    assert_eq!(name, "x");
                    assert!(
                        matches!(expr, Expr::MethodCall { method, .. } if method == "unwrap_or_else"),
                        "to_string() default should use unwrap_or_else, got: {:?}",
                        expr
                    );
                    // Verify the closure wraps a to_string method call
                    if let Expr::MethodCall { args, .. } = expr {
                        assert_eq!(args.len(), 1);
                        assert!(
                            matches!(
                                &args[0],
                                Expr::Closure {
                                    body: crate::ir::ClosureBody::Expr(inner),
                                    ..
                                } if matches!(inner.as_ref(), Expr::MethodCall { method, .. } if method == "to_string")
                            ),
                            "closure body should contain to_string call"
                        );
                    }
                }
                other => panic!("expected Let with unwrap_or_else, got: {:?}", other),
            }
        }
        other => panic!("expected Fn item, got: {:?}", other),
    }
}

#[test]
fn test_object_destructuring_param_default_other_generates_unwrap_or() {
    // Uses 42 (non-zero number) to differentiate from the existing test_object_destructuring_param_default_number_generates_unwrap_or
    // which uses 0. This verifies the generic "other" branch produces unwrap_or for any non-string, non-toString default.
    let fn_decl = parse_fn_decl("function f({ x = 42 }: { x?: number }): void { console.log(x); }");
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Private, false)
        .unwrap();

    match &items[0] {
        Item::Fn { body, .. } => {
            assert!(!body.is_empty());
            match &body[0] {
                Stmt::Let {
                    name,
                    init: Some(expr),
                    ..
                } => {
                    assert_eq!(name, "x");
                    assert!(
                        matches!(expr, Expr::MethodCall { method, .. } if method == "unwrap_or"),
                        "numeric default should use unwrap_or, got: {:?}",
                        expr
                    );
                    // Verify the default value is passed directly (not in a closure)
                    if let Expr::MethodCall { args, .. } = expr {
                        assert_eq!(args.len(), 1);
                        assert!(
                            matches!(&args[0], Expr::NumberLit(_)),
                            "unwrap_or arg should be a NumberLit, got: {:?}",
                            args[0]
                        );
                    }
                }
                other => panic!("expected Let with unwrap_or, got: {:?}", other),
            }
        }
        other => panic!("expected Fn item, got: {:?}", other),
    }
}

// --- Nest and rest pattern tests ---

#[test]
fn test_object_destructuring_param_nested_object_generates_recursive_expansion() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "T".to_string(),
        TypeDef::new_struct(
            vec![(
                "a".to_string(),
                RustType::Named {
                    name: "Inner".to_string(),
                    type_args: vec![],
                },
            )],
            HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Inner".to_string(),
        TypeDef::new_struct(
            vec![
                ("b".to_string(), RustType::String),
                ("c".to_string(), RustType::F64),
            ],
            HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ a: { b, c } }: T): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();

    match items.last().unwrap() {
        Item::Fn { body, params, .. } => {
            assert_eq!(params[0].name, "t");
            // body[0] = let b = t.a.b;
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "b".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("t".to_string())),
                            field: "a".to_string(),
                        }),
                        field: "b".to_string(),
                    }),
                }
            );
            // body[1] = let c = t.a.c;
            assert_eq!(
                body[1],
                Stmt::Let {
                    mutable: false,
                    name: "c".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("t".to_string())),
                            field: "a".to_string(),
                        }),
                        field: "c".to_string(),
                    }),
                }
            );
        }
        other => panic!("expected Item::Fn, got {:?}", other),
    }
}

#[test]
fn test_object_destructuring_param_rest_generates_synthetic_struct() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
                ("z".to_string(), RustType::F64),
            ],
            HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ x, ...rest }: Point): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let (items, _) = Transformer::for_module(&tctx, &mut synthetic)
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();

    match items.last().unwrap() {
        Item::Fn { body, .. } => {
            // body[0] = let x = point.x;
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
            // body[1] = let rest = _TypeLitN { y: point.y, z: point.z };
            match &body[1] {
                Stmt::Let {
                    name,
                    init:
                        Some(Expr::StructInit {
                            name: struct_name,
                            fields,
                            ..
                        }),
                    ..
                } => {
                    assert_eq!(name, "rest");
                    assert!(
                        struct_name.starts_with("_TypeLit"),
                        "expected synthetic struct name, got {struct_name}"
                    );
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].0, "y");
                    assert_eq!(fields[1].0, "z");
                }
                other => panic!("expected Let with StructInit, got {:?}", other),
            }
        }
        other => panic!("expected Item::Fn, got {:?}", other),
    }

    // Verify synthetic struct was registered
    let struct_items: Vec<_> = synthetic
        .all_items()
        .into_iter()
        .filter(|item| matches!(item, Item::Struct { .. }))
        .collect();
    assert!(
        !struct_items.is_empty(),
        "expected synthetic rest struct to be registered"
    );
}

#[test]
fn test_object_destructuring_param_rest_excludes_explicit_fields() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        TypeDef::new_struct(
            vec![
                ("a".to_string(), RustType::String),
                ("b".to_string(), RustType::F64),
                ("c".to_string(), RustType::Bool),
                ("d".to_string(), RustType::String),
            ],
            HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ a, b, ...rest }: Config): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let (items, _) = Transformer::for_module(&tctx, &mut synthetic)
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();

    match items.last().unwrap() {
        Item::Fn { body, .. } => {
            // Find the rest statement
            let rest_stmt = body
                .iter()
                .find(|s| matches!(s, Stmt::Let { name, .. } if name == "rest"));
            match rest_stmt {
                Some(Stmt::Let {
                    init: Some(Expr::StructInit { fields, .. }),
                    ..
                }) => {
                    // Only c and d should remain (a and b are explicitly destructured)
                    let field_names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
                    assert_eq!(
                        field_names,
                        vec!["c", "d"],
                        "rest struct should exclude explicitly destructured fields a and b"
                    );
                }
                other => panic!("expected Let with StructInit for rest, got {:?}", other),
            }
        }
        other => panic!("expected Item::Fn, got {:?}", other),
    }
}

#[test]
fn test_object_destructuring_param_rest_unknown_type_returns_error() {
    // Type "UnknownType" is not registered in the registry
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo({ x, ...rest }: UnknownType): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let result = Transformer::for_module(&tctx, &mut synthetic).convert_fn_decl(
        &fn_decl,
        Visibility::Public,
        false,
    );

    assert!(
        result.is_err(),
        "rest pattern with unknown type should return error"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found in registry"),
        "error should mention type not found in registry, got: {err_msg}"
    );
}

// --- lookup_field_type tests ---

#[test]
fn test_object_destructuring_param_nested_with_known_type_resolves_field_types() {
    // When the parent type is a Named type with known fields,
    // lookup_field_type should find the field type from the registry.
    // We test this indirectly: nested destructuring with rest requires field type resolution.
    let mut reg = TypeRegistry::new();
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
    reg.register(
        "Inner".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::String),
            ],
            HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    // Nested destructuring with rest requires lookup_field_type to resolve Inner's fields
    let fn_decl = parse_fn_decl("function foo({ inner: { x, ...rest } }: Outer): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let (items, _) = Transformer::for_module(&tctx, &mut synthetic)
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();

    match items.last().unwrap() {
        Item::Fn { body, .. } => {
            // body[0] = let x = outer.inner.x;
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "x".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("outer".to_string())),
                            field: "inner".to_string(),
                        }),
                        field: "x".to_string(),
                    }),
                }
            );
            // body[1] = let rest = _TypeLitN { y: outer.inner.y };
            match &body[1] {
                Stmt::Let {
                    name,
                    init: Some(Expr::StructInit { fields, .. }),
                    ..
                } => {
                    assert_eq!(name, "rest");
                    assert_eq!(fields.len(), 1, "rest should contain only the 'y' field");
                    assert_eq!(fields[0].0, "y");
                }
                other => panic!("expected Let with StructInit, got {:?}", other),
            }
        }
        other => panic!("expected Item::Fn, got {:?}", other),
    }
}

#[test]
fn test_object_destructuring_param_nested_option_type_unwraps_inner() {
    // When the field type is Option<Named>, lookup_field_type should unwrap
    // to the inner Named type and look up its fields.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Wrapper".to_string(),
        TypeDef::new_struct(
            vec![(
                "data".to_string(),
                RustType::Option(Box::new(RustType::Named {
                    name: "Data".to_string(),
                    type_args: vec![],
                })),
            )],
            HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Data".to_string(),
        TypeDef::new_struct(
            vec![
                ("a".to_string(), RustType::String),
                ("b".to_string(), RustType::F64),
            ],
            HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    // data field is Option<Data>, nested destructuring should still resolve fields
    let fn_decl = parse_fn_decl("function foo({ data: { a, ...rest } }: Wrapper): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let (items, _) = Transformer::for_module(&tctx, &mut synthetic)
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();

    match items.last().unwrap() {
        Item::Fn { body, .. } => {
            // body[0] = let a = wrapper.data.a;
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "a".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("wrapper".to_string())),
                            field: "data".to_string(),
                        }),
                        field: "a".to_string(),
                    }),
                }
            );
            // body[1] = let rest = _TypeLitN { b: wrapper.data.b };
            match &body[1] {
                Stmt::Let {
                    name,
                    init: Some(Expr::StructInit { fields, .. }),
                    ..
                } => {
                    assert_eq!(name, "rest");
                    assert_eq!(fields.len(), 1, "rest should contain only the 'b' field");
                    assert_eq!(fields[0].0, "b");
                }
                other => panic!("expected Let with StructInit, got {:?}", other),
            }
        }
        other => panic!("expected Item::Fn, got {:?}", other),
    }
}

#[test]
fn test_object_destructuring_param_nested_unknown_type_skips_field_lookup() {
    // When the parent type is not Named or Option<Named> (e.g., just a Vec or primitive),
    // lookup_field_type returns None.
    // Without type info, nested rest should fail with UnsupportedSyntaxError.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Container".to_string(),
        TypeDef::new_struct(
            vec![(
                "items".to_string(),
                RustType::Vec(Box::new(RustType::String)), // Vec type — not Named, not Option
            )],
            HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    // items field is Vec<String>, lookup_field_type returns None, rest expansion should fail
    let fn_decl = parse_fn_decl("function foo({ items: { a, ...rest } }: Container): void {}");
    let mut synthetic = SyntheticTypeRegistry::new();
    let result = Transformer::for_module(&tctx, &mut synthetic).convert_fn_decl(
        &fn_decl,
        Visibility::Public,
        false,
    );

    assert!(
        result.is_err(),
        "nested rest with non-Named/Option field type should fail"
    );
}
