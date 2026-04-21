//! Cross-cutting generator behavior not tied to one `Item` variant:
//! multi-item blank-line separation and `Expr::Regex` rendering (raw
//! string escape rules).

use super::*;
use crate::ir::{Expr, Item, Visibility};

#[test]
fn test_generate_multiple_items_separated_by_blank_line() {
    let items = vec![
        Item::Struct {
            vis: Visibility::Public,
            name: "A".to_string(),
            type_params: vec![],
            fields: vec![],
            is_unit_struct: false,
        },
        Item::Struct {
            vis: Visibility::Public,
            name: "B".to_string(),
            type_params: vec![],
            fields: vec![],
            is_unit_struct: false,
        },
    ];
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub struct A {
}

#[derive(Debug, Clone, PartialEq)]
pub struct B {
}";
    assert_eq!(generate(&items), expected);
}

// --- Expr::Regex tests ---

#[test]
fn test_generate_regex_backslash_pattern_uses_raw_string() {
    let expr = Expr::Regex {
        pattern: r"\d+".to_string(),
        global: false,
        sticky: false,
    };
    let output = generate_expr(&expr);
    assert_eq!(output, r#"Regex::new(r"\d+").unwrap()"#);
}

#[test]
fn test_generate_regex_quote_pattern_uses_raw_hash_string() {
    let expr = Expr::Regex {
        pattern: r#"a"b"#.to_string(),
        global: false,
        sticky: false,
    };
    let output = generate_expr(&expr);
    assert_eq!(output, r###"Regex::new(r#"a"b"#).unwrap()"###);
}

#[test]
fn test_generate_regex_simple_pattern_uses_raw_string() {
    let expr = Expr::Regex {
        pattern: "pattern".to_string(),
        global: false,
        sticky: false,
    };
    let output = generate_expr(&expr);
    assert_eq!(output, r#"Regex::new(r"pattern").unwrap()"#);
}
