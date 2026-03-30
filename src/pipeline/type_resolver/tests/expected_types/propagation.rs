use super::*;
use swc_common::Spanned;

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
            type_params: vec![],
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

// ── Class property initializer expected type ──

#[test]
fn test_class_prop_expected_type_set_before_resolve() {
    // class Foo { field: Options = { strict: true } }
    // Expected type Named("Options") should be set on the object literal
    let res = resolve(
        r#"
        interface Options {
            strict: boolean;
            name?: string;
        }
        class Foo {
            field: Options = { strict: true };
        }
        "#,
    );

    let has_options_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Options"));
    assert!(
        has_options_expected,
        "class prop initializer should have Options expected type"
    );
}

// ── Private method expected type propagation ──

#[test]
fn test_private_method_body_gets_expected_types() {
    // Private method bodies should be visited for type resolution
    let res = resolve(
        r#"
        interface Config {
            host: string;
            port: number;
        }
        class Server {
            #getConfig(): Config {
                return { host: "localhost", port: 8080 };
            }
        }
        "#,
    );

    // Return statement should have expected type from method return annotation
    let has_config_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "private method return should have Config expected type"
    );
}

// ── Type parameter constraint resolution in expected types ──

#[test]
fn test_type_param_constraint_resolved_in_default_param() {
    // function f<T extends Options>(opts: T = {})
    // Expected type on {} should be Named("Options"), not Named("T")
    let res = resolve(
        r#"
        interface Options {
            strict?: boolean;
        }
        function f<T extends Options>(opts: T = ({} as T)) {
            return opts;
        }
        "#,
    );

    // The default value should have expected type resolved to Options (constraint),
    // not T (type param name)
    let has_constraint_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Options"));
    assert!(
        has_constraint_expected,
        "default param expected type should resolve type param T to constraint Options"
    );
}

// ── Private prop initializer expected type ──

#[test]
fn test_private_prop_expected_type_propagation() {
    let res = resolve(
        r#"
        class App {
            #cache: Record<string, string> = {};
        }
        "#,
    );

    // Private prop with Record type annotation should have HashMap expected type
    let has_hashmap = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "HashMap"));
    assert!(
        has_hashmap,
        "private prop initializer should have HashMap expected type from Record<string, string>"
    );
}

// ── Type parameter constraint resolution — ||/?? fallback ──

#[test]
fn test_fallback_expected_resolves_type_params() {
    // In `options.field || {}`, if options is generic T extends { field: Config },
    // the {} should get Config as expected type, not the type param.
    let res = resolve(
        r#"
        interface Config {
            host: string;
        }
        function f<T extends Config>(opts: T) {
            const result = opts || { host: "default" };
        }
        "#,
    );

    // The fallback {} should have expected type resolved through constraint
    let has_config = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config,
        "fallback || {{}} should have Config expected type from type param constraint"
    );
}

// ── Type parameter constraint resolution — variable declaration ──

#[test]
fn test_var_decl_expected_type_resolves_type_params() {
    let res = resolve(
        r#"
        interface Options {
            debug?: boolean;
        }
        function f<T extends Options>() {
            const x: T = { debug: true } as T;
        }
        "#,
    );

    let has_options = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Options"));
    assert!(
        has_options,
        "var decl with type param annotation should resolve to constraint type"
    );
}

// ── Type parameter constraint resolution — return statement ──

#[test]
fn test_return_expected_type_resolves_type_params() {
    let res = resolve(
        r#"
        interface Config {
            host: string;
        }
        function f<T extends Config>(): T {
            return { host: "localhost" } as T;
        }
        "#,
    );

    let has_config = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config,
        "return statement should resolve type param T to constraint Config in expected type"
    );
}

// ── Method body preserves class type param constraints ──

#[test]
fn test_method_body_inherits_class_type_param_constraints() {
    // class Foo<T extends Config> { method<U>(): T { return { host: "x" } as T } }
    // Inside method body, T should still resolve to Config (class constraint),
    // even though the method has its own type param U.
    let res = resolve(
        r#"
        interface Config {
            host: string;
        }
        class Foo<T extends Config> {
            method<U>(x: U): T {
                return { host: "localhost" } as T;
            }
        }
        "#,
    );

    let has_config = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config,
        "method body should resolve class type param T to Config even with method type param U"
    );
}

