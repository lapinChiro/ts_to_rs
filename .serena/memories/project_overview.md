# ts_to_rs overview
- Purpose: CLI tool that converts TypeScript source code into equivalent Rust code.
- Primary stack: Rust 2021 CLI using clap, SWC parser/AST (`swc_ecma_parser`, `swc_ecma_ast`), serde/serde_json, anyhow.
- Supporting tooling: Node/TypeScript helper under `tools/extract-types` using TypeScript Compiler API and Vitest; E2E TS scripts under `tests/e2e` using `tsx`.
- Architecture pipeline: parser/module graph -> type collection/registry -> type resolver -> transformer -> generator -> output writer. `README.md` and `CLAUDE.md` describe this as a multi-pass AST-to-AST pipeline.
- Repository structure: `src/` Rust implementation, `tests/` integration/snapshot/E2E tests, `scripts/` benchmarks and analysis helpers, `backlog/` task PRDs, `plan.md` active plan, `TODO` prioritized issue inventory, `doc/` design/completed feature docs, `.claude/` project-specific Claude Code rules/commands/skills.
- Current planning state: `plan.md` says current work is Batch 4b / I-312b Phase 4-5 (registry TsTypeInfo migration + validation). `TODO` is the master prioritized issue list with stable IDs and tiers.
- CI: `.github/workflows/ci.yml` runs `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89`, plus installs Node deps in `tests/e2e`.
- Claude-specific workflow exists in `.claude/` with rules, commands, skills, and permissions. `CLAUDE.md` is the central operating manual.