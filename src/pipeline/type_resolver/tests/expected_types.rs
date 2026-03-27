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
