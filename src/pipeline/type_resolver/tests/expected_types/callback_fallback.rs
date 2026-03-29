use super::*;

// --- Arrow/Fn expected type propagation (I-267) ---

#[test]
fn test_propagate_expected_callback_arrow_return_object_literal() {
    // When a callback parameter has a function type with a known return type,
    // the return expression's object literal should get that return type as expected type.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Result".to_string(),
        TypeDef::new_struct(
            vec![
                ("ok".to_string(), RustType::Bool),
                ("message".to_string(), RustType::String),
            ],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "process".to_string(),
        TypeDef::Function {
            params: vec![(
                "cb".to_string(),
                RustType::Fn {
                    params: vec![],
                    return_type: Box::new(RustType::Named {
                        name: "Result".to_string(),
                        type_args: vec![],
                    }),
                },
            )],
            return_type: Some(RustType::Named {
                name: "Result".to_string(),
                type_args: vec![],
            }),
            has_rest: false,
        },
    );

    let res = resolve_with_reg(
        r#"process(() => { return { ok: true, message: "done" }; });"#,
        &reg,
    );

    // The object literal { ok: true, message: "done" } should have expected type Result
    let result_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "Result"))
        .count();
    assert!(
        result_count >= 1,
        "object literal in callback return should have Named(\"Result\") expected type, found {} Result entries in expected_types",
        result_count
    );
}

#[test]
fn test_propagate_expected_logical_or_fallback_empty_object_no_annotation() {
    // `const opts = getOptions() || {}` — NO type annotation on variable.
    // The `{}` should get expected type from the left operand's resolved type.
    // This is the pattern in Hono: `const verifyOpts = options.verification || {}`
    let mut reg = TypeRegistry::new();
    reg.register(
        "Options".to_string(),
        TypeDef::new_struct(
            vec![(
                "verification".to_string(),
                RustType::Named {
                    name: "VerifyOpts".to_string(),
                    type_args: vec![],
                },
            )],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "VerifyOpts".to_string(),
        TypeDef::new_struct(
            vec![("algo".to_string(), RustType::String)],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"function test(options: Options): void {
            const verifyOpts = options.verification || {};
        }"#,
        &reg,
    );

    // The `{}` in `options.verification || {}` should have expected type VerifyOpts
    let verify_opts_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "VerifyOpts"))
        .count();
    assert!(
        verify_opts_count >= 1,
        "empty object in || fallback should have Named(\"VerifyOpts\") expected type from left operand, found {} VerifyOpts entries",
        verify_opts_count
    );
}

#[test]
fn test_propagate_expected_nullish_coalescing_fallback_empty_object_no_annotation() {
    // `const x = val ?? {}` — NO type annotation. The `{}` should get type from `val`.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        TypeDef::new_struct(
            vec![("host".to_string(), RustType::String)],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"function test(val: Config): void {
            const x = val ?? {};
        }"#,
        &reg,
    );

    let config_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "Config"))
        .count();
    assert!(
        config_count >= 1,
        "empty object in ?? fallback should have Named(\"Config\") expected type from left operand, found {} Config entries",
        config_count
    );
}

#[test]
fn test_propagate_expected_typed_var_arrow_return_object_literal() {
    // const make: () => Point = () => { return { x: 1, y: 2 } }
    // The arrow's return type should come from the variable's type annotation.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"const make: () => Point = () => { return { x: 1, y: 2 }; };"#,
        &reg,
    );

    // The object literal { x: 1, y: 2 } should have expected type Point
    let point_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "Point"))
        .count();
    assert!(
        point_count >= 1,
        "object literal in typed arrow return should have Named(\"Point\") expected type, found {} Point entries",
        point_count
    );
}
