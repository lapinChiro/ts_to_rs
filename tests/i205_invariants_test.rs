//! I-205 Invariants verification tests (Spec stage TS-5 + framework v1.6 audit support、
//! F-deep-deep-2 fix 2026-04-28、T13 で INV-5 fill-in 完了 2026-05-01)。
//!
//! Spec stage で 6 invariants (INV-1〜INV-6) verification method (Rule 8 (8-c)) を
//! concrete test contract として author。**Fill-in 状態 (2026-05-01 post T13)**:
//! - **INV-5 (Visibility consistency)**: T13 (13-c) で fill-in 完了、`#[ignore]` 解除済
//!   (getter + setter symmetric counterpart 2 件 = Layer 3 cross-axis completeness)
//! - **INV-1〜4 / INV-6**: T15 (`/check_job` 4-layer review + 13-rule self-applied verify)
//!   で fill-in 予定、`#[ignore]` 状態維持
//!
//! 各 invariant の verification statement (a/b/c/d) は backlog/I-205-getter-setter-dispatch-framework.md
//! `## Invariants` section 参照。
//!
//! **Lesson source (deep deep review F-deep-deep-2)**: 当初 invariant verification
//! tests を PRD `### Invariant verification tests` section の SPEC TEXT のみで record、
//! actual Rust test code 不在 = "deferred verification = unverified claim" compromise。
//! Spec stage で stub `#[test] #[ignore]` を author し structural commitment 確立、
//! Implementation Stage で fill in する pattern を採用 (= PRD 2.7 cell 15 lock-in test
//! と同 pattern symmetric)。

use ts_to_rs::transpile;

/// INV-1: Receiver type member kind dispatch consistency
///
/// 全 read context (A1) で `obj.x` が emit される際、receiver type の
/// `methods.get(field).kind` 検査結果に基づく dispatch が **全 emit path で一貫**。
///
/// **Verification (Spec stage TS-5 contract)**:
/// 4 receiver shape (Ident / chain / call_result / cond_branch) 各々で同 (B getter, field) →
/// 同 emit IR (`Expr::MethodCall { object, method, args: vec![] }` 構造同) を probe。
#[test]
#[ignore = "I-205 INV-1 verification stub: Implementation Stage T15 で fill in (transpile + IR struct compare across 4 receiver shapes)"]
fn test_invariant_1_dispatch_consistency_across_call_sites() {
    // Implementation Stage T15: 4 receiver shape probes、IR struct token-level compare
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T15 で実装");
}

/// INV-2: External (E1) と internal (E2 this) dispatch path symmetry
///
/// external `obj.x` と internal `this.x` (同 class、同 type) の dispatch logic が
/// **token-level identical**、共通 helper を介して emit。
///
/// **Verification**: TestTransformer (external context) vs TestTransformer (internal class scope)
/// の両 path output IR token-level identical を probe。
#[test]
#[ignore = "I-205 INV-2 verification stub: Implementation Stage T15 で fill in"]
fn test_invariant_2_external_internal_dispatch_symmetry() {
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T15 で実装");
}

/// INV-3: Compound assign desugar の receiver evaluation 1 回
///
/// `obj.x += v` の desugar `obj.set_x(obj.x() + v)` で `obj` は **1 回のみ evaluated**。
/// side-effect-having receiver (e.g., `getInstance().x += v`) では temp binding。
///
/// **Verification**: Side-effect counting test (counter() 呼出回数を count、TS と
/// Rust output で一致 verify)。
#[test]
#[ignore = "I-205 INV-3 verification stub: Implementation Stage T15 で fill in (side-effect counter probe)"]
fn test_invariant_3_compound_assign_receiver_eval_once() {
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T15 で実装");
}

/// INV-4: Method kind tracking propagation chain integrity
///
/// SWC AST `method.kind` (Method/Getter/Setter) が `collect_class_info` →
/// `MethodSignature.kind` → `convert_method_info_to_sig` → `resolve_method_sig` →
/// dispatch logic に **lossless propagate**、Default::default() (= Method) で
/// fallthrough する path 不在。
///
/// **Verification**: Propagation chain test (各 stage で kind が intermediate state に
/// preserve される事を probe)。
#[test]
#[ignore = "I-205 INV-4 verification stub: Implementation Stage T15 で fill in (propagation chain probe across collect/convert/resolve stages)"]
fn test_invariant_4_kind_propagation_lossless() {
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T15 で実装");
}

