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
            assert_eq!(circle_fields, &[("radius".to_string(), RustType::F64)]);
            let square_fields = variant_fields.get("Square").expect("Square variant");
            assert_eq!(square_fields, &[("side".to_string(), RustType::F64)]);
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
        let (name, ty) = &fields[0];
        assert_eq!(name, "x");
        assert!(
            matches!(ty, RustType::Named { .. }),
            "x should be a Named type (synthetic enum), got: {ty:?}"
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
    use crate::transformer::any_narrowing::{build_any_enum_variants, collect_any_constraints};
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
            params.iter().any(|(_, ty)| matches!(ty, RustType::Any)),
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
