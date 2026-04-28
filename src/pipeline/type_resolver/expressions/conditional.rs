//! Conditional (ternary) expression type resolution.
//!
//! `a ? b : c` resolves to:
//! - `Option<T>` if either branch is null/undefined or already `Option<T>` (with the
//!   non-null branch supplying `T`).
//! - The common type if both branches resolve to the same type.
//! - A synthetic union enum if branches resolve to different known types.
//! - Whichever branch is known if the other is unknown; `Unknown` if both are.
//!
//! Type assertion forms (`TsAs`, `TsTypeAssertion`, `TsNonNull`) live in
//! [`super::assertions`] — they share a single architectural concern (tighten the
//! inferred type of `inner_expr`) distinct from this module's branch-selection logic.

use swc_ecma_ast as ast;

use super::super::*;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_cond_expr(&mut self, cond: &ast::CondExpr) -> ResolvedType {
        // Ternary: resolve test and both branches.
        // Test must be resolved so sub-expression types (e.g., variable Idents
        // in `x !== null`) are available in expr_types for NarrowingGuard lookup.
        self.resolve_expr(&cond.test);
        // If either branch is null/undefined or already Option<T>,
        // the result is Option<T>.
        let cons = self.resolve_expr(&cond.cons);
        let alt = self.resolve_expr(&cond.alt);

        let cons_is_null = crate::pipeline::narrowing_patterns::is_null_or_undefined(&cond.cons);
        let alt_is_null = crate::pipeline::narrowing_patterns::is_null_or_undefined(&cond.alt);
        let cons_is_option = matches!(&cons, ResolvedType::Known(RustType::Option(_)));
        let alt_is_option = matches!(&alt, ResolvedType::Known(RustType::Option(_)));

        let produces_option = cons_is_null || alt_is_null || cons_is_option || alt_is_option;

        if produces_option {
            // Pick the non-null branch's type as the value type. `wrap_optional`
            // is idempotent so an already-Option branch stays single-wrapped.
            let value_type = if cons_is_null { &alt } else { &cons };
            match value_type {
                ResolvedType::Known(ty) => ResolvedType::Known(ty.clone().wrap_optional()),
                ResolvedType::Unknown => ResolvedType::Known(RustType::Any.wrap_optional()),
            }
        } else {
            match (&cons, &alt) {
                // Both known and same type → return that type
                (ResolvedType::Known(c), ResolvedType::Known(a)) if c == a => cons,
                // Both known but different types → generate union
                (ResolvedType::Known(c), ResolvedType::Known(a)) => {
                    let union_types = vec![c.clone(), a.clone()];
                    let enum_name = self.synthetic.register_union(&union_types);
                    ResolvedType::Known(RustType::Named {
                        name: enum_name,
                        type_args: vec![],
                    })
                }
                // One unknown → prefer the known one
                (ResolvedType::Known(_), ResolvedType::Unknown) => cons,
                (ResolvedType::Unknown, ResolvedType::Known(_)) => alt,
                // Both unknown
                _ => ResolvedType::Unknown,
            }
        }
    }
}
