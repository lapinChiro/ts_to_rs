//! Shared test helpers for integration tests that invoke `audit-prd-rule10-compliance.py`
//! against synthetic PRD doc fixtures.
//!
//! Usage pattern (Rust integration test convention):
//! ```ignore
//! #[path = "common/mod.rs"]
//! mod common;
//! use common::{run_audit, count_violations_containing, count_violations_for_task_id};
//! ```
//!
//! Established for PRD I-D-main T1 phase (= /check_job Action Item #5 DRY refactor)
//! to eliminate duplicate `run_audit` / `count_violations` impls across
//! `tests/i_d_main_audit_extensions_test.rs` and
//! `tests/i_d_pre_audit_extensions_test.rs`.

#![allow(dead_code)] // 各 integration test crate が異なる subset を使用する設計
use std::process::Command;

/// Run `audit-prd-rule10-compliance.py` on a fixture path, return (exit_code, stderr).
///
/// stderr に audit script の violation messages (= `FAIL: N compliance violation(s):` +
/// individual violation lines) が含まれる。Tests assert against exit code + stderr content。
pub fn run_audit(fixture: &str) -> (i32, String) {
    let output = Command::new("python3")
        .arg("scripts/audit-prd-rule10-compliance.py")
        .arg(fixture)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "python3 audit-prd-rule10-compliance.py failed for {}: {}",
                fixture, e
            )
        });
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit = output.status.code().expect("exit code present");
    (exit, stderr)
}

/// Count audit violations containing a specific substring in stderr.
///
/// 例: `count_violations_containing(&stderr, "v3-4 violation")` で v3-4 violation
/// 数 を count、`count_violations_containing(&stderr, "Cartesian product completeness")` で
/// cartesian-related violation 数 を count。
pub fn count_violations_containing(stderr: &str, substr: &str) -> usize {
    stderr.matches(substr).count()
}

/// Count audit violations for a specific task ID using the canonical
/// `{task_id} violation` substring pattern (= PRD I-D-pre audit script convention)。
///
/// Equivalent to `count_violations_containing(stderr, &format!("{} violation", task_id))`.
pub fn count_violations_for_task_id(stderr: &str, task_id: &str) -> usize {
    let pattern = format!("{} violation", task_id);
    stderr.matches(&pattern).count()
}

/// Run Path E utility (`scripts/verify_prd_self_audits.py`) on a fixture, return total
/// drift count parsed from stdout `Total drifts (...): N` line。
///
/// Used for dual-verify tests (= same fixture を audit script + Path E 両方経由で run、
/// 1-to-1 mapping invariant verify)。
pub fn path_e_total_drifts(fixture: &str) -> usize {
    let output = Command::new("python3")
        .arg("scripts/verify_prd_self_audits.py")
        .arg(fixture)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "python3 verify_prd_self_audits.py failed for {}: {}",
                fixture, e
            )
        });
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("Total drifts") {
            if let Some(n_str) = rest.rsplit(':').next() {
                if let Ok(n) = n_str.trim().parse::<usize>() {
                    return n;
                }
            }
        }
    }
    panic!(
        "could not parse 'Total drifts' line from Path E stdout:\n{}",
        stdout
    )
}
