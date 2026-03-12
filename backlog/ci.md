# CI（GitHub Actions）

## 背景・動機

品質チェック（`cargo fmt --check`, `cargo clippy`, `cargo test`）は現在手動で実行している。開発者が実行を忘れると壊れたコードがマージされるリスクがある。CI を導入することで、全ての PR・push に対して品質ゲートを自動適用する。

## ゴール

- `main` ブランチへの push と PR に対して、`cargo fmt --check` / `cargo clippy` / `cargo test` が自動実行される
- いずれかが失敗したら CI が赤になる

## スコープ

### 対象

- GitHub Actions ワークフローの作成
- `cargo fmt --check` ステップ
- `cargo clippy --all-targets --all-features -- -D warnings` ステップ
- `cargo test` ステップ
- Rust ツールチェインのキャッシュ設定

### 対象外

- デプロイ / リリース自動化
- テストカバレッジ計測（CI 安定後に別途追加）
- Docker ベースのビルド環境
- crates.io への公開

## 設計

### 技術的アプローチ

GitHub Actions の標準的な Rust ワークフローを使用する。

1. `dtolnay/rust-toolchain@stable` でツールチェインをセットアップ
2. `Swatinem/rust-cache@v2` で `target/` と依存関係をキャッシュ
3. `cargo fmt --all --check`、`cargo clippy`、`cargo test` を順次実行

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `.github/workflows/ci.yml` | 新規作成 |

## 作業ステップ

- [ ] Step 1: `.github/workflows/ci.yml` を作成
- [ ] Step 2: push して CI が動作することを確認
- [ ] Step 3: 意図的にテスト失敗させて CI が赤になることを確認

## テスト計画

| # | 検証項目 | 期待結果 | 種別 |
|---|----------|----------|------|
| 1 | `main` への push | CI が実行され全ステップ green | 正常系 |
| 2 | PR 作成 | CI が実行される | 正常系 |
| 3 | fmt 違反のコミット | CI が赤になる | 異常系 |

## 完了条件

- CI ワークフローが `main` への push と PR で自動実行される
- `cargo fmt --check` / `cargo clippy` / `cargo test` の3ステップが全て含まれる
- 現時点で CI が green である
