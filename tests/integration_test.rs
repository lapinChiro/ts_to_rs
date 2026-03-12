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
fn test_mixed() {
    let input = fs::read_to_string("tests/fixtures/mixed.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    insta::assert_snapshot!(output);
}
