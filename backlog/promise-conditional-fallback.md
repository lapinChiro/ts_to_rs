# Promise の union 内展開と conditional type フォールバック改善

## 背景・動機

2 つの関連する問題:

1. **Promise の展開が不完全**: async 関数の返り値型のみ `Promise<T>` → `T` に展開される。union 内 `Response | Promise<Response>` では `Promise` が空の struct として生成される
2. **conditional type フォールバックが `()`**: 変換失敗時に `RustType::Unit` のプレースホルダーが生成され、コンパイルは通るが意味的に誤り

## ゴール

- union 内の `Promise<T>` が `T` に展開され、重複バリアントが統合される
- conditional type フォールバックが true branch の型を返す（`()` ではなく）

## スコープ

### 対象

- `convert_union_type` で union メンバーの `Promise<T>` を `T` に展開
- `convert_type_alias_items` の conditional type フォールバックを改善

### 対象外

- 完全な conditional type 評価エンジン
- `Promise.all()` / `Promise.race()` のセマンティクス

## 設計

### 技術的アプローチ

**Promise 展開:**

`convert_union_type` 内で、各メンバーを `convert_ts_type` した後、`Promise<T>` → `T` の展開を適用。展開後に重複バリアントがあれば統合（`Response | Promise<Response>` → `Response` 1 バリアントに）。

**conditional type フォールバック:**

現在の `Err(_)` 分岐で `RustType::Unit` を返す代わりに、true branch を `convert_ts_type` で変換して返す。変換不可なら `serde_json::Value` にフォールバック。

### 影響範囲

- `src/transformer/types/mod.rs` — `convert_union_type`、`convert_type_alias_items`
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: `Response | Promise<Response>` → 単一バリアント enum のテスト追加
- [ ] ステップ2（GREEN）: union 内 Promise 展開 + 重複排除
- [ ] ステップ3（RED）: conditional type フォールバックが true branch を返すテスト追加
- [ ] ステップ4（GREEN）: conditional type フォールバック改善
- [ ] ステップ5: Quality check

## テスト計画

- `Response | Promise<Response>` → `Response`（展開 + 統合）
- `string | Promise<string>` → `String`（プリミティブも同様）
- `T extends X ? A : B` フォールバック → `A`（`()` ではなく）
- 回帰: 既存の union、conditional type テスト

## 完了条件

- Promise が union 内で正しく展開される
- conditional type フォールバックが意味のある型を返す
- 全テスト pass、0 errors / 0 warnings
