//! Binary expression type resolution (`Bin` arm of `resolve_expr_inner`).
//!
//! Covers comparison / arithmetic / logical / nullish-coalescing operators. The
//! `LogicalOr` arm uses [`TypeResolver::propagate_fallback_expected`] to push the LHS
//! type onto an object-literal RHS (`x || {}` pattern); `??` propagates LHS/RHS
//! expected types directly inline (I-022 unified runtime-nullability propagation).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::super::*;
use crate::pipeline::type_resolution::Span;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_bin_expr(&mut self, bin: &ast::BinExpr) -> ResolvedType {
        use ast::BinaryOp::*;
        match bin.op {
            Lt | LtEq | Gt | GtEq | EqEq | NotEq | EqEqEq | NotEqEq | In | InstanceOf => {
                // Resolve operands to register their expr_types (used by Transformer
                // for typeof comparison, enum string comparison, in operator, etc.)
                self.resolve_expr(&bin.left);
                self.resolve_expr(&bin.right);
                ResolvedType::Known(RustType::Bool)
            }
            Add => {
                let left_ty = self.resolve_expr(&bin.left);
                let right_ty = self.resolve_expr(&bin.right);
                if matches!(left_ty, ResolvedType::Known(RustType::String))
                    || matches!(right_ty, ResolvedType::Known(RustType::String))
                {
                    ResolvedType::Known(RustType::String)
                } else {
                    ResolvedType::Known(RustType::F64)
                }
            }
            Sub | Mul | Div | Mod | Exp | BitAnd | BitOr | BitXor | LShift | RShift
            | ZeroFillRShift => {
                self.resolve_expr(&bin.left);
                self.resolve_expr(&bin.right);
                ResolvedType::Known(RustType::F64)
            }
            LogicalAnd => {
                // Both sides must be resolved to register all sub-expression types
                // (e.g., `typeof x === "string" && typeof y === "number"` needs both
                // x and y registered in expr_types for narrowing guard resolution)
                let left = self.resolve_expr(&bin.left);
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    left
                }
            }
            LogicalOr => {
                let left = self.resolve_expr(&bin.left);
                // `x || {}` — propagate left operand's type to fallback object literal
                if let ResolvedType::Known(ref left_ty) = left {
                    self.propagate_fallback_expected(&bin.right, left_ty);
                }
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    left
                }
            }
            NullishCoalescing => {
                let left = self.resolve_expr(&bin.left);
                let lhs_span = Span::from_swc(bin.left.span());
                let rhs_span = Span::from_swc(bin.right.span());

                // The `??` operator asserts runtime nullability of LHS regardless
                // of its static TS type (e.g., `arr[i]` has TS type `T` yet may be
                // undefined at runtime; `Option<T>` already-nullable). Unified
                // propagation: always set LHS span expected to `Option<inner>` so
                // `convert_member_expr` emits Option-preserving IR (`.get().cloned()`
                // or `.get().cloned().flatten()` for `Vec<Option<T>>`), and set RHS
                // span expected to `inner` (the final NC result type).
                //
                // `inner` is the unwrapped element type: `T` for non-Option LHS,
                // and the Option's inner type for Option LHS. Both paths produce
                // identical LHS-expected (`Option<inner>`), only differing in how
                // `inner` is computed. I-022.
                if let ResolvedType::Known(left_ty) = &left {
                    let inner = match left_ty {
                        RustType::Option(inner) => inner.as_ref().clone(),
                        other => other.clone(),
                    };
                    let lhs_expected = RustType::Option(Box::new(inner.clone()));
                    self.result
                        .expected_types
                        .insert(lhs_span, lhs_expected.clone());
                    self.propagate_expected(&bin.left, &lhs_expected);

                    // Preserve chain-case Option RHS: if a parent `propagate_expected`
                    // (the `Bin(NullishCoalescing)` arm) has already set RHS span to
                    // `Option<T>`, keep it so inner operands in an `a ?? b ?? c` chain
                    // produce Option-preserving IR. Otherwise (terminate case) use
                    // `inner` — the unwrapped final NC result type.
                    let existing_rhs = self.result.expected_types.get(&rhs_span).cloned();
                    let rhs_expected = match existing_rhs {
                        Some(RustType::Option(_)) => existing_rhs.unwrap(),
                        _ => inner.clone(),
                    };
                    self.result
                        .expected_types
                        .insert(rhs_span, rhs_expected.clone());
                    self.propagate_expected(&bin.right, &rhs_expected);
                }
                // LHS type unknown: leave expected types unset (existing
                // untyped-expression behavior).
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    left
                }
            }
        }
    }

    /// `x || {}` パターンで、右辺がオブジェクトリテラルの場合に左辺の解決済み型を
    /// expected type として右辺に伝播する。
    ///
    /// `??` (NullishCoalescing) は I-022 以降、この helper を使わず上記 arm で直接
    /// LHS/RHS の expected type を propagate する (runtime nullability を LHS に
    /// 反映するため汎用 propagation が必要)。本 helper は LogicalOr 専用。
    fn propagate_fallback_expected(&mut self, rhs: &ast::Expr, left_ty: &RustType) {
        if matches!(rhs, ast::Expr::Object(_)) {
            let resolved = self.resolve_type_params_in_type(left_ty);
            let rhs_span = Span::from_swc(rhs.span());
            self.result
                .expected_types
                .insert(rhs_span, resolved.clone());
            self.propagate_expected(rhs, &resolved);
        }
    }
}
