//! Shared mechanism for I-399 (E2E test isolation defect): per-test
//! content-hash-derived bin names eliminate the path collision that caused
//! stale-binary leak under concurrent slot-pool execution.
//!
//! Used by:
//! - `tests/e2e_test.rs` (`E2eRunnerInstance::run_with_source` /
//!   `run_with_multi_file_sources`) for production E2E test invocation.
//! - `tests/i399_isolation_test.rs` for structural lock-in of the hash
//!   function's invariants and the cargo per-bin fingerprint behavior that
//!   the I-399 fix relies on.
//!
//! `tests/i399_runner_mech.rs` is treated by cargo as an integration test
//! binary (it has no `#[test]` functions of its own). It exposes a single
//! pure function and is consumed via `#[path = "i399_runner_mech.rs"] mod
//! i399_runner_mech;` from sibling test files. This mirrors the existing
//! `tests/test_helpers.rs` pattern.
//!
//! See `backlog/I-399-e2e-test-isolation-defect.md` for the full design
//! rationale.

/// Computes a deterministic content-hash-derived bin name from Rust source
/// content. Used to generate unique `[[bin]]` names per test source,
/// eliminating the path collision that caused I-399 stale-binary leak (=
/// different sources sharing `src/main.rs` led to cargo's per-package
/// fingerprint occasionally reusing stale binaries under concurrent
/// slot-pool execution).
///
/// Uses FNV-1a 64-bit hash (deterministic across processes, no external
/// dependency, sufficient collision resistance for the ≤1k-test scale of
/// this suite). The 16-hex-char hash is truncated to 12 and prefixed with
/// `b` to form a valid Rust identifier (cargo bin names follow Rust ident
/// rules; a leading digit is forbidden, hence the `b` prefix).
///
/// Same content → same bin name → cargo cache reuse (correct binary).
/// Different content → different bin name → cargo fresh build (no stale
/// binary risk).
pub fn content_hash_bin_name(source: &str) -> String {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut hash: u64 = FNV_OFFSET;
    for byte in source.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    let full = format!("{hash:016x}");
    format!("b{}", &full[..12])
}
