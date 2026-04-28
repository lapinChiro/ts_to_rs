use super::*;
use crate::registry::MethodKind;

#[test]
fn test_convert_fn_decl_default_number_param_wraps_in_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(x: number = 0): void {}");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            // Parameter type should be Option<f64>
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
            // Body should start with `let x = x.unwrap_or(0.0);`
            assert!(
                !body.is_empty(),
                "body should contain unwrap_or expansion statement"
            );
            match &body[0] {
                Stmt::Let {
                    name,
                    init,
                    mutable,
                    ..
                } => {
                    assert_eq!(name, "x");
                    assert!(!mutable);
                    // init should be a method call: x.unwrap_or(0.0)
                    match init.as_ref().unwrap() {
                        Expr::MethodCall {
                            object,
                            method,
                            args,
                        } => {
                            assert_eq!(method, "unwrap_or");
                            assert!(matches!(object.as_ref(), Expr::Ident(n) if n == "x"));
                            assert_eq!(args.len(), 1);
                            assert!(
                                matches!(&args[0], Expr::NumberLit(n) if *n == 0.0),
                                "expected NumberLit(0.0), got {:?}",
                                &args[0]
                            );
                        }
                        other => panic!("expected MethodCall, got {other:?}"),
                    }
                }
                other => panic!("expected Let statement, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_string_param_wraps_in_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(name: string = \"hello\"): void {}");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::String)))
            );
            match &body[0] {
                Stmt::Let { name, init, .. } => {
                    assert_eq!(name, "name");
                    match init.as_ref().unwrap() {
                        Expr::MethodCall { method, args, .. } => {
                            assert_eq!(method, "unwrap_or_else");
                            // arg should be || "hello".to_string()
                            assert_eq!(args.len(), 1);
                            match &args[0] {
                                Expr::Closure { body, .. } => match body {
                                    crate::ir::ClosureBody::Expr(expr) => match expr.as_ref() {
                                        Expr::MethodCall { object, method, .. } => {
                                            assert_eq!(method, "to_string");
                                            assert!(matches!(
                                                object.as_ref(),
                                                Expr::StringLit(s) if s == "hello"
                                            ));
                                        }
                                        other => panic!("expected MethodCall, got {other:?}"),
                                    },
                                    other => panic!("expected ClosureBody::Expr, got {other:?}"),
                                },
                                other => panic!("expected Closure, got {other:?}"),
                            }
                        }
                        other => panic!("expected MethodCall, got {other:?}"),
                    }
                }
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_bool_param_wraps_in_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(flag: boolean = true): void {}");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::Bool)))
            );
            match &body[0] {
                Stmt::Let { init, .. } => match init.as_ref().unwrap() {
                    Expr::MethodCall { method, args, .. } => {
                        assert_eq!(method, "unwrap_or");
                        assert!(matches!(&args[0], Expr::BoolLit(true)));
                    }
                    other => panic!("expected MethodCall, got {other:?}"),
                },
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_empty_object_uses_unwrap_or_default() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(options: Config = {}): void {}");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::Named {
                    name: "Config".to_string(),
                    type_args: vec![],
                })))
            );
            match &body[0] {
                Stmt::Let { init, .. } => match init.as_ref().unwrap() {
                    Expr::MethodCall { method, args, .. } => {
                        assert_eq!(method, "unwrap_or_default");
                        assert!(args.is_empty());
                    }
                    other => panic!("expected MethodCall, got {other:?}"),
                },
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_param_mixed_with_normal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(a: number, b: number = 10): void {}");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            // First param: normal
            assert_eq!(params[0].name, "a");
            assert_eq!(params[0].ty, Some(RustType::F64));
            // Second param: Option<f64>
            assert_eq!(params[1].name, "b");
            assert_eq!(
                params[1].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
            // Body should have unwrap_or expansion for b
            match &body[0] {
                Stmt::Let { name, .. } => assert_eq!(name, "b"),
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_new_expr_uses_unwrap_or_default() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // new Map() → unwrap_or_default()
    let fn_decl = parse_fn_decl("function foo(m: Map = new Map()): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert!(!items.is_empty());
}

#[test]
fn test_convert_fn_decl_default_variable_ref_uses_unwrap_or() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // = baseMimes → unwrap_or(baseMimes)
    let fn_decl = parse_fn_decl("function foo(x: number = defaultVal): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert!(!items.is_empty());
}

#[test]
fn test_convert_fn_decl_default_empty_array_uses_unwrap_or_default() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // = [] → unwrap_or_default()
    let fn_decl = parse_fn_decl("function foo(x: string[] = []): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert!(!items.is_empty());
}

#[test]
fn test_convert_fn_decl_default_negative_number() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // = -1 → unwrap_or(-1.0)
    let fn_decl = parse_fn_decl("function foo(x: number = -1): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert!(!items.is_empty());
}

#[test]
fn test_convert_fn_decl_rest_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ...args: number[] → args: Vec<f64>
    let fn_decl = parse_fn_decl("function foo(...args: number[]): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match &items[0] {
        Item::Fn { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_inline_type_literal_single_field_generates_struct() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(opts: { x: number }): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Public,
            name: "FooOpts".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "x".to_string(),
                ty: RustType::F64,
            }],
            is_unit_struct: false,
        }
    );
    match &items[1] {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Named {
                    name: "FooOpts".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_inline_type_literal_multiple_fields_generates_struct() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function bar(config: { x: number, y: string }): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Public,
            name: "BarConfig".to_string(),
            type_params: vec![],
            fields: vec![
                StructField {
                    vis: Some(Visibility::Public),
                    name: "x".to_string(),
                    ty: RustType::F64,
                },
                StructField {
                    vis: Some(Visibility::Public),
                    name: "y".to_string(),
                    ty: RustType::String,
                },
            ],
            is_unit_struct: false,
        }
    );
}