// ── Function call argument type param resolution ──

#[test]
fn test_call_arg_expected_type_resolves_type_params() {
    // class Container<T extends Options> { add(item: T) {} }
    // container.add({}) → expected type for {} should be Options, not T
    let res = resolve(
        r#"
        interface Options {
            debug?: boolean;
        }
        class Container<T extends Options> {
            add(item: T): void {
                const x = item;
            }
            run(): void {
                this.add({ debug: true });
            }
        }
        "#,
    );

    let has_options = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Options"));
    assert!(
        has_options,
        "call argument expected type should resolve type param T to constraint Options"
    );
}

// ── Object literal field propagation with synthetic types ──

#[test]
fn test_propagate_expected_object_lit_fields_from_synthetic_type() {
    // const x: { name: string; count: number } = { name: "hello", count: 42 }
    // The inline type becomes _TypeLitN. propagate_expected with Named("_TypeLitN")
    // should resolve fields via resolve_object_lit_fields → resolve_struct_fields_by_name,
    // setting String expected on "hello" and F64 expected on 42.
    let res = resolve(
        r#"
        const x: { name: string; count: number } = { name: "hello", count: 42 };
        "#,
    );

    // "hello" should have expected type String
    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "field 'name' value should have String expected type from synthetic struct"
    );

    // 42 should have expected type F64
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "field 'count' value should have F64 expected type from synthetic struct"
    );
}

#[test]
fn test_new_expr_explicit_type_args_in_return_type() {
    // `new Container<Config>(value)` should return Named("Container", [Named("Config")])
    // (not Named("Container", []) which is the current buggy behavior).
    let source = r#"
        interface Config { host: string; }
        class Container<T> {
            value: T;
            constructor(value: T) { this.value = value; }
        }
        const c = new Container<Config>({ host: "localhost" });
    "#;
    let res = resolve(source);
    // The `new Container<Config>(...)` expression should have type
    // Named("Container", [Named("Config")])
    let has_container_with_type_args = res.expr_types.values().any(|ty| {
        matches!(ty, ResolvedType::Known(RustType::Named { name, type_args })
            if name == "Container" && !type_args.is_empty())
    });
    assert!(
        has_container_with_type_args,
        "new Container<Config>(...) should return Named('Container', [Config]), not empty type_args"
    );
}

#[test]
fn test_new_expr_explicit_type_args_resolve_constructor_params() {
    // `new Container<Config>(value)` where constructor is `(value: T)`:
    // explicit type arg `Config` should instantiate `T` → `Config`,
    // so the argument `{ host: "localhost" }` gets expected type Config.
    let source = r#"
        interface Config { host: string; }
        class Container<T> {
            value: T;
            constructor(value: T) { this.value = value; }
        }
        const c = new Container<Config>({ host: "localhost" });
    "#;
    let res = resolve(source);
    let has_config_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "argument to new Container<Config>(...) should have expected type Config"
    );
}

#[test]
fn test_call_explicit_type_args_resolve_param_expected() {
    // `identity<Config>({ host: "localhost" })` where identity is `function identity<T>(x: T): T`
    // explicit type arg `Config` should instantiate `T` → `Config` for the param.
    let source = r#"
        interface Config { host: string; }
        function identity<T>(x: T): T { return x; }
        const c = identity<Config>({ host: "localhost" });
    "#;
    let res = resolve(source);
    let has_config_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "argument to identity<Config>(...) should have expected type Config"
    );
}

#[test]
fn test_call_explicit_type_args_resolve_return_type() {
    // `create<Config>()` where create is `function create<T>(): T`
    // explicit type arg `Config` should resolve return type to Config.
    let source = r#"
        interface Config { host: string; }
        function create<T>(): T { return {} as T; }
        const c: Config = create<Config>();
    "#;
    let res = resolve(source);
    let has_config_return = res.expr_types.values().any(
        |ty| matches!(ty, ResolvedType::Known(RustType::Named { name, .. }) if name == "Config"),
    );
    assert!(
        has_config_return,
        "create<Config>() should return type Config"
    );
}

