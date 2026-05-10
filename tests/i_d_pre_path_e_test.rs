//! PRD I-D-pre Path E utility tests (= scripts/verify_prd_self_audits.py own behavior
//! auto-verify、Cells 1+2+5 audit utility part、4 axes + F6/F7 fix + Axis 3 extension)。
//!
//! **Stub state (2026-05-11 post Spec stage v1)**: Implementation Phase 2 (T1-pre-6) で
//! fill in 予定、`#[ignore]` 状態維持。
//!
//! Test structure: utility own behavior auto-verify
//! - **4 axes × (positive + negative) = 8 base tests**
//! - **F6 fix verify**: Axis 1 allow-list 動作 (= Scope partition exception flag suppress
//!   + その他 missing cells flag)
//! - **F7 fix verify**: Axis 2 post-v15 wording presence 検出 (= TS-X heading 内でも
//!   v15+ wording なら flag = blanket exclude 解消)
//! - **Axis 3 extension verify**: cell-slot vocabulary fork drift detection
//!
//! Synthetic fixtures = `tests/fixtures/i_d_pre/{positive,negative}/path_e_*.md`

use std::fs;
use std::process::Command;

/// Helper: run Path E utility on a fixture path, return (exit_code, stdout).
fn run_path_e(fixture: &str) -> (i32, String) {
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
    let exit = output.status.code().expect("exit code present");
    (exit, stdout)
}

/// Helper: extract drift count for a specific axis from Path E stdout.
fn axis_drift_count(stdout: &str, axis_num: u32) -> usize {
    let marker = format!("=== Axis {} (", axis_num);
    let header_line = stdout
        .lines()
        .find(|l| l.starts_with(&marker))
        .unwrap_or_else(|| panic!("axis {} marker not found in stdout:\n{}", axis_num, stdout));
    // Extract count from "...): N drifts ==="
    let count_part = header_line.rsplit("):").next().unwrap_or("");
    let n: usize = count_part
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("could not parse drift count from line: '{}'", header_line));
    n
}

/// Path E Axis 1 (cross-reference cell consistency) F6 fix verify
///
/// **Positive**: Scope (policy=full) section omits matrix cell → Axis 1 flags 1 drift
/// **Negative**: Scope lists all matrix cells → Axis 1 flags 0
#[test]
fn test_path_e_axis1_allow_list_replacement() {
    let (pos_exit, pos_stdout) =
        run_path_e("tests/fixtures/i_d_pre/positive/path_e_axis1_partition_violation.md");
    assert_eq!(
        pos_exit, 1,
        "positive should exit 1; stdout:\n{}",
        pos_stdout
    );
    assert_eq!(
        axis_drift_count(&pos_stdout, 1),
        1,
        "positive Axis 1 expected 1 drift; stdout:\n{}",
        pos_stdout
    );
    assert!(
        pos_stdout.contains("policy=full, expected full enumeration"),
        "positive Axis 1 should reference policy=full; stdout:\n{}",
        pos_stdout
    );

    let (neg_exit, neg_stdout) =
        run_path_e("tests/fixtures/i_d_pre/negative/path_e_axis1_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative should exit 0; stdout:\n{}",
        neg_stdout
    );
    assert_eq!(
        axis_drift_count(&neg_stdout, 1),
        0,
        "negative Axis 1 expected 0 drifts; stdout:\n{}",
        neg_stdout
    );
}

/// Path E Axis 2 (status pending verdict) F7 fix verify
///
/// **Positive**: TS-pre-X heading 内 で post-v15 wording 残存 → Axis 2 flags drifts
/// **Negative**: TS-pre-X heading 内 pre-v15 wording のみ → Axis 2 flags 0
#[test]
fn test_path_e_axis2_post_v15_wording_detection() {
    let (pos_exit, pos_stdout) =
        run_path_e("tests/fixtures/i_d_pre/positive/path_e_axis2_post_v15_violation.md");
    assert_eq!(
        pos_exit, 1,
        "positive should exit 1; stdout:\n{}",
        pos_stdout
    );
    let pos_count = axis_drift_count(&pos_stdout, 2);
    assert!(
        pos_count >= 1,
        "positive Axis 2 expected >= 1 drift (got {}); stdout:\n{}",
        pos_count,
        pos_stdout
    );
    assert!(
        pos_stdout.contains("F7 fix"),
        "positive Axis 2 should mention F7 fix; stdout:\n{}",
        pos_stdout
    );
    assert!(
        pos_stdout.contains("post-v15 wording"),
        "positive Axis 2 should mention post-v15 wording; stdout:\n{}",
        pos_stdout
    );

    let (neg_exit, neg_stdout) =
        run_path_e("tests/fixtures/i_d_pre/negative/path_e_axis2_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative should exit 0; stdout:\n{}",
        neg_stdout
    );
    assert_eq!(
        axis_drift_count(&neg_stdout, 2),
        0,
        "negative Axis 2 expected 0 drifts; stdout:\n{}",
        neg_stdout
    );
}

