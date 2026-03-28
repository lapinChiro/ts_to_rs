use super::*;
use crate::registry::MethodSignature;

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

#[test]
fn test_propagate_expected_var_decl_object_literal_sets_struct_name() {
    // 1-2: const p: Point = { x: 1, y: 2 } → object literal gets Named("Point")
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg("const p: Point = { x: 1, y: 2 };", &reg);

    // The object literal span should have expected = Named("Point")
    let has_point_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Point"));
    assert!(
        has_point_expected,
        "object literal should have Named(\"Point\") as expected type"
    );

    // Field value spans should also have expected types (propagated from struct fields)
    // Count expected_types entries — should be more than just the top-level initializer
    assert!(
        res.expected_types.len() >= 3,
        "expected at least 3 entries (initializer + 2 field values), got {}",
        res.expected_types.len()
    );
}

#[test]
fn test_propagate_expected_var_decl_array_vec_sets_element_type() {
    // VarDecl + array literal propagation: const a: number[] = [1, 2] → each element gets F64
    let res = resolve("const a: number[] = [1, 2];");

    let f64_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::F64))
        .count();
    // Each array element should get F64 as expected type
    assert!(
        f64_expected_count >= 2,
        "each array element should have F64 as expected, got {} F64 entries",
        f64_expected_count
    );
}

#[test]
fn test_propagate_expected_var_decl_tuple_sets_positional_types() {
    // VarDecl + tuple propagation: const t: [string, number] = ["a", 1]
    let res = resolve(r#"const t: [string, number] = ["a", 1];"#);

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_string_expected,
        "first tuple element should expect String"
    );
    assert!(has_f64_expected, "second tuple element should expect F64");
}

#[test]
fn test_propagate_expected_return_object_sets_field_types() {
    // 1-3: function f(): Point { return { x: 1, y: 2 }; }
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg("function f(): Point { return { x: 1, y: 2 }; }", &reg);

    // Return value object literal should have Named("Point") as expected
    let has_point_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Point"));
    assert!(
        has_point_expected,
        "return object literal should have Named(\"Point\") as expected"
    );
}

#[test]
fn test_propagate_expected_call_arg_from_registry_fn() {
    // 1-4: Registry function の引数 expected が propagate される
    let mut reg = TypeRegistry::new();
    reg.register(
        "greet".to_string(),
        crate::registry::TypeDef::Function {
            params: vec![("name".to_string(), RustType::String)],
            return_type: None,
            has_rest: false,
        },
    );

    let res = resolve_with_reg(r#"greet("hello");"#, &reg);

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "call argument should have String as expected from registry function params"
    );
}

#[test]
fn test_propagate_expected_call_arg_from_scope_fn_type() {
    // 1-4a: scope 内の Fn 型変数から引数の expected を設定
    let res = resolve(
        r#"
        function callHandler(handler: (name: string) => void) {
            handler("hello");
        }
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "call argument should have String as expected from Fn type in scope"
    );
}

#[test]
fn test_propagate_expected_switch_case_gets_discriminant_type() {
    // 1-5: switch(dir) { case "up": } where dir: Direction
    let mut reg = TypeRegistry::new();
    reg.register(
        "Direction".to_string(),
        crate::registry::TypeDef::Enum {
            type_params: vec![],
            variants: vec![],
            tag_field: None,
            variant_fields: Default::default(),
            string_values: Default::default(),
        },
    );

    let res = resolve_with_reg(
        r#"
        function f(dir: Direction) {
            switch (dir) {
                case "up":
                    break;
            }
        }
        "#,
        &reg,
    );

    let has_direction_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Direction"));
    assert!(
        has_direction_expected,
        "switch case value should have Direction as expected type"
    );
}

#[test]
fn test_propagate_expected_assign_rhs_gets_lhs_type() {
    // 1-6: let x: Point; x = { x: 1, y: 2 }
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f() {
            let x: Point = { x: 0, y: 0 };
            x = { x: 1, y: 2 };
        }
        "#,
        &reg,
    );

    // Count Named("Point") expected entries — should include both var decl init AND assignment RHS
    let point_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "Point"))
        .count();
    assert!(
        point_expected_count >= 2,
        "both var decl init and assignment RHS should have Named(\"Point\") as expected, got {}",
        point_expected_count
    );
}

