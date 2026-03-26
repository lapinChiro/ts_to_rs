use super::*;

#[test]
fn test_convert_expr_object_literal_with_type_hint_basic() {
    // { x: 1, y: 2 } with expected Named("Point") from type annotation
    let f = TctxFixture::from_source("const p: Point = { x: 1, y: 2 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expr::NumberLit(1.0)),
                ("y".to_string(), Expr::NumberLit(2.0)),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_mixed_field_types() {
    let f =
        TctxFixture::from_source(r#"const c: Config = { name: "foo", count: 42, active: true };"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Config".to_string(),
            fields: vec![
                ("name".to_string(), Expr::StringLit("foo".to_string())),
                ("count".to_string(), Expr::NumberLit(42.0)),
                ("active".to_string(), Expr::BoolLit(true)),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_single_field() {
    let f = TctxFixture::from_source("const w: Wrapper = { value: 10 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Wrapper".to_string(),
            fields: vec![("value".to_string(), Expr::NumberLit(10.0))],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_empty() {
    let f = TctxFixture::from_source("const e: Empty = {};");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Empty".to_string(),
            fields: vec![],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_without_type_hint_errors() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const obj = { x: 1 };");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&swc_expr);
    assert!(result.is_err());
}

#[test]
fn test_convert_expr_object_spread_last_position_expands_remaining_fields() {
    // { x: 10, ...rest } → Point { x: 10.0, y: rest.y }
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const p: Point = { x: 10, ...rest };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expr::NumberLit(10.0)),
                (
                    "y".to_string(),
                    Expr::FieldAccess {
                        object: Box::new(Expr::Ident("rest".to_string())),
                        field: "y".to_string(),
                    }
                ),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_spread_middle_position_expands_remaining_fields() {
    // { a: 1, ...rest, c: 3 } → S { a: 1.0, c: 3.0, b: rest.b }
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "S".to_string(),
        TypeDef::new_struct(
            vec![
                ("a".to_string(), RustType::F64),
                ("b".to_string(), RustType::F64),
                ("c".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const s: S = { a: 1, ...rest, c: 3 };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "S".to_string(),
            fields: vec![
                ("a".to_string(), Expr::NumberLit(1.0)),
                ("c".to_string(), Expr::NumberLit(3.0)),
                (
                    "b".to_string(),
                    Expr::FieldAccess {
                        object: Box::new(Expr::Ident("rest".to_string())),
                        field: "b".to_string(),
                    }
                ),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_object_spread_unregistered_type_generates_struct_update() {
    // {...a, key: 1} — TypeRegistry 未登録 → struct update syntax
    let f = TctxFixture::from_source("const p: Point = { ...other, x: 10 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![("x".to_string(), Expr::NumberLit(10.0))],
            base: Some(Box::new(Expr::Ident("other".to_string()))),
        }
    );
}

#[test]
fn test_convert_object_spread_multiple_registered_generates_merged_fields() {
    // {...a, ...b} — 複数スプレッド + TypeRegistry 登録済み
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const p: Point = { ...a, ...b };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // 最初の spread (a) はフィールド展開、最後の spread (b) は base
    match &result {
        Expr::StructInit { base, fields, .. } => {
            assert!(base.is_some(), "expected base for last spread");
            // a のフィールドが展開されている
            assert!(
                fields.iter().any(|(k, _)| k == "x" || k == "y"),
                "expected expanded fields from first spread, got {fields:?}"
            );
        }
        other => panic!("expected StructInit, got {other:?}"),
    }
}

#[test]
fn test_convert_expr_object_spread_with_override() {
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const p: Point = { ...other, x: 10 };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Spread expands to field-by-field access: x is overridden, y from base
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expr::NumberLit(10.0)),
                (
                    "y".to_string(),
                    Expr::FieldAccess {
                        object: Box::new(Expr::Ident("other".to_string())),
                        field: "y".to_string(),
                    }
                ),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_call_resolves_object_arg_from_registry() {
    // function draw(p: Point): void {}
    // draw({ x: 0, y: 0 })  →  draw(Point { x: 0.0, y: 0.0 })
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "draw".to_string(),
        TypeDef::Function {
            params: vec![(
                "p".to_string(),
                RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: None,
            has_rest: false,
        },
    );

    let source = "draw({ x: 0, y: 0 });";
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "draw".to_string(),
            args: vec![Expr::StructInit {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), Expr::NumberLit(0.0)),
                    ("y".to_string(), Expr::NumberLit(0.0)),
                ],
                base: None,
            }],
        }
    );
}

#[test]
fn test_convert_expr_object_literal_nested_resolves_field_type_from_registry() {
    // interface Origin { x: number; y: number; }
    // interface Rect { origin: Origin; w: number; }
    // const r: Rect = { origin: { x: 0, y: 0 }, w: 10 }
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Origin".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Rect".to_string(),
        TypeDef::new_struct(
            vec![
                (
                    "origin".to_string(),
                    RustType::Named {
                        name: "Origin".to_string(),
                        type_args: vec![],
                    },
                ),
                ("w".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::from_source_with_reg(
        "const r: Rect = { origin: { x: 0, y: 0 }, w: 10 };",
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Rect".to_string(),
            fields: vec![
                (
                    "origin".to_string(),
                    Expr::StructInit {
                        name: "Origin".to_string(),
                        fields: vec![
                            ("x".to_string(), Expr::NumberLit(0.0)),
                            ("y".to_string(), Expr::NumberLit(0.0)),
                        ],
                        base: None,
                    }
                ),
                ("w".to_string(), Expr::NumberLit(10.0)),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_shorthand_single() {
    // const p: Foo = { x }  →  Foo { x: x }
    let f = TctxFixture::from_source("const p: Foo = { x };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Foo".to_string(),
            fields: vec![("x".to_string(), Expr::Ident("x".to_string()))],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_shorthand_mixed_with_key_value() {
    // const p: Foo = { x, y: 2 }  →  Foo { x: x, y: 2.0 }
    let f = TctxFixture::from_source("const p: Foo = { x, y: 2 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Foo".to_string(),
            fields: vec![
                ("x".to_string(), Expr::Ident("x".to_string())),
                ("y".to_string(), Expr::NumberLit(2.0)),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_shorthand_with_registry_field_type() {
    // const u: User = { name }  where name: String → User { name: name }
    // (Ident values don't get .to_string() — only string literals do)
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "User".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const u: User = { name };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "User".to_string(),
            fields: vec![("name".to_string(), Expr::Ident("name".to_string()))],
            base: None,
        }
    );
}

#[test]
fn test_convert_object_lit_all_computed_keys_generates_hashmap() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // { [key]: "val" } (no type hint) → HashMap::from(vec![(key, "val".to_string())])
    let module =
        crate::parser::parse_typescript(r#"const x: Record<string, string> = { [key]: "val" };"#)
            .unwrap();
    let stmt = match &module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(v))) => {
            &v.decls[0]
        }
        _ => panic!("expected var decl"),
    };
    let init = stmt.init.as_ref().unwrap();
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(init)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "HashMap::from".to_string(),
            args: vec![Expr::Vec {
                elements: vec![Expr::Tuple {
                    elements: vec![
                        Expr::Ident("key".to_string()),
                        Expr::StringLit("val".to_string()),
                    ],
                }],
            }],
        }
    );
}
