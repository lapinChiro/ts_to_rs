//! PRD I-D-pre Rule wording tests (= Layer 2 rule file 内 specific text pattern 存在
//! grep-based assertion tests)。
//!
//! **Stub state (2026-05-11 post Spec stage v1)**: Implementation Phase 5 (T2-pre-1 +
//! T2-pre-2) で fill in 予定、`#[ignore]` 状態維持。
//!
//! Test structure: 各 rule wording strengthening について
//! - rule file 内 specific text pattern 存在を grep-based に assert
//! - Versioning section v1.8 entry 存在 verify
//!
//! 各 test fn name は backlog/I-D-pre-audit-mechanism-bootstrap.md
//! `## Spec→Impl Dispatch Arm Mapping` table の Test contract path と 1-to-1 sync。

use std::fs;

/// Extract the body section of a rule file (= content before `## Versioning`).
/// Used to verify rule wording is present in the actual rule body, not just in
/// the Versioning history section (= /check_job deep deep C/H fix: substring
/// existence-only assertions can false-positive PASS when wording is moved to
/// Versioning only).
fn body_before_versioning(content: &str) -> &str {
    content
        .split_once("\n## Versioning")
        .map(|(body, _)| body)
        .unwrap_or(content)
}

/// Cell 4 / v11-7 / T2-pre-1: check-job-review-layers.md Layer 1 sub-step
/// factual accuracy semantic check
///
/// **Verification**: rule file body (= Versioning 除く) 内に sub-step (4) wording +
/// 3 hard-coded mechanism references 存在 + Versioning section v1.8 entry 存在
#[test]
fn test_layer1_factual_accuracy_semantic_check_documented() {
    let content =
        fs::read_to_string(".claude/rules/check-job-review-layers.md").expect("rule file exists");
    let body = body_before_versioning(&content);
    assert!(
        body.contains("factual accuracy semantic check"),
        "Layer 1 sub-step (4) 'factual accuracy semantic check' wording 不在 in rule body = T2-pre-1 未完了"
    );
    assert!(
        body.contains("意味と一致"),
        "Layer 1 sub-step (4) '意味と一致' semantic wording 不在 in rule body = T2-pre-1 未完了"
    );
    for sub_label in &["(4-1)", "(4-2)", "(4-3)"] {
        assert!(
            body.contains(sub_label),
            "Layer 1 sub-step structural sub-label '{sub_label}' 不在 in rule body = sub-rule structure incomplete"
        );
    }
    for script in &[
        "scripts/verify_line_refs.py",
        "scripts/audit-handoff-doc-line-refs.py",
        "scripts/verify_prd_self_audits.py",
    ] {
        assert!(
            body.contains(script),
            "Hard-coded script reference '{script}' 不在 in rule body = structural enforcement mechanism reference missing"
        );
    }
    assert!(
        content.contains("v1.8"),
        "Versioning section v1.8 entry 不在 = T2-pre-1 self-applied integration 未完了"
    );
}

/// Cell 5 / v13-5 / T2-pre-2: spec-stage-adversarial-checklist.md Rule 9 / Rule 13
/// sub-rule cell numbering convention single-source-of-truth
///
/// **Verification**: rule file body 内に Rule 9 (d) + Rule 13 (13-6) sub-rule wording +
/// 3 hard-coded audit reference 存在 + Versioning section v1.8 entry 存在
#[test]
fn test_rule9_cell_numbering_convention_documented() {
    let content = fs::read_to_string(".claude/rules/spec-stage-adversarial-checklist.md")
        .expect("rule file exists");
    let body = body_before_versioning(&content);
    assert!(
        body.contains("cell numbering convention"),
        "'cell numbering convention' wording 不在 in rule body = T2-pre-2 未完了"
    );
    assert!(
        body.contains("single-source-of-truth"),
        "'single-source-of-truth' wording 不在 in rule body = T2-pre-2 未完了"
    );
    assert!(
        body.contains("matrix #"),
        "canonical identifier 'matrix #' wording 不在 in rule body = T2-pre-2 未完了"
    );
    for sub_label in &[
        "(d-1)", "(d-2)", "(d-3)", "(13-6-a)", "(13-6-b)", "(13-6-c)",
    ] {
        assert!(
            body.contains(sub_label),
            "Sub-rule structural label '{sub_label}' 不在 in rule body = sub-rule structure incomplete"
        );
    }
    for audit_ref in &[
        "verify_cell_numbering_drift_detection",
        "has_cell_numbering_convention_section",
        "CELL_SLOT_AS_IDENTIFIER_RE",
    ] {
        assert!(
            body.contains(audit_ref),
            "Hard-coded audit reference '{audit_ref}' 不在 in rule body = structural enforcement mechanism reference missing"
        );
    }
    assert!(
        content.contains("v1.8"),
        "Versioning section v1.8 entry 不在 = T2-pre-2 self-applied integration 未完了"
    );
}
