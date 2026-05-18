//! PRD I-D-main Rule wording tests (= T2 phase 13 sub-tasks の lock-in test contracts =
//! Layer 2 rule file 内 specific text pattern 存在 grep-based assertion tests)。
//!
//! **State (2026-05-18、PRD I-D-main Iteration v26 Spec への逆戻り完了 + T2 phase implement)**:
//! T2-1〜T2-15 (T2-9 + T2-14 = I-D-pre migration excluded、計 **13 sub-tasks**) の各 sub-task
//! に対応する rule file wording を grep-based assertion で verify。各 test は I-D-pre
//! precedent (`tests/i_d_pre_rule_wording_test.rs`) を leverage:
//!
//! - `body_before_versioning()` helper (= `tests/common/mod.rs`) で extract された body を
//!   target (= I-D-main Iteration v26 Spec への逆戻り Fix 2 normative contract = `body_before_versioning()`
//!   helper + Versioning absence assertion を rule corpus 全 file 一貫 normative contract 化)
//! - `assert!(!content.contains("\n## Versioning"))` で 2026-05-12 Rules 改善 batch A2 fix
//!   "履歴の external delegation" structural integrity を test contract に embed
//! - sub-rule structural label (例: `(5-5)` / `(8-3)` / `(9-5)` 等) 存在を verify (= Rule 8 / 9
//!   2-level nested numeric sub-rule scheme 維持)
//!
//! 各 test fn name は `backlog/I-D-main-framework-rule-integration-cohesive-batch.md`
//! `## Spec→Impl Dispatch Arm Mapping` table の Test contract path と 1-to-1 sync。

#[path = "common/mod.rs"]
mod common;
use common::body_before_versioning;
use std::fs;

/// Helper: assert that the body contains a specific wording, with informative failure message.
fn assert_body_contains(body: &str, needle: &str, sub_task: &str) {
    assert!(
        body.contains(needle),
        "'{needle}' wording 不在 in rule body = {sub_task} 未完了"
    );
}

/// Helper: assert that the rule file does NOT contain `## Versioning` section
/// (= 2026-05-12 Rules 改善 batch A2 fix normative contract = "履歴の external delegation"、
/// I-D-pre precedent + I-D-main Iteration v26 Spec への逆戻り Fix 2 cross-PRD propagation)。
fn assert_versioning_absent(content: &str, rule_file: &str) {
    assert!(
        !content.contains("\n## Versioning"),
        "`## Versioning` section が残存 in {rule_file} = rule_review_list Group A2 '履歴の external delegation' 違反"
    );
}

/// Helper: read rule file and return (full content, body before Versioning).
fn read_rule(rule_file: &str) -> (String, String) {
    let content = fs::read_to_string(rule_file).expect("rule file exists");
    let body = body_before_versioning(&content).to_string();
    (content, body)
}

// ----------------------------------------------------------------------------
// T2-1 (cell 3 / v2-1): Rule 5 (5-5) fixture tsx runtime empirical observation
// ----------------------------------------------------------------------------

