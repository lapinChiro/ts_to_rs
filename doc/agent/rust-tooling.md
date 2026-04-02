# Rust Tooling

## Core Commands

- Build: `cargo build`
- Fast check: `cargo check`
- Test: `cargo test`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Format check: `cargo fmt --all --check`
- Coverage: `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89`
- Auto-fix: `cargo fix --allow-dirty --allow-staged`

## Rust-specific Rules

- library code で `unwrap()` / `expect()` を使わない
- `unsafe` は使わない
- public types/functions には doc comments を付ける

## Benchmark / Utilities

- Hono benchmark: `./scripts/hono-bench.sh`
- Both modes benchmark: `./scripts/hono-bench.sh --both`
- File length check: `./scripts/check-file-lines.sh`
- Error inspection: `python3 scripts/inspect-errors.py`

## rust-analyzer

- Rust diagnostics を無視しない
- 設定変更後は analyzer refresh が必要になる場合がある