/// INV-5: Visibility consistency (private accessor 外部 access 不能、T13 (13-c) で
/// fill-in 完了 2026-05-01、Option B 採用)
///
/// `private get x() {}` / `private set x(v) {}` (TS keyword `private`) を持つ class の
/// external `obj.x` access は **必ず Tier 2 honest error reclassify**。
///
/// ## Option A vs Option B reachability audit (T13 (13-b)、Hono empirical 2026-05-01)
///
/// Hono codebase 284 TS files 全件で `private get` / `private set` 0 件 (= reachability
/// = 0)。Option A (= `MethodSignature.accessibility` field 追加 + 50+ site Rule 9 (c)
/// Field-addition symmetric audit + dispatch arm で `UnsupportedSyntaxError::new("access
/// to private accessor", _)` emit) は 0 件 reachability の concern に対し overengineering、
/// recurring problem evidence (I-383 T8' / I-205 T2 で latent kind drop 2 度連続) を考慮し
/// **Option B (status quo)** を採用。
///
/// ## Option B mechanism (Rust visibility = Tier 2 honest error 自動 surface)
///
/// - `resolve_member_visibility(Some(Private), _)` → `Visibility::Private` (= no `pub`
///   modifier on generated method)
/// - External access `obj.x` → cell 2 (B2 getter) dispatch fires regardless of accessibility
///   = `obj.x()` MethodCall emit
/// - Rust compile time に E0624 visibility error = **Tier 2 honest error 自動 surface**
///   (separate consumer module の場合)
///
/// ## Verification (本 integration test の test contract)
///
/// `transpile` 出力の Rust source を probe し、
/// 1. `private get x()` → 生成 method に `pub` modifier **不在** (`fn x(&self)` form)
/// 2. `public get y()` → 生成 method に `pub` modifier **存在** (`pub fn y(&self)` form)
/// 3. External `obj.x` access → `obj.x()` MethodCall emit (cell 2 dispatch fires regardless)
///
/// 本 test は **visibility preservation** を assertion で固定 (silent semantic change から
/// 保護)。Rust compile-time E0624 surface は本 transpile output のみでは観測不能 (separate
/// module compilation context が必要) のため、**生成側 visibility marker preservation** を
/// proxy として検証する。
#[test]
fn test_invariant_5_private_accessor_external_access_tier2() {
    let src = "export class Foo { \
               private _n: number = 0; \
               private get x(): number { return this._n; } \
               public get y(): number { return this._n; } }\n\
               export function main(): void { \
               const f = new Foo(); const v = f.x; const w = f.y; console.log(v, w); }";
    let rust = transpile(src).expect("Option B: external access on private accessor must succeed at transpile (Tier 2 surfaces at Rust compile time via E0624, not at transpile time)");

    // (1) private getter: no `pub` modifier
    assert!(
        rust.contains("fn x(&self)"),
        "private getter `x` must be emitted (with or without pub), got Rust:\n{rust}"
    );
    assert!(
        !rust.contains("pub fn x(&self)"),
        "INV-5 violation: private getter `x` must NOT have `pub` modifier (Visibility::Private \
         preservation = Rust E0624 honest error surface mechanism), got Rust:\n{rust}"
    );

    // (2) public getter: `pub` modifier present
    assert!(
        rust.contains("pub fn y(&self)"),
        "public getter `y` must have `pub` modifier (visibility distinguishes private vs \
         public), got Rust:\n{rust}"
    );

    // (3) External access dispatches to MethodCall regardless of accessibility
    assert!(
        rust.contains("f.x()"),
        "external access on private getter must emit cell 2 dispatch (`f.x()` MethodCall) \
         regardless of accessibility (visibility is preserved on definition side, dispatch \
         is uniform), got Rust:\n{rust}"
    );
    assert!(
        rust.contains("f.y()"),
        "external access on public getter must emit cell 2 dispatch (`f.y()` MethodCall), \
         got Rust:\n{rust}"
    );
}

/// INV-5 symmetric probe: private setter (T13 (13-c) Layer 3 cross-axis completeness、
/// getter / setter は Decision Table A / B の symmetric counterpart pair、Option B
/// mechanism は両 kind に等しく適用される invariant)。
///
/// `private set x(v)` → `fn set_x(&mut self, v: T)` (no `pub`)、external `obj.x = v` →
/// `obj.set_x(v)` MethodCall (cell 14 dispatch fires regardless of accessibility)。
/// Rust E0624 visibility error は separate consumer module 経由で surface (本 transpile
/// output のみでは観測不能、`set_x` の visibility marker preservation を proxy verify)。
#[test]
fn test_invariant_5_private_setter_external_write_tier2() {
    let src = "export class Foo { \
               private _n: number = 0; \
               private set x(v: number) { this._n = v; } \
               public set y(v: number) { this._n = v; } }\n\
               export function main(): void { \
               const f = new Foo(); f.x = 5; f.y = 6; }";
    let rust = transpile(src).expect("Option B: external write to private setter must succeed at transpile (Tier 2 surfaces at Rust compile time via E0624)");

    // (1) private setter: no `pub` modifier on `set_x`
    assert!(
        rust.contains("fn set_x(&mut self"),
        "private setter `set_x` must be emitted, got Rust:\n{rust}"
    );
    assert!(
        !rust.contains("pub fn set_x"),
        "INV-5 violation: private setter `set_x` must NOT have `pub` modifier (Visibility::\
         Private preservation = Rust E0624 honest error surface mechanism), got Rust:\n{rust}"
    );

    // (2) public setter: `pub` modifier present
    assert!(
        rust.contains("pub fn set_y(&mut self"),
        "public setter `set_y` must have `pub` modifier (visibility distinguishes private \
         vs public), got Rust:\n{rust}"
    );

    // (3) External write dispatches to set_x / set_y MethodCall regardless of accessibility
    assert!(
        rust.contains("f.set_x(5.0)"),
        "external write on private setter must emit cell 14 dispatch (`f.set_x(5.0)` \
         MethodCall) regardless of accessibility, got Rust:\n{rust}"
    );
    assert!(
        rust.contains("f.set_y(6.0)"),
        "external write on public setter must emit cell 14 dispatch (`f.set_y(6.0)` \
         MethodCall), got Rust:\n{rust}"
    );
}

/// INV-6: Scope boundary preservation (`this.x` ↔ external `obj.x` semantic distinction)
///
/// `this.x` の dispatch logic は class scope state (`enclosing_class_name`) に依存し、
/// `obj.x` (E1 external、`obj` が偶然 self を refer しても) と **同一 dispatch logic**
/// (P1 TC39 faithful) ではあるが、**dispatch trigger source は明示区別**。
///
/// **Verification**: scope state lookup (this.x) と receiver expr type lookup (obj.x) の
/// trigger source 区別 verify、両 path output IR is token-level identical when class is
/// same type。
#[test]
#[ignore = "I-205 INV-6 verification stub: Implementation Stage T15 で fill in (scope state vs receiver type lookup path separation probe)"]
fn test_invariant_6_scope_boundary_preservation() {
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T15 で実装");
}
