use super::*;

// --- I-269: Option<T> spread source unwrap ---

#[test]
fn test_spread_option_unwrap_resolves_fields() {
    // { origin: "*", ...options } where options: Option<CORSOptions>
    // → CORSOptions fields should be expanded into the merged field list
    let mut reg = TypeRegistry::new();
    reg.register(
        "CORSOptions".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("origin".to_string(), RustType::String),
                ("methods".to_string(), RustType::String),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function cors(options: CORSOptions | null) {
            const defaults = { origin: "*", ...options };
        }
        "#,
        &reg,
    );

    // The object literal should get an expected type (anonymous struct)
    // because merge_object_fields should successfully expand CORSOptions fields
    // through Option unwrap.
    let has_obj_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_obj_expected,
        "object literal with Option<Struct> spread should have anonymous struct expected type"
    );
}

// --- I-268: Type parameter constraint resolution ---

#[test]
fn test_spread_type_param_constraint_resolves_fields() {
    // { ...env, extra: 1 } where env: E (E extends Env)
    // → Env's fields should be resolved from the constraint
    let mut reg = TypeRegistry::new();
    reg.register(
        "Env".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("bindings".to_string(), RustType::Any),
                ("variables".to_string(), RustType::Any),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f<E extends Env>(env: E) {
            const result = { ...env, extra: 1 };
        }
        "#,
        &reg,
    );

    // The object literal should have an expected type (anonymous struct) because
    // the type parameter E's constraint Env provides field information.
    let has_obj_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_obj_expected,
        "object literal with type param spread should have anonymous struct expected type"
    );
}

#[test]
fn test_spread_type_param_no_constraint_returns_unknown() {
    // { ...t } where t: T (no constraint)
    // → Cannot resolve fields, object literal should not get expected type
    let res = resolve(
        r#"
        function f<T>(t: T) {
            const result = { ...t, extra: 1 };
        }
        "#,
    );

    // Without constraint, merge_object_fields should fail → no expected type on obj
    let has_anon = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        !has_anon,
        "unconstrained type param spread should NOT produce anonymous struct"
    );
}

// --- T4: Generic type_args instantiation ---

#[test]
fn test_spread_generic_type_args_instantiation() {
    // { ...container } where container: Container<String>
    // → Container<T> has field `value: T`, should become `value: String`
    use crate::ir::TypeParam;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Container".to_string(),
        crate::registry::TypeDef::Struct {
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
            }],
            fields: vec![(
                "value".to_string(),
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            )],
            methods: Default::default(),
            constructor: None,
            extends: vec![],
            is_interface: false,
        },
    );

    let (res, synthetic) = resolve_with_reg_and_synthetic(
        r#"
        function f(container: Container<string>) {
            const result = { ...container, extra: true };
        }
        "#,
        &reg,
    );

    // Spread of Container<String> + explicit field → anonymous struct with
    // instantiated fields: value: String (not T), extra: Bool.
    let has_anon = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_anon,
        "spread of Container<String> with extra field should produce anonymous struct"
    );

    // Verify the anonymous struct has `value: String` (T instantiated to String)
    let has_instantiated_field = synthetic.all_items().iter().any(|item| {
        matches!(item, crate::ir::Item::Struct { fields, .. }
            if fields.iter().any(|f| f.name == "value" && matches!(f.ty, RustType::String)))
    });
    assert!(
        has_instantiated_field,
        "Container<String>'s field 'value' should be instantiated to String, not T"
    );
}

// --- Nested: Option + type param ---

#[test]
fn test_spread_option_type_param_nested() {
    // { ...opt } where opt: Option<E>, E extends Env
    // → unwrap Option, then resolve constraint to Env fields
    let mut reg = TypeRegistry::new();
    reg.register(
        "Env".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("bindings".to_string(), RustType::Any)],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f<E extends Env>(opt?: E) {
            const result = { ...opt, extra: 1 };
        }
        "#,
        &reg,
    );

    // Should resolve: Option<E> → E → constraint Env → fields
    let has_obj_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_obj_expected,
        "Option<TypeParam> spread should resolve through both Option unwrap and constraint"
    );
}

// --- Regression: existing Named struct spread behavior ---

#[test]
fn test_spread_named_struct_existing_behavior() {
    // Verify existing behavior: { ...base, extra: 1 } where base: Config
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("host".to_string(), RustType::String),
                ("port".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let (res, synthetic) = resolve_with_reg_and_synthetic(
        r#"
        function f(base: Config) {
            const result = { ...base, extra: true };
        }
        "#,
        &reg,
    );

    // Should produce anonymous struct with host, port, extra fields
    let has_anon = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_anon,
        "existing Named struct spread should still produce anonymous struct"
    );

    // Verify anonymous struct has 3 fields by checking the synthetic registry items
    let anon_count = synthetic
        .all_items()
        .iter()
        .filter(|item| matches!(item, crate::ir::Item::Struct { fields, .. } if fields.len() == 3))
        .count();
    assert!(
        anon_count >= 1,
        "anonymous struct should have 3 fields (host, port, extra)"
    );
}

// --- Spread on inline object type (SyntheticTypeRegistry) ---

#[test]
fn test_spread_inline_object_type_resolves_fields() {
    // { ...opts, extra: 1 } where opts: { name: string; count: number }
    // The inline type creates a _TypeLitN in SyntheticTypeRegistry.
    // resolve_spread_source_fields should find fields via resolve_struct_fields_by_name.
    let res = resolve(
        r#"
        function f(opts: { name: string; count: number }) {
            const merged = { ...opts, extra: true };
        }
        "#,
    );

    // The object literal should get an expected type (anonymous struct with 3 fields)
    let has_obj_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_obj_expected,
        "spread of inline object type should produce anonymous struct expected type"
    );
}
