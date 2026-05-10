//! PRD I-D-pre Method A utility tests (= scripts/verify_line_refs.py own behavior
//! auto-verify、Cell 4 / v11-7 audit utility part)。
//!
//! **Stub state (2026-05-11 post Spec stage v1)**: Implementation Phase 2 (T1-pre-5) で
//! fill in 予定、`#[ignore]` 状態維持。
//!
//! Test structure: utility own behavior auto-verify
//! - **Positive test**: synthetic PRD doc fixture (= heading-based line-ref drift 含む)
//!   で utility が detect (= drift list 非空)
//! - **Negative test**: drift 不在 fixture で utility no-detection PASS
//! - **Metadata header verify**: utility script 内 metadata header (= purpose / coverage
//!   scope / regression-tested status / I-D-pre Cell 4 v11-7 binding) 存在 verify
//!
//! Synthetic fixtures = `tests/fixtures/i_d_pre/{positive,negative}/method_a_*.md`

use std::fs;
use std::process::Command;

/// Method A formal lock-in: heading-based line-ref drift detection
///
/// **Positive (drift 検出)**:
/// - synthetic PRD `method_a_drift.md` = `## Section X` (line N) を `(line M)` で claim、
///   actual M ≠ N → drift detect (exit code 1 + stdout "Drifts ...: <non-zero>")
///
/// **Negative (no-drift PASS)**:
/// - synthetic PRD `method_a_clean.md` = 全 line refs accurate → no detection
///   (exit code 0 + stdout "Drifts ...: 0")
#[test]
fn test_method_a_line_ref_drift_detection() {
    // Positive: drift fixture
    let positive_output = Command::new("python3")
        .arg("scripts/verify_line_refs.py")
        .arg("tests/fixtures/i_d_pre/positive/method_a_drift.md")
        .output()
        .expect("python3 verify_line_refs.py invocation succeeds");
    let positive_stdout = String::from_utf8_lossy(&positive_output.stdout);
    let positive_exit = positive_output.status.code().expect("exit code present");

    assert_eq!(
        positive_exit, 1,
        "positive drift fixture should exit code 1; got {}, stdout:\n{}",
        positive_exit, positive_stdout
    );
    // A8 fix (/check_job L1-5): exact drift count assertion (= positive fixture is
    // designed to introduce exactly 2 heading-based line-ref drifts、Path E test の
    // axis_drift_count helper と consistency 維持)
    assert!(
        positive_stdout.contains("Drifts (heuristic-detected, requires human triage): 2"),
        "positive stdout should contain exactly 2 drifts (= fixture spec); stdout:\n{}",
        positive_stdout
    );

    // Negative: clean fixture
    let negative_output = Command::new("python3")
        .arg("scripts/verify_line_refs.py")
        .arg("tests/fixtures/i_d_pre/negative/method_a_clean.md")
        .output()
        .expect("python3 verify_line_refs.py invocation succeeds");
    let negative_stdout = String::from_utf8_lossy(&negative_output.stdout);
    let negative_exit = negative_output.status.code().expect("exit code present");

    assert_eq!(
        negative_exit, 0,
        "negative clean fixture should exit code 0; got {}, stdout:\n{}",
        negative_exit, negative_stdout
    );
    assert!(
        negative_stdout.contains("Drifts (heuristic-detected, requires human triage): 0"),
        "negative stdout should contain '0' drift count; stdout:\n{}",
        negative_stdout
    );
}

/// Method A formal lock-in: utility metadata header embed
///
/// **Verification**: scripts/verify_line_refs.py 内に formal lock-in metadata header
/// (= purpose / coverage scope / regression-tested status / "I-D-pre Cell 4 v11-7"
/// binding) 存在 verify
#[test]
fn test_method_a_utility_metadata_header_embed() {
    let content =
        fs::read_to_string("scripts/verify_line_refs.py").expect("Method A utility exists");

    // Required metadata header markers (per PRD I-D-pre Cell 4 spec)
    let required_markers = [
        "Method A formal lock-in utility",
        "PRD I-D-pre Cell 4",
        "v11-7",
        "regression-tested",
        "Path B split adoption 2026-05-11",
        "tests/i_d_pre_method_a_test.rs",
    ];
    for marker in &required_markers {
        assert!(
            content.contains(marker),
            "Method A utility metadata header missing required marker '{}'; \
             expected per PRD I-D-pre Cell 4 spec",
            marker
        );
    }
}
