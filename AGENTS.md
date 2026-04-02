# AGENTS.md

## Language

- ユーザーへの応答は日本語で行う。
- コード、コメント、技術文書は英語でもよい。
- コミットメッセージは日本語で提案する。

## Repository

- このリポジトリは TypeScript を Rust に変換する CLI ツール `ts_to_rs` である。
- 中核は Rust 実装で、補助的に Node/TypeScript ベースの型抽出・E2E 補助がある。
- 既存の `Claude Code` 環境は維持する。Codex 用ファイルは追加であり置き換えではない。

## First Reads

大きい作業に入る前に、少なくとも以下を確認する。

- `plan.md`
- `TODO`
- `doc/agent/workflow.md`
- `doc/agent/quality-gates.md`
- `doc/agent/task-management.md`

必要に応じて以下も読む。

- `doc/agent/project-overview.md`
- `doc/agent/code-review.md`
- `doc/agent/rust-tooling.md`
- `CLAUDE.md`

## Core Rules

- 新機能またはバグ修正では `tdd` skill を使う。
- 完了報告前に `quality-check` skill を使う。
- 変更に伴い `plan.md`、`README.md`、`CLAUDE.md`、関連 doc comments が不正確になるなら更新する。
- 変換機能の変更では E2E テスト追加または拡張を必須とする。
- `0 errors, 0 warnings` を維持する。
- Rust library code では `unwrap()` / `expect()` を使わない。テストコードのみ可。
- `unsafe` は禁止。

## Git Boundary

- `git add`, `git commit`, `git push`, `git reset`, `git checkout`, `git switch`, `git stash`, `git merge`, `git rebase` は実行しない。
- Git の最終操作はユーザーが行う。
- 必要ならコミットメッセージ案だけを提示する。

## Task System

- `TODO` は PRD 化前の issue inventory。
- `backlog/` は PRD。
- `plan.md` は現在の実行順序。
- out-of-scope の発見事項は適切に `TODO` へ記録する。

## Skills

以下の skill を優先して使う。

- `tdd`
- `quality-check`
- `investigation`
- `refactoring-check`
- `backlog-management`
- `todo-audit`

## Commands

主要コマンド:

- `cargo build`
- `cargo check`
- `cargo test`
- `cargo fix --allow-dirty --allow-staged`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all --check`
- `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89`
- `./scripts/check-file-lines.sh`
- `./scripts/hono-bench.sh`
