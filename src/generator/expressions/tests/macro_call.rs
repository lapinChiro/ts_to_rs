//! `Expr::MacroCall` rendering: template synthesis based on arg count,
//! macro name passthrough (`println`, `eprintln`, ...), and
//! `{}` vs `{:?}` selection via the `use_debug` parallel vector.

use super::*;
use crate::ir::Expr;

#[test]
fn test_generate_expr_macro_call_no_args() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![],
        use_debug: vec![],
    };
    assert_eq!(generate_expr(&expr), "println!()");
}

#[test]
fn test_generate_expr_macro_call_single_string_literal() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![Expr::StringLit("hello".to_string())],
        use_debug: vec![false],
    };
    assert_eq!(generate_expr(&expr), "println!(\"hello\")");
}

#[test]
fn test_generate_expr_macro_call_single_ident() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![Expr::Ident("x".to_string())],
        use_debug: vec![false],
    };
    assert_eq!(generate_expr(&expr), "println!(\"{}\", x)");
}

#[test]
fn test_generate_expr_macro_call_multiple_args() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![
            Expr::StringLit("value:".to_string()),
            Expr::Ident("x".to_string()),
        ],
        use_debug: vec![false, false],
    };
    assert_eq!(generate_expr(&expr), "println!(\"{} {}\", \"value:\", x)");
}

#[test]
fn test_generate_expr_macro_call_eprintln() {
    let expr = Expr::MacroCall {
        name: "eprintln".to_string(),
        args: vec![Expr::Ident("err".to_string())],
        use_debug: vec![false],
    };
    assert_eq!(generate_expr(&expr), "eprintln!(\"{}\", err)");
}

#[test]
fn test_generate_expr_macro_call_use_debug_single() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![Expr::Ident("arr".to_string())],
        use_debug: vec![true],
    };
    assert_eq!(generate_expr(&expr), "println!(\"{:?}\", arr)");
}

#[test]
fn test_generate_expr_macro_call_use_debug_mixed() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![
            Expr::StringLit("items:".to_string()),
            Expr::Ident("arr".to_string()),
        ],
        use_debug: vec![false, true],
    };
    assert_eq!(
        generate_expr(&expr),
        "println!(\"{} {:?}\", \"items:\", arr)"
    );
}
