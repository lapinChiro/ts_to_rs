use std::fs;
use ts_to_rs::{transpile, transpile_collecting, transpile_with_builtins};

/// Generates a snapshot test from a fixture file name.
///
/// Derives the fixture path from the test name: `test_foo_bar` → `tests/fixtures/foo-bar.input.ts`.
///
/// Variants:
/// - `snapshot_test!(test_foo)` — uses `transpile()` (no builtins, errors on unsupported)
/// - `snapshot_test!(test_foo, collecting)` — uses `transpile_collecting()` (no builtins, collects unsupported)
/// - `snapshot_test!(test_foo, builtins)` — uses `transpile_with_builtins()` (builtins loaded, collects unsupported)
macro_rules! snapshot_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            let fixture = stringify!($name)
                .strip_prefix("test_")
                .unwrap_or(stringify!($name))
                .replace('_', "-");
            let input = fs::read_to_string(format!("tests/fixtures/{fixture}.input.ts")).unwrap();
            let output = transpile(&input).unwrap();
            insta::assert_snapshot!(output);
        }
    };
    ($name:ident, collecting) => {
        #[test]
        fn $name() {
            let fixture = stringify!($name)
                .strip_prefix("test_")
                .unwrap_or(stringify!($name))
                .replace('_', "-");
            let input = fs::read_to_string(format!("tests/fixtures/{fixture}.input.ts")).unwrap();
            let (output, unsupported) = transpile_collecting(&input).unwrap();
            insta::assert_snapshot!(output);
            if !unsupported.is_empty() {
                let json = serde_json::to_string_pretty(&unsupported).unwrap();
                insta::assert_snapshot!(format!("{}_unsupported", stringify!($name)), json);
            }
        }
    };
    ($name:ident, builtins) => {
        #[test]
        fn $name() {
            let fixture = stringify!($name)
                .strip_prefix("test_")
                .unwrap_or(stringify!($name))
                .replace('_', "-");
            let input = fs::read_to_string(format!("tests/fixtures/{fixture}.input.ts")).unwrap();
            let (output, unsupported) = transpile_with_builtins(&input).unwrap();
            insta::assert_snapshot!(output);
            if !unsupported.is_empty() {
                let json = serde_json::to_string_pretty(&unsupported).unwrap();
                insta::assert_snapshot!(format!("{}_unsupported", stringify!($name)), json);
            }
        }
    };
}

// ── transpile (no builtins, errors on unsupported) ──────────────────────

snapshot_test!(test_import_export);
snapshot_test!(test_basic_types);
snapshot_test!(test_optional_fields);
snapshot_test!(test_functions);
snapshot_test!(test_classes);
snapshot_test!(test_closures);
snapshot_test!(test_generics);
snapshot_test!(test_function_calls);
snapshot_test!(test_error_handling);
snapshot_test!(test_loops);
snapshot_test!(test_mixed);
snapshot_test!(test_enum);
snapshot_test!(test_array_literal);
snapshot_test!(test_object_literal);
snapshot_test!(test_type_registry);
snapshot_test!(test_string_to_string);
snapshot_test!(test_ternary);
snapshot_test!(test_class_inheritance, collecting);
snapshot_test!(test_break_continue);
snapshot_test!(test_object_destructuring);
snapshot_test!(test_console_api);
snapshot_test!(test_general_for_loop);
snapshot_test!(test_string_methods);
snapshot_test!(test_array_methods);
snapshot_test!(test_do_while);
snapshot_test!(test_array_destructuring, collecting);
snapshot_test!(test_number_parse_api);
snapshot_test!(test_math_api, collecting);
snapshot_test!(test_async_await);
snapshot_test!(test_async_class_method);
snapshot_test!(test_const_primitive);
snapshot_test!(test_callable_interface_param_rename);
snapshot_test!(test_callable_interface_inner);
snapshot_test!(test_type_infer_unannotated);
snapshot_test!(test_unary_operators, collecting);
snapshot_test!(test_void_type);
snapshot_test!(test_nullish_coalescing);
snapshot_test!(test_type_assertion);
snapshot_test!(test_optional_chaining);
snapshot_test!(test_keyword_types);
snapshot_test!(test_array_spread);
snapshot_test!(test_object_spread);
snapshot_test!(test_string_literal_union);
snapshot_test!(test_getter_setter);
snapshot_test!(test_default_params);
snapshot_test!(test_union_type);
snapshot_test!(test_indexed_access_type, collecting);
snapshot_test!(test_builtin_api_batch);
snapshot_test!(test_inline_type_literal_param);
snapshot_test!(test_interface_mixed);
snapshot_test!(test_discriminated_union);
snapshot_test!(test_intersection_type);
snapshot_test!(test_abstract_class);
snapshot_test!(test_conditional_type);
snapshot_test!(test_switch);
snapshot_test!(test_param_properties);
snapshot_test!(test_update_expr);
snapshot_test!(test_class_default_params);
snapshot_test!(test_fn_expr);
snapshot_test!(test_var_type_arrow);
snapshot_test!(test_var_type_alias_arrow);
snapshot_test!(test_regex_literal);
snapshot_test!(test_call_signature_rest);
snapshot_test!(test_callable_interface, collecting);
snapshot_test!(test_type_alias_utility);
snapshot_test!(test_any_type_narrowing);
snapshot_test!(test_type_narrowing);
snapshot_test!(test_union_fallback);
snapshot_test!(test_generic_class);
snapshot_test!(test_array_builtin_methods);
snapshot_test!(test_intersection_methods);
snapshot_test!(test_intersection_empty_object, collecting);
snapshot_test!(test_intersection_fallback, collecting);
snapshot_test!(test_intersection_union_distribution, collecting);
snapshot_test!(test_typeof_const);
snapshot_test!(test_assignment_expected_type);
snapshot_test!(test_as_type_expected);
snapshot_test!(test_ternary_union);
snapshot_test!(test_explicit_type_args);
snapshot_test!(test_private_member_expected_type);