#[test]
fn test_convert_fn_decl_inline_type_literal_mixed_with_normal_param_generates_struct() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function baz(name: string, opts: { x: number }): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert_eq!(items.len(), 2);
    match &items[0] {
        Item::Struct { name, .. } => assert_eq!(name, "BazOpts"),
        _ => panic!("expected Item::Struct"),
    }
    match &items[1] {
        Item::Fn { params, .. } => {
            assert_eq!(params[0].ty, Some(RustType::String));
            assert_eq!(
                params[1].ty,
                Some(RustType::Named {
                    name: "BazOpts".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_inline_type_literal_empty_generates_empty_struct() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function qux(opts: {}): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Public,
            name: "QuxOpts".to_string(),
            type_params: vec![],
            fields: vec![],
            is_unit_struct: false,
        }
    );
}

#[test]
fn test_convert_fn_decl_default_param_inline_type_generates_struct() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_fn_decl("function f(x: { a: string } = {}): void { }");
    let (items, _warnings) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&decl, Visibility::Public, false)
        .unwrap();
    // Should produce: struct + function
    let has_struct = items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "FX"));
    assert!(
        has_struct,
        "inline type literal in default param should generate struct, got: {items:?}"
    );
}

#[test]
fn test_convert_fn_decl_inline_type_literal_optional_field_wraps_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(opts: { name?: string }): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].name, "name");
            assert_eq!(
                fields[0].ty,
                RustType::Option(Box::new(RustType::String)),
                "optional field should be Option<String>"
            );
        }
        other => panic!("expected Item::Struct, got {other:?}"),
    }
}

#[test]
fn test_convert_fn_decl_inline_type_literal_optional_nullable_no_double_wrap() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // name?: string | null → should be Option<String>, NOT Option<Option<String>>
    let fn_decl = parse_fn_decl("function foo(opts: { name?: string | null }): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].name, "name");
            assert_eq!(
                fields[0].ty,
                RustType::Option(Box::new(RustType::String)),
                "optional nullable field should be Option<String>, not Option<Option<String>>"
            );
        }
        other => panic!("expected Item::Struct, got {other:?}"),
    }
}

