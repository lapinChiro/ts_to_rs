use super::*;
use crate::parser::parse_typescript;
use crate::transformer::statements::helpers::{
    extract_conditional_assignment, generate_falsy_condition, generate_truthiness_condition,
};
use swc_ecma_ast::ModuleItem;

/// Parse a TS expression statement and return the SWC Expr.
fn parse_expr(source: &str) -> swc_ecma_ast::Expr {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => *expr_stmt.expr.clone(),
        _ => panic!("expected expression statement"),
    }
}

// ─── extract_conditional_assignment ─────────────────────────────────

#[test]
fn test_extract_conditional_assignment_bare_assignment_returns_some() {
    let expr = parse_expr("x = expr;");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some for bare assignment");
    assert_eq!(ca.var_name, "x");
    assert!(ca.outer_comparison.is_none());
    // rhs should be the identifier `expr`
    assert!(matches!(ca.rhs, swc_ecma_ast::Expr::Ident(id) if id.sym == "expr"));
}

#[test]
fn test_extract_conditional_assignment_comparison_with_left_assign_returns_outer() {
    // (x = expr) > 0
    let expr = parse_expr("(x = expr) > 0;");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some for comparison with left assignment");
    assert_eq!(ca.var_name, "x");
    let outer = ca.outer_comparison.expect("should have outer comparison");
    assert_eq!(outer.op, swc_ecma_ast::BinaryOp::Gt);
    assert!(outer.assign_on_left);
    // other_operand should be the number 0
    assert!(matches!(
        outer.other_operand,
        swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Num(_))
    ));
}

#[test]
fn test_extract_conditional_assignment_comparison_with_right_assign_returns_outer() {
    // 0 < (x = expr)
    let expr = parse_expr("0 < (x = expr);");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some for comparison with right assignment");
    assert_eq!(ca.var_name, "x");
    let outer = ca.outer_comparison.expect("should have outer comparison");
    assert_eq!(outer.op, swc_ecma_ast::BinaryOp::Lt);
    assert!(!outer.assign_on_left);
}

#[test]
fn test_extract_conditional_assignment_no_assignment_returns_none() {
    let expr = parse_expr("x > 0;");
    let result = extract_conditional_assignment(&expr);
    assert!(result.is_none(), "plain comparison should return None");
}

#[test]
fn test_extract_conditional_assignment_nested_parens_unwraps() {
    // (((x = expr))) — multiple layers of parens should be unwrapped
    let expr = parse_expr("(((x = expr)));");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some after unwrapping nested parens");
    assert_eq!(ca.var_name, "x");
    assert!(ca.outer_comparison.is_none());
    assert!(matches!(ca.rhs, swc_ecma_ast::Expr::Ident(id) if id.sym == "expr"));
}

// ─── generate_truthiness_condition / generate_falsy_condition ────────

#[test]
fn test_generate_truthiness_condition_f64_generates_not_eq_zero() {
    let result = generate_truthiness_condition("val", &RustType::F64);
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("val".to_string())),
            op: BinOp::NotEq,
            right: Box::new(Expr::NumberLit(0.0)),
        }
    );
}

#[test]
fn test_generate_truthiness_condition_string_generates_not_is_empty() {
    let result = generate_truthiness_condition("s", &RustType::String);
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }),
        }
    );
}

#[test]
fn test_generate_truthiness_condition_bool_generates_ident() {
    let result = generate_truthiness_condition("flag", &RustType::Bool);
    assert_eq!(result, Expr::Ident("flag".to_string()));
}

#[test]
fn test_generate_falsy_condition_is_inverse_of_truthiness() {
    // For F64: truthiness is `!= 0.0`, falsy is `== 0.0`
    let truth_f64 = generate_truthiness_condition("v", &RustType::F64);
    let falsy_f64 = generate_falsy_condition("v", &RustType::F64);
    assert_eq!(
        truth_f64,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("v".to_string())),
            op: BinOp::NotEq,
            right: Box::new(Expr::NumberLit(0.0)),
        }
    );
    assert_eq!(
        falsy_f64,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("v".to_string())),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        }
    );

    // For String: truthiness is `!s.is_empty()`, falsy is `s.is_empty()`
    let truth_str = generate_truthiness_condition("s", &RustType::String);
    let falsy_str = generate_falsy_condition("s", &RustType::String);
    assert_eq!(
        truth_str,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }),
        }
    );
    assert_eq!(
        falsy_str,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "is_empty".to_string(),
            args: vec![],
        }
    );

    // For Bool: truthiness is `flag`, falsy is `!flag`
    let truth_bool = generate_truthiness_condition("flag", &RustType::Bool);
    let falsy_bool = generate_falsy_condition("flag", &RustType::Bool);
    assert_eq!(truth_bool, Expr::Ident("flag".to_string()));
    assert_eq!(
        falsy_bool,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident("flag".to_string())),
        }
    );
}
