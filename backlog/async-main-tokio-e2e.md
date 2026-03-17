# async fn main への `#[tokio::main]` 生成 + async/await E2E テスト

## 背景・動機

現在、`async function main()` を含む TS コードを変換すると `async fn main()` が生成されるが、Rust では async main には `#[tokio::main]` アトリビュートが必須。アトリビュートがないとコンパイルエラーになり、変換結果がそのまま使えない。

また、async/await の変換は単体テスト（スナップショット）でコード生成を検証しているが、実際にコンパイル・実行して TS と同じ出力になるかを検証する E2E テストがない。

## ゴール

1. `async function main()` を含む TS コードの変換結果に `#[tokio::main]` が付与され、コンパイル・実行可能になる
2. async/await の主要パターンが E2E テスト（TS 実行結果 = Rust 実行結果）でカバーされている

## スコープ

### 対象

- IR (`Item::Fn`) へのアトリビュートフィールド追加
- Transformer: `async fn main` 検出時に `#[tokio::main]` アトリビュートを設定
- Generator: アトリビュートの出力
- スナップショットテストの更新
- async/await E2E テストスクリプトの追加

### 対象外

- `main` 以外の関数へのアトリビュート付与（`#[test]` 等）
- Cargo.toml への依存クレート自動追加（I-30 のスコープ）— E2E ランナーには tokio が既存
- ネストした async 関数（I-29）
- async void のセマンティクス差異（I-21）

## 設計

### 技術的アプローチ

**IR 変更**: `Item::Fn` に `attributes: Vec<String>` フィールドを追加する。各要素は `#[...]` の内側の文字列（例: `"tokio::main"`）。

理由: `serde_tag: Option<String>` のような用途固定フィールドではなく、汎用的なアトリビュートリストにすることで、将来の `#[test]`、`#[derive(...)]` 等に自然に拡張できる。現時点では `#[tokio::main]` のみが使用するが、汎用設計のコストは Vec フィールド 1 つであり、専用フィールドと差がない。

**Transformer 変更**: `convert_fn_decl` / `convert_top_level_items` で、関数名が `"main"` かつ `is_async` の場合に `attributes` に `"tokio::main"` を追加する。

**Generator 変更**: `Item::Fn` の出力時、`attributes` の各要素を `#[{attr}]\n` として関数宣言の前に出力する。

### 影響範囲

- `src/ir.rs` — `Item::Fn` のフィールド追加
- `src/transformer/functions/mod.rs` — async main 検出ロジック
- `src/transformer/mod.rs` — `Item::Fn` 構築箇所（attributes フィールド追加）
- `src/generator/mod.rs` — アトリビュート出力
- `tests/fixtures/async-await.input.ts` — async main パターン追加
- `tests/e2e/scripts/` — async E2E テストスクリプト追加

## 作業ステップ

- [ ] ステップ 1: IR に `attributes: Vec<String>` フィールドを追加し、既存の `Item::Fn` 構築箇所を `attributes: vec![]` で更新。コンパイル通過を確認
- [ ] ステップ 2: Generator でアトリビュート出力を実装。テスト: attributes に値を入れたとき `#[...]` が出力されることをスナップショットで検証
- [ ] ステップ 3: Transformer で async main 検出 → attributes 設定を実装。テスト: `async function main()` のスナップショットに `#[tokio::main]` が含まれることを検証
- [ ] ステップ 4: async/await E2E テストスクリプトを追加。以下のパターンをカバー:
  - async fn main + 単一 await
  - async fn 間の呼び出しチェーン
  - Promise<T> 戻り値型のアンラップ（number, string）
  - 複数の逐次 await

## テスト計画

- **スナップショット**: `async function main()` → `#[tokio::main]\nasync fn main()` の生成確認
- **スナップショット**: 非 async の `function main()` にはアトリビュートが付かないことの確認
- **スナップショット**: `async function notMain()` にはアトリビュートが付かないことの確認
- **E2E**: async/await スクリプトの TS 実行結果と Rust 実行結果が一致

## 完了条件

- `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- `cargo fmt --all --check` がパス
- `cargo test` が全件パス
- `cargo llvm-cov` がカバレッジ閾値を満たす
- async fn main の変換結果に `#[tokio::main]` が含まれる（スナップショット）
- async/await E2E テストが TS/Rust 出力一致で合格