/// Helper: create a TypeRegistry with a trait type (methods-only interface).
fn reg_with_trait(name: &str) -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String).into()],
            return_type: None,
            has_rest: false,
            type_params: vec![],
            kind: MethodKind::Method,
        }],
    );
    reg.register(
        name.to_string(),
        TypeDef::new_interface(vec![], vec![], methods, vec![]),
    );
    reg
}

#[test]
fn test_convert_fn_param_trait_type_generates_dyn_ref() {
    // function foo(g: Greeter): void { } → fn foo(g: &dyn Greeter) { }
    let reg = reg_with_trait("Greeter");
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(g: Greeter): void { }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Ref(Box::new(RustType::DynTrait(
                    "Greeter".to_string()
                ))))
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_return_trait_type_generates_box_dyn() {
    // function foo(): Greeter { ... } → fn foo() -> Box<dyn Greeter> { ... }
    let reg = reg_with_trait("Greeter");
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(): Greeter { return null as any; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    match items.last().unwrap() {
        Item::Fn { return_type, .. } => {
            // I-387: Box は StdCollection variant で構造化される。
            assert_eq!(
                *return_type,
                Some(RustType::StdCollection {
                    kind: crate::ir::StdCollectionKind::Box,
                    args: vec![RustType::DynTrait("Greeter".to_string())],
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_param_struct_type_unchanged() {
    // struct type (has fields, not trait) should NOT be wrapped in &dyn
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
            ],
            HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(p: Point): void { }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            // Should remain as Named, not &dyn
            assert_eq!(
                params[0].ty,
                Some(RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_default_param_number_no_annotation_infers_f64() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // No type annotation, default is 0 → infer f64
    let fn_decl = parse_fn_decl("function foo(x = 0): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_default_param_string_no_annotation_infers_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(x = \"hello\"): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::String)))
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_default_param_bool_no_annotation_infers_bool() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(x = true): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::Bool)))
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_optional_param_wraps_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function foo(x: number, y?: string): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match items.last().unwrap() {
        Item::Fn { params, body, .. } => {
            // First param: number → f64 (not optional)
            assert_eq!(params[0].ty, Some(RustType::F64));
            // Second param: string? → Option<String>
            assert_eq!(
                params[1].ty,
                Some(RustType::Option(Box::new(RustType::String)))
            );
            // Optional params without default should NOT generate expansion stmts
            assert!(
                body.is_empty(),
                "optional param without default should not generate expansion statements"
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_optional_param_already_option_no_double_wrap() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // y?: string | null → should be Option<String>, NOT Option<Option<String>>
    let fn_decl = parse_fn_decl("function foo(y?: string | null): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::String))),
                "optional nullable param should be Option<String>, not Option<Option<String>>"
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_optional_param_no_type_annotation_wraps_option_any() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x? (no type annotation) → Option<Any>
    let fn_decl = parse_fn_decl("function foo(x?): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::Any))),
                "optional param without type annotation should be Option<Any>"
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_optional_param_inline_type_literal_wraps_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // opts?: { name: string } → Option<Named>
    let fn_decl = parse_fn_decl("function foo(opts?: { name: string }): void {}");
    let (items, _) = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    match items.last().unwrap() {
        Item::Fn { params, .. } => match &params[0].ty {
            Some(RustType::Option(inner)) => {
                assert!(
                    matches!(inner.as_ref(), RustType::Named { .. }),
                    "expected Option<Named>, got Option<{:?}>",
                    inner
                );
            }
            other => panic!(
                "expected Option<Named> for optional inline type literal, got {:?}",
                other
            ),
        },
        _ => panic!("expected Item::Fn"),
    }
}
