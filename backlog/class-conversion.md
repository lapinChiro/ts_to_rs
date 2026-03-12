# クラス → struct + impl 変換

## 背景・動機

TS プロジェクトでクラスは頻出する構造だが、現在は変換できずスキップされる。クラスを `struct` + `impl` ブロックに変換できれば、対応可能な TS コードの範囲が大幅に広がる。

## ゴール

- TS の `class` 宣言が Rust の `struct` + `impl` ブロックに変換される
- プロパティ → struct フィールド、メソッド → impl 内の関数 として変換される
- `constructor` → `new()` 関連関数として変換される

## スコープ

### 対象

- プロパティ宣言（型注釈付き）→ struct フィールド
- メソッド → `impl` ブロック内の `fn`（`&self` / `&mut self` レシーバ付き）
- `constructor` → `pub fn new(...) -> Self` 関連関数
- `export class` → `pub struct` + `pub` メソッド
- `this.x` → `self.x`

### 対象外

- `static` メソッド・プロパティ
- アクセス修飾子（`private`, `protected`）による visibility 制御
- 継承（`extends`）
- インターフェース実装（`implements`）
- getter / setter
- デコレータ
- 抽象クラス（`abstract`）

## 設計

### 技術的アプローチ

1. **IR の拡張**: `Item::Impl` バリアントを追加
   ```rust
   Item::Impl {
       struct_name: String,
       methods: Vec<Method>,
   }
   ```
   `Method` は `Item::Fn` と似た構造だが、`self` パラメータの有無を表現する。

2. **Transformer の拡張**: `Decl::Class` を処理
   - SWC の `ClassDecl` からプロパティとメソッドを抽出
   - プロパティ → `Item::Struct` のフィールドに変換
   - メソッド → `Item::Impl` 内の `Method` に変換
   - `constructor` → `new()` 関連関数に変換
   - メソッドの第一引数に `&self` を追加

3. **Expression の拡張**: `this.x` → `self.x`
   - `Expr::Member` を IR に追加: `Expr::FieldAccess { object: Box<Expr>, field: String }`
   - `this` → `self` に置換

4. **Generator の拡張**: `Item::Impl` の出力
   ```rust
   impl StructName {
       pub fn new(...) -> Self { ... }
       pub fn method(&self, ...) -> ReturnType { ... }
   }
   ```

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/ir.rs` | `Item::Impl`, `Method`, `Expr::FieldAccess` 追加 |
| `src/transformer/mod.rs` | `Decl::Class` の処理追加 |
| `src/transformer/types.rs` or 新規 `src/transformer/classes.rs` | クラス変換ロジック |
| `src/transformer/expressions.rs` | `MemberExpr` (`this.x`) の変換 |
| `src/generator.rs` | `Item::Impl`, `Expr::FieldAccess` の生成 |
| `tests/fixtures/` | クラス変換のテスト fixture |

## 作業ステップ

- [ ] Step 1: IR に `Expr::FieldAccess` を追加し、generator で `object.field` を出力
- [ ] Step 2: transformer で `MemberExpr` → `Expr::FieldAccess` を変換（`this` → `self`）
- [ ] Step 3: IR に `Method` と `Item::Impl` を追加
- [ ] Step 4: generator で `impl` ブロックを出力
- [ ] Step 5: transformer で `ClassDecl` のプロパティ → `Item::Struct` のフィールドに変換
- [ ] Step 6: transformer で `constructor` → `new()` 関連関数に変換
- [ ] Step 7: transformer で通常メソッド → `&self` メソッドに変換
- [ ] Step 8: E2E fixture テスト追加

## テスト計画

| # | 入力 | 期待出力 | 種別 |
|---|------|----------|------|
| 1 | `class Foo { x: number; }` | `struct Foo { x: f64 }` | プロパティのみ |
| 2 | `class Foo { constructor(x: number) { this.x = x; } }` | `impl Foo { pub fn new(x: f64) -> Self { ... } }` | constructor |
| 3 | `class Foo { greet(): string { return "hi"; } }` | `impl Foo { pub fn greet(&self) -> String { ... } }` | メソッド |
| 4 | `this.name` 式 | `self.name` | this 変換 |
| 5 | `export class Foo { ... }` | `pub struct` + `pub fn` | export 対応 |
| 6 | プロパティ + constructor + メソッドの複合 | struct + impl の完全な出力 | 複合 |

## 完了条件

- 上記テストが全パス
- 既存テスト全パス
- 生成コードがコンパイル検証テストを通る
- `cargo clippy` 0警告、`cargo fmt --check` 0エラー