// --- Type argument inference (I-286c S3) ---

#[test]
fn test_infer_type_arg_from_literal_arg() {
    // `id("hello")` where `function id<T>(x: T): T` should infer T = String,
    // so the call expression's return type is String (not Named("T")).
    let source = r#"
        function id<T>(x: T): T { return x; }
        const result = id("hello");
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // Find the call expression `id("hello")` AST node
    let var_decl = match &file.module.body[1] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(v))) => v,
        _ => panic!("expected var decl"),
    };
    let init = var_decl.decls[0].init.as_ref().unwrap();
    let call_span = Span::from_swc(init.span());
    let call_type = res
        .expr_types
        .get(&call_span)
        .expect("call expression should have resolved type");
    // The call `id("hello")` should resolve to String, not Named("T")
    assert!(
        matches!(call_type, ResolvedType::Known(RustType::String)),
        "id('hello') should infer T=String and return String, got {:?}",
        call_type
    );
}

#[test]
fn test_infer_type_arg_propagates_expected_to_later_params() {
    // `fn wrap<T>(x: T, label: string): T` called with `wrap({ host: "localhost", port: 80 }, "tag")`:
    // Without explicit type args, we can't infer T from a bare object literal (no type info).
    // But with `fn process<T extends Base>(x: T): T` and a constrained T,
    // the constraint should be used as expected type.
    let source = r#"
        interface Config { host: string; port: number; }
        function identity<T extends Config>(x: T): T { return x; }
        identity({ host: "localhost", port: 80 });
    "#;
    let res = resolve(source);
    // The argument should have expected type Config (from T's constraint)
    let has_config_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "arg to identity<T extends Config>(...) should have expected type Config from constraint"
    );
}

#[test]
fn test_infer_type_arg_new_expr_from_arg() {
    // `new Box("hello")` where `class Box<T> { constructor(v: T) }` should
    // infer T = String from the argument, so the return type includes type_args.
    let source = r#"
        class Box<T> {
            value: T;
            constructor(v: T) { this.value = v; }
        }
        const b = new Box("hello");
    "#;
    let res = resolve(source);
    // The `new Box("hello")` expression should have type Named("Box", [String])
    let has_box_with_string = res.expr_types.values().any(|ty| {
        matches!(ty, ResolvedType::Known(RustType::Named { name, type_args })
            if name == "Box" && type_args.iter().any(|t| matches!(t, RustType::String)))
    });
    assert!(
        has_box_with_string,
        "new Box('hello') should infer T=String and return Named('Box', [String])"
    );
}

#[test]
fn test_resolve_type_params_terminates_on_unconstrained_param() {
    // Regression test: `resolve_type_params_in_type` must not infinite-loop
    // when called on a return type containing an unconstrained type parameter.
    // e.g., `function id<T>(x: T): T` → return type Named("T") with no constraint.
    // This triggered an infinite loop in directory mode when `resolve_type_params_in_type`
    // was applied to return types in `resolve_call_expr`.
    let source = r#"
        function id<T>(x: T): T { return x; }
        function wrap<A, B>(a: A, b: B): A { return a; }
        const x = id(42);
        const y = wrap("hello", true);
    "#;
    // If this completes without hanging, the test passes
    let _res = resolve(source);
}

#[test]
fn test_resolve_type_params_terminates_on_self_referential_constraint() {
    // Pathological case: a type param whose constraint references itself.
    // This can happen with generic defaults like `<T extends T>` (invalid TS but
    // our constraint map could theoretically contain it via bugs).
    // The depth limit in resolve_type_params_impl should prevent infinite recursion.
    let source = r#"
        function complex<T extends Record<string, T>>(x: T): T { return x; }
        const r = complex({ key: {} });
    "#;
    // Must complete without hanging
    let _res = resolve(source);
}
