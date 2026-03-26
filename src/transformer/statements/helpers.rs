//! Helper types and functions for statement conversion.
//!
//! Conditional assignment extraction, truthiness/falsy condition generation,
//! and related utilities used by the control flow conversion module.

use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType, UnOp};

/// Represents a conditional assignment extracted from a condition expression.
///
/// Covers patterns like `if (x = expr)` and `if ((x = expr) > 0)`.
pub(super) struct ConditionalAssignment<'a> {
    /// The variable name being assigned to
    pub(super) var_name: String,
    /// The right-hand side of the assignment
    pub(super) rhs: &'a ast::Expr,
    /// If the assignment was inside a comparison, the outer comparison details.
    /// `None` for bare assignments like `if (x = expr)`.
    pub(super) outer_comparison: Option<OuterComparison<'a>>,
}

/// Details of a comparison expression wrapping a conditional assignment.
pub(super) struct OuterComparison<'a> {
    /// The binary operator (e.g., `>`, `!==`)
    pub(super) op: ast::BinaryOp,
    /// The other operand of the comparison (not the assignment side)
    pub(super) other_operand: &'a ast::Expr,
    /// Whether the assignment was on the left side of the comparison
    pub(super) assign_on_left: bool,
}

/// Extracts a conditional assignment from a condition expression, if present.
///
/// Recognizes:
/// - Bare assignment: `x = expr` (possibly wrapped in parens)
/// - Assignment inside comparison: `(x = expr) > 0`, `(x = expr) !== null`
pub(super) fn extract_conditional_assignment(
    expr: &ast::Expr,
) -> Option<ConditionalAssignment<'_>> {
    // Unwrap parentheses
    let expr = unwrap_parens(expr);

    // Pattern 1: bare assignment `x = expr`
    if let ast::Expr::Assign(assign) = expr {
        if assign.op == ast::AssignOp::Assign {
            if let Some(var_name) = extract_assign_target_name(&assign.left) {
                return Some(ConditionalAssignment {
                    var_name,
                    rhs: &assign.right,
                    outer_comparison: None,
                });
            }
        }
    }

    // Pattern 2: comparison with assignment on one side: `(x = expr) > 0`
    if let ast::Expr::Bin(bin) = expr {
        if is_comparison_op(bin.op) {
            // Check left side for assignment
            if let Some(assign) = extract_assign_from_expr(&bin.left) {
                return Some(ConditionalAssignment {
                    var_name: assign.0,
                    rhs: assign.1,
                    outer_comparison: Some(OuterComparison {
                        op: bin.op,
                        other_operand: &bin.right,
                        assign_on_left: true,
                    }),
                });
            }
            // Check right side for assignment
            if let Some(assign) = extract_assign_from_expr(&bin.right) {
                return Some(ConditionalAssignment {
                    var_name: assign.0,
                    rhs: assign.1,
                    outer_comparison: Some(OuterComparison {
                        op: bin.op,
                        other_operand: &bin.left,
                        assign_on_left: false,
                    }),
                });
            }
        }
    }

    None
}

/// Unwraps nested parentheses from an expression.
pub(super) fn unwrap_parens(expr: &ast::Expr) -> &ast::Expr {
    match expr {
        ast::Expr::Paren(p) => unwrap_parens(&p.expr),
        _ => expr,
    }
}

/// Extracts the variable name from an assignment target.
fn extract_assign_target_name(target: &ast::AssignTarget) -> Option<String> {
    match target {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
            Some(ident.id.sym.to_string())
        }
        _ => None,
    }
}

/// Extracts an assignment expression from a (possibly parenthesized) expression.
fn extract_assign_from_expr(expr: &ast::Expr) -> Option<(String, &ast::Expr)> {
    let expr = unwrap_parens(expr);
    if let ast::Expr::Assign(assign) = expr {
        if assign.op == ast::AssignOp::Assign {
            if let Some(name) = extract_assign_target_name(&assign.left) {
                return Some((name, &assign.right));
            }
        }
    }
    None
}

/// Returns true if the operator is a comparison (not logical).
fn is_comparison_op(op: ast::BinaryOp) -> bool {
    matches!(
        op,
        ast::BinaryOp::EqEq
            | ast::BinaryOp::NotEq
            | ast::BinaryOp::EqEqEq
            | ast::BinaryOp::NotEqEq
            | ast::BinaryOp::Lt
            | ast::BinaryOp::LtEq
            | ast::BinaryOp::Gt
            | ast::BinaryOp::GtEq
    )
}

/// Generates a truthiness check expression for a given type.
///
/// Returns `None` for Option types (which use `if let` / `while let` instead).
pub(super) fn generate_truthiness_condition(var_name: &str, ty: &RustType) -> Expr {
    match ty {
        RustType::F64 => Expr::BinaryOp {
            left: Box::new(Expr::Ident(var_name.to_string())),
            op: BinOp::NotEq,
            right: Box::new(Expr::NumberLit(0.0)),
        },
        RustType::String => Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident(var_name.to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }),
        },
        RustType::Bool => Expr::Ident(var_name.to_string()),
        // Fallback for unknown types: use the variable as-is (may need manual fixing)
        _ => Expr::Ident(var_name.to_string()),
    }
}

/// Generates a falsy check condition (the inverse of truthiness) for loop break.
pub(super) fn generate_falsy_condition(var_name: &str, ty: &RustType) -> Expr {
    match ty {
        RustType::F64 => Expr::BinaryOp {
            left: Box::new(Expr::Ident(var_name.to_string())),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        },
        RustType::String => Expr::MethodCall {
            object: Box::new(Expr::Ident(var_name.to_string())),
            method: "is_empty".to_string(),
            args: vec![],
        },
        RustType::Bool => Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident(var_name.to_string())),
        },
        _ => Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident(var_name.to_string())),
        },
    }
}
