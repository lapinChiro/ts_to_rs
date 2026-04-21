//! `Expr::BinaryOp` rendering: plain binary arithmetic and the bitwise
//! operator cast rules (TS `&|^<<>>` numbers are `f64`; Rust bitwise
//! requires integer operands, so the generator emits `((x as i64) OP
//! (y as i64)) as f64` wrappers). `>>>` casts via `u32`.
//!
//! Also verifies nested bitwise composition and arithmetic mixed with
//! a bitwise child (arithmetic itself is NOT cast — only the bitwise
//! subtree is).

use super::*;
use crate::ir::{BinOp, Expr};

#[test]
fn test_generate_expr_binary_op() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Add,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "a + b");
}

#[test]
fn test_generate_expr_bitwise_and_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::BitAnd,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) & (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_or_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::BitOr,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) | (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_xor_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::BitXor,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) ^ (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_shl_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Shl,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) << (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_shr_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Shr,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) >> (b as i64)) as f64");
}

#[test]
fn test_generate_expr_unsigned_shr_casts_to_u32() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::UShr,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(
        generate_expr(&expr),
        "((a as i32 as u32) >> (b as u32)) as f64"
    );
}

#[test]
fn test_generate_expr_bitwise_nested_or_and() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::BitAnd,
            right: Box::new(Expr::Ident("b".to_string())),
        }),
        op: BinOp::BitOr,
        right: Box::new(Expr::Ident("c".to_string())),
    };
    assert_eq!(
        generate_expr(&expr),
        "((((a as i64) & (b as i64)) as f64 as i64) | (c as i64)) as f64"
    );
}

#[test]
fn test_generate_expr_arithmetic_with_bitwise_no_cast_on_arithmetic() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Add,
        right: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("b".to_string())),
            op: BinOp::BitAnd,
            right: Box::new(Expr::Ident("c".to_string())),
        }),
    };
    assert_eq!(
        generate_expr(&expr),
        "a + (((b as i64) & (c as i64)) as f64)"
    );
}
