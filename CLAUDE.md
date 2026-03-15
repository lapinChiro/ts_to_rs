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
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89  # カバレッジ計測（閾値89%、main.rs除外）
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

- `unwrap()` / `expect()` はテストコードのみ許可。ライブラリコードでは `Result` で伝播
- `unsafe` ブロック禁止（必要な場合はコメントで理由を明記し、ユーザーの承認を得ること）
- `clone()` は初版では許容するが、不要な clone はコメントで TODO を残す
- pub な型・関数にはドキュメントコメント (`///`) を付ける

## Quality Standards

全ての変更に対し **0エラー・0警告** を維持すること。作業完了時は /quality-check を実行する。

カバレッジ閾値のラチェット運用: 実測値が閾値を 2pt 以上上回ったら、閾値を 1pt 引き上げる。

## 行動規範

- **Git 操作の制限**: `git commit` / `push` / `merge` はユーザーのみが行う。Claude はコミットメッセージの提案のみ
- **判断軸のある質問**: 選択肢・メリデメ・推奨案を提示して確認する。「これでよいですか？」のような判断軸のない質問をしない。自分で判断できることは判断して進める
- **検証の原則**: 検証は「検証項目の列挙 → 期待結果の定義 → 実行 → 判定」の順。結果を見てから「期待通り」と後付けしない
- **デバッグ**: 修正が 1 回で成功しなかったら、次の修正前に根本原因の仮説を立てる。同じ修正を 2 回繰り返さない
- **後回しの記録**: スコープ外の課題を発見したら、内容と理由を `TODO` に詳細に記録する
- **ドキュメント同期**: コード変更時に plan.md, README.md, CLAUDE.md, doc コメントが不正確になっていないか確認・更新する
- **rust-analyzer**: 作業開始時に `rust_analyzer_set_workspace` を実行。構成変更後は再読み込み + diagnostics 確認。diagnostics のエラーを無視しない

## ワークフロー

以下の状況では対応する Skill を必ず呼び出すこと:

- 新機能・バグ修正の着手 → /tdd
- 作業完了時（コミット前） → /quality-check
- 機能追加完了後 → /refactoring-check
- 開発セッションの最後（コミット前） → /todo-audit
- backlog/ の操作 → /backlog-management
- backlog/ が空で作業依頼を受けた → /backlog-replenishment
- PRD の作成 → /prd-template
- TODO が空で作業依頼を受けた → /todo-replenishment
- 調査タスク → /investigation
- TODO の棚卸し（定期的、または大きな機能追加の完了後） → /todo-grooming
- 変換正当性の監査（定期的、または大規模変更後） → /correctness-audit
- ルールの作成・変更 → /rule-writing, /rule-maintenance

## 自発的改善の原則

問題や不整合を発見したら、ユーザーに指摘される前に自発的に調査・修正すること:

- 警告・エラー・不整合を「一時的な問題」として安易に無視しない
- 問題の根本原因を特定してから対処する
- 「動いているから良い」ではなく「正しい状態か」を基準にする

## 学習プロトコル

ユーザーからClaude自身の挙動に関する修正指示を受けたとき:

1. 指示を一般化・抽象化する（特定のケースではなくパターンとして）
2. 保存先を判断する:
   - プロジェクト固有のルール → `.claude/rules/` に追記または新規作成
   - 個人的な好み → `~/.claude/CLAUDE.md` に追記
3. 書き込む内容と保存先をユーザーに提示し、確認を得てから書き込む
