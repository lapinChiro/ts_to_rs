use super::*;

// I-286: Vec→Array method mapping tests

/// Helper to create a RustType representing a type parameter (e.g., T, U).
fn type_param(name: &str) -> RustType {
    RustType::Named {
        name: name.to_string(),
        type_args: vec![],
    }
}

/// Creates a TypeRegistry with Array<T> definition including push and map methods.
fn create_registry_with_array_methods() -> TypeRegistry {
    use crate::ir::TypeParam;

    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();

    // push(...items: T[]): number
    methods.insert(
        "push".to_string(),
        vec![MethodSignature {
            params: vec![(
                "items".to_string(),
                RustType::Vec(Box::new(type_param("T"))),
            )
                .into()],
            return_type: Some(RustType::F64),
            has_rest: true,
            type_params: vec![],
        }],
    );

    // map(callbackfn: (value: T, index: number, array: T[]) => U): U[]
    methods.insert(
        "map".to_string(),
        vec![MethodSignature {
            params: vec![(
                "callbackfn".to_string(),
                RustType::Fn {
                    params: vec![
                        type_param("T"),
                        RustType::F64,
                        RustType::Vec(Box::new(type_param("T"))),
                    ],
                    return_type: Box::new(type_param("U")),
                },
            )
                .into()],
            return_type: Some(RustType::Vec(Box::new(type_param("U")))),
            has_rest: false,
            type_params: vec![],
        }],
    );

    reg.register(
        "Array".to_string(),
        crate::registry::TypeDef::Struct {
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
            }],
            fields: vec![("length".to_string(), RustType::F64).into()],
            methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );

    reg
}

#[test]
fn test_vec_push_propagates_element_type_to_argument() {
    // arr.push({...}) should propagate the element type to the push argument
    let mut reg = create_registry_with_array_methods();
    reg.register(
        "Item".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("name".to_string(), RustType::String).into()],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function test(arr: Item[]) {
            arr.push({ name: "x" });
        }
        "#,
        &reg,
    );

    // The object literal { name: "x" } should have Item as expected type
    let has_item_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Item"));
    assert!(
        has_item_expected,
        "push argument object literal should have Named(\"Item\") as expected type. Got: {:?}",
        res.expected_types.values().collect::<Vec<_>>()
    );
}

#[test]
fn test_vec_map_callback_param_gets_element_type() {
    // arr.map(item => ...) should set item's type to the element type
    let mut reg = create_registry_with_array_methods();
    reg.register(
        "Item".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("name".to_string(), RustType::String).into()],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function test(arr: Item[]) {
            arr.map(item => item.name);
        }
        "#,
        &reg,
    );

    // The arrow function argument should have Fn type as expected
    // (the callback's param type = Item from Array<Item>.map)
    let has_fn_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Fn { .. }));
    assert!(
        has_fn_expected,
        "map callback should have Fn type as expected (with Item param). Got: {:?}",
        res.expected_types.values().collect::<Vec<_>>()
    );
}

// I-288: Tests using real builtin types (resolve_with_builtins)

#[test]
fn test_vec_push_expected_type_with_real_builtins() {
    // Uses actual ecmascript.json Array.push signature (has_rest=true, params=[Vec<T>])
    let res = resolve_with_builtins(
        r#"
        interface Item { name: string }
        function test(arr: Item[]) {
            arr.push({ name: "x" });
        }
        "#,
    );

    let has_item_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Item"));
    assert!(
        has_item_expected,
        "push argument should have Named(\"Item\") as expected type from real Array.push signature"
    );
}

#[test]
fn test_vec_map_callback_with_real_builtins() {
    // Uses actual ecmascript.json Array.map signature to infer callback param type
    let res = resolve_with_builtins(
        r#"
        interface Item { name: string }
        function test(arr: Item[]) {
            arr.map(item => item.name);
        }
        "#,
    );

    let has_fn_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Fn { .. }));
    assert!(
        has_fn_expected,
        "map callback should have Fn type as expected from real Array.map signature"
    );
}

#[test]
fn test_vec_filter_callback_with_real_builtins() {
    let res = resolve_with_builtins(
        r#"
        interface Item { active: boolean }
        function test(arr: Item[]) {
            arr.filter(item => item.active);
        }
        "#,
    );

    let has_fn_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Fn { .. }));
    assert!(
        has_fn_expected,
        "filter callback should have Fn type as expected from real Array.filter signature"
    );
}

// I-290: Member callee arg resolution order (T5)

#[test]
fn test_member_callee_args_resolved_before_overload_selection() {
    // When a method call has overloads differing only by arg type (not count),
    // the correct overload should be selected based on the resolved arg type.
    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "process".to_string(),
        vec![
            MethodSignature {
                params: vec![("x".to_string(), RustType::String).into()],
                return_type: Some(RustType::String),
                has_rest: false,
                type_params: vec![],
            },
            MethodSignature {
                params: vec![("x".to_string(), RustType::F64).into()],
                return_type: Some(RustType::F64),
                has_rest: false,
                type_params: vec![],
            },
        ],
    );
    reg.register(
        "Processor".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );

    let res = resolve_with_reg(
        r#"
        function test(p: Processor) {
            const result = p.process(42);
        }
        "#,
        &reg,
    );

    // The call p.process(42) should resolve to the F64 overload (arg type = F64).
    // Before the fix, collect_resolved_arg_types was called before args were resolved,
    // so Stage 4 of select_overload was ineffective.
    let has_f64_expr = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        has_f64_expr,
        "p.process(42) should resolve to F64 overload (Stage 4 arg type matching)"
    );
}
