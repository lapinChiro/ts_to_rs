# CLAUDE.md

TypeScript → Rust 変換 Codemod CLI ツール。

## Tech Stack

- **言語**: Rust
- **TS解析**: swc_ecma_parser + swc_ecma_ast
- **CLI**: clap
- **テスト**: cargo test + insta (スナップショット)
- **Lint**: clippy
- **フォーマット**: rustfmt

## Key Commands

```bash
cargo build                # デバッグビルド
cargo build --release      # リリースビルド
cargo check                # 高速な型チェック
cargo test                 # 全テスト実行
cargo clippy --all-targets --all-features -- -D warnings  # lint
cargo fmt --all --check    # フォーマットチェック
cargo llvm-cov --fail-under-lines 85   # カバレッジ計測（閾値85%）
cargo llvm-cov --html                  # HTMLレポート生成（target/llvm-cov/html/）
```

## Architecture

ディレクトリ構成は [README.md](README.md#ディレクトリ構成) を参照。

変換パイプライン: TS source → `parser` (SWC AST) → `transformer` (IR) → `generator` (Rust source)

## Core Principles

以下の3原則を常に遵守すること:

- **KISS**: 過剰設計を避けよ。最小限の複雑さで現在の要件を満たすこと。
- **YAGNI**: 要求されていない機能・改善・拡張を作るな。今必要なものだけを実装せよ。
- **DRY + 直交性**: DRYは「知識の重複」を排除する原則であり、「コードの見た目の重複」を排除する原則ではない。共通化によってモジュール間の結合が増えるなら、重複を残せ。

## Code Conventions

- `unwrap()` / `expect()` はテストコードのみ許可（詳細は `.claude/rules/testing.md`）。ライブラリコードでは `Result` で伝播
- `unsafe` ブロック禁止（必要な場合はコメントで理由を明記し、ユーザーの承認を得ること）
- `clone()` は初版では許容するが、不要な clone はコメントで TODO を残す
- pub な型・関数にはドキュメントコメント (`///`) を付ける

## Quality Standards

全ての変更に対し **0エラー・0警告** を維持すること。詳細は `.claude/rules/quality-check.md` を参照。

## 自発的改善の原則

問題や不整合を発見したら、ユーザーに指摘される前に自発的に調査・修正すること。これはツール固有の話ではなく、開発全般に適用される原則である:

- 警告・エラー・不整合を「一時的な問題」として安易に無視しない
- 問題の根本原因を特定してから対処する
- 開発環境・ツールチェインの問題も自分の責任範囲として扱う
- 「動いているから良い」ではなく「正しい状態か」を基準にする

## 学習プロトコル

ユーザーからClaude自身の挙動に関する修正指示を受けたとき:

1. 指示を一般化・抽象化する（特定のケースではなくパターンとして）
2. 保存先を判断する:
   - プロジェクト固有のルール → `.claude/rules/` に追記または新規作成
   - 個人的な好み → `~/.claude/CLAUDE.md` に追記
3. 書き込む内容と保存先をユーザーに提示し、確認を得てから書き込む
