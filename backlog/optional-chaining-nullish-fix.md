# optional chaining / nullish coalescing の非 Option 型対応

## 背景・動機

`x?.y` → `x.as_ref().map(|_v| _v.y)` および `x ?? y` → `x.unwrap_or_else(|| y)` は、`x` が `Option` 型である前提で変換している。非 null 型の変数に対して使用するとコンパイル不可。また、ネストした optional chaining `x?.y?.z` は `Option<Option<T>>` を生成する。

関連コード: `src/transformer/expressions/mod.rs` の optional chaining 処理（182-251行目付近）、nullish coalescing 処理（81-92行目付近）。

## ゴール

- 非 Option 型に対する `x?.y` が、`x.y` として通常のフィールドアクセスに変換される（TS の optional chaining は非 null 型に対しても有効であり、単にアクセスするだけ）
- 非 Option 型に対する `x ?? y` が、`x` をそのまま返す（非 null なら fallback 不要）
- `Option` 型に対する `x?.y` は従来通り `.map()` パターンを使用
- ネスト `x?.y?.z` が `Option<T>` を返す（`Option<Option<T>>` ではなく）

## スコープ

### 対象

- `convert_opt_chain_expr` で受信側の型を判定し、Option/非 Option で分岐
- `convert_nullish_coalescing` で同様の分岐
- ネスト optional chaining の `.and_then()` への変換

### 対象外

- 型推論インフラの構築（既存の TypeRegistry と型注記情報で判定）
- メソッドチェーン `x?.method()?.method2()` の完全対応

## 設計

### 技術的アプローチ

現在は `expected` 型情報なしに一律 `.as_ref().map()` を生成している。修正方針:

1. optional chaining の対象式の型を推論する（TypeRegistry + 型注記から）
2. 型が `Option` の場合 → 既存の `.as_ref().map()` パターン（ただし `.and_then()` でフラット化）
3. 型が非 Option / 不明の場合 → 通常のフィールドアクセスに変換（TS では非 null 型への `?.` は単なるアクセスと同等）
4. nullish coalescing も同様：`Option` → `.unwrap_or_else()`、非 Option → `x`

### 影響範囲

- `src/transformer/expressions/mod.rs` — optional chaining、nullish coalescing の変換ロジック
- テストファイル

## 作業ステップ

- [ ] ステップ1（RED）: 非 Option 型への `x?.y` が通常アクセスになるテスト追加
- [ ] ステップ2（GREEN）: optional chaining で型判定分岐を実装
- [ ] ステップ3（RED）: ネスト `x?.y?.z` が `Option<T>` を返すテスト追加
- [ ] ステップ4（GREEN）: `.map()` → `.and_then()` への変更
- [ ] ステップ5（RED）: 非 Option 型への `x ?? y` が `x` になるテスト追加
- [ ] ステップ6（GREEN）: nullish coalescing の型判定分岐
- [ ] ステップ7: Quality check

## テスト計画

- 非 Option 型への optional chaining → 通常アクセス
- Option 型への optional chaining → `.and_then()` パターン
- ネスト optional chaining → フラットな `Option<T>`
- 非 Option 型への nullish coalescing → そのまま返す
- 回帰: 既存の optional chaining / nullish coalescing テスト

## 完了条件

- 非 Option 型への optional chaining / nullish coalescing がコンパイル可能な Rust を生成する
- ネスト optional chaining が `Option<Option<T>>` を生成しない
- 全テスト pass、0 errors / 0 warnings
