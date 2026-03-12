# 期待型の伝播（expected_type propagation）

## 背景・動機

現在の `convert_expr` は式の「期待される型」を知らない。このため、生成コードが Rust の型システムに合わない場面が複数ある:

- `String` フィールドに文字列リテラルを渡すと `&str` になりコンパイルエラー（`Config { name: "foo" }` → `"foo".to_string()` が必要）
- `Vec<String>` の要素に `"hello"` を渡すと同様のエラー
- 関数の戻り値が `String` のとき `return "ok"` がコンパイルエラー

場当たり的なワークアラウンド（`convert_expr_with_type_hint`, `ensure_owned_string`）が存在するが、統一されておらず、新機能追加のたびに個別対応が必要。

## ゴール

`convert_expr` が期待される型（`Option<&RustType>`）を受け取り、`StringLit` を `String` 型のコンテキストで使用する場合に自動で `.to_string()` を付与する。

具体的に以下の 4 ケースでコンパイル可能な Rust コードが生成される:

1. 型注記付き変数宣言: `const s: string = "hello"` → `let s: String = "hello".to_string()`
2. 構造体フィールド初期化: `Config { name: "foo" }` → `Config { name: "foo".to_string() }`
3. 関数の return 文: 戻り値型が `String` の関数で `return "ok"` → `return "ok".to_string()`
4. 配列要素: `const a: string[] = ["a", "b"]` → `vec!["a".to_string(), "b".to_string()]`

既存のワークアラウンド（`convert_expr_with_type_hint`, `ensure_owned_string`）は新しい仕組みに統合・削除する。

## スコープ

### 対象

- `convert_expr` のシグネチャを `convert_expr(expr, expected_type: Option<&RustType>)` に変更
- `convert_expr_with_type_hint(expr, type_hint: Option<&str>)` を廃止し、新シグネチャに統合
- `ensure_owned_string()` を廃止し、`convert_expr` 内の `StringLit` 処理で一般化
- `StringLit` + `RustType::String` の組み合わせで `.to_string()` を自動付与
- 呼び出し元（`statements.rs`, `classes.rs`, `functions.rs`, `mod.rs`）の更新
- `Vec<String>` の配列要素への expected_type 伝播

### 対象外

- `TypeRegistry` の導入（Phase 2 として別 PRD）
- 関数引数のオブジェクトリテラル型推論（TypeRegistry が前提）
- ネストしたオブジェクトリテラルの型推論（TypeRegistry が前提）
- `&str` と `String` の使い分け最適化（初版は全て `String`）

## 設計

### 技術的アプローチ

#### 1. `convert_expr` のシグネチャ変更

```rust
// 旧: 2 つの関数が存在
pub fn convert_expr(expr: &ast::Expr) -> Result<Expr>
pub fn convert_expr_with_type_hint(expr: &ast::Expr, type_hint: Option<&str>) -> Result<Expr>

// 新: 1 つに統合
pub fn convert_expr(expr: &ast::Expr, expected: Option<&RustType>) -> Result<Expr>
```

#### 2. StringLit の処理

`convert_lit` に expected_type を渡し、`RustType::String` が期待される場合は `Expr::MethodCall { object: StringLit, method: "to_string" }` で包む。

#### 3. 配列要素への伝播

`convert_array_lit` に expected_type を渡す。`RustType::Vec(inner)` が期待される場合、各要素の変換に `Some(&inner)` を渡す。

#### 4. オブジェクトリテラルの統合

`convert_object_lit` は `type_hint: Option<&str>` の代わりに `expected: Option<&RustType>` を受け取る。`RustType::Named { name, .. }` から構造体名を取得する（現在の `type_hint` と同等だが型が統一される）。

#### 5. return 文の処理

`convert_stmt` に関数の戻り値型を伝播する仕組みが必要。`convert_stmt_list` / `convert_stmt` に `return_type: Option<&RustType>` パラメータを追加し、`Stmt::Return` の変換時に `convert_expr(expr, return_type)` を呼ぶ。

これにより `functions.rs` の `ensure_owned_string` と `wrap_returns_in_ok` 内の特殊処理が不要になる。

### 影響範囲

- `src/transformer/expressions.rs` — `convert_expr` シグネチャ変更、`convert_lit` に expected_type 追加、`convert_expr_with_type_hint` 廃止
- `src/transformer/statements.rs` — `convert_stmt` / `convert_stmt_list` に return_type パラメータ追加、`extract_named_type_hint` 廃止
- `src/transformer/functions.rs` — `ensure_owned_string` 廃止、`wrap_returns_in_ok` の簡素化
- `src/transformer/classes.rs` — `convert_expr` 呼び出し元の更新
- `src/transformer/mod.rs` — `convert_expr` 呼び出し元の更新

## 作業ステップ

- [ ] ステップ1: `convert_expr` のシグネチャ変更 — `expected: Option<&RustType>` を追加し、全呼び出し元で `None` を渡して既存テストを通す
- [ ] ステップ2: `StringLit` + `String` 期待型 — `convert_lit` で `RustType::String` が期待される場合に `.to_string()` を付与。型注記付き変数宣言のテストを追加
- [ ] ステップ3: 構造体フィールド初期化 — `convert_object_lit` を `Option<&RustType>` に変更し、フィールド値の変換に expected_type なし（StringLit が struct フィールドの場合は TypeRegistry なしでは型が不明なため、ここでは対応しない。compile_test の fixture で検証）
- [ ] ステップ4: return 文への伝播 — `convert_stmt` に `return_type` を追加し、`Stmt::Return` で expected_type として使用。`ensure_owned_string` を廃止
- [ ] ステップ5: 配列要素への伝播 — `convert_array_lit` に expected_type を渡し、`Vec<T>` の要素型を伝播
- [ ] ステップ6: `convert_expr_with_type_hint` の廃止 — 残存する呼び出しを新シグネチャに移行、関数を削除
- [ ] ステップ7: compile_test fixture の更新 — object-literal fixture に String フィールドのテストケースを追加し、コンパイル成功を確認
- [ ] ステップ8: スナップショットテスト — 既存スナップショットの更新と新規 fixture（string-to-string.input.ts）の追加

## テスト計画

- 正常系: `const s: string = "hello"` → `let s: String = "hello".to_string()`
- 正常系: `const a: string[] = ["a", "b"]` → `vec!["a".to_string(), "b".to_string()]`
- 正常系: 関数戻り値 `String` で `return "ok"` → `"ok".to_string()` が tail expression
- 正常系: `Config { name: "foo" }` で String フィールド → `"foo".to_string()`（fixture でコンパイル検証）
- 正常系: expected_type が `None` の場合、既存の挙動が維持される（`StringLit` はそのまま）
- 正常系: expected_type が `F64` や `Bool` の場合、StringLit に影響なし
- 境界値: ネストした式（`vec![vec!["a"]]` で `Vec<Vec<String>>`）
- 回帰: 既存の全スナップショットテストが通ること

## 完了条件

- `convert_expr_with_type_hint` が削除されている
- `ensure_owned_string` が削除されている
- 上記 4 ケース（変数宣言・struct 初期化・return 文・配列要素）で `.to_string()` が自動付与される
- object-literal fixture に String フィールドを含むケースが追加され、compile_test が通る
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
