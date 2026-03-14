# `object` keyword の型変換対応

## 背景・動機

Hono の `types.ts` で `type Bindings = object` / `type Variables = object` が使われている（2件）。`convert_ts_type` に `TsObjectKeyword` の処理がなく、catchall エラーに落ちる。

`object` keyword は「プリミティブ以外の任意の値」を表す汎用的な型であり、Hono のようなフレームワークで頻出する。

## ゴール

`object` keyword type が `serde_json::Value` に変換される。

## スコープ

### 対象

- `convert_ts_type` に `TsKeywordTypeKind::TsObjectKeyword` のアームを 1 つ追加
- `RustType::Named { name: "serde_json::Value", type_args: vec![] }` を返す

### 対象外

- `Object` 型の詳細な構造推論
- `object` 型を使った intersection 型（`object & { key: string }` 等）の特別処理

## 設計

### 技術的アプローチ

- `src/transformer/types/mod.rs` の `convert_ts_type` 関数に `TsKeywordTypeKind::TsObjectKeyword` のアームを追加する
- `serde_json` は既に `Cargo.toml` の依存に含まれているため、追加の依存設定は不要
- 他のキーワード型（`string` → `String`, `number` → `f64` 等）と同じパターンで実装する

### 影響範囲

- `src/transformer/types/mod.rs` — `convert_ts_type` のマッチアーム追加
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: `type X = object` の変換テストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: `TsObjectKeyword` アーム追加
- [ ] ステップ3: Quality check

## テスト計画

- `type X = object` → `type X = serde_json::Value`
- プロパティ型 `x: object` → `x: serde_json::Value`
- 回帰: 既存のキーワード型（`string`, `number`, `boolean` 等）が変更なく動作すること

## 完了条件

- `object` keyword が `serde_json::Value` に変換される
- 既存のテストがすべてパスする
- `cargo test`, `cargo clippy`, `cargo fmt --check` が 0 エラー・0 警告
