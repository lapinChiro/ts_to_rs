//! PRD 2.7 (I-198 + I-199 + I-200 cohesive batch) Transformer layer behavioral
//! lock-in tests.
//!
//! Spec stage Problem Space matrix の各 cell に対し、Transformer layer の actual
//! behavior (= UnsupportedSyntaxError honest error report / format 統一 / NA cell の
//! defensive coding) を direct verify する。PRD T12 (1) "全 ✓ cell の regression
//! lock-in" + (3) Transformer side honest error format 統一 lock-in を本 file で完全充足。
//!
//! Coverage:
//! - **cell 7** (`ClassMember::AutoAccessor`): Tier 2 honest error reported via
//!   `UnsupportedSyntaxError::new("AutoAccessor", aa.span)` (`classes/mod.rs:165-171`
//!   既実装、Implementation Revision 2 で wording 反映)
//! - **cell 12** (`Prop::Method`): `UnsupportedSyntaxError::new("Prop::Method", ...)`
//! - **cell 13** (`Prop::Getter`): `UnsupportedSyntaxError::new("Prop::Getter", ...)`
//! - **cell 14** (`Prop::Setter`): `UnsupportedSyntaxError::new("Prop::Setter", ...)`
//! - **cell 15** (`Prop::Assign`、Implementation Revision 2):
//!   `UnsupportedSyntaxError::new("Prop::Assign", ...)` (= NA → Tier 2 honest error
//!   reclassify、SWC parser empirical で accept 確認後の structural fix)
//! - **cell 17** (`_` arm format 統一 + line/col message): `UnsupportedSyntaxError`
//!   が `resolve_unsupported()` 経由で line/col 含む user-facing transparent message
//!   に変換される end-to-end verify

use super::*;

// -----------------------------------------------------------------------------
// cell 7: ClassMember::AutoAccessor — Tier 2 honest error
// -----------------------------------------------------------------------------

#[test]
fn test_auto_accessor_emits_unsupported_syntax_error() {
    // PRD 2.7 cell 7 lock-in: AutoAccessor (`accessor x: T = init`) は Transformer で
    // `UnsupportedSyntaxError::new("AutoAccessor", aa.span)` を return
    // (`src/transformer/classes/mod.rs:165-171` 既実装、Q1 (b) 確定)。
    // 完全 Tier 1 化は I-201-A (decorator なし subset) + I-201-B (decorator framework)。
    let src = r#"
        class Container {
            accessor x: string = "default";
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.iter().any(|u| u.kind == "AutoAccessor"),
        "PRD 2.7 cell 7: AutoAccessor must surface as `UnsupportedSyntaxError` with \
         kind=\"AutoAccessor\"、got: {unsupported:?}"
    );
}

// -----------------------------------------------------------------------------
// cell 12-14: Prop::Method/Getter/Setter — Tier 2 honest error (T10 改修)
// -----------------------------------------------------------------------------

#[test]
fn test_prop_method_emits_unsupported_syntax_error() {
    // PRD 2.7 cell 12 lock-in: Prop::Method は Transformer で
    // `UnsupportedSyntaxError::new("Prop::Method", ...)` 経由 honest error。
    // 完全 Tier 1 化は I-202 (object literal Tier 1 化) で別 PRD。
    //
    // Note: `convert_object_lit` の event-loop は struct_name (Named expected) 取得 +
    // discriminated union check の **後** に dispatch、interface 型 (= TypeRegistry に
    // Named struct 登録) で expected を Named 化、object literal に method を含めると
    // event-loop で Prop::Method dispatch → UnsupportedSyntaxError return。
    let src = r#"
        interface Container {
            label: string;
        }
        function build(): Container {
            return {
                label: "hi",
                method() {
                    return 42;
                }
            };
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.iter().any(|u| u.kind == "Prop::Method"),
        "PRD 2.7 cell 12: Prop::Method must surface as `UnsupportedSyntaxError` with \
         kind=\"Prop::Method\"、got: {unsupported:?}"
    );
}

#[test]
fn test_prop_getter_emits_unsupported_syntax_error() {
    // PRD 2.7 cell 13 lock-in: Prop::Getter Tier 2 honest error。
    let src = r#"
        interface Container {
            label: string;
        }
        function build(): Container {
            return {
                label: "hi",
                get name() {
                    return "hello";
                }
            };
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.iter().any(|u| u.kind == "Prop::Getter"),
        "PRD 2.7 cell 13: Prop::Getter must surface as `UnsupportedSyntaxError` with \
         kind=\"Prop::Getter\"、got: {unsupported:?}"
    );
}

#[test]
fn test_prop_setter_emits_unsupported_syntax_error() {
    // PRD 2.7 cell 14 lock-in: Prop::Setter Tier 2 honest error。
    let src = r#"
        interface Container {
            label: string;
        }
        function build(): Container {
            return {
                label: "hi",
                set name(v: string) {
                    console.log(v);
                }
            };
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.iter().any(|u| u.kind == "Prop::Setter"),
        "PRD 2.7 cell 14: Prop::Setter must surface as `UnsupportedSyntaxError` with \
         kind=\"Prop::Setter\"、got: {unsupported:?}"
    );
}

