use super::*;

// I-286: Vec→Array method mapping tests

/// Helper to create a RustType representing a type parameter (e.g., T, U).
use crate::registry::MethodKind;
fn type_param(name: &str) -> RustType {
    RustType::TypeVar {
        name: name.to_string(),
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
            kind: MethodKind::Method,
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
            kind: MethodKind::Method,
        }],
    );

    reg.register(
        "Array".to_string(),
        crate::registry::TypeDef::Struct {
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
                default: None,
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

// ── Step 2 (RC-2): remapped-method optional-param stripping ─────────────

/// Step 2 structural invariant: for methods remapped by `map_method_call`, the
/// TypeResolver MUST NOT propagate `Option<T>` expected types onto trailing
/// optional arguments. Violating this re-introduces the spurious `Some(arg)`
/// wraps (e.g. `s.slice(Some(1.0) as i64..Some(3.0) as i64)`) and trailing
/// `None` fills (e.g. `s.starts_with("hello", None)`) that Step 2 resolved.
#[test]
fn test_remapped_method_optional_param_is_not_propagated_as_expected() {
    // `startsWith(searchString: string, position?: number): boolean`
    // Calling with BOTH args — `position` would otherwise receive
    // `Option<F64>` as the expected type from the TS signature. Step 2 strips
    // trailing optional params for remapped methods so `position`'s arg gets
    // no expected type propagated (no `Some(0.0)` wrap at the call site).
    let res = resolve_with_builtins(
        r#"
        function test(s: string) {
            s.startsWith("hi", 0);
        }
        "#,
    );
    let has_option_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Option(_)));
    assert!(
        !has_option_expected,
        "remapped startsWith must NOT propagate `Option<F64>` expected to the \
         optional `position` arg. Got expected types: {:?}",
        res.expected_types.values().collect::<Vec<_>>()
    );
}

#[test]
fn test_non_remapped_builtin_method_optional_param_still_propagates() {
    // Reverse direction: `Array.fill(value, start?, end?)` is NOT in
    // REMAPPED_METHODS (falls through to passthrough in map_method_call), so
    // its optional `start`/`end` params MUST still propagate their `Option<F64>`
    // expected types. This guards against accidentally widening the optional
    // stripping to all method calls (which would break legitimate user calls
    // that rely on Option<T>-driven arg wrapping at the call site).
    let res = resolve_with_builtins(
        r#"
        function test(arr: number[]) {
            arr.fill(0, 0, 5);
        }
        "#,
    );
    let has_option_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Option(_)));
    assert!(
        has_option_expected,
        "non-remapped Array.fill must propagate `Option<F64>` expected to its \
         optional `start`/`end` args. Expected types: {:?}",
        res.expected_types.values().collect::<Vec<_>>()
    );
}

#[test]
fn test_remapped_method_required_fn_param_still_propagated() {
    // filter(predicate, thisArg?) — predicate is required and must still receive
    // its Fn-typed expected type so closure param types resolve inside the body.
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
        "remapped filter should still propagate its required predicate param's \
         Fn type so the closure param is inferred. Trailing optional thisArg \
         is dropped but required params survive."
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
                kind: MethodKind::Method,
            },
            MethodSignature {
                params: vec![("x".to_string(), RustType::F64).into()],
                return_type: Some(RustType::F64),
                has_rest: false,
                type_params: vec![],
                kind: MethodKind::Method,
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
    // so Stage 3 of select_overload was ineffective.
    let has_f64_expr = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        has_f64_expr,
        "p.process(42) should resolve to F64 overload (Stage 3 arg type matching)"
    );
}
