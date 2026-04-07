//! I-379 integration test: TS `null` / `undefined` および各 auto-fill 経路で
//! 生成される値式の `None` は構造化された `Expr::BuiltinVariantValue(None)` に
//! 変換され、generator では bare `None` として render される。
//!
//! 旧 IR `Expr::Ident("None")` は walker が `is_external` 事後フィルタで除外する
//! 構造的脆弱性を持っていた。本テスト群は I-379 の構造化を lock-in し、
//! 後続 PRD で誤って文字列 encoding に戻ることを防ぐ。

use ts_to_rs::pipeline::transpile_single;

#[test]
fn ts_null_literal_in_option_context_renders_as_bare_none() {
    // `?? null` は nullish coalescing で `unwrap_or(None)` に lower される。
    // I-379 の `is_copy_literal: true` により `unwrap_or_else(|| None)` ではなく
    // eager な `unwrap_or(None)` を選択する (idiomatic 改善)。
    let ts_source = r#"
function f(c: { req: { header: (n: string) => string | undefined } }): string | null {
    return c.req.header("x") ?? null;
}
"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");
    assert!(
        rust.contains("unwrap_or(None)"),
        "expected `unwrap_or(None)` (eager), got:\n{rust}"
    );
    assert!(
        !rust.contains("unwrap_or_else(|| None)"),
        "must NOT emit lazy `unwrap_or_else(|| None)`, got:\n{rust}"
    );
}

#[test]
fn ts_undefined_identifier_in_option_context_renders_as_bare_none() {
    // `const x: number | undefined = undefined;` の `undefined` 識別子は
    // `Expr::BuiltinVariantValue(None)` を生成する。Some(None) wrapping は禁止。
    let ts_source = r#"
function f() {
    const x: number | undefined = undefined;
    return x;
}
"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");
    assert!(
        rust.contains("= None"),
        "expected `= None` assignment, got:\n{rust}"
    );
    assert!(
        !rust.contains("Some(None)"),
        "must NOT wrap `None` in `Some(_)`, got:\n{rust}"
    );
}

#[test]
fn ts_optional_field_omitted_auto_fills_none() {
    // discriminated union object literal で省略された Option フィールドは
    // `Expr::BuiltinVariantValue(None)` で auto-fill される。
    let ts_source = r#"
interface Item { name: string; value?: number; }
function make(): Item { return { name: "test" }; }
"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");
    // 構造的初期化に `value: None` が現れる。
    assert!(
        rust.contains("value: None"),
        "expected `value: None` auto-fill, got:\n{rust}"
    );
}

#[test]
fn ts_undefined_argument_in_call_position_renders_as_bare_none() {
    // call site で `undefined` を引数として明示的に渡すケース。
    // `mod.rs:95` の `undefined` 識別子ハンドラが `Expr::BuiltinVariantValue(None)`
    // を生成し、引数位置に bare `None` として render される。
    let ts_source = r#"
class C {
    f(x: number, y: number | undefined): number { return x; }
    caller(): number { return this.f(1, undefined); }
}
"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");
    assert!(
        rust.contains("self.f(1.0, None)"),
        "expected `self.f(1.0, None)` with bare None argument, got:\n{rust}"
    );
}
