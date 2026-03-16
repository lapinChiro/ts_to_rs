# 小規模ジェネレータ・トランスフォーマ修正

対象 TODO: I-85, I-84, I-87

## 背景・動機

3 件の独立した変換バグがコンパイルエラーを引き起こす。いずれも修正箇所が限定的で、互いに干渉しない。

1. **I-85**: `NaN` → `f64::NAN`、`Infinity` → `f64::INFINITY` への変換が欠落
2. **I-84**: static メソッド呼び出しが `Foo.method()` で生成されるが `Foo::method()` であるべき
3. **I-87**: 関数内のローカル interface/type 宣言が `unsupported statement` エラー

## ゴール

- `NaN` / `Infinity` が正しい Rust 定数に変換される
- static メソッド呼び出しが `::` セパレータで生成される
- 関数内の interface/type 宣言が正しく変換される
- 各修正に対応する E2E テストが PASS する

## スコープ

### 対象

- I-85: 式変換（`src/transformer/expressions/mod.rs`）で `NaN`/`Infinity` 識別子を検出し定数に変換
- I-84: メソッド呼び出し生成（`src/generator/expressions.rs`）で static メソッドを `::` セパレータに
- I-87: 文変換（`src/transformer/statements/mod.rs`）で `convert_stmt_list` 内の `Decl::TsInterface` / `Decl::TsTypeAlias` を処理

### 対象外

- `Number.NaN` / `Number.POSITIVE_INFINITY` 等の Number オブジェクト経由のアクセス
- static プロパティ（メソッドのみ対象）
- 関数内のクラス宣言

## 設計

### 技術的アプローチ

#### I-85: NaN/Infinity

`convert_ident` で `NaN` → `Expr::Ident("f64::NAN")`, `Infinity` → `Expr::Ident("f64::INFINITY")` に変換。

#### I-84: static メソッド `::`

Generator の `generate_method_call` で、object が大文字始まりの識別子（型名）かつメソッドが static（self なし）の場合、`.` を `::` に変更。Transformer 側で static メソッドをマークする方が正確だが、Generator 側の大文字ヒューリスティックで初版は対応。

#### I-87: ローカル型宣言

`convert_stmt_list` で `Stmt::Decl(Decl::TsInterface(...))` / `Stmt::Decl(Decl::TsTypeAlias(...))` をハンドルし、トップレベルと同じ変換関数を呼ぶ。

### 影響範囲

- `src/transformer/expressions/mod.rs` — I-85
- `src/generator/expressions.rs` — I-84
- `src/transformer/statements/mod.rs` — I-87
- `tests/e2e/scripts/` — E2E テスト追加

## 作業ステップ

- [ ] ステップ 1: I-85 — `NaN`/`Infinity` 変換。ユニットテスト + `number_api.ts` E2E 拡充
- [ ] ステップ 2: I-84 — static メソッド `::` 生成。ユニットテスト + `advanced_classes.ts` E2E 拡充
- [ ] ステップ 3: I-87 — ローカル型宣言対応。ユニットテスト + E2E テスト
- [ ] ステップ 4: 全テスト退行チェック

## テスト計画

- **I-85**: `NaN` → `f64::NAN`、`Infinity` → `f64::INFINITY` が E2E で正しい値を出力
- **I-84**: `ClassName.staticMethod()` → `ClassName::static_method()` でコンパイル・実行可能
- **I-87**: 関数内 interface 宣言を含むスクリプトが変換・コンパイル・実行可能

## 完了条件

- [ ] 3 件すべて修正済み、E2E テスト PASS
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] `cargo fmt --all --check` が PASS
- [ ] `cargo test` が全 PASS
