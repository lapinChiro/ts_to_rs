//! I-177-B canonical leaf type resolution helpers.
//!
//! PRD I-177-B (Plan η Step 2): `narrowed_type` 優先 → `expr_type` fallback の
//! 「leaf 位置における型 lookup」 knowledge を `FileTypeResolution` 1 箇所に集約する
//! canonical primitive (`resolve_var_type` / `resolve_expr_type`)。 Production code 内
//! 3 site (`Transformer::get_type_for_var` / `Transformer::get_expr_type` /
//! `transformer::return_wrap::collect_expr_leaf_types`) を本 helper 経由に統一し、
//! DRY violation を構造的に解消する。
//!
//! 以下の 5 test は matrix cells #1〜#7, #11 を direct 検証する
//! (PRD Problem Space matrix 参照)。

use super::super::*;

fn dummy_swc_span(lo: u32, hi: u32) -> swc_common::Span {
    swc_common::Span::new(swc_common::BytePos(lo), swc_common::BytePos(hi))
}

#[test]
fn test_resolve_var_type_returns_narrowed_when_active() {
    // Matrix cell #2: narrow active かつ expr_type も present → narrowed が優先される
    // (これが本 PRD 修正対象 cell #9 の正解 invariant の primitive lock-in)。
    use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
    let mut resolution = FileTypeResolution::empty();
    resolution.narrow_events.push(NarrowEvent::Narrow {
        var_name: "x".to_string(),
        scope_start: 10,
        scope_end: 50,
        narrowed_type: RustType::F64,
        trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard("number".to_string())),
    });
    let span = Span { lo: 25, hi: 26 };
    resolution.expr_types.insert(
        span,
        ResolvedType::Known(RustType::Named {
            name: "F64OrString".to_string(),
            type_args: vec![],
        }),
    );

    let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
    assert!(matches!(resolved, Some(RustType::F64)));
}

#[test]
fn test_resolve_var_type_returns_declared_when_outside_scope() {
    // Matrix cell #1: narrow none + expr_type present → expr_type を返す。
    let mut resolution = FileTypeResolution::empty();
    let span = Span { lo: 25, hi: 26 };
    resolution
        .expr_types
        .insert(span, ResolvedType::Known(RustType::F64));

    let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
    assert!(matches!(resolved, Some(RustType::F64)));
}

#[test]
fn test_resolve_var_type_returns_none_when_neither_present() {
    // narrow none + expr_type Unknown → None。
    let resolution = FileTypeResolution::empty();
    let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
    assert!(resolved.is_none());
}

#[test]
fn test_resolve_var_type_returns_declared_when_suppressed() {
    // Matrix cell #3: EarlyReturnComplement narrow + closure-reassign → suppression
    // で narrowed_type は None、expr_type fallback で declared を返す。
    // (I-177-D suppression dispatch が canonical primitive 経由でも正しく effect する
    // ことの lock-in。)
    use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
    let mut resolution = FileTypeResolution::empty();
    resolution.narrow_events.push(NarrowEvent::Narrow {
        var_name: "x".to_string(),
        scope_start: 10,
        scope_end: 50,
        narrowed_type: RustType::F64,
        trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
    });
    resolution.narrow_events.push(NarrowEvent::ClosureCapture {
        var_name: "x".to_string(),
        enclosing_fn_body: Span { lo: 0, hi: 100 },
    });
    let span = Span { lo: 25, hi: 26 };
    resolution.expr_types.insert(
        span,
        ResolvedType::Known(RustType::Option(Box::new(RustType::F64))),
    );

    let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
    assert!(matches!(resolved, Some(RustType::Option(_))));
}

#[test]
fn test_resolve_expr_type_delegates_to_var_type_for_ident() {
    // Matrix cell #5: Ident expr で narrow active → narrowed を返す
    // (resolve_expr_type が Ident path で resolve_var_type に delegate する invariant)。
    use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
    use swc_ecma_ast as ast;

    let mut resolution = FileTypeResolution::empty();
    resolution.narrow_events.push(NarrowEvent::Narrow {
        var_name: "x".to_string(),
        scope_start: 10,
        scope_end: 50,
        narrowed_type: RustType::F64,
        trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard("number".to_string())),
    });
    let span = Span { lo: 25, hi: 26 };
    resolution.expr_types.insert(
        span,
        ResolvedType::Known(RustType::Named {
            name: "F64OrString".to_string(),
            type_args: vec![],
        }),
    );

    let ident_expr = ast::Expr::Ident(ast::Ident {
        span: dummy_swc_span(25, 26),
        sym: "x".into(),
        optional: false,
        ctxt: Default::default(),
    });

    let resolved = resolution.resolve_expr_type(&ident_expr);
    assert!(matches!(resolved, Some(RustType::F64)));
}

#[test]
fn test_resolve_expr_type_uses_expr_type_for_non_ident() {
    // Matrix cell #7 / #11: 非 Ident expr (NumLit) は narrow に subject されない
    // → expr_type のみを参照。
    use swc_ecma_ast as ast;

    let mut resolution = FileTypeResolution::empty();
    let span = Span { lo: 25, hi: 30 };
    resolution
        .expr_types
        .insert(span, ResolvedType::Known(RustType::F64));

    let lit_expr = ast::Expr::Lit(ast::Lit::Num(ast::Number {
        span: dummy_swc_span(25, 30),
        value: 42.0,
        raw: None,
    }));

    let resolved = resolution.resolve_expr_type(&lit_expr);
    assert!(matches!(resolved, Some(RustType::F64)));
}
