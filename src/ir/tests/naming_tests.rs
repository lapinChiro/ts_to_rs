use super::super::*;

// -- sanitize_field_name tests --

#[test]
fn test_sanitize_field_name_hyphen_replaced() {
    assert_eq!(sanitize_field_name("Content-Type"), "Content_Type");
}

#[test]
fn test_sanitize_field_name_brackets_removed() {
    assert_eq!(sanitize_field_name("foo[0]"), "foo0");
}

#[test]
fn test_sanitize_field_name_underscore_only_becomes_field() {
    assert_eq!(sanitize_field_name("_"), "_field");
}

#[test]
fn test_sanitize_field_name_digit_prefix_escaped() {
    assert_eq!(sanitize_field_name("0abc"), "_0abc");
}

#[test]
fn test_sanitize_field_name_empty_becomes_empty_sentinel() {
    assert_eq!(sanitize_field_name(""), "_empty");
}

#[test]
fn test_sanitize_field_name_normal_passthrough() {
    assert_eq!(sanitize_field_name("name"), "name");
}

#[test]
fn test_sanitize_field_name_keyword_not_escaped() {
    // キーワードエスケープは generator (escape_ident) の責務。
    // sanitize_field_name は文字レベルのサニタイズのみ。
    assert_eq!(sanitize_field_name("type"), "type");
}

// -- sanitize_rust_type_name tests --

#[test]
fn test_sanitize_rust_type_name_prefixes_prelude_types() {
    assert_eq!(sanitize_rust_type_name("Result"), "TsResult");
    assert_eq!(sanitize_rust_type_name("Option"), "TsOption");
    assert_eq!(sanitize_rust_type_name("String"), "TsString");
    assert_eq!(sanitize_rust_type_name("Vec"), "TsVec");
    assert_eq!(sanitize_rust_type_name("Box"), "TsBox");
    assert_eq!(sanitize_rust_type_name("Some"), "TsSome");
    assert_eq!(sanitize_rust_type_name("None"), "TsNone");
    assert_eq!(sanitize_rust_type_name("Ok"), "TsOk");
    assert_eq!(sanitize_rust_type_name("Err"), "TsErr");
    assert_eq!(sanitize_rust_type_name("Self"), "TsSelf");
}

#[test]
fn test_sanitize_rust_type_name_preserves_non_prelude() {
    assert_eq!(sanitize_rust_type_name("MyType"), "MyType");
    assert_eq!(sanitize_rust_type_name("Context"), "Context");
    assert_eq!(sanitize_rust_type_name("User"), "User");
    assert_eq!(sanitize_rust_type_name("ResultType"), "ResultType");
}

// -- string_to_pascal_case tests --

#[test]
fn test_string_to_pascal_case_empty_string() {
    assert_eq!(string_to_pascal_case(""), "");
}

#[test]
fn test_string_to_pascal_case_special_chars_only() {
    assert_eq!(string_to_pascal_case("---"), "");
}

#[test]
fn test_string_to_pascal_case_single_char() {
    assert_eq!(string_to_pascal_case("a"), "A");
}

#[test]
fn test_string_to_pascal_case_consecutive_delimiters() {
    assert_eq!(string_to_pascal_case("foo--bar"), "FooBar");
}

#[test]
fn test_string_to_pascal_case_already_pascal_lowercases_segments() {
    // "FooBar" has no delimiter → treated as single segment → lowercased then capitalized
    assert_eq!(string_to_pascal_case("FooBar"), "Foobar");
}

#[test]
fn test_string_to_pascal_case_all_caps_with_underscore() {
    assert_eq!(string_to_pascal_case("FOO_BAR"), "FooBar");
}

// -- camel_to_snake tests --

#[test]
fn test_camel_to_snake_simple() {
    assert_eq!(camel_to_snake("byteLength"), "byte_length");
}

#[test]
fn test_camel_to_snake_acronym() {
    assert_eq!(camel_to_snake("toISOString"), "to_iso_string");
}

#[test]
fn test_camel_to_snake_all_upper_acronym() {
    assert_eq!(camel_to_snake("XMLHTTPRequest"), "xmlhttp_request");
}

#[test]
fn test_camel_to_snake_already_snake() {
    assert_eq!(camel_to_snake("already_snake"), "already_snake");
}

#[test]
fn test_camel_to_snake_single_word() {
    assert_eq!(camel_to_snake("name"), "name");
}

#[test]
fn test_camel_to_snake_single_char() {
    assert_eq!(camel_to_snake("x"), "x");
}

#[test]
fn test_camel_to_snake_pascal_case() {
    assert_eq!(camel_to_snake("ByteLength"), "byte_length");
}

#[test]
fn test_camel_to_snake_all_uppercase() {
    assert_eq!(camel_to_snake("URL"), "url");
}

#[test]
fn test_camel_to_snake_empty() {
    assert_eq!(camel_to_snake(""), "");
}
