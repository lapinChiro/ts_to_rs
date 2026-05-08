// Stub entry point that keeps the runner Cargo project a valid binary
// crate. The pool template (`tests/e2e/rust-runner/`) is copied into a
// fresh per-runner temp directory at session start; from there each E2E
// test writes its converted Rust source to a content-hash-derived bin
// path (`src/<hash>.rs` for single-file flow, `src/<hash>/main.rs` +
// sibling `src/<hash>/<mod>.rs` for multi-file flow) and registers a
// matching `[[bin]]` entry in the slot-local `Cargo.toml`. The harness
// then runs `cargo run --bin <hash>` (= I-399 structural fix; see
// `tests/e2e_test.rs::E2eRunnerInstance::run_with_source` /
// `run_with_multi_file_sources` and `tests/i399_runner_mech.rs`).
//
// This file's content is therefore never executed at test time — it
// exists solely so the template directory is a valid Cargo project
// (loadable by IDE / rust-analyzer, supporting `cargo generate-lockfile`,
// and required by cargo's autobins=true so the package builds without an
// explicit `[lib]` section).
fn main() {}
