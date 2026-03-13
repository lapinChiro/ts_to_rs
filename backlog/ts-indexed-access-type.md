# TsIndexedAccessType（`E['Bindings']`）の変換

## 背景・動機

Hono の `context.ts` では `E['Bindings']` のように型パラメータのプロパティにアクセスする構文が使われている。TypeScript の Indexed Access Type は `T[K]` の形式で型のメンバーにアクセスする。現在この構文は未対応のため変換エラーになる。

## ゴール

`E['Bindings']` が `E::Bindings`（関連型アクセス）に変換される。文字列リテラルキーを識別子に変換し、Rust の関連型構文として出力する。

## スコープ

### 対象

- `TsIndexedAccessType` で、インデックスが文字列リテラル（`T['Key']`）のケースを `T::Key` に変換
- 型注記（パラメータ型、戻り値型、フィールド型）の全位置で動作する

### 対象外

- インデックスが数値リテラルのケース（`T[0]`）
- インデックスが型パラメータのケース（`T[K]`）
- ネストしたインデックスアクセス（`T['A']['B']`）

## 設計

### 技術的アプローチ

`convert_ts_type` に `TsIndexedAccessType` の match arm を追加する:

1. `obj_type` を変換して基底型名を取得
2. `index_type` が `TsLitType(Str)` の場合、文字列リテラルの値を識別子として抽出
3. `RustType::Named { name: "E::Bindings", generics: vec![] }` を生成

### 影響範囲

- `src/transformer/types.rs` — `convert_ts_type` に match arm を追加

## 作業ステップ

- [ ] ステップ1（RED）: `E['Bindings']` の型変換ユニットテストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: `convert_ts_type` に `TsIndexedAccessType` の処理を追加
- [ ] ステップ3: E2E テスト（fixture）を追加
- [ ] ステップ4（REFACTOR）: コードの整理

## テスト計画

- 正常系: `E['Bindings']` → `E::Bindings`
- 正常系: 関数パラメータ型での使用 `function foo(b: E['Bindings']): void`
- 正常系: 戻り値型での使用 `function foo(): E['Bindings']`
- 異常系: インデックスが文字列リテラル以外の場合にエラーを返す

## 完了条件

- `T['Key']` 形式の indexed access type が `T::Key` に変換される
- 全位置（パラメータ型、戻り値型、フィールド型）で動作する
- `cargo fmt --all --check` / `cargo clippy` / `cargo test` が 0 エラー・0 警告
