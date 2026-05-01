//! I-205 inheritance traversal integration test contracts (Spec stage F-deep-deep-4
//! commitment、T13 で fill-in 完了 2026-05-01)。
//!
//! Spec stage で `lookup_method_sigs_in_inheritance_chain` helper の test contracts を
//! `#[test] #[ignore]` stub として author (deep deep review F-deep-deep-4 = "deferred
//! verification = unverified claim" compromise の elimination)。Implementation Stage T13
//! で各 stub を **integration-level transpile probe** として fill-in、`#[ignore]` 解除で
//! green-ify。
//!
//! ## Layered test design
//!
//! 本 file は **integration-level (= TS source → transpile pipeline → Rust source 文字列)**。
//! Registry-level (= `TypeRegistry` 直接 build → `lookup_method_sigs_in_inheritance_chain`
//! 直接呼出) の unit test は `src/transformer/expressions/tests/i_205/read.rs` の
//! `test_b7_traversal_*` 系列で cover (cycle / direct / single-step / multi-step N=2 /
//! N>=3 / partial cycle、計 7 件)。本 file は **TS user code → transpile output** chain
//! 全体で B7 dispatch arm が正しく fire することを verify する end-to-end probe。
//!
//! ## Test contracts (Spec stage commitment)
//!
//! 1. Single-level inheritance: `Sub extends Base` で external `s.x` → Tier 2 honest error
//! 2. Multi-level inheritance: `Sub extends Mid extends Base` で N=2 step traversal → 同
//! 3. Cycle resilience: `A extends B / B extends A` の degenerate input を transpile が
//!    panic / infinite loop なく処理 (method access なしの safety-only verify)
//! 4. B1 vs B7 disambiguation: 直接定義 (B2 dispatch、Ok) vs 継承 (B7 dispatch、Err) の
//!    output 形 byte-level distinction

use ts_to_rs::transpile;

/// (1) Single-level inheritance: `Sub extends Base` で `Base { get x() }`、external `s.x`。
///
/// Expected: `transpile` returns `Err` with kind starting with "inherited accessor access"。
/// = registry helper の N=1 step traversal 経由 B7 dispatch arm (Tier 2 honest error
/// reclassify) が integration pipeline 全体で fire する事を verify。
#[test]
fn test_lookup_method_kind_single_level_inherited_getter() {
    let src = "class Base { _n: number = 42; get x(): number { return this._n; } }\n\
               class Sub extends Base {}\n\
               function main(): void { const s = new Sub(); const v = s.x; console.log(v); }";
    let err = transpile(src).expect_err("single-level inherited accessor access must Err");
    let msg = err.to_string();
    assert!(
        msg.contains("inherited accessor access"),
        "expected 'inherited accessor access' Tier 2 honest error, got: {msg}"
    );
}

/// (2) Multi-level inheritance: `Sub extends Mid extends Base` (N=2 step) で external `s.x`。
///
/// Expected: `transpile` returns `Err` with kind starting with "inherited accessor access"。
/// registry helper の **recursive descent N>=2 step** が integration pipeline 全体で機能
/// する事を verify (registry-level の `test_b7_traversal_multi_step_*` 拡張版、TS source
/// 経由で end-to-end fire 確認)。
#[test]
fn test_lookup_method_kind_multi_level_inherited_getter() {
    let src = "class Base { _n: number = 0; get x(): number { return this._n; } }\n\
               class Mid extends Base {}\n\
               class Sub extends Mid {}\n\
               function main(): void { const s = new Sub(); const v = s.x; console.log(v); }";
    let err = transpile(src).expect_err("multi-level (N=2) inherited accessor access must Err");
    let msg = err.to_string();
    assert!(
        msg.contains("inherited accessor access"),
        "expected 'inherited accessor access' Tier 2 honest error for N=2 step, got: {msg}"
    );
}

