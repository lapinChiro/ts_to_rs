//! Compound logical assignment (`&&=` / `||=`) desugar to conditional-assign (I-161).
//!
//! TS `x &&= y` / `x ||= y` semantically equal `if (x) x = y` / `if (!x) x = y`.
//! Rust's `&&` / `||` are `bool`-only, so the naive `x = x && y` emission fails
//! with E0308 / E0369 for non-bool LHS (`F64`, `String`, `Option<T>`, `Named`, …).
//!
//! This module desugars the operator per effective LHS type:
//!
//! - Primitive (`Bool` / `F64` / `String` / `Primitive(int)`):
//!   `if <truthy(x)> { x = y; }` (and the falsy form for `||=`).
//! - `Option<T>`: `if <truthy(x)> { x = Some(y); }` — the RHS is typed as
//!   inner `T` (TypeResolver propagation in `expressions::rhs_expected_for_compound`)
//!   and wrapped in `Some(_)` at emission time to restore the Option shape.
//! - `Option<synthetic union>`: per-variant `match` guard (via
//!   `truthy_predicate_for_expr`'s synthetic-union arm); otherwise the
//!   structure mirrors primitives.
//! - Always-truthy (`Named non-union` / `Vec` / `Fn` / `StdCollection` /
//!   `DynTrait` / `Ref` / `Tuple`): const-fold
//!   - `&&=` stmt → `x = y;`, expr → `{ x = y; x[.clone()] }`
//!   - `||=` stmt → no-op (empty), expr → `x[.clone()]` (original x, not `()`)
//! - `Any` / `TypeVar`: blocked (I-050 umbrella / generic bounds PRD).
//!
//! # Emission shape
//!
//! | Context | `&&=` (predicate-based) | `\|\|=` (predicate-based) |
//! |---------|------------------------|-------------------------|
//! | Stmt    | `if <truthy(x)> { x = wrap?(y); }` | `if <falsy(x)> { x = wrap?(y); }` |
//! | Expr    | `{ <stmt>; x[.clone()] }` | `{ <stmt>; x[.clone()] }` |
//!
//! The stmt-form is emitted when the compound assign appears as a bare
//! [`ast::Stmt::Expr`] (intercepted by [`Transformer::try_convert_compound_logical_assign_stmt`]).
//! The expr-form is emitted by [`Transformer::convert_assign_expr`] otherwise
//! (expression context: call arg, ternary branch, return, etc.).

use anyhow::Result;
use swc_common::{Span, Spanned};
use swc_ecma_ast as ast;

use crate::ir::{BuiltinVariant, CallTarget, Expr, RustType, Stmt};
use crate::pipeline::synthetic_registry::SyntheticTypeRegistry;
use crate::transformer::helpers::truthy::{
    falsy_predicate_for_expr, is_always_truthy_type, truthy_predicate_for_expr, TempBinder,
};
use crate::transformer::{Transformer, UnsupportedSyntaxError};

impl<'a> Transformer<'a> {
    /// Intercepts `ast::Stmt::Expr` of shape `x &&= y` / `x ||= y` and emits
    /// `if` statement form instead of the expression-context block form.
    ///
    /// Paren / TsAs / TsNonNull / TsTypeAssertion / TsConstAssertion wrappers
    /// around the Assign expression are runtime-no-op type markers, so the
    /// intercept peeks through them (via [`peek_through_type_assertions`])
    /// before pattern matching — `(x &&= y);` / `(x &&= y as T);` stmt forms
    /// take the same stmt-form emission as bare `x &&= y;`.
    ///
    /// Returns `Ok(None)` when the expression is not a compound logical
    /// assign, so the caller continues with the generic `convert_expr` path.
    pub(crate) fn try_convert_compound_logical_assign_stmt(
        &mut self,
        expr: &ast::Expr,
    ) -> Result<Option<Vec<Stmt>>> {
        let expr = crate::transformer::helpers::peek_through::peek_through_type_assertions(expr);
        let ast::Expr::Assign(assign) = expr else {
            return Ok(None);
        };
        if !matches!(
            assign.op,
            ast::AssignOp::AndAssign | ast::AssignOp::OrAssign
        ) {
            return Ok(None);
        }
        let (target, lhs_type) = self.resolve_compound_logical_assign_target(assign)?;
        let right = self.convert_expr(&assign.right)?;
        let stmts = self.desugar_compound_logical_assign_stmts(
            target,
            right,
            &lhs_type,
            assign.op,
            assign.span(),
        )?;
        Ok(Some(stmts))
    }

    /// Expression-context desugar: returns a block expression that performs
    /// the conditional assign and yields the current value of the LHS.
    ///
    /// Used from [`Self::convert_assign_expr`] when the compound logical
    /// assign appears nested inside a larger expression. Thin wrapper around
    /// the free function [`desugar_compound_logical_assign_expr`] so the core
    /// dispatch logic is unit-testable without a `Transformer` instance.
    pub(crate) fn desugar_compound_logical_assign_expr(
        &mut self,
        target: Expr,
        right: Expr,
        lhs_type: &RustType,
        op: ast::AssignOp,
        span: Span,
    ) -> Result<Expr> {
        desugar_compound_logical_assign_expr(self.synthetic, target, right, lhs_type, op, span)
    }

    /// Statement-form desugar: thin wrapper around the free function
    /// [`desugar_compound_logical_assign_stmts`].
    pub(crate) fn desugar_compound_logical_assign_stmts(
        &self,
        target: Expr,
        right: Expr,
        lhs_type: &RustType,
        op: ast::AssignOp,
        span: Span,
    ) -> Result<Vec<Stmt>> {
        desugar_compound_logical_assign_stmts(self.synthetic, target, right, lhs_type, op, span)
    }

