//! PRD I-D-pre Rule wording tests (= Layer 2 rule file 内 specific text pattern 存在
//! grep-based assertion tests)。
//!
//! **State (2026-05-18 post I-D-main Iteration v26 Spec への逆戻り)**: T2-pre-1 +
//! T2-pre-2 fully implemented + `body_before_versioning()` helper を `tests/common/mod.rs`
//! に移動済 (= I-D-main rule wording tests と DRY 解消)。
//!
//! Test structure: 各 rule wording strengthening について
//! - rule file 内 specific text pattern 存在を grep-based に assert
//! - rule-review batch 後の normative contract
//!   (Versioning section external delegation / numeric sub-rule normalization)
//!   を verify
//!
//! 各 test fn name は backlog/I-D-pre-audit-mechanism-bootstrap.md
//! `## Spec→Impl Dispatch Arm Mapping` table の Test contract path と 1-to-1 sync。

#[path = "common/mod.rs"]
mod common;
use common::body_before_versioning;
use std::fs;

/// Cell 4 / v11-7 / T2-pre-1: check-job-review-layers.md Layer 1 sub-step
/// factual accuracy semantic check
///
/// **Verification**: rule file body 内に sub-step (4) wording +
/// 3 hard-coded mechanism references 存在 + rule review batch で導入された
/// `## Versioning` external delegation が維持されている
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
        !content.contains("\n## Versioning"),
        "`## Versioning` section が残存 = rule_review_list Group A2 '履歴の external delegation' 違反"
    );
}

/// Cell 5 / v13-5 / T2-pre-2: spec-stage-adversarial-checklist.md Rule 9 / Rule 13
/// sub-rule cell numbering convention single-source-of-truth
///
/// **Verification**: rule file body 内に Rule 9 (9-4) + Rule 13 (13-6) sub-rule wording +
/// 3 hard-coded audit reference 存在 + legacy orphan label `(d-N)` が除去され
/// normalized numeric labels が維持されている
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
        "(9-4-1)", "(9-4-2)", "(9-4-3)", "(13-6-a)", "(13-6-b)", "(13-6-c)",
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
        !body.contains("(d-1)") && !body.contains("(d-2)") && !body.contains("(d-3)"),
        "Legacy orphan label `(d-N)` 残存 = rule_review_list Group C3/D1 violation"
    );
    assert!(
        !content.contains("\n## Versioning"),
        "`## Versioning` section が残存 = rule_review_list Group A2 '履歴の external delegation' 違反"
    );
}
