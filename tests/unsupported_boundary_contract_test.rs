//! Unsupported boundary contract tests.
//!
//! These tests distinguish:
//! - parser success + transformer unsupported collection
//! - parser failure before unsupported collection is possible
//!
//! The goal is to make `swc_*` upgrades observable when syntax moves across
//! that boundary.

use ts_to_rs::{transpile_collecting, UnsupportedSyntax};

fn only_unsupported(source: &str) -> UnsupportedSyntax {
    let (_output, unsupported) =
        transpile_collecting(source).expect("source should parse and collect unsupported syntax");
    assert_eq!(
        unsupported.len(),
        1,
        "expected exactly one unsupported item, got {unsupported:?}"
    );
    unsupported.into_iter().next().unwrap()
}

#[test]
fn test_export_default_stays_in_parse_success_transformer_unsupported_bucket() {
    let unsupported = only_unsupported("export default 42;\n");
    assert_eq!(unsupported.kind, "ExportDefaultExpr");
    assert_eq!(unsupported.location, "1:1");
}

#[test]
fn test_tagged_template_stays_in_parse_success_transformer_unsupported_bucket() {
    let (_output, unsupported) = transpile_collecting("function foo() { const x = tag`hello`; }\n")
        .expect("tagged template should parse and collect unsupported syntax");
    assert!(
        !unsupported.is_empty(),
        "tagged template should remain in parse-success / unsupported-collection bucket"
    );
}

#[test]
fn test_decorator_stays_in_parse_error_bucket() {
    let err = transpile_collecting("@sealed\nclass Foo {}\n")
        .expect_err("decorators should fail during parsing, not unsupported collection");
    let msg = err.to_string();
    assert!(
        msg.contains("failed to parse: input.ts"),
        "decorator parse rejection should remain parse-stage, got: {msg}"
    );
}

#[test]
fn test_malformed_interface_stays_in_parse_error_bucket() {
    let err = transpile_collecting("interface { }\n")
        .expect_err("malformed interface should fail during parsing");
    let msg = err.to_string();
    assert!(
        msg.contains("failed to parse: input.ts"),
        "malformed interface should remain parse-stage error, got: {msg}"
    );
}