    /// Resolves the target IR [`Expr`] and LHS [`RustType`] for the compound
    /// logical assign.
    fn resolve_compound_logical_assign_target(
        &mut self,
        assign: &ast::AssignExpr,
    ) -> Result<(Expr, RustType)> {
        match &assign.left {
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
                let name = ident.id.sym.to_string();
                let ty = self
                    .get_type_for_var(&name, ident.id.span)
                    .cloned()
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "compound logical assign on unresolved ident type",
                            assign.span(),
                        )
                    })?;
                Ok((Expr::Ident(name), ty))
            }
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => {
                let member_expr = ast::Expr::Member(member.clone());
                let ty = self.get_expr_type(&member_expr).cloned().ok_or_else(|| {
                    UnsupportedSyntaxError::new(
                        "compound logical assign on unresolved member type",
                        assign.span(),
                    )
                })?;
                let target = self.convert_member_expr_for_write(member)?;
                Ok((target, ty))
            }
            _ => Err(UnsupportedSyntaxError::new(
                "unsupported compound logical assign target",
                assign.span(),
            )
            .into()),
        }
    }
}

/// Free-function form of stmt-context desugar (I-161).
///
/// Takes [`SyntheticTypeRegistry`] explicitly so unit tests can exercise the
/// dispatch logic without constructing a full `Transformer`. The `Transformer`
/// method delegates here.
pub(crate) fn desugar_compound_logical_assign_stmts(
    synthetic: &SyntheticTypeRegistry,
    target: Expr,
    right: Expr,
    lhs_type: &RustType,
    op: ast::AssignOp,
    span: Span,
) -> Result<Vec<Stmt>> {
    // Any / TypeVar: blocked.
    if matches!(lhs_type, RustType::Any | RustType::TypeVar { .. }) {
        return Err(UnsupportedSyntaxError::new(
            "compound logical assign on Any/TypeVar (I-050 umbrella / generic bounds)",
            span,
        )
        .into());
    }
    // Always-truthy: const-fold.
    if is_always_truthy_type(lhs_type, synthetic) {
        return Ok(const_fold_always_truthy_stmts(target, right, lhs_type, op));
    }
    // Predicate-based.
    let mut binder = TempBinder::new();
    let predicate = match op {
        ast::AssignOp::AndAssign => {
            truthy_predicate_for_expr(&target, lhs_type, synthetic, &mut binder)
        }
        ast::AssignOp::OrAssign => {
            falsy_predicate_for_expr(&target, lhs_type, synthetic, &mut binder)
        }
        _ => unreachable!("only AndAssign/OrAssign dispatch here"),
    };
    let Some(predicate) = predicate else {
        return Err(UnsupportedSyntaxError::new(
            "compound logical assign on unsupported type (truthy predicate unavailable)",
            span,
        )
        .into());
    };
    let value = compound_assign_value(right, lhs_type);
    let assign_stmt = Stmt::Expr(Expr::Assign {
        target: Box::new(target),
        value: Box::new(value),
    });
    Ok(vec![Stmt::If {
        condition: predicate,
        then_body: vec![assign_stmt],
        else_body: None,
    }])
}

/// Free-function form of expr-context desugar (I-161).
pub(crate) fn desugar_compound_logical_assign_expr(
    synthetic: &SyntheticTypeRegistry,
    target: Expr,
    right: Expr,
    lhs_type: &RustType,
    op: ast::AssignOp,
    span: Span,
) -> Result<Expr> {
    let is_copy = lhs_type.is_copy_type();
    let mut stmts = desugar_compound_logical_assign_stmts(
        synthetic,
        target.clone(),
        right,
        lhs_type,
        op,
        span,
    )?;
    // Tail: the LHS's current value. Copy types return by copy; non-Copy
    // types clone to avoid moving out of the original location.
    let tail = if is_copy {
        target
    } else {
        Expr::MethodCall {
            object: Box::new(target),
            method: "clone".to_string(),
            args: vec![],
        }
    };
    // Optimisation: when stmts is empty (||= on always-truthy stmt context
    // is inert), expr-context still needs to return the tail.
    if stmts.is_empty() {
        return Ok(tail);
    }
    stmts.push(Stmt::TailExpr(tail));
    Ok(Expr::Block(stmts))
}

/// Stmts emitted when the LHS is always-truthy.
///
/// - `&&=`: the truthy branch always fires → unconditional `x = y`.
/// - `||=`: the falsy branch never fires → no-op (empty stmt list).
fn const_fold_always_truthy_stmts(
    target: Expr,
    right: Expr,
    lhs_type: &RustType,
    op: ast::AssignOp,
) -> Vec<Stmt> {
    match op {
        ast::AssignOp::AndAssign => {
            let value = compound_assign_value(right, lhs_type);
            vec![Stmt::Expr(Expr::Assign {
                target: Box::new(target),
                value: Box::new(value),
            })]
        }
        ast::AssignOp::OrAssign => vec![],
        _ => unreachable!("only AndAssign/OrAssign pass through const-fold"),
    }
}

/// Wraps the RHS value in `Some(_)` when the LHS is `Option<T>`, otherwise
/// returns the RHS unchanged.
///
/// The RHS has already been typed against the inner `T` thanks to
/// `rhs_expected_for_compound` propagation (`type_resolver::expressions`).
/// At emission time we restore the `Option` shape so the IR-level `Assign`
/// is `Option<T> := Option<T>`.
fn compound_assign_value(right: Expr, lhs_type: &RustType) -> Expr {
    match lhs_type {
        RustType::Option(_) => Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
            args: vec![right],
        },
        _ => right,
    }
}
