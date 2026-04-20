//! Helper types and functions for statement conversion.
//!
//! Conditional assignment extraction, truthiness/falsy condition generation,
//! and related utilities used by the control flow conversion module.

use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType, UnOp};
use crate::transformer::helpers::truthy;

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
/// Delegates to [`truthy::truthy_predicate`] for supported primitives
/// (`F64` / `String` / `Bool` / integer primitives) and falls back to a
/// bare identifier for unsupported types.
///
/// # Fallback contract
///
/// The `Expr::Ident(var_name)` fallback produces `if var { ... }` which is
/// a Rust type error for all non-`Bool` types. This is an intentional
/// _compile-time fence_: callers that hit the fallback with a non-`Bool`
/// type will discover the gap via `rustc`, rather than silently
/// generating incorrect behaviour. The fallback is expected only in the
/// two existing call sites:
///
/// 1. [`crate::transformer::statements::control_flow::Transformer::convert_if_with_conditional_assignment`]:
///    enters only for `if (x = expr)` where `x` has an already-resolved
///    type — primitives are handled by [`truthy::truthy_predicate`] and
///    richer types use dedicated narrow paths earlier in `convert_if_stmt`.
/// 2. `generate_falsy_condition` loop-break uses (see below).
///
/// Do NOT widen this fallback to emit `.is_some()` or similar guesses —
/// that would introduce silent semantic changes (Tier 1) instead of
/// surfacing gaps as compile errors.
pub(super) fn generate_truthiness_condition(var_name: &str, ty: &RustType) -> Expr {
    truthy::truthy_predicate(var_name, ty).unwrap_or_else(|| Expr::Ident(var_name.to_string()))
}

/// Generates a falsy check condition (De Morgan inverse of truthiness).
///
/// Used for loop break conditions. Shares the compile-time fence contract
/// described on [`generate_truthiness_condition`].
pub(super) fn generate_falsy_condition(var_name: &str, ty: &RustType) -> Expr {
    truthy::falsy_predicate(var_name, ty).unwrap_or_else(|| Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::Ident(var_name.to_string())),
    })
}
