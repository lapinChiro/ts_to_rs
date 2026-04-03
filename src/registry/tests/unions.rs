use super::*;

// --- string literal union enum registration ---

#[test]
fn test_build_registry_string_literal_union_registers_enum() {
    let module = parse_typescript(r#"type Direction = "up" | "down" | "left" | "right";"#).unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("Direction")
        .expect("Direction should be registered");
    match def {
        TypeDef::Enum {
            variants,
            string_values,
            ..
        } => {
            assert_eq!(variants, &["Up", "Down", "Left", "Right"]);
            assert_eq!(string_values.get("up").unwrap(), "Up");
            assert_eq!(string_values.get("down").unwrap(), "Down");
            assert_eq!(string_values.get("left").unwrap(), "Left");
            assert_eq!(string_values.get("right").unwrap(), "Right");
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_build_registry_ts_enum_has_empty_string_values() {
    let module = parse_typescript("enum Color { Red, Green, Blue }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Color").unwrap() {
        TypeDef::Enum { string_values, .. } => {
            assert!(
                string_values.is_empty(),
                "TS enum should have empty string_values"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

// --- discriminated union registration ---

#[test]
fn test_build_registry_discriminated_union_registers_enum() {
    let module = parse_typescript(
        r#"type Shape = { kind: "circle", radius: number } | { kind: "square", side: number };"#,
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Shape").expect("Shape should be registered");
    match def {
        TypeDef::Enum {
            type_params: _,
            variants,
            string_values,
            tag_field,
            variant_fields,
        } => {
            assert_eq!(variants, &["Circle", "Square"]);
            assert_eq!(tag_field.as_deref(), Some("kind"));
            assert_eq!(string_values.get("circle").unwrap(), "Circle");
            assert_eq!(string_values.get("square").unwrap(), "Square");
            let circle_fields = variant_fields.get("Circle").expect("Circle variant");
            assert_eq!(
                circle_fields,
                &[("radius".to_string(), RustType::F64).into()]
            );
            let square_fields = variant_fields.get("Square").expect("Square variant");
            assert_eq!(square_fields, &[("side".to_string(), RustType::F64).into()]);
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_build_registry_discriminated_union_unit_variant() {
    let module =
        parse_typescript(r#"type Status = { type: "active" } | { type: "inactive" };"#).unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Status").expect("Status should be registered");
    match def {
        TypeDef::Enum {
            variants,
            tag_field,
            variant_fields,
            ..
        } => {
            assert_eq!(variants, &["Active", "Inactive"]);
            assert_eq!(tag_field.as_deref(), Some("type"));
            assert!(
                variant_fields.get("Active").unwrap().is_empty(),
                "unit variant should have no fields"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

// --- synthetic union ---

#[test]
fn test_build_registry_with_union_field() {
    let module = crate::parser::parse_typescript("interface Foo { x: string | number; }").unwrap();
    let mut synthetic = SyntheticTypeRegistry::new();
    let reg = build_registry_with_synthetic(&module, &mut synthetic);

    let foo = reg.get("Foo");
    assert!(foo.is_some(), "Foo should be in registry");

    if let Some(TypeDef::Struct { fields, .. }) = foo {
        assert_eq!(fields.len(), 1, "Foo should have 1 field");
        let field = &fields[0];
        assert_eq!(field.name, "x");
        assert!(
            matches!(field.ty, RustType::Named { .. }),
            "x should be a Named type (synthetic enum), got: {:?}",
            field.ty
        );
    } else {
        panic!("Foo should be a Struct");
    }

    assert!(
        !synthetic.all_items().is_empty(),
        "SyntheticTypeRegistry should contain the union enum"
    );
}

#[test]
fn test_build_registry_union_dedup() {
    let module = crate::parser::parse_typescript(
        "interface A { x: string | number; } interface B { y: string | number; }",
    )
    .unwrap();
    let mut synthetic = SyntheticTypeRegistry::new();
    let _reg = build_registry_with_synthetic(&module, &mut synthetic);

    let enum_count = synthetic
        .all_items()
        .iter()
        .filter(|item| matches!(item, crate::ir::Item::Enum { .. }))
        .count();
    assert_eq!(
        enum_count, 1,
        "same union type should produce only 1 enum (dedup)"
    );
}

// --- any narrowing ---

#[test]
fn test_analyze_any_params_registers_enum() {
    use crate::pipeline::any_narrowing::{build_any_enum_variants, collect_any_constraints};
    use swc_ecma_ast as ast;

    let module = crate::parser::parse_typescript(
        r#"function foo(x: any) { if (typeof x === "string") { return x; } }"#,
    )
    .unwrap();
    let reg = build_registry(&module);

    let foo_def = reg.get("foo");
    assert!(foo_def.is_some(), "foo should be in registry");
    if let Some(TypeDef::Function { params, .. }) = foo_def {
        assert!(
            params.iter().any(|f| matches!(f.ty, RustType::Any)),
            "foo should have an any-typed parameter"
        );
    }

    if let Some(ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl)))) =
        module.body.first()
    {
        if let Some(body) = &fn_decl.function.body {
            let constraints = collect_any_constraints(body, &["x".to_string()]);
            if let Some(constraint) = constraints.get("x") {
                let variants = build_any_enum_variants(constraint);
                assert!(
                    !variants.is_empty(),
                    "should generate variants for any-typed parameter"
                );
            }
        }
    }
}

// --- collection phase unit tests (TsTypeInfo) ---

/// TypeScript の type alias 宣言をパースして `TsTypeAliasDecl` を返す。
fn parse_type_alias(source: &str) -> swc_ecma_ast::TsTypeAliasDecl {
    let module = parse_typescript(source).unwrap();
    for item in &module.body {
        if let swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
            swc_ecma_ast::Decl::TsTypeAlias(alias),
        )) = item
        {
            return *alias.clone();
        }
    }
    panic!("no TsTypeAliasDecl found in source: {source}");
}

#[test]
fn test_collect_string_literal_union_stores_raw_strings() {
    let alias = parse_type_alias(r#"type Dir = "up" | "down";"#);
    let ts_def = super::super::unions::try_collect_string_literal_union(&alias)
        .expect("should detect string literal union");

    match ts_def {
        TypeDef::Enum {
            variants,
            string_values,
            ..
        } => {
            // Collection phase stores raw strings, NOT PascalCase
            assert_eq!(variants, vec!["up".to_string(), "down".to_string()]);
            assert_eq!(string_values.get("up"), Some(&"up".to_string()));
            assert_eq!(string_values.get("down"), Some(&"down".to_string()));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_collect_discriminated_union_stores_raw_strings_and_ts_type_info() {
    use crate::ts_type_info::TsTypeInfo;

    let alias = parse_type_alias(
        r#"type Shape = { kind: "circle"; r: number } | { kind: "square"; s: string };"#,
    );
    let ts_def = super::super::unions::try_collect_discriminated_union(&alias)
        .expect("should detect discriminated union");

    match ts_def {
        TypeDef::Enum {
            variants,
            string_values,
            tag_field,
            variant_fields,
            ..
        } => {
            // Collection phase stores raw strings, NOT PascalCase
            assert_eq!(variants, vec!["circle".to_string(), "square".to_string()]);
            assert_eq!(string_values.get("circle"), Some(&"circle".to_string()));
            assert_eq!(tag_field, Some("kind".to_string()));

            // variant_fields keys are raw strings
            let circle = variant_fields.get("circle").expect("circle variant");
            assert_eq!(circle.len(), 1);
            assert_eq!(circle[0].name, "r");
            // Field types are TsTypeInfo, NOT RustType
            assert_eq!(circle[0].ty, TsTypeInfo::Number);
            assert!(!circle[0].optional);

            let square = variant_fields.get("square").expect("square variant");
            assert_eq!(square.len(), 1);
            assert_eq!(square[0].name, "s");
            assert_eq!(square[0].ty, TsTypeInfo::String);
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_collect_discriminated_union_optional_field_not_option_wrapped() {
    use crate::ts_type_info::TsTypeInfo;

    let alias =
        parse_type_alias(r#"type Msg = { type: "a"; x?: number } | { type: "b"; y: string };"#);
    let ts_def = super::super::unions::try_collect_discriminated_union(&alias)
        .expect("should detect discriminated union");

    if let TypeDef::Enum { variant_fields, .. } = ts_def {
        let a_fields = variant_fields.get("a").expect("a variant");
        assert_eq!(a_fields.len(), 1);
        assert_eq!(a_fields[0].name, "x");
        // TsTypeInfo::Number (NOT Option-wrapped) — Option wrapping deferred to resolve
        assert_eq!(a_fields[0].ty, TsTypeInfo::Number);
        assert!(a_fields[0].optional, "optional flag should be set");
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn test_collect_string_literal_union_non_string_returns_none() {
    // Numeric literal union should NOT be detected as string literal union
    let alias = parse_type_alias("type Bits = 0 | 1;");
    assert!(
        super::super::unions::try_collect_string_literal_union(&alias).is_none(),
        "numeric literal union should return None"
    );
}

// --- transpile integration ---

#[test]
fn test_transpile_collecting_synthetic_output() {
    let source = "export function foo(x: string | number): void { }";
    let (output, _unsupported) = crate::transpile_collecting(source).unwrap();
    assert!(
        output.contains("enum"),
        "transpile output should contain synthetic enum for union type, got: {output}"
    );
    assert!(
        output.contains("fn foo"),
        "transpile output should contain the function"
    );
}
