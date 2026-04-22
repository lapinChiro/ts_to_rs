//! Peek through runtime-no-op AST wrappers (I-171 T2).
//!
//! TS `as` / `!` / `<T>` / `as const` / `(...)` wrappers have no runtime
//! effect — they only influence type checking. When matching on the
//! runtime shape of an expression (e.g., `convert_unary_expr` Bang arm,
//! `detect_early_return_narrowing`, `convert_if_stmt`), these wrappers
//! must be transparently unwrapped so syntactic variants do not produce
//! emission gaps.
//!
//! This helper centralises the unwrap logic so every caller agrees on
//! *which* wrappers are runtime-no-op. Before I-171 this knowledge lived
//! in `statements/helpers.rs::unwrap_parens` which only handled `Paren`,
//! leaving `TsAs`/`TsNonNull`/`TsTypeAssertion`/`TsConstAssertion` as
//! silent emission gaps (e.g., `!<x as T>` fall-through to bare
//! `!<non-bool>` Rust compile error).
//!
//! ## Scope
//!
//! Unwraps:
//! - [`ast::Expr::Paren`] — grouping
//! - [`ast::Expr::TsAs`] — `e as T`
//! - [`ast::Expr::TsNonNull`] — `e!`
//! - [`ast::Expr::TsTypeAssertion`] — `<T>e` legacy syntax
//! - [`ast::Expr::TsConstAssertion`] — `e as const`
//!
//! Does NOT unwrap:
//! - `TsSatisfies` — I-115 dependency (convert_expr refuses it)
//! - `TsInstantiation` — generic instantiation expression
//!
//! Recurses through nested wrappers so `(((x as T)!) as const)` returns
//! the innermost `x`.

use swc_ecma_ast as ast;

/// Unwraps runtime-no-op AST wrappers from an expression.
///
/// Returns the innermost expression after peeling off `Paren`, `TsAs`,
/// `TsNonNull`, `TsTypeAssertion`, and `TsConstAssertion` layers.
/// Recurses, so nested wrappers are all removed in a single call.
pub(crate) fn peek_through_type_assertions(expr: &ast::Expr) -> &ast::Expr {
    match expr {
        ast::Expr::Paren(p) => peek_through_type_assertions(&p.expr),
        ast::Expr::TsAs(ta) => peek_through_type_assertions(&ta.expr),
        ast::Expr::TsNonNull(tn) => peek_through_type_assertions(&tn.expr),
        ast::Expr::TsTypeAssertion(tta) => peek_through_type_assertions(&tta.expr),
        ast::Expr::TsConstAssertion(tca) => peek_through_type_assertions(&tca.expr),
        _ => expr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_common::{BytePos, Span, DUMMY_SP};
    use swc_ecma_ast as ast;

    fn ident(name: &str) -> ast::Expr {
        ast::Expr::Ident(ast::Ident {
            span: DUMMY_SP,
            sym: name.into(),
            optional: false,
            ctxt: Default::default(),
        })
    }

    fn span(lo: u32, hi: u32) -> Span {
        Span::new(BytePos(lo), BytePos(hi))
    }

    fn ty_ann_unknown() -> Box<ast::TsType> {
        Box::new(ast::TsType::TsKeywordType(ast::TsKeywordType {
            span: DUMMY_SP,
            kind: ast::TsKeywordTypeKind::TsUnknownKeyword,
        }))
    }

    #[test]
    fn identity_on_bare_ident() {
        let e = ident("x");
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::Ident(id) if id.sym.as_ref() == "x"));
    }

    #[test]
    fn unwraps_paren() {
        let e = ast::Expr::Paren(ast::ParenExpr {
            span: span(1, 4),
            expr: Box::new(ident("x")),
        });
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::Ident(id) if id.sym.as_ref() == "x"));
    }

    #[test]
    fn unwraps_ts_as() {
        let e = ast::Expr::TsAs(ast::TsAsExpr {
            span: span(1, 10),
            expr: Box::new(ident("x")),
            type_ann: ty_ann_unknown(),
        });
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::Ident(id) if id.sym.as_ref() == "x"));
    }

    #[test]
    fn unwraps_ts_non_null() {
        let e = ast::Expr::TsNonNull(ast::TsNonNullExpr {
            span: span(1, 3),
            expr: Box::new(ident("x")),
        });
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::Ident(id) if id.sym.as_ref() == "x"));
    }

    #[test]
    fn unwraps_ts_type_assertion() {
        let e = ast::Expr::TsTypeAssertion(ast::TsTypeAssertion {
            span: span(1, 10),
            expr: Box::new(ident("x")),
            type_ann: ty_ann_unknown(),
        });
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::Ident(id) if id.sym.as_ref() == "x"));
    }

    #[test]
    fn unwraps_ts_const_assertion() {
        let e = ast::Expr::TsConstAssertion(ast::TsConstAssertion {
            span: span(1, 10),
            expr: Box::new(ident("x")),
        });
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::Ident(id) if id.sym.as_ref() == "x"));
    }

    #[test]
    fn recurses_through_nested_wrappers() {
        // (((x as T)!) as const)
        let inner = ident("x");
        let as_t = ast::Expr::TsAs(ast::TsAsExpr {
            span: span(1, 8),
            expr: Box::new(inner),
            type_ann: ty_ann_unknown(),
        });
        let bang = ast::Expr::TsNonNull(ast::TsNonNullExpr {
            span: span(1, 9),
            expr: Box::new(as_t),
        });
        let as_const = ast::Expr::TsConstAssertion(ast::TsConstAssertion {
            span: span(1, 18),
            expr: Box::new(bang),
        });
        let paren = ast::Expr::Paren(ast::ParenExpr {
            span: span(0, 19),
            expr: Box::new(as_const),
        });
        let unwrapped = peek_through_type_assertions(&paren);
        assert!(matches!(unwrapped, ast::Expr::Ident(id) if id.sym.as_ref() == "x"));
    }

    #[test]
    fn does_not_unwrap_ts_satisfies() {
        // TsSatisfies is blocked on I-115; peek-through must stop at the wrapper.
        let e = ast::Expr::TsSatisfies(ast::TsSatisfiesExpr {
            span: span(1, 14),
            expr: Box::new(ident("x")),
            type_ann: ty_ann_unknown(),
        });
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::TsSatisfies(_)));
    }

    #[test]
    fn does_not_unwrap_unary_expr() {
        // `!x` is not a type-only wrapper; peek-through must not strip it.
        let e = ast::Expr::Unary(ast::UnaryExpr {
            span: span(1, 3),
            op: ast::UnaryOp::Bang,
            arg: Box::new(ident("x")),
        });
        let unwrapped = peek_through_type_assertions(&e);
        assert!(matches!(unwrapped, ast::Expr::Unary(_)));
    }

    #[test]
    fn double_neg_after_peek_through_stops_at_unary() {
        // !(x as T) — outer Bang is observable; peek-through only unwraps the arg.
        let as_t = ast::Expr::TsAs(ast::TsAsExpr {
            span: span(1, 9),
            expr: Box::new(ident("x")),
            type_ann: ty_ann_unknown(),
        });
        // peek-through on the *outer* expression returns the Bang as-is.
        let unary = ast::Expr::Unary(ast::UnaryExpr {
            span: span(0, 10),
            op: ast::UnaryOp::Bang,
            arg: Box::new(as_t),
        });
        let unwrapped = peek_through_type_assertions(&unary);
        assert!(matches!(unwrapped, ast::Expr::Unary(_)));
    }
}
