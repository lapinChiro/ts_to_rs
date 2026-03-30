use super::*;

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
