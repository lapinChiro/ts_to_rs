//! TypeScript type-assertion expressions (`TsAs`, `TsTypeAssertion`, `TsNonNull`).
//!
//! All four TS type-assertion forms have a single architectural concern: "tighten
//! the inferred type of `inner_expr` according to the asserted type." Three of them
//! ([`TsAs`](ast::TsAsExpr) `expr as T`, [`TsTypeAssertion`](ast::TsTypeAssertion) `<T>expr`,
//! [`TsNonNull`](ast::TsNonNullExpr) `expr!`) carry non-trivial logic and live here;
//! `TsConstAssertion` (`expr as const`) is a pure passthrough and stays inline in
//! [`super::mod`]'s dispatcher.
//!
//! ## DRY: [`Self::resolve_type_assertion_inner`]
//!
//! `as T` and `<T>expr` differ only in SWC AST shape — their semantic action is
//! token-level identical (convert TS type → propagate as expected → resolve inner →
//! wrap for value position). The shared helper consolidates this 4-step logic so
//! [`Self::resolve_ts_as_expr`] and [`Self::resolve_ts_type_assertion_expr`] become
//! 1-line dispatchers.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::super::*;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
use crate::transformer::type_position::{wrap_trait_for_position, TypePosition};

impl<'a> TypeResolver<'a> {
    /// Shared 4-step logic for `expr as T` and `<T>expr`.
    ///
    /// The two forms differ only in SWC AST shape, not in semantics:
    /// 1. Convert `type_ann` to a [`RustType`] via [`convert_ts_type`].
    /// 2. Insert the converted type as the expected type at `inner.span()` and
    ///    propagate to inner sub-expressions (so `{...x} as SomeType` resolves the
    ///    object literal against `SomeType`'s field shape, etc.).
    /// 3. Resolve the inner expression for its expr_type entry + nested call resolution.
    /// 4. Wrap the asserted type for `TypePosition::Value` (e.g., `dyn Trait` →
    ///    `Box<dyn Trait>`).
    ///
    /// Returns [`ResolvedType::Known`] of the wrapped asserted type, or
    /// [`ResolvedType::Unknown`] if `convert_ts_type` fails.
    fn resolve_type_assertion_inner(
        &mut self,
        type_ann: &ast::TsType,
        inner: &ast::Expr,
    ) -> ResolvedType {
        let as_type = convert_ts_type(type_ann, self.synthetic, self.registry).ok();
        if let Some(ref ty) = as_type {
            let expr_span = Span::from_swc(inner.span());
            self.result.expected_types.insert(expr_span, ty.clone());
            self.propagate_expected(inner, ty);
        }
        // Resolve inner expression to register its type and trigger nested
        // call resolution (e.g., `foo(bar(x) as T)` needs bar's args typed).
        self.resolve_expr(inner);
        as_type
            .map(|ty| {
                let wrapped = wrap_trait_for_position(ty, TypePosition::Value, self.registry);
                ResolvedType::Known(wrapped)
            })
            .unwrap_or(ResolvedType::Unknown)
    }

    /// `expr as T` — propagate `T` as expected type to `expr`, return `T` for
    /// the assertion's value position.
    pub(super) fn resolve_ts_as_expr(&mut self, ts_as: &ast::TsAsExpr) -> ResolvedType {
        self.resolve_type_assertion_inner(&ts_as.type_ann, &ts_as.expr)
    }

    /// `<T>expr` — semantically identical to `expr as T`, separated only by SWC AST
    /// shape (legacy angle-bracket form vs `as T` form).
    pub(super) fn resolve_ts_type_assertion_expr(
        &mut self,
        assertion: &ast::TsTypeAssertion,
    ) -> ResolvedType {
        self.resolve_type_assertion_inner(&assertion.type_ann, &assertion.expr)
    }

    /// `expr!` (non-null assertion) — unwrap `Option<T>` to `T`. Non-Option inner
    /// types pass through unchanged (TS allows `!` on already-non-null types as a no-op).
    pub(super) fn resolve_ts_non_null_expr(
        &mut self,
        ts_non_null: &ast::TsNonNullExpr,
    ) -> ResolvedType {
        let inner = self.resolve_expr(&ts_non_null.expr);
        match inner {
            ResolvedType::Known(RustType::Option(inner_ty)) => ResolvedType::Known(*inner_ty),
            other => other, // Not Option — return as-is
        }
    }
}
