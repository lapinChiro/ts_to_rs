//! Shared AST pattern detection utilities for narrowing.
//!
//! Pure functions that extract narrowing-related patterns from SWC AST nodes.
//! Used by both [`super::type_resolver::narrowing`] (type pre-computation) and
//! [`crate::transformer::expressions::patterns`] (code generation).

use swc_ecma_ast as ast;

/// Extracts the typeof operand and comparison string from a binary expression.
///
/// Handles both orderings:
/// - `typeof x === "string"` → `Some((&x_expr, "string"))`
/// - `"string" === typeof x` → `Some((&x_expr, "string"))`
///
/// Returns `None` if the expression is not a typeof comparison.
pub(crate) fn extract_typeof_and_string(bin: &ast::BinExpr) -> Option<(&ast::Expr, String)> {
    // Only comparison operators can form typeof checks
    if !matches!(
        bin.op,
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq | ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq
    ) {
        return None;
    }

    // Left is typeof, right is string literal
    if let ast::Expr::Unary(unary) = bin.left.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.right.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    // Right is typeof, left is string literal
    if let ast::Expr::Unary(unary) = bin.right.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.left.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    None
}

/// Returns true if the expression is a `null` literal or the `undefined` identifier.
pub(crate) fn is_null_or_undefined(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
        || matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_typescript;
    use swc_ecma_ast::{ModuleItem, Stmt};

    /// Helper: parse a TS expression statement and return the SWC Expr.
    fn parse_expr(source: &str) -> ast::Expr {
        let module = parse_typescript(source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => *expr_stmt.expr.clone(),
            _ => panic!("expected expression statement"),
        }
    }

    fn as_bin_expr(expr: &ast::Expr) -> &ast::BinExpr {
        match expr {
            ast::Expr::Bin(bin) => bin,
            _ => panic!("expected binary expression"),
        }
    }

    // === extract_typeof_and_string ===

    #[test]
    fn test_extract_typeof_and_string_normal_order() {
        let expr = parse_expr(r#"typeof x === "string""#);
        let bin = as_bin_expr(&expr);
        let (operand, type_str) = extract_typeof_and_string(bin).expect("should extract");
        assert!(matches!(operand, ast::Expr::Ident(ident) if ident.sym.as_ref() == "x"));
        assert_eq!(type_str, "string");
    }

    #[test]
    fn test_extract_typeof_and_string_reversed_order() {
        let expr = parse_expr(r#""number" === typeof y"#);
        let bin = as_bin_expr(&expr);
        let (operand, type_str) = extract_typeof_and_string(bin).expect("should extract");
        assert!(matches!(operand, ast::Expr::Ident(ident) if ident.sym.as_ref() == "y"));
        assert_eq!(type_str, "number");
    }

    #[test]
    fn test_extract_typeof_and_string_non_typeof_unary_returns_none() {
        // !x === "string" — not a typeof expression
        let expr = parse_expr(r#"!x === "string""#);
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_none());
    }

    #[test]
    fn test_extract_typeof_and_string_typeof_with_number_rhs_returns_none() {
        let expr = parse_expr("typeof x === 42");
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_none());
    }

    #[test]
    fn test_extract_typeof_and_string_no_typeof_returns_none() {
        let expr = parse_expr("x === y");
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_none());
    }

    #[test]
    fn test_extract_typeof_and_string_addition_operator_returns_none() {
        // typeof x + "string" is addition, not comparison — must return None
        let expr = parse_expr(r#"typeof x + "string""#);
        let bin = as_bin_expr(&expr);
        assert!(
            extract_typeof_and_string(bin).is_none(),
            "addition with typeof should not be detected as typeof comparison"
        );
    }

    #[test]
    fn test_extract_typeof_and_string_neq_operator_returns_some() {
        // typeof x !== "string" should still be detected
        let expr = parse_expr(r#"typeof x !== "string""#);
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_some());
    }

    // === is_null_or_undefined ===

    #[test]
    fn test_is_null_or_undefined_null_literal_returns_true() {
        let expr = parse_expr("null");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_undefined_ident_returns_true() {
        let expr = parse_expr("undefined");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_other_ident_returns_false() {
        let expr = parse_expr("x");
        assert!(!is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_number_literal_returns_false() {
        let expr = parse_expr("42");
        assert!(!is_null_or_undefined(&expr));
    }
}
