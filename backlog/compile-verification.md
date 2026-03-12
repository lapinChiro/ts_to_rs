# 生成コードのコンパイル検証

## 背景・動機

現在のテストはスナップショットで「生成された Rust コードの文字列」を検証しているが、その文字列が実際に `rustc` でコンパイル可能かは検証していない。
生成コードがコンパイルできなければツールとして実用に耐えないため、コンパイル可能性の自動検証が必要。

## ゴール

- CI/テストの一部として、生成された Rust コードが `rustc` でコンパイルできることを自動検証する

## スコープ

### 対象

- 既存の fixture（basic-types, optional-fields, functions, mixed）の生成コードのコンパイル検証
- 今後追加される fixture も自動的に検証対象になる仕組み

### 対象外

- 生成コードの実行時テスト（コンパイルが通ることのみ検証）
- 生成コードの意味的正しさの検証（入力 TS と同じ動作をするかの検証）

## 設計

### 技術的アプローチ

テストケース内で以下を行う:

1. fixture の `.input.ts` を `transpile()` で変換
2. 生成された Rust コードを一時ファイルに書き出す
3. `rustc --edition 2021 --crate-type lib <tmp>.rs` でコンパイルを試行
4. 終了コード 0 ならパス、それ以外なら `rustc` の stderr を含めてテスト失敗

### 実装方式

- `tests/compile_test.rs` に統合テストとして実装
- `tests/fixtures/` のすべての `.input.ts` を動的に列挙してテスト
- `std::process::Command` で `rustc` を呼び出す
- 一時ファイルは `tempfile` クレートまたは `std::env::temp_dir()` を使用

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `tests/compile_test.rs` | 新規作成 |
| `Cargo.toml` | dev-dependencies に `tempfile` 追加（必要な場合） |

## 作業ステップ

- [ ] Step 1: 単一 fixture のコンパイル検証テストを書く（RED）
- [ ] Step 2: テストが通ることを確認（GREEN — 既存 fixture は通るはず）
- [ ] Step 3: 全 fixture を動的に列挙するテストに拡張
- [ ] Step 4: コンパイルが通らない fixture がある場合、生成コードを修正

## テスト計画

| # | 入力 | 期待結果 | 種別 |
|---|------|----------|------|
| 1 | basic-types.input.ts の生成コード | `rustc` で正常コンパイル | 正常系 |
| 2 | optional-fields.input.ts の生成コード | `rustc` で正常コンパイル | 正常系 |
| 3 | functions.input.ts の生成コード | `rustc` で正常コンパイル | 正常系 |
| 4 | mixed.input.ts の生成コード | `rustc` で正常コンパイル | 正常系 |

## 完了条件

- 全 fixture の生成コードが `rustc` でコンパイル可能
- 既存テストが引き続き全パス
- `cargo clippy` 0警告、`cargo fmt --check` 0エラー