/// Path E Axis 3 (label namespace + cell-slot vocabulary fork) Axis 3 extension verify
///
/// **Positive**: "cell-slot N" / "cell-slot #N" identifier-level fork → flags
/// **Negative**: descriptive uses ("cell-slot occurrence" / "cell-slot vocabulary fork") → no flag
#[test]
fn test_path_e_axis3_cell_slot_vocabulary_coverage() {
    let (pos_exit, pos_stdout) =
        run_path_e("tests/fixtures/i_d_pre/positive/path_e_axis3_identifier_violation.md");
    assert_eq!(
        pos_exit, 1,
        "positive should exit 1; stdout:\n{}",
        pos_stdout
    );
    let pos_count = axis_drift_count(&pos_stdout, 3);
    assert!(
        pos_count >= 1,
        "positive Axis 3 expected >= 1 drift (got {}); stdout:\n{}",
        pos_count,
        pos_stdout
    );
    assert!(
        pos_stdout.contains("Axis 3 extension"),
        "positive Axis 3 should mention Axis 3 extension; stdout:\n{}",
        pos_stdout
    );

    let (neg_exit, neg_stdout) =
        run_path_e("tests/fixtures/i_d_pre/negative/path_e_axis3_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative should exit 0; stdout:\n{}",
        neg_stdout
    );
    assert_eq!(
        axis_drift_count(&neg_stdout, 3),
        0,
        "negative Axis 3 expected 0 drifts; stdout:\n{}",
        neg_stdout
    );
}

/// Path E Axis 4 (external file drift) baseline preservation verify
///
/// **Positive**: Impact Area table claims external file size != actual → flags
/// **Negative**: no external file size claims → no flag
#[test]
fn test_path_e_axis4_external_file_drift_detection() {
    let (pos_exit, pos_stdout) =
        run_path_e("tests/fixtures/i_d_pre/positive/path_e_axis4_external_drift.md");
    assert_eq!(
        pos_exit, 1,
        "positive should exit 1; stdout:\n{}",
        pos_stdout
    );
    assert_eq!(
        axis_drift_count(&pos_stdout, 4),
        1,
        "positive Axis 4 expected 1 drift; stdout:\n{}",
        pos_stdout
    );
    assert!(
        pos_stdout.contains("Cargo.toml") && pos_stdout.contains("999999999"),
        "positive Axis 4 should report Cargo.toml drift detail; stdout:\n{}",
        pos_stdout
    );

    let (neg_exit, neg_stdout) =
        run_path_e("tests/fixtures/i_d_pre/negative/path_e_axis4_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative should exit 0; stdout:\n{}",
        neg_stdout
    );
    assert_eq!(
        axis_drift_count(&neg_stdout, 4),
        0,
        "negative Axis 4 expected 0 drifts; stdout:\n{}",
        neg_stdout
    );
}

/// Path E formal lock-in: utility metadata header embed
///
/// **Verification**: scripts/verify_prd_self_audits.py 内に formal lock-in metadata
/// header (= purpose / coverage scope / 4 axes + F6/F7 fix + Axis 3 extension /
/// regression-tested status / "I-D-pre Cells 1, 2, 5" binding) 存在 verify
#[test]
fn test_path_e_utility_metadata_header_embed() {
    let content =
        fs::read_to_string("scripts/verify_prd_self_audits.py").expect("Path E utility exists");

    let required_markers = [
        "Path E formal lock-in utility",
        "PRD I-D-pre Cells 1, 2, 5",
        "F6 fix integrated",
        "F7 fix integrated",
        "Axis 3 extension integrated",
        "regression-tested",
        "Path B split adoption 2026-05-11",
        "tests/i_d_pre_path_e_test.rs",
    ];
    for marker in &required_markers {
        assert!(
            content.contains(marker),
            "Path E utility metadata header missing required marker '{}'; \
             expected per PRD I-D-pre Cells 1+2+5 spec",
            marker
        );
    }
}