// ── transpile_collecting (no builtins, collects unsupported) ────────────

snapshot_test!(test_interface_methods, collecting);
snapshot_test!(test_narrowing_truthy_instanceof, collecting);
snapshot_test!(test_trait_coercion, collecting);
snapshot_test!(test_anon_struct_inference, collecting);
snapshot_test!(test_instanceof_builtin, collecting);

// ── transpile_with_builtins ────────────────────────────────────────────

snapshot_test!(test_vec_method_expected_type, builtins);
snapshot_test!(test_external_type_struct, builtins);
// Tests below use same fixture as another test but with builtins loaded.
// Can't use macro (fixture name ≠ test name).

#[test]
fn test_string_methods_with_builtins() {
    let input = fs::read_to_string("tests/fixtures/string-methods.input.ts").unwrap();
    let (output, unsupported) = transpile_with_builtins(&input).unwrap();
    insta::assert_snapshot!(output);
    if !unsupported.is_empty() {
        let json = serde_json::to_string_pretty(&unsupported).unwrap();
        insta::assert_snapshot!("test_string_methods_with_builtins_unsupported", json);
    }
}

#[test]
fn test_instanceof_builtin_with_builtins() {
    let input = fs::read_to_string("tests/fixtures/instanceof-builtin.input.ts").unwrap();
    let (output, unsupported) = transpile_with_builtins(&input).unwrap();
    insta::assert_snapshot!(output);
    if !unsupported.is_empty() {
        let json = serde_json::to_string_pretty(&unsupported).unwrap();
        insta::assert_snapshot!("test_instanceof_builtin_with_builtins_unsupported", json);
    }
}

// ── Custom tests (non-macro: require specialized assertions) ───────────

#[test]
fn test_unsupported_syntax_default_errors() {
    let input = fs::read_to_string("tests/fixtures/unsupported-syntax.input.ts").unwrap();
    let result = transpile(&input);
    assert!(
        result.is_err(),
        "transpile should error on unsupported syntax by default"
    );
}

#[test]
fn test_unsupported_syntax_collecting_output() {
    let input = fs::read_to_string("tests/fixtures/unsupported-syntax.input.ts").unwrap();
    let (output, unsupported) = transpile_collecting(&input).unwrap();
    insta::assert_snapshot!("unsupported_syntax_rust_output", output);
    let json = serde_json::to_string_pretty(&unsupported).unwrap();
    insta::assert_snapshot!("unsupported_syntax_json_report", json);
}

#[test]
fn test_multi_var_decl() {
    let input = fs::read_to_string("tests/fixtures/multi-var-decl.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    assert!(
        output.contains("let a") && output.contains("let b"),
        "multi-var const should expand to separate lets: {output}"
    );
    assert!(
        output.contains("let mut x") && output.contains("let y"),
        "multi-var let: x is reassigned (let mut), y is not (let): {output}"
    );
    assert!(
        !output.contains("let mut y"),
        "y is never reassigned, should not be let mut: {output}"
    );
    insta::assert_snapshot!(output);
}

#[test]
fn test_nullable_return() {
    let input = fs::read_to_string("tests/fixtures/nullable-return.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    assert!(
        !output.contains("Some(if"),
        "ternary return should not be wrapped in Some(): {output}"
    );
    assert!(
        output.contains(r#"Some("found".to_string())"#),
        "direct return of literal should be wrapped in Some(): {output}"
    );
    insta::assert_snapshot!(output);
}