#[test]
fn test_rule5_fixture_tsx_runtime_empirical_observation_required() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-1";
    assert_body_contains(&body, "tsx runtime empirical observation", sub_task);
    assert_body_contains(&body, "fixture content 正当性", sub_task);
    assert_body_contains(&body, "cjs vs ESM", sub_task);
    assert_body_contains(&body, "(5-5)", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-2 (cell 9 / v4-3): Rule 9 (9-5) Spec→Impl Dispatch Arm Mapping table
// ----------------------------------------------------------------------------

#[test]
fn test_rule9_dispatch_arm_mapping_table_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-2";
    assert_body_contains(&body, "Spec→Impl Dispatch Arm Mapping table", sub_task);
    assert_body_contains(&body, "1-to-1 correspondence", sub_task);
    assert_body_contains(&body, "verify_dispatch_arm_mapping_table", sub_task);
    assert_body_contains(&body, "(9-5)", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-3 (cell 11 / v5-2): Rule 6 (6-5) dense matrix density limit + spec-table-driven generator
// ----------------------------------------------------------------------------

#[test]
fn test_rule6_dense_matrix_generator_recommendation_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-3";
    assert_body_contains(&body, "dense matrix", sub_task);
    assert_body_contains(&body, "manual-tracking density limit", sub_task);
    assert_body_contains(&body, "spec-table-driven generator", sub_task);
    assert_body_contains(&body, "single source-of-truth", sub_task);
    assert_body_contains(&body, "(6-5)", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-4 (cell 13 / v6-2): Rule 8 (8-3) invariant double-partition coverage
// ----------------------------------------------------------------------------

#[test]
fn test_rule8_invariant_double_partition_coverage_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-4";
    assert_body_contains(&body, "double-partition", sub_task);
    assert_body_contains(&body, "symmetric verify", sub_task);
    assert_body_contains(&body, "library mode vs executable mode", sub_task);
    assert_body_contains(
        &body,
        "Cartesian product cells の cross-reference",
        sub_task,
    );
    assert_body_contains(&body, "(8-3)", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-5 (cell 14 / v11-1): Rule 9 (9-6) substitute / rewrite logic dispatch arm symmetric
// ----------------------------------------------------------------------------

#[test]
fn test_rule9_substitute_logic_dispatch_arm_symmetric_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-5";
    assert_body_contains(&body, "substitute / rewrite logic", sub_task);
    assert_body_contains(
        &body,
        "sync substitute / async substitute / no substitute",
        sub_task,
    );
    assert_body_contains(&body, "3 arm 全てが test cell coverage", sub_task);
    assert_body_contains(&body, "(9-6)", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-6 (cell 15 / v11-3): Rule 10 axis (i) caller-supplied wrap context awareness
// ----------------------------------------------------------------------------

#[test]
fn test_rule10_axis_i_caller_wrap_context_awareness_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-6";
    assert_body_contains(&body, "caller-supplied wrap context awareness", sub_task);
    assert_body_contains(&body, "rewrite / substitute / IR-injection logic", sub_task);
    assert_body_contains(&body, "double-await structural bug", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-7 (cell 16 / v11-4): Layer 1 (Mechanical) decision table direct unit test coverage
// ----------------------------------------------------------------------------

#[test]
fn test_layer1_decision_table_direct_unit_test_documented() {
    let (content, body) = read_rule(".claude/rules/check-job-review-layers.md");
    let sub_task = "T2-7";
    assert_body_contains(&body, "decision table cell direct unit test", sub_task);
    assert_body_contains(
        &body,
        "新 public API / dispatch table / decision table",
        sub_task,
    );
    assert_body_contains(&body, "indirect coverage のみ", sub_task);
    assert_versioning_absent(&content, ".claude/rules/check-job-review-layers.md");
}

// ----------------------------------------------------------------------------
// T2-8 (cell 18 / v11-6): Rule 10 default check axis - double-source consistency
// ----------------------------------------------------------------------------

#[test]
fn test_rule10_double_source_consistency_axis_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-8";
    assert_body_contains(&body, "double-source consistency", sub_task);
    assert_body_contains(&body, "double-source / triple-source surfaces", sub_task);
    assert_body_contains(&body, "handoff doc + script comment", sub_task);
    assert_body_contains(&body, "token-level に accurate な双方 update", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-10 (cell 20 / v11-8): Rule 13 (13-7) Pending verdict severity Critical default
// ----------------------------------------------------------------------------

#[test]
fn test_rule13_pending_verdict_severity_critical_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-10";
    assert_body_contains(&body, "Pending verdict severity", sub_task);
    assert_body_contains(&body, "severity default = Critical", sub_task);
    assert_body_contains(&body, "Spec stage 移行 block", sub_task);
    assert_body_contains(&body, "findings count を ≥1", sub_task);
    assert_body_contains(&body, "(13-7)", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-11 (cell 22 / v11-10): Rule 8 (8-1) (c) multi-dispatch flow empirical probe
// ----------------------------------------------------------------------------

#[test]
fn test_rule8_c_multi_dispatch_flow_empirical_probe_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task = "T2-11";
    assert_body_contains(&body, "複数 dispatch flow", sub_task);
    assert_body_contains(&body, "prototype probe", sub_task);
    assert_body_contains(
        &body,
        "全 flow を prototype probe で empirical cover",
        sub_task,
    );
    assert_body_contains(&body, "Verification method 必須要件", sub_task);
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-12 (cell 23 / v11-11): Rule 10 default check axis - test infra (cargo profile / rustc)
// ----------------------------------------------------------------------------

#[test]
fn test_rule10_test_infra_axis_documented() {
    let (content, body) = read_rule(".claude/rules/spec-stage-adversarial-checklist.md");
    let sub_task =
        "T2-12 (post-Iteration v27 L3-1 fix: Axis F/G → (k)/(l) lowercase scheme normalize)";
    assert_body_contains(&body, "test infra defect PRD", sub_task);
    assert_body_contains(&body, "cargo profile", sub_task);
    assert_body_contains(&body, "debug/release", sub_task);
    assert_body_contains(&body, "rustc version variance", sub_task);
    // Post-Iteration v27 L3-1 fix: axis naming scheme normalize to lowercase (a)-(l)
    assert_body_contains(&body, "(k) **cargo profile = debug/release**", sub_task);
    assert_body_contains(&body, "(l) **rustc version variance**", sub_task);
    // Naming scheme single-source-of-truth assertion: capital "Axis F" / "Axis G" wording
    // should NOT appear in the body post v27 fix (= dual naming scheme structural prevention)
    assert!(
        !body.contains("Axis F (cargo profile"),
        "旧 capital 'Axis F (cargo profile' wording 残存 in rule body = post-Iteration v27 L3-1 normalize 未完了 = naming scheme dual inconsistency"
    );
    assert!(
        !body.contains("Axis G (rustc version variance)"),
        "旧 capital 'Axis G (rustc version variance)' wording 残存 in rule body = post-Iteration v27 L3-1 normalize 未完了 = naming scheme dual inconsistency"
    );
    assert_versioning_absent(
        &content,
        ".claude/rules/spec-stage-adversarial-checklist.md",
    );
}

// ----------------------------------------------------------------------------
// T2-13 (cell 25 / v12-2): Layer 3 (Structural cross-axis) Spec wording vs 実体 cross-check
// ----------------------------------------------------------------------------

#[test]
fn test_layer3_spec_vs_implementation_cross_check_documented() {
    let (content, body) = read_rule(".claude/rules/check-job-review-layers.md");
    let sub_task = "T2-13";
    assert_body_contains(&body, "Spec wording vs 実体 infra work", sub_task);
    assert_body_contains(&body, "Layer 3 default check axis", sub_task);
    assert_body_contains(&body, "spec wording の実体整合性", sub_task);
    assert_body_contains(&body, "第三者視点で empirical verify", sub_task);
    assert_versioning_absent(&content, ".claude/rules/check-job-review-layers.md");
}

// ----------------------------------------------------------------------------
// T2-15 (cell 30 / v13-7): Layer 4 /check_job recursion convergence criterion
//   = Hybrid M-1+M-2+M-3 mechanisms + C-1〜C-4 4-条件 final rule
// ----------------------------------------------------------------------------

#[test]
fn test_check_job_recursion_convergence_documented() {
    let (content, body) = read_rule(".claude/rules/check-job-review-layers.md");
    let sub_task = "T2-15";
    assert_body_contains(&body, "recursion convergence criterion", sub_task);
    // Hybrid mechanisms (M-1 / M-2 / M-3)
    assert_body_contains(&body, "M-1", sub_task);
    assert_body_contains(&body, "M-2", sub_task);
    assert_body_contains(&body, "M-3", sub_task);
    assert_body_contains(&body, "severity classification", sub_task);
    assert_body_contains(&body, "diminishing returns detection", sub_task);
    assert_body_contains(&body, "Meta-finding tracking", sub_task);
    // Final rule (C-1 ~ C-4)
    assert_body_contains(&body, "C-1", sub_task);
    assert_body_contains(&body, "C-2", sub_task);
    assert_body_contains(&body, "C-3", sub_task);
    assert_body_contains(&body, "C-4", sub_task);
    assert_body_contains(&body, "Critical=0", sub_task);
    assert_body_contains(&body, "High=0", sub_task);
    assert_body_contains(&body, "trajectory diminishing", sub_task);
    assert_body_contains(&body, "meta-finding ratio <= 50%", sub_task);
    assert_versioning_absent(&content, ".claude/rules/check-job-review-layers.md");
}
