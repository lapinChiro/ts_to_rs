use super::*;

#[test]
fn test_propagate_expected_new_expr_uses_constructor_params() {
    // new Server("main", { ... }) should use constructor param types (String, Options),
    // NOT struct field types (Bool, String)
    let mut reg = TypeRegistry::new();
    reg.register(
        "Options".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("host".to_string(), RustType::String),
                ("port".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "Server".to_string(),
        crate::registry::TypeDef::Struct {
            type_params: vec![],
            fields: vec![
                ("running".to_string(), RustType::Bool),
                ("host".to_string(), RustType::String),
            ],
            methods: Default::default(),
            constructor: Some(vec![MethodSignature {
                params: vec![
                    ("name".to_string(), RustType::String),
                    (
                        "options".to_string(),
                        RustType::Named {
                            name: "Options".to_string(),
                            type_args: vec![],
                        },
                    ),
                ],
                return_type: None,
                has_rest: false,
            }]),
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );

    let res = resolve_with_reg(
        r#"const s = new Server("main", { host: "localhost", port: 8080 });"#,
        &reg,
    );

    // Verify constructor param types are used, NOT struct field types
    let options_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "Options"))
        .count();
    assert_eq!(
        options_count, 1,
        "exactly one arg should have Named(\"Options\") from constructor param"
    );

    // Bool should NOT appear in expected types — it's a struct field, not a constructor param
    let has_bool = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Bool));
    assert!(
        !has_bool,
        "Bool should NOT appear as expected type (it's a struct field, not a constructor param)"
    );
}

#[test]
fn test_propagate_expected_new_expr_falls_back_to_fields_without_constructor() {
    // When no constructor is defined, fall back to field types (existing behavior)
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::Struct {
            type_params: vec![],
            fields: vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            methods: Default::default(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );

    let res = resolve_with_reg("const p = new Point(1, 2);", &reg);

    let f64_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::F64))
        .count();
    assert_eq!(
        f64_count, 2,
        "without constructor, each field type should be propagated to args"
    );
}

#[test]
fn test_collect_class_info_registers_constructor_params() {
    // Class declarations with constructor should have constructor params in TypeRegistry
    let res = resolve(
        r#"
        class MyService {
            name: string;
            constructor(name: string, count: number) {
                this.name = name;
            }
        }
        const s = new MyService("test", 42);
        "#,
    );

    // Constructor has (name: string, count: number), so expected types should include
    // String for "test" and F64 for 42
    let string_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::String))
        .count();
    let f64_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::F64))
        .count();
    assert!(
        string_count >= 1,
        "new expr arg 'test' should have String expected from constructor, got {string_count}"
    );
    assert!(
        f64_count >= 1,
        "new expr arg 42 should have F64 expected from constructor, got {f64_count}"
    );
}
