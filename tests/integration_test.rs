use std::fs;
use ts_to_rs::transpile;

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
