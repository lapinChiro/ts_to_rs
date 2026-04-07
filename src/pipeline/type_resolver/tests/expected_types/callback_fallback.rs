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
                ("ok".to_string(), RustType::Bool).into(),
                ("message".to_string(), RustType::String).into(),
            ],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "process".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![(
                "cb".to_string(),
                RustType::Fn {
                    params: vec![],
                    return_type: Box::new(RustType::Named {
                        name: "Result".to_string(),
                        type_args: vec![],
                    }),
                },
            )
                .into()],
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
            )
                .into()],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "VerifyOpts".to_string(),
        TypeDef::new_struct(
            vec![("algo".to_string(), RustType::String).into()],
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
            vec![("host".to_string(), RustType::String).into()],
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
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
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

// ── Inline object type + fallback chain (resolve_member_type + propagate_fallback_expected) ──

#[test]
fn test_fallback_on_inline_type_member_access() {
    // Pattern: `options.verification || {}` where options has an INLINE type
    // (not a Named type in TypeRegistry, but a synthetic struct from TsTypeLit).
    // This tests the full chain: resolve_member_type → synthetic field lookup →
    // propagate_fallback_expected → expected type on {}.
    let mut reg = TypeRegistry::new();
    reg.register(
        "VerifyOpts".to_string(),
        TypeDef::new_struct(
            vec![("algo".to_string(), RustType::String).into()],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"function test(options: { verification: VerifyOpts }): void {
            const v = options.verification || {};
        }"#,
        &reg,
    );

    // The `{}` should have expected type VerifyOpts (from inline type's field)
    let verify_opts_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "VerifyOpts"))
        .count();
    assert!(
        verify_opts_count >= 1,
        "empty object in || fallback should have Named(\"VerifyOpts\") from inline type field, found {} entries",
        verify_opts_count
    );
}

#[test]
fn test_fallback_on_constrained_type_param_member_access() {
    // Pattern: `options.verification || {}` where options: T extends { verification: VerifyOpts }
    // Tests: resolve_member_type → type_param_constraints → field lookup → fallback propagation.
    let mut reg = TypeRegistry::new();
    reg.register(
        "BaseOpts".to_string(),
        TypeDef::new_struct(
            vec![(
                "verification".to_string(),
                RustType::Named {
                    name: "VerifyOpts".to_string(),
                    type_args: vec![],
                },
            )
                .into()],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "VerifyOpts".to_string(),
        TypeDef::new_struct(
            vec![("algo".to_string(), RustType::String).into()],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"function test<T extends BaseOpts>(options: T): void {
            const v = options.verification || {};
        }"#,
        &reg,
    );

    let verify_opts_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "VerifyOpts"))
        .count();
    assert!(
        verify_opts_count >= 1,
        "empty object in || fallback should have Named(\"VerifyOpts\") from constrained type param, found {} entries",
        verify_opts_count
    );
}

// ── OptChain on synthetic type ──

#[test]
fn test_opt_chain_on_inline_object_type() {
    // opts?.name where opts: { name: string } | undefined
    // OptChain unwraps Option<_TypeLitN> → _TypeLitN, then resolve_member_type
    // looks up "name" via resolve_struct_fields_by_name → String.
    // Final result is Option<String>.
    let res = resolve(
        r#"
        function f(opts?: { name: string }) {
            const n = opts?.name;
        }
        "#,
    );

    let has_option_string = res.expr_types.values().any(|t| {
        matches!(
            t,
            ResolvedType::Known(RustType::Option(inner)) if matches!(inner.as_ref(), RustType::String)
        )
    });
    assert!(
        has_option_string,
        "opts?.name on inline type should resolve to Option<String>"
    );
}

// ── G6: resolve_fn_type_info with callable interface (call_signatures) ──

#[test]
fn test_callable_interface_return_type_propagated_to_arrow() {
    // interface GetCookie { (c: string): Cookie }
    // const getCookie: GetCookie = (c) => { return { name: "test" } }
    // The arrow body should know the return type is Cookie
    let mut reg = TypeRegistry::new();
    reg.register(
        "Cookie".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String).into()],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "GetCookie".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: Default::default(),
            constructor: None,
            call_signatures: vec![MethodSignature {
                params: vec![("c".to_string(), RustType::String).into()],
                return_type: Some(RustType::Named {
                    name: "Cookie".to_string(),
                    type_args: vec![],
                }),
                has_rest: false,
                type_params: vec![],
            }],
            extends: vec![],
            is_interface: true,
        },
    );

    let source = r#"const getCookie: GetCookie = (c) => { return { name: "test" } };"#;
    let res = resolve_with_reg(source, &reg);

    // The object literal { name: "test" } should have Cookie as expected type.
    // Verify that exactly the object literal span (not the whole expression) has Cookie.
    let cookie_entries: Vec<_> = res
        .expected_types
        .iter()
        .filter(|(_, ty)| matches!(ty, RustType::Named { name, .. } if name == "Cookie"))
        .collect();
    assert_eq!(
        cookie_entries.len(),
        1,
        "exactly one expected type should be Cookie, got: {cookie_entries:?}"
    );
    // The Cookie expected type should be on the object literal, not on the whole arrow
    let (span, _) = cookie_entries[0];
    let obj_lit_src = &source[span.lo as usize..span.hi as usize];
    assert!(
        obj_lit_src.contains("name"),
        "Cookie expected type should be on the object literal containing 'name', got: '{obj_lit_src}'"
    );
}
