# ネスト関数宣言の変換

## 背景・動機

Hono の `compose.ts` で `async function dispatch(i: number)` がクロージャ内で宣言されている。`convert_stmt` に `Decl::Fn` の処理がなく、unsupported statement エラーになる。

TypeScript では関数内に関数を宣言するパターンが一般的であり、Rust ではクロージャ束縛（`let f = |params| { body }`）に変換するのが自然である。

## ゴール

関数内で宣言された関数（ネスト関数）が `let dispatch = |i: f64| { ... }` のようなクロージャ束縛に変換される。

## スコープ

### 対象

- `convert_stmt` に `ast::Stmt::Decl(ast::Decl::Fn(fn_decl))` のアームを追加
- ネスト関数を `Stmt::Let { name, init: Some(Expr::Closure { ... }) }` に変換
- async 関数の対応（Rust の async closure は nightly のため、通常の closure + async block で対応）

### 対象外

- ジェネレータ関数
- 関数のホイスティング（TS では宣言前に呼べるが、Rust では不可）
- トップレベル関数宣言（既存の `convert_module_item` で対応済み）

## 設計

### 技術的アプローチ

- `convert_stmt` に `Stmt::Decl(Decl::Fn(fn_decl))` のマッチアームを追加する
- `fn_decl` のパラメータ・返り値型・body を既存の変換関数（`convert_params`, `convert_ts_type`, `convert_block_stmt` 等）で処理する
- 変換結果を `Stmt::Let` + `Expr::Closure` の IR ノードとして生成する
- async 関数の場合: クロージャの body 全体を `async { ... }` ブロックで囲む形にする（stable Rust で動作する形式）
- IR に `Expr::Closure` バリアントが存在しない場合は追加が必要

### 影響範囲

- `src/transformer/` — `convert_stmt` のマッチアーム追加
- `src/ir.rs`（または IR 定義ファイル）— `Expr::Closure` バリアント追加（未存在の場合）
- `src/generator/` — `Expr::Closure` の出力処理追加（未存在の場合）
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: `function f() { function inner(x: number): number { return x; } }` の変換テストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: IR に `Expr::Closure` バリアントを追加（必要な場合）
- [ ] ステップ3（GREEN）: `convert_stmt` に `Decl::Fn` アーム追加
- [ ] ステップ4（GREEN）: Generator で `Expr::Closure` を出力
- [ ] ステップ5: E2E テスト追加（async ネスト関数を含む）
- [ ] ステップ6: Quality check

## テスト計画

- シンプルなネスト関数 → クロージャ束縛（`let inner = |x: f64| -> f64 { x }`）
- async ネスト関数 → async block 付きクロージャ束縛
- パラメータと返り値型を持つネスト関数
- 回帰: トップレベル関数宣言が変更なく動作すること

## 完了条件

- ネスト関数宣言がクロージャ束縛に変換される
- async ネスト関数が stable Rust で動作する形式に変換される
- 既存のテストがすべてパスする
- `cargo test`, `cargo clippy`, `cargo fmt --check` が 0 エラー・0 警告
