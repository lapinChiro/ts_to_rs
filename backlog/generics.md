# ジェネリクス対応

## 背景・動機

TS のジェネリクスは型安全なコードで頻出する。`interface Foo<T>` や `function identity<T>(x: T): T` が変換できないと、実用的な TS プロジェクトへの対応が限定される。Rust も同様にジェネリクスをサポートしており、構文の対応関係が明確なため変換しやすい。

## ゴール

- ジェネリック型パラメータ付きの `interface` / `type` / `function` が Rust のジェネリクスに変換される
- ジェネリック型引数（`Foo<string>` など）が型参照として正しく変換される

## スコープ

### 対象

- interface のジェネリクス: `interface Foo<T> { value: T; }` → `struct Foo<T> { value: T }`
- type alias のジェネリクス: `type Pair<A, B> = { first: A; second: B; }` → `struct Pair<A, B> { ... }`
- 関数のジェネリクス: `function identity<T>(x: T): T` → `fn identity<T>(x: T) -> T`
- ジェネリック型引数の参照: `Foo<string>` → `Foo<String>`
- 複数型パラメータ: `<A, B, C>`

### 対象外

- 型制約（`<T extends Foo>`）→ Rust の trait bound（設計判断が多いため別 PRD）
- デフォルト型パラメータ（`<T = string>`）
- 条件型（`T extends U ? X : Y`）
- マップ型（`{ [K in keyof T]: ... }`）

## 設計

### 技術的アプローチ

1. **IR の拡張**: 型パラメータを表現するフィールドを追加
   - `Item::Struct` に `type_params: Vec<String>` を追加
   - `Item::Fn` に `type_params: Vec<String>` を追加
   - `RustType::Named(String)` を `RustType::Named { name: String, type_args: Vec<RustType> }` に変更

2. **Transformer の拡張**:
   - `TsInterfaceDecl` / `TsTypeAliasDecl` / `FnDecl` の `type_params` から型パラメータ名を抽出
   - `TsTypeRef` で型引数がある場合（`Array` 以外）、`RustType::Named` に `type_args` を含める

3. **Generator の拡張**:
   - `struct Foo<T, U>` / `fn foo<T>(x: T) -> T` のフォーマット
   - `Foo<String>` の型引数出力

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/ir.rs` | `Item::Struct`, `Item::Fn` に `type_params` 追加、`RustType::Named` 拡張 |
| `src/transformer/types.rs` | 型パラメータ抽出、ジェネリック型引数の変換 |
| `src/transformer/functions.rs` | 関数の型パラメータ抽出 |
| `src/generator.rs` | ジェネリクスの出力 |
| `tests/fixtures/` | ジェネリクスのテスト fixture |

## 作業ステップ

- [ ] Step 1: IR の `RustType::Named` を `name` + `type_args` に拡張し、既存コードを修正
- [ ] Step 2: generator で `Named` 型の `type_args` 出力を実装
- [ ] Step 3: transformer で `TsTypeRef` のジェネリック型引数を変換
- [ ] Step 4: IR の `Item::Struct` に `type_params` を追加し、generator で出力
- [ ] Step 5: transformer で interface / type alias の型パラメータを抽出
- [ ] Step 6: IR の `Item::Fn` に `type_params` を追加し、generator / transformer を対応
- [ ] Step 7: E2E fixture テスト追加

## テスト計画

| # | 入力 | 期待出力 | 種別 |
|---|------|----------|------|
| 1 | `interface Box<T> { value: T; }` | `struct Box<T> { value: T }` | 正常系 |
| 2 | `type Pair<A, B> = { first: A; second: B; }` | `struct Pair<A, B> { first: A, second: B }` | 複数パラメータ |
| 3 | `function identity<T>(x: T): T { return x; }` | `fn identity<T>(x: T) -> T { x }` | 関数ジェネリクス |
| 4 | `let x: Box<string>` (フィールド内) | `Box<String>` | 型引数の参照 |
| 5 | `interface Foo { name: string; }` (型パラメータなし) | `struct Foo { name: String }` (変更なし) | 回帰テスト |

## 完了条件

- 上記テストが全パス
- 既存テスト全パス
- 生成コードがコンパイル検証テストを通る
- `cargo clippy` 0警告、`cargo fmt --check` 0エラー
