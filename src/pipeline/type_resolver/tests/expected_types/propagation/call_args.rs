use super::*;

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
