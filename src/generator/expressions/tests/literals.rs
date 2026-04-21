//! Pure literal `Expr` variants — no receiver shape decisions or call
//! dispatch — plus `escape_rust_string` character-level escape unit
//! tests.
//!
//! Covers: `NumberLit`, `BoolLit`, `StringLit`, `Ident`, `Tuple`,
//! `Unit`, `IntLit`, `Deref`, `Ref`, `escape_rust_string`.

use super::*;
use crate::ir::Expr;

#[test]
fn test_generate_expr_number_whole() {
    assert_eq!(generate_expr(&Expr::NumberLit(42.0)), "42.0");
}

#[test]
fn test_generate_expr_number_fractional() {
    assert_eq!(generate_expr(&Expr::NumberLit(2.71)), "2.71");
}

#[test]
fn test_generate_expr_bool_true() {
    assert_eq!(generate_expr(&Expr::BoolLit(true)), "true");
}

#[test]
fn test_generate_expr_bool_false() {
    assert_eq!(generate_expr(&Expr::BoolLit(false)), "false");
}

#[test]
fn test_generate_expr_string_lit() {
    assert_eq!(
        generate_expr(&Expr::StringLit("hello".to_string())),
        "\"hello\""
    );
}

#[test]
fn test_generate_expr_ident() {
    assert_eq!(generate_expr(&Expr::Ident("foo".to_string())), "foo");
}

#[test]
fn test_generate_expr_tuple_literal() {
    let expr = Expr::Tuple {
        elements: vec![
            Expr::MethodCall {
                object: Box::new(Expr::StringLit("a".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            },
            Expr::NumberLit(1.0),
        ],
    };
    assert_eq!(generate_expr(&expr), r#"("a".to_string(), 1.0)"#);
}

#[test]
fn test_generate_expr_deref_renders_star() {
    let expr = Expr::Deref(Box::new(Expr::Ident("x".to_string())));
    assert_eq!(generate_expr(&expr), "*x");
}

#[test]
fn test_generate_expr_ref_renders_ampersand() {
    let expr = Expr::Ref(Box::new(Expr::Ident("sep".to_string())));
    assert_eq!(generate_expr(&expr), "&sep");
}

#[test]
fn test_generate_expr_ref_number_renders_ampersand_literal() {
    let expr = Expr::Ref(Box::new(Expr::NumberLit(0.0)));
    assert_eq!(generate_expr(&expr), "&0.0");
}

#[test]
fn test_generate_expr_unit_renders_parens() {
    assert_eq!(generate_expr(&Expr::Unit), "()");
}

#[test]
fn test_generate_expr_int_lit_positive_renders_number() {
    assert_eq!(generate_expr(&Expr::IntLit(42)), "42");
}

#[test]
fn test_generate_expr_int_lit_negative_renders_negative() {
    assert_eq!(generate_expr(&Expr::IntLit(-1)), "-1");
}

#[test]
fn test_generate_expr_int_lit_zero_renders_zero() {
    assert_eq!(generate_expr(&Expr::IntLit(0)), "0");
}

// --- escape_rust_string (character-level escape) ---

#[test]
fn test_escape_rust_string_backslash() {
    assert_eq!(escape_rust_string(r"a\b"), r"a\\b");
}

#[test]
fn test_escape_rust_string_double_quote() {
    assert_eq!(escape_rust_string(r#"say "hello""#), r#"say \"hello\""#);
}

#[test]
fn test_escape_rust_string_newline_tab() {
    assert_eq!(escape_rust_string("a\nb"), r"a\nb");
    assert_eq!(escape_rust_string("a\tb"), r"a\tb");
}

#[test]
fn test_escape_rust_string_plain_text_unchanged() {
    assert_eq!(escape_rust_string("hello world"), "hello world");
}

#[test]
fn test_escape_rust_string_null_and_control_chars() {
    assert_eq!(escape_rust_string("\0"), r"\0");
    assert_eq!(escape_rust_string("\r"), r"\r");
}

#[test]
fn test_generate_string_lit_with_special_chars() {
    let expr = Expr::StringLit(r#"a"b\c"#.to_string());
    assert_eq!(generate_expr(&expr), r#""a\"b\\c""#);
}