// -----------------------------------------------------------------------------
// cell 15: Prop::Assign — Tier 2 honest error (Implementation Revision 2 fix)
// -----------------------------------------------------------------------------

#[test]
fn test_prop_assign_emits_unsupported_syntax_error() {
    // PRD 2.7 cell 15 (Implementation Revision 2、critical Spec gap fix) lock-in:
    // 当初 NA + `unreachable!()` 設計を SWC parser empirical observation で覆し、
    // Tier 2 honest error reclassify。`UnsupportedSyntaxError::new("Prop::Assign", ...)`
    // 経由で明確に reject (silent semantic change risk 排除)。
    let src = r#"
        interface Container {
            x: number;
        }
        function build(): Container {
            return { x = 1 };
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.iter().any(|u| u.kind == "Prop::Assign"),
        "PRD 2.7 cell 15 (Implementation Revision 2): Prop::Assign must surface as \
         `UnsupportedSyntaxError` with kind=\"Prop::Assign\" (= NA → Tier 2 reclassify、\
         SWC parser empirical で accept 確認後の structural fix)、got: {unsupported:?}"
    );
}

#[test]
fn test_prop_assign_in_discriminated_union_context_emits_unsupported() {
    // cell 15 corollary: `convert_discriminated_union_object_lit` site (= 別 caller)
    // でも Prop::Assign を honest error 化 (= file 単位 coherent application、
    // T10 file-scope wildcard 0 達成 lock-in)。
    let src = r#"
        type Tagged = { kind: "a"; x: number } | { kind: "b"; y: string };
        function build(): Tagged {
            return { kind: "a", x = 1 };
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.iter().any(|u| u.kind == "Prop::Assign"),
        "PRD 2.7 cell 15 (Implementation Revision 2): Prop::Assign in discriminated \
         union variant context も honest error。got: {unsupported:?}"
    );
}

// -----------------------------------------------------------------------------
// cell 17: format 統一 + line/col message verify (`_` arm 削除 + UnsupportedSyntaxError 経由)
// -----------------------------------------------------------------------------

#[test]
fn test_unsupported_prop_resolves_to_line_col_message() {
    // PRD 2.7 cell 17 lock-in: data_literals.rs の `_ => Err(anyhow!(...))` 既存 wildcard
    // arm を削除し、`UnsupportedSyntaxError::new("Prop::*", span)` 経由に format 統一
    // (broken window 解消)。
    //
    // 本 test は end-to-end で `resolve_unsupported()` 経由 line/col 含む user-facing
    // transparent message を verify (= byte_pos → line:col 解決 + format 統一)。
    let src = r#"
interface Container {
    label: string;
}
function build(): Container {
    return { label: "hi", foo() { return 1; } };
}
"#;
    let f = TctxFixture::from_source(src);
    let (_items, raw_unsupported) = f.transform_collecting(src);
    assert!(
        !raw_unsupported.is_empty(),
        "PRD 2.7 cell 17: Prop::Method should surface as UnsupportedSyntaxError"
    );
    // raw → resolved (line/col 含む)
    let resolved: Vec<_> = raw_unsupported
        .into_iter()
        .map(|raw| crate::resolve_unsupported(src, raw))
        .collect();
    assert!(
        resolved.iter().any(|u| u.kind == "Prop::Method"),
        "PRD 2.7 cell 17: resolved kind must preserve \"Prop::Method\"、got: {resolved:?}"
    );
    let line_col_format_present = resolved.iter().any(|u| {
        // location field format は "line:col" (= `src/lib.rs:101 format!("{line}:{col}")`)
        u.location.contains(':')
            && u.location
                .split(':')
                .all(|p| p.parse::<usize>().is_ok() && !p.is_empty())
    });
    assert!(
        line_col_format_present,
        "PRD 2.7 cell 17: resolve_unsupported() must produce line:col format location、\
         got: {:?}",
        resolved.iter().map(|u| &u.location).collect::<Vec<_>>()
    );
}

// -----------------------------------------------------------------------------
// cell 10-11: Prop::KeyValue / Shorthand regression lock-in
// -----------------------------------------------------------------------------

#[test]
fn test_prop_keyvalue_regression_emits_struct_init() {
    // PRD 2.7 cell 10 regression lock-in: Prop::KeyValue は existing Tier 1、
    // T10 改修 (`_` arm 削除 + Method/Getter/Setter/Assign explicit enumerate) 後も
    // KeyValue handle path が cohesion 維持していること verify。
    let src = r#"
        type Point = { x: number; y: number };
        function make(): Point {
            return { x: 1, y: 2 };
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.is_empty(),
        "PRD 2.7 cell 10: Prop::KeyValue regression — must produce no UnsupportedSyntaxError、\
         got: {unsupported:?}"
    );
}

#[test]
fn test_prop_shorthand_regression_emits_struct_init() {
    // PRD 2.7 cell 11 regression lock-in: Prop::Shorthand existing Tier 1。
    let src = r#"
        type Point = { x: number; y: number };
        function make(x: number, y: number): Point {
            return { x, y };
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported.is_empty(),
        "PRD 2.7 cell 11: Prop::Shorthand regression — must produce no UnsupportedSyntaxError、\
         got: {unsupported:?}"
    );
}
