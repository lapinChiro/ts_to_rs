# Project Overview

## Purpose

`ts_to_rs` は TypeScript コードを等価な Rust コードへ変換する CLI ツールである。

## Stack

- Rust 2021
- CLI: `clap`
- Parser/AST: `swc_ecma_parser`, `swc_ecma_ast`
- Serialization: `serde`, `serde_json`
- Errors: `anyhow`
- Snapshot tests: `insta`
- Helper tooling: `tools/extract-types/` with TypeScript Compiler API and Vitest

## Architecture

大まかなパイプライン:

```text
TS source
  -> parser / module graph
  -> type collection / registry
  -> type resolver
  -> transformer
  -> generator
  -> output writer
```

## Important Paths

- `src/`: Rust 実装本体
- `tests/`: integration, compile, snapshot, E2E
- `tests/e2e/scripts/`: TypeScript 実行サンプル
- `tools/extract-types/`: TypeScript 型抽出補助
- `scripts/`: benchmark と補助スクリプト
- `TODO`: PRD 化前の issue inventory
- `backlog/`: PRD
- `plan.md`: 現在の作業順
- `.claude/`: 既存の Claude Code ワークフロー
- `.codex/`: Codex 用設定
- `.agents/skills/`: Codex 用 shared skills

## Baseline Commands

- `cargo build`
- `cargo check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all --check`
- `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89`
- `./scripts/check-file-lines.sh`
