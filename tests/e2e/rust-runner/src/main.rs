// Stub entry point for the Cargo project skeleton. Each E2E test runs in a
// per-runner temp directory (`E2eRunnerPool`); pool init copies this file to
// the per-runner src/, after which `reset_single_file_main` /
// `reset_multi_file_sources` overwrite it with the converted Rust source under
// test. This file's content is therefore never executed at test time — it
// exists only so that the template directory is a valid Cargo project (loadable
// by IDE/rust-analyzer, supporting `cargo generate-lockfile` etc.).
fn main() {}
