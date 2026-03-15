# 型変換のエッジケース修正

## 背景・動機

型変換で複数のエッジケースが確認されている:

1. **タプルの optional 要素**: `[string, number?]` → `(String, f64)` になり `Option` にならない
2. **union 内の `never`**: `T | never` は `T` と等価だが enum バリアントが生成される
3. **union 内の `void`**: `string | void` の挙動が未定義
4. **intersection 型注記フォールバック**: `A & B` → `A` に縮退（TODO あり。extra_items 伝搬で解決可能）
5. **conditional type フォールバック**: `()` プレースホルダー（promise-conditional-fallback.md と重複、そちらで対応）

## ゴール

- タプルの optional 要素が `Option<T>` に変換される
- `T | never` が `T` に簡約される
- `string | void` が `Option<String>` に変換される
- intersection の型注記位置でフィールド統合 struct が生成される

## スコープ

### 対象

- タプル変換で optional 要素の検出
- union 変換で `never` の除去、`void` の `null`/`undefined` と同等の扱い
- intersection 型注記位置で `extra_items` を使った struct 生成

### 対象外

- rest elements in tuples `[string, ...number[]]`
- conditional type の評価エンジン（別 PRD）

## 設計

### 技術的アプローチ

**タプル optional 要素**: SWC AST の `TsTupleElement` に `optional` フラグがあるか確認。ある場合は `Option<T>` でラップ。

**never 除去**: `convert_union_type` で `TsKeywordTypeKind::TsNeverKeyword` を null/undefined と同様にフィルタリング。全除去後に非 null 型が 1 つなら直接返す。

**void の union 処理**: `convert_union_type` で `TsVoidKeyword` も null/undefined と同様に扱う（`void` は `undefined` と等価）。

**intersection 型注記位置**: `convert_ts_type` の intersection アームで、全メンバーが `TsTypeLit` の場合にフィールドを統合し、`extra_items` に struct を生成して `Named` 参照を返す。

### 影響範囲

- `src/transformer/types/mod.rs` — タプル変換、union 変換、intersection 変換
- テストファイル

## 作業ステップ

- [ ] ステップ1（RED）: `T | never` → `T` のテスト追加
- [ ] ステップ2（GREEN）: union 内 never 除去
- [ ] ステップ3（RED）: `string | void` → `Option<String>` のテスト追加
- [ ] ステップ4（GREEN）: union 内 void 処理
- [ ] ステップ5（RED）: intersection 型注記位置で struct 生成のテスト追加
- [ ] ステップ6（GREEN）: intersection 型注記位置で extra_items に struct push
- [ ] ステップ7: Quality check

## テスト計画

- `T | never` → `T`（never 除去）
- `string | void` → `Option<String>`（void = null 扱い）
- `x: { a: string } & { b: number }` → struct 生成 + Named 参照
- 回帰: 既存の union、tuple テスト

## 完了条件

- never/void が union 内で正しく処理される
- intersection 型注記位置でコンパイル可能な Rust が生成される
- 全テスト pass、0 errors / 0 warnings