/// (3) Circular inheritance resilience: `A extends B / B extends A` (parser accepts、TS
/// spec TS2506 violation) を transpile が **無限ループせず通常 return** する事を verify。
///
/// 本 fixture は class declarations のみ (method access なし) のため、registry の
/// `lookup_method_sigs_in_inheritance_chain` は member access path で invocation されない。
/// 検証対象は **registry construction phase の cycle resilience** + transpile 全体が
/// graceful に終了する事。Member access path の cycle prevention (visited HashSet) は
/// `test_b7_traversal_cycle_does_not_infinite_loop` (registry-level unit test) で検証済。
///
/// **Test contract**: empirically 確認済 (CLI probe `cargo run -- /tmp/probe_circular.ts`)
/// = transpile は `Ok` (空 method の class は extension chain に依らず変換成功)。本
/// assertion は: (a) 無限ループしない、かつ (b) 現状 expected behavior である `Ok` を
/// 返す事を **regression lock-in** として固定 (Err 化の silent regression 防御)。
#[test]
fn test_lookup_method_kind_circular_inheritance_prevention() {
    let src = "class A extends B {}\n\
               class B extends A {}\n";
    let result = transpile(src);
    assert!(
        result.is_ok(),
        "circular extends declarations (no member access) must transpile to Ok (regression \
         lock-in、empty method bodies での registry construction cycle resilience); \
         registry-level cycle prevention with method access は \
         `test_b7_traversal_cycle_does_not_infinite_loop` で unit-tested。\
         got Err: {:?}",
        result.err()
    );
}

/// (4) B1/B2 (direct) vs B7 (inherited) disambiguation: 同 method name `x` を持つ 2
/// シナリオで direct access は B2 getter dispatch (MethodCall、Ok)、inherited access は
/// B7 dispatch (Tier 2 honest error、Err)。**両 path を本 test 内で対称的に probe**
/// (single fixture file 内 2 シナリオ side-by-side、disambiguation の binary 対比を
/// integration level で完結)。
///
/// - Direct (Foo { get x() } + f.x): registry helper `is_inherited = false` → cell 2 (B2
///   getter) dispatch で `f.x()` MethodCall emit、transpile は **Ok** 帰着
/// - Inherited (Bar extends Foo + b.x): registry helper `is_inherited = true` → cell 8 (B7)
///   dispatch で Tier 2 honest error "inherited accessor access" emit、transpile は **Err** 帰着
///
/// 本 test は registry-level の `test_b7_traversal_direct_hit_returns_not_inherited` /
/// `test_b7_traversal_single_step_returns_inherited_flag` の end-to-end pair version、
/// disambiguation 対比を integration test 単独で完結 (test 1 と independent な lock-in)。
#[test]
fn test_lookup_method_kind_direct_vs_inherited_disambiguation() {
    // Direct path: B2 getter dispatch → Ok + MethodCall emit
    let direct_src = "class Foo { _n: number = 1; get x(): number { return this._n; } }\n\
                      function main(): void { const f = new Foo(); const v = f.x; console.log(v); }";
    let direct_rs = transpile(direct_src)
        .expect("direct path (B2 getter dispatch) must succeed = `is_inherited=false` arm");
    assert!(
        direct_rs.contains("f.x()"),
        "direct B2 getter dispatch must emit `f.x()` MethodCall, got Rust:\n{direct_rs}"
    );
    assert!(
        !direct_rs.contains("inherited"),
        "direct path must not produce inherited Tier 2 error in Rust output, got:\n{direct_rs}"
    );

    // Inherited path: B7 inherited dispatch → Err with "inherited accessor access"
    let inherited_src = "class Foo { _n: number = 1; get x(): number { return this._n; } }\n\
                         class Bar extends Foo {}\n\
                         function main(): void { const b = new Bar(); const v = b.x; console.log(v); }";
    let inherited_err = transpile(inherited_src)
        .expect_err("inherited path (B7 dispatch) must Err = `is_inherited=true` arm");
    let msg = inherited_err.to_string();
    assert!(
        msg.contains("inherited accessor access"),
        "inherited B7 dispatch must Err with 'inherited accessor access' Tier 2 honest error, \
         got: {msg}"
    );
}
