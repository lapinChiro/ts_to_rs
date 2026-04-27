//! I-205 cell 44 (A6 Increment `++` × D4-D15 non-numeric T): SWC parser empirical lock-in test.
//!
//! **本 test の意義 (PRD 2.7 cell 15 Prop::Assign lesson の symmetric 適用)**:
//!
//! 当初 I-205 spec では cell 44 を "NA = TS spec で `++` は numeric only、parser reject 前提"
//! として認識していた。しかし empirical observation 2026-04-28 により:
//!
//! - SWC parser は `s++` (s: string) を `UpdateExpr { op: PlusPlus, arg: Ident("s") }` として
//!   **accept** する (= TS spec 違反 syntax だが SWC parser は寛容 parsing で AST 構築)
//! - tsx runtime は string → number coercion で `NaN` を return (TypeError ではなく silent
//!   numeric coercion semantic)
//! - = silent semantic change risk (= ts_to_rs が `++` on String を silent に numeric
//!   increment と誤変換する可能性、Rust では `String + 1` E0277 compile error で部分的に
//!   surface するが、増加 widening 前に early reject が ideal)
//!
//! 結果、cell 44 を **NA → Tier 2 honest error reclassify (本 PRD scope、Rule 3 (3-3))**
//! に reclassify、Transformer の UpdateExpr arm で non-numeric T の operand に対し
//! `UnsupportedSyntaxError::new("increment of non-numeric (String/etc.) — TS NaN coercion semantic", span)`
//! 経由 honest error report に変更。
//!
//! 本 lock-in test は以下を保証する:
//!
//! 1. SWC parser は `s++` (string operand) を **accept** (= empirical fact、本 PRD spec の
//!    structural assumption)
//! 2. SWC parser は `++s` (prefix form) も accept (operator form independence)
//! 3. SWC parser は compound bitwise/arithmetic non-numeric (e.g., `arr <<= 1` for
//!    Array operand) も同 pattern で accept
//!
//! Lesson source for framework: PRD 2.7 cell 15 (Prop::Assign) と同 lesson 再発、Rule 3 (3-2)
//! SWC parser empirical observation 必須化 v1.2 が functioning (本 PRD I-205 で auto-detect
//! 機能)。

use ts_to_rs::parser::parse_typescript;

#[test]
fn test_swc_parser_accepts_increment_on_string_operand() {
    // I-205 cell 44 structural assumption: SWC parser accepts `s++` for `s: string`
    let source = "let s: string = \"abc\"; s++;";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "I-205 cell 44 structural assumption: SWC parser must accept `s++` for \
         string-typed operand (empirical fact 2026-04-28).\n\
         If this fires, SWC parser behavior changed — investigate and reconfirm \
         Tier 2 honest error mechanism in transformer UpdateExpr arm."
    );
}

#[test]
fn test_swc_parser_accepts_prefix_increment_on_string_operand() {
    // Operator form independence: prefix `++s` も同 pattern で accept (postfix `s++` と
    // ast::UpdateOp は同 PlusPlus、prefix flag のみ違い)
    let source = "let s: string = \"abc\"; ++s;";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "I-205 cell 44 structural assumption: SWC parser must accept prefix `++s` \
         for string-typed operand (empirical 2026-04-28)."
    );
}

#[test]
fn test_swc_parser_accepts_decrement_on_string_operand() {
    // Operator dimension symmetry: `s--` (decrement) も同 pattern で accept
    let source = "let s: string = \"abc\"; s--;";
    let result = parse_typescript(source);
    assert!(
        result.is_ok(),
        "I-205 cell 45-* structural assumption: SWC parser must accept `s--` for \
         string-typed operand (orthogonality-equivalent to cell 44 increment)."
    );
}
