# in 演算子の正確な変換（I-165 + I-107 バッチ）

## 背景・動機

`"key" in obj` が、型不明・非文字列キー・複雑な RHS で `true` にハードコードされる。また動的キーの `in` 演算子が未対応。

## ゴール

`in` 演算子が静的 `true` にフォールバックせず、正しい Rust コードに変換される。

## スコープ

### 対象

- 静的キー + 既知型: 現状のフィールド存在チェックを維持
- 静的キー + 未知型: `true` ではなくエラーにする
- 動的キー: HashMap 的なパターンへの変換（`obj.contains_key(&key)`）

### 対象外

- JavaScript の prototype chain 上のプロパティチェック

## 設計

### 技術的アプローチ

- 既知の struct 型: `has_field("key")` → コンパイル時に `true`/`false` 判定（現状維持、正しい）
- 未知型 / 動的 RHS: **エラーにする**（`true` ハードコード除去）
- HashMap / Record 型: `obj.contains_key("key")` に変換

### 影響範囲

- `src/transformer/expressions/mod.rs` — `convert_in_expr` および関連

## 作業ステップ

- [ ] ステップ1: `true` ハードコードの除去、未知型はエラーに（RED → GREEN）
- [ ] ステップ2: 動的キーの HashMap 変換（RED → GREEN）
- [ ] ステップ3: E2E テスト

## 完了条件

- [ ] `in` 演算子が `true` をハードコードしない
- [ ] 既知 struct 型でのフィールドチェックは正しく維持