#[test]
fn test_propagate_expected_nullish_coalescing_rhs_gets_inner_type() {
    // 1-7: opt ?? "default" where opt: string | null (Option<String>)
    let res = resolve(
        r#"
        function f(opt: string | null) {
            const result = opt ?? "default";
        }
        "#,
    );

    // The RHS "default" should have String as expected (inner of Option<String>)
    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "nullish coalescing RHS should have String as expected (inner of Option<String>)"
    );
}

#[test]
fn test_propagate_expected_ternary_branches_get_expected() {
    // 1-10: const s: string = c ? "a" : "b" → both branches get String
    let res = resolve(r#"const s: string = true ? "a" : "b";"#);

    // Count String expected entries
    let string_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::String))
        .count();
    // At minimum: "a" and "b" should both have String expected
    assert!(
        string_expected_count >= 2,
        "both ternary branches should have String expected, got {}",
        string_expected_count
    );
}

#[test]
fn test_propagate_expected_class_prop_initializer_gets_annotation_type() {
    // 1-8: class C { static x: string = "hi" }
    let res = resolve(
        r#"
        class C {
            static x: string = "hello";
        }
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "class property initializer should have String as expected from annotation"
    );
}

#[test]
fn test_propagate_expected_du_object_lit_fields() {
    let res = resolve(
        r#"
        type Shape = { kind: "circle"; radius: number } | { kind: "square"; side: number };
        const s: Shape = { kind: "circle", radius: 42 };
        "#,
    );

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "DU variant field 'radius' should have expected type f64"
    );
}

#[test]
fn test_propagate_expected_hashmap_value() {
    let res = resolve(
        r#"
        const m: Record<string, number> = { [key]: 42 };
        "#,
    );

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "HashMap value should have expected type f64"
    );
}

#[test]
fn test_propagate_expected_arrow_expr_body() {
    let res = resolve(
        r#"
        const f = (): string => "hello";
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "arrow expression body should have expected type String from return annotation"
    );
}

#[test]
fn test_propagate_expected_rest_param_args() {
    let res = resolve(
        r#"
        function foo(a: number, ...rest: string[]): void {}
        foo(1, "hello", "world");
        "#,
    );

    let string_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::String))
        .count();
    assert!(
        string_expected_count >= 2,
        "rest args 'hello' and 'world' should have expected type String, got {string_expected_count}"
    );
}

#[test]
fn test_propagate_expected_opt_chain_method_args() {
    let res = resolve(
        r#"
        interface Obj {
            greet(name: string): void;
        }
        declare const obj: Obj | undefined;
        obj?.greet("hello");
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "opt chain method arg should have expected type String"
    );
}

#[test]
fn test_resolve_arrow_return_type_from_fn_type_alias() {
    // Variable type annotation with function type alias should propagate
    // return type to arrow body, enabling nested object literal struct resolution
    let res = resolve(
        r#"
        interface ConnInfo { remote: RemoteInfo; }
        interface RemoteInfo { address: string; }
        type GetConnInfo = (host: string) => ConnInfo;
        const getConnInfo: GetConnInfo = (host: string) => ({
            remote: { address: host },
        });
        "#,
    );

    // The nested object literal { address: host } should have expected type
    // Named("RemoteInfo") — propagated through: GetConnInfo → ConnInfo → remote field
    let has_remote_info_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "RemoteInfo"));
    assert!(
        has_remote_info_expected,
        "nested object literal should have expected type RemoteInfo from fn type alias return type"
    );
}

#[test]
fn test_resolve_arrow_explicit_annotation_takes_priority_over_expected() {
    // Arrow's own return type annotation should take priority over expected type
    let res = resolve(
        r#"
        const f: (x: number) => string = (x: number): number => 42;
        "#,
    );

    // The return value `42` should have expected type f64 (from arrow's own annotation),
    // not String (from variable annotation)
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "arrow's own return annotation (number) should take priority"
    );
}

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
            )],
            return_type: Some(RustType::F64),
            has_rest: true,
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
            )],
            return_type: Some(RustType::Vec(Box::new(type_param("U")))),
            has_rest: false,
        }],
    );

    reg.register(
        "Array".to_string(),
        crate::registry::TypeDef::Struct {
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
            }],
            fields: vec![("length".to_string(), RustType::F64)],
            methods,
            constructor: None,
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
            vec![("name".to_string(), RustType::String)],
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
            vec![("name".to_string(), RustType::String)],
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
                params: vec![("x".to_string(), RustType::String)],
                return_type: Some(RustType::String),
                has_rest: false,
            },
            MethodSignature {
                params: vec![("x".to_string(), RustType::F64)],
                return_type: Some(RustType::F64),
                has_rest: false,
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
