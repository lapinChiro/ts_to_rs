use std::fs;
use ts_to_rs::{transpile, transpile_collecting};

#[test]
fn test_import_export() {
    let input = fs::read_to_string("tests/fixtures/import-export.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_basic_types() {
    let input = fs::read_to_string("tests/fixtures/basic-types.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_optional_fields() {
    let input = fs::read_to_string("tests/fixtures/optional-fields.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_functions() {
    let input = fs::read_to_string("tests/fixtures/functions.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_classes() {
    let input = fs::read_to_string("tests/fixtures/classes.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_closures() {
    let input = fs::read_to_string("tests/fixtures/closures.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_generics() {
    let input = fs::read_to_string("tests/fixtures/generics.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_function_calls() {
    let input = fs::read_to_string("tests/fixtures/function_calls.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_error_handling() {
    let input = fs::read_to_string("tests/fixtures/error_handling.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_loops() {
    let input = fs::read_to_string("tests/fixtures/loops.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_mixed() {
    let input = fs::read_to_string("tests/fixtures/mixed.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_enum() {
    let input = fs::read_to_string("tests/fixtures/enum.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_array_literal() {
    let input = fs::read_to_string("tests/fixtures/array-literal.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_object_literal() {
    let input = fs::read_to_string("tests/fixtures/object-literal.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_type_registry() {
    let input = fs::read_to_string("tests/fixtures/type-registry.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_string_to_string() {
    let input = fs::read_to_string("tests/fixtures/string-to-string.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_ternary() {
    let input = fs::read_to_string("tests/fixtures/ternary.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_class_inheritance() {
    let input = fs::read_to_string("tests/fixtures/class-inheritance.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_break_continue() {
    let input = fs::read_to_string("tests/fixtures/break-continue.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_object_destructuring() {
    let input = fs::read_to_string("tests/fixtures/object-destructuring.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_console_api() {
    let input = fs::read_to_string("tests/fixtures/console-api.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_general_for_loop() {
    let input = fs::read_to_string("tests/fixtures/general-for-loop.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
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
fn test_string_methods() {
    let input = fs::read_to_string("tests/fixtures/string-methods.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_array_methods() {
    let input = fs::read_to_string("tests/fixtures/array-methods.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_do_while() {
    let input = fs::read_to_string("tests/fixtures/do-while.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_array_destructuring() {
    let input = fs::read_to_string("tests/fixtures/array-destructuring.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_number_parse_api() {
    let input = fs::read_to_string("tests/fixtures/number-parse-api.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_math_api() {
    let input = fs::read_to_string("tests/fixtures/math-api.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_async_await() {
    let input = fs::read_to_string("tests/fixtures/async-await.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_unary_operators() {
    let input = fs::read_to_string("tests/fixtures/unary-operators.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_void_type() {
    let input = fs::read_to_string("tests/fixtures/void-type.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_nullish_coalescing() {
    let input = fs::read_to_string("tests/fixtures/nullish-coalescing.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_type_assertion() {
    let input = fs::read_to_string("tests/fixtures/type-assertion.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_optional_chaining() {
    let input = fs::read_to_string("tests/fixtures/optional-chaining.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_keyword_types() {
    let input = fs::read_to_string("tests/fixtures/keyword-types.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_array_spread() {
    let input = fs::read_to_string("tests/fixtures/array-spread.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_object_spread() {
    let input = fs::read_to_string("tests/fixtures/object-spread.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_string_literal_union() {
    let input = fs::read_to_string("tests/fixtures/string-literal-union.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_getter_setter() {
    let input = fs::read_to_string("tests/fixtures/getter-setter.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_default_params() {
    let input = fs::read_to_string("tests/fixtures/default-params.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_unsupported_syntax_default_errors() {
    let input = fs::read_to_string("tests/fixtures/unsupported-syntax.input.ts").unwrap();
    let result = transpile(&input);
    assert!(
        result.is_err(),
        "transpile should error on unsupported syntax by default"
    );
}
