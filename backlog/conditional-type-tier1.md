# conditional type — Tier 1: 自動変換可能パターン

## 背景・動機

Hono のソースコードには約 37 件の conditional type（`T extends X ? Y : Z`）が使われている。このうち約 13 件は TypeScript と Rust の間に明確な対応関係があり、自動変換が可能なパターンである。Hono 変換の完了にはこれらの自動変換が不可欠。

## ゴール

以下の 4 パターンの conditional type が自動的に Rust コードに変換される:

1. **型フィルタリング**: `T extends X ? T : never` → trait bound（`T: X`）
2. **単純型変換**: `T extends X ? Y : Z` → trait の associated type
3. **型述語**: `T extends X ? true : false` → marker trait
4. **`infer` 抽出**: `T extends Foo<infer U> ? U : never` → associated type（`T::Output`）

## スコープ

### 対象

- `TsConditionalType` AST ノードの解析
- 上記 4 パターンの検出と Rust コードへの変換
- Hono に存在する約 13 件の Tier 1 パターンが変換される

### 対象外

- Tier 2（フォールバック出力）— 別 PRD
- テンプレートリテラル型を含む conditional type（Group A）
- 再帰的 conditional type（Group B）
- 高階型操作（Group C）
- 複雑ネスト conditional type（Group D）

## 設計

### 技術的アプローチ

`transformer/types.rs` に `convert_conditional_type` 関数を追加する:

1. `TsConditionalType` の構造を解析:
   - `check_type`: 判定対象の型
   - `extends_type`: extends 制約
   - `true_type`: 条件が真の場合の型
   - `false_type`: 条件が偽の場合の型

2. パターンマッチングで 4 パターンを検出:
   - `false_type` が `never` かつ `true_type` が `check_type` と同一 → 型フィルタリング（trait bound）
   - `true_type` と `false_type` が異なる具体型 → associated type による分岐
   - `true_type` が `true`、`false_type` が `false` → marker trait
   - `extends_type` に `infer` キーワードを含む → associated type 抽出

3. 変換先の IR:
   - 型フィルタリング → `RustType` に trait bound 情報を付与（既存の generics 変換と統合）
   - associated type → `type Output = Y where T: X` 形式の type alias
   - marker trait → `trait IsFoo {}` + `impl IsFoo for T {}`
   - `infer` 抽出 → associated type reference（`<T as Foo>::Output`）

### 影響範囲

- `src/transformer/types/mod.rs` — `convert_conditional_type` 関数追加、`convert_type_alias` からの呼び出し
- `src/ir.rs` — 必要に応じて associated type の IR 表現を追加
- `src/generator/mod.rs` — associated type の生成

## 作業ステップ

- [ ] ステップ1: Hono の実パターンを分析し、各パターンの具体的な入力と期待出力を定義する
- [ ] ステップ2（RED）: 型フィルタリングパターン `T extends X ? T : never` のテストを追加し、失敗を確認
- [ ] ステップ3（GREEN）: `TsConditionalType` のパース基盤と型フィルタリングの変換を実装
- [ ] ステップ4（RED→GREEN）: 単純型変換パターン `T extends X ? Y : Z` の変換を実装
- [ ] ステップ5（RED→GREEN）: 型述語パターン `T extends X ? true : false` の変換を実装
- [ ] ステップ6（RED→GREEN）: `infer` 抽出パターンの変換を実装
- [ ] ステップ7: E2E テスト（fixture）を追加
- [ ] ステップ8（REFACTOR）: パターン検出ロジックの整理

## テスト計画

各パターンについて Hono の実例に基づくテスト:

- 型フィルタリング: `type Filter<T> = T extends string ? T : never` → trait bound
- 単純型変換: `type Convert<T> = T extends string ? number : boolean` → associated type
- 型述語: `type IsString<T> = T extends string ? true : false` → marker trait
- `infer` 抽出: `type Unwrap<T> = T extends Promise<infer U> ? U : never` → associated type
- パターンに該当しない conditional type → エラー（Tier 2 で対応）
- ネストしていない単純なケースのみ

## 完了条件

- 4 パターンの conditional type が自動的に Rust コードに変換される
- Hono の Tier 1 該当パターン（約 13 件）が変換可能になる
- パターンに該当しない conditional type は明示的なエラーメッセージを返す
- `cargo fmt --all --check` / `cargo clippy` / `cargo test` が 0 エラー・0 警告
