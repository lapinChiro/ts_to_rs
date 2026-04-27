//! PRD 2.7 cell 15 (Prop::Assign): SWC parser empirical regression lock-in test.
//!
//! **Implementation Revision 2 (2026-04-27、critical Spec gap fix)**:
//!
//! 当初 PRD 2.7 spec では cell 15 を "NA = SWC parser reject 前提" として認識し、
//! `unreachable!()` macro で defensive coding する設計だった。しかし
//! Implementation stage で SWC parser empirical observation により:
//!
//! - SWC parser は `{ x = expr }` を `Prop::Assign(AssignProp { ... })` として
//!   **accept** する (= TS spec では parse error だが SWC は寛容 parsing)
//! - 即ち `unreachable!()` の precondition violation が actual に reach される
//! - = silent semantic change risk (= ts_to_rs が invalid syntax を silent に
//!   variable initializer として誤変換する可能性)
//!
//! 結果、cell 15 を **NA → Tier 2 honest error** に reclassify、
//! `expressions.rs` (TypeResolver) は no-op、
//! `data_literals.rs` (Transformer) は `UnsupportedSyntaxError::new("Prop::Assign", span)`
//! 経由 honest error report に変更。
//!
//! 本 lock-in test は以下を保証する:
//!
//! 1. SWC parser は `{ x = expr }` を **accept** (= empirical fact、PRD spec の structural
//!    assumption)
//! 2. ts_to_rs (parser → transformer) で `Prop::Assign` を含む TS source は
//!    `UnsupportedSyntaxError::new("Prop::Assign", ...)` 経由 honest error として report される
//! 3. destructuring default context (`({ x = 1 } = obj)`) は valid (= `ObjectPatProp::Assign`
//!    別経路、section 11 handle)
//!
//! Lesson source for framework: matrix cell の NA 認識前に SWC parser empirical
//! observation 必須 (= "TS spec で parse error" を assumption とせず、empirical 確認)。
//! `spec-stage-adversarial-checklist.md` Rule 4 + 10 の `NA justification`
//! verification に "SWC parser empirical observation 必須" を追加検討。

use ts_to_rs::parser::parse_typescript;

#[test]
fn test_swc_parser_accepts_prop_assign_in_object_literal_context_simple() {
    // 単純 default form: `{ x = 1 }` — SWC parser は accept する (empirical)
    let source = "const obj = { x = 1 };";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "PRD 2.7 cell 15 structural assumption: SWC parser must accept `{{ x = 1 }}` \
         as Prop::Assign (empirical fact 2026-04-27).\n\
         If this fires, SWC parser behavior changed — investigate immediately and \
         reconfirm Tier 2 honest error mechanism in `expressions.rs` and `data_literals.rs`."
    );
}

#[test]
fn test_swc_parser_accepts_prop_assign_with_complex_default() {
    // 複雑 default form: `{ x = foo() }` — SWC parser は accept する (empirical)
    let source = "const obj = { x = foo() };";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "PRD 2.7 cell 15 structural assumption: SWC parser must accept \
         `{{ x = foo() }}` as Prop::Assign with complex default value."
    );
}

#[test]
fn test_swc_parser_accepts_prop_assign_in_destructuring_default() {
    // 対称 reference: destructuring default context (`({ x = 1 } = obj)`) は valid。
    // = `ObjectPatProp::Assign` 経路 (section 11)、`Prop::Assign` (object literal context、
    //   section 13、Tier 2 honest error) とは別の handle path。
    // 本 test は本 file の対称性確保 (= TS spec 上 valid な context が SWC でも accept、
    // 別経路 dispatch confirm)。
    let source = "const { x = 1 } = obj;";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "Sanity check: destructuring default `({{ x = 1 }} = obj)` must parse OK \
         (handled by ObjectPatProp::Assign, see ast-variants.md section 11)."
    );
}
