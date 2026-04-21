//! `Expr::Closure` rendering: expr-body vs block-body, the return type
//! brace rule (an expr body gets braces when a return type is present,
//! since `|x| -> T y` is not valid Rust), param annotation presence,
//! and the no-params shorthand `|| ...`.

use super::*;
use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType, Stmt};

#[test]
fn test_generate_closure_expr_body() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x: f64| x + 1.0");
}

#[test]
fn test_generate_closure_block_body() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::F64),
        body: ClosureBody::Block(vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        })]),
    };
    let expected = "|x: f64| -> f64 {\n    x + 1.0\n}";
    assert_eq!(generate_expr(&expr), expected);
}

#[test]
fn test_generate_closure_no_params() {
    let expr = Expr::Closure {
        params: vec![],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::NumberLit(42.0))),
    };
    assert_eq!(generate_expr(&expr), "|| 42.0");
}

#[test]
fn test_generate_closure_param_no_type_annotation() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: None,
        }],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x| x + 1.0");
}

#[test]
fn test_generate_closure_expr_body_with_return_type_has_braces() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::F64),
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Mul,
            right: Box::new(Expr::NumberLit(2.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x: f64| -> f64 { x * 2.0 }");
}

#[test]
fn test_generate_closure_expr_body_without_return_type_no_braces() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Mul,
            right: Box::new(Expr::NumberLit(2.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x: f64| x * 2.0");
}
