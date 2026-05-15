//! PRD I-D-main Audit script extensions tests (= scripts/audit-prd-rule10-compliance.py
//! 新 verify functions の synthetic PRD doc fixture-based positive + negative tests)。
//!
//! Test structure: 各 audit function に対し
//! - **Positive test**: synthetic PRD doc fixture (= 故意に違反 pattern 含む) で
//!   audit function が detect する (= violation list 非空)
//! - **Negative test**: synthetic PRD doc fixture (= 違反 pattern なし) で
//!   audit function PASS (= violation list 空)
//! - **Gate test**: `## Cell Numbering Convention` section 不在 fixture で
//!   Option α auto-detect gate により audit skip (= retroactive compliance
//!   pending PRDs を audit out-of-scope に自動分類) を verify
//!
//! Synthetic fixtures = `tests/fixtures/i_d_main/{positive,negative}/*.md`。
//! Fixtures README: `tests/fixtures/i_d_main/README.md`。
//!
//! 各 test fn name は backlog/I-D-main-framework-rule-integration-cohesive-batch.md
//! `## Spec→Impl Dispatch Arm Mapping` table の Test contract path と 1-to-1 sync。
//!
//! Test helpers (`run_audit`, `count_violations_containing`) は
//! `tests/common/mod.rs` で共有 (= /check_job T1 phase review Action Item #5
//! DRY refactor 由来)。

#[path = "common/mod.rs"]
mod common;
use common::{count_violations_containing, run_audit};

// ---------------------------------------------------------------------------
// T1-1: verify_cartesian_product_completeness (cell 1 / R-1)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_cartesian_completeness_detects_implicit_omission() {
    let fixture = "tests/fixtures/i_d_main/positive/cartesian_implicit_omission.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with implicit cell omission, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    // Verify Cartesian product completeness violation is reported
    let cp_violation_count =
        count_violations_containing(&stderr, "Cartesian product completeness violation");
    assert!(
        cp_violation_count >= 1,
        "Expected at least 1 'Cartesian product completeness violation' message, \
         got {} (stderr=\n{})",
        cp_violation_count,
        stderr
    );
    // Verify the missing cell # is identified (cell 3 in fixture)
    assert!(
        stderr.contains("[3]") || stderr.contains("cells [3]"),
        "Expected violation message to identify missing cell [3], stderr=\n{}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// T1-2: verify_no_duplicate_top_level_matrix (cell 4 / v3-4)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_detects_duplicate_top_level_matrix() {
    let fixture = "tests/fixtures/i_d_main/positive/duplicate_top_level_matrix.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with duplicate top-level matrix, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v3_4_count = count_violations_containing(&stderr, "v3-4 violation");
    assert!(
        v3_4_count >= 1,
        "Expected at least 1 'v3-4 violation' message, got {} (stderr=\n{})",
        v3_4_count,
        stderr
    );
    assert!(
        stderr.contains("組合せマトリクス") && stderr.contains("count=2"),
        "Expected violation message to identify duplicate `### 組合せマトリクス` \
         sub-section with count=2, stderr=\n{}",
        stderr
    );
}

#[test]
fn test_audit_no_false_positive_on_single_matrix() {
    // Reuse cartesian_complete.md (= single proper matrix table)
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (_exit, stderr) = run_audit(fixture);
    let v3_4_count = count_violations_containing(&stderr, "v3-4 violation");
    assert_eq!(
        v3_4_count, 0,
        "Expected 0 'v3-4 violation' messages on single-matrix fixture, \
         got {} (stderr=\n{})",
        v3_4_count, stderr
    );
}

// ---------------------------------------------------------------------------
// T1-3: verify_dispatch_tree_pseudocode_syntactic (cell 5 / v3-5)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_detects_dispatch_tree_duplicate_match_arms() {
    let fixture = "tests/fixtures/i_d_main/positive/dispatch_tree_duplicate_arms.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with duplicate match arms, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v3_5_count = count_violations_containing(&stderr, "v3-5 violation");
    assert!(
        v3_5_count >= 1,
        "Expected at least 1 'v3-5 violation' message, got {} (stderr=\n{})",
        v3_5_count,
        stderr
    );
    assert!(
        stderr.contains("(Foo, Bar)"),
        "Expected violation message to identify the duplicate `(Foo, Bar)` pattern, \
         stderr=\n{}",
        stderr
    );
}

#[test]
fn test_audit_no_v3_5_false_positive_on_no_pseudocode() {
    // Fixture with no Rust pseudocode in Design section should trivially PASS v3-5
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (_exit, stderr) = run_audit(fixture);
    let v3_5_count = count_violations_containing(&stderr, "v3-5 violation");
    assert_eq!(
        v3_5_count, 0,
        "Expected 0 'v3-5 violation' messages on no-pseudocode fixture, \
         got {} (stderr=\n{})",
        v3_5_count, stderr
    );
}

// ---------------------------------------------------------------------------
// T1-5: verify_dispatch_tree_axis_tuple_consistency (cell 7 / v4-1)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_dispatch_tree_axis_tuple_definition_match() {
    let fixture = "tests/fixtures/i_d_main/positive/dispatch_tree_axis_tuple_mismatch.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with axis-tuple mismatch, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v4_1_count = count_violations_containing(&stderr, "v4-1 violation");
    assert!(
        v4_1_count >= 1,
        "Expected at least 1 'v4-1 violation' message, got {} (stderr=\n{})",
        v4_1_count,
        stderr
    );
    assert!(
        stderr.contains("axis-tuple") && stderr.contains("cell 3"),
        "Expected semantic violation message identifying cell 3 axis-tuple \
         fall-through, stderr=\n{}",
        stderr
    );
    assert!(
        stderr.contains("Qux") && stderr.contains("Bar"),
        "Expected violation message to identify (Qux, Bar) axis-tuple of cell 3, \
         stderr=\n{}",
        stderr
    );
}

#[test]
fn test_audit_dispatch_tree_count_based_fallback_detects_underprovisioned_arms() {
    // T1-5 count-based fallback path coverage (= /check_problem G-1 由来):
    // Matrix header に Axis columns 不在 → semantic verify path disabled →
    // count-based fallback で active_cells > explicit_arms + has_wildcard violation detect
    let fixture = "tests/fixtures/i_d_main/positive/dispatch_tree_count_based_fallback.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for count-based fallback under-provisioned arms, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v4_1_count = count_violations_containing(&stderr, "v4-1 violation");
    assert!(
        v4_1_count >= 1,
        "Expected at least 1 'v4-1 violation' message in count-based fallback path, \
         got {} (stderr=\n{})",
        v4_1_count,
        stderr
    );
    assert!(
        stderr.contains("count-based fallback"),
        "Expected violation message to identify count-based fallback path, \
         stderr=\n{}",
        stderr
    );
    assert!(
        stderr.contains("3 active") && stderr.contains("2 explicit"),
        "Expected violation message to identify '3 active cells' vs '2 explicit arms', \
         stderr=\n{}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// T1-6: verify_dispatch_arm_mapping_table (cell 9 / v4-3)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_dispatch_arm_mapping_completeness_one_to_one() {
    let fixture = "tests/fixtures/i_d_main/positive/dispatch_arm_mapping_incomplete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with incomplete mapping table, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v4_3_count = count_violations_containing(&stderr, "v4-3 violation");
    assert!(
        v4_3_count >= 1,
        "Expected at least 1 'v4-3 violation' message, got {} (stderr=\n{})",
        v4_3_count,
        stderr
    );
    assert!(
        stderr.contains("[3]") || stderr.contains("matrix cells [3]"),
        "Expected violation message to identify missing cell [3], stderr=\n{}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// T1-8: verify_pseudocode_underscore_arm_self_applied (cell 12 / v6-1)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_pseudocode_predicate_underscore_arm_compliance() {
    let fixture = "tests/fixtures/i_d_main/positive/pseudocode_underscore_arm.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with `_` arm in pseudocode, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v6_1_count = count_violations_containing(&stderr, "v6-1 violation");
    assert!(
        v6_1_count >= 1,
        "Expected at least 1 'v6-1 violation' message, got {} (stderr=\n{})",
        v6_1_count,
        stderr
    );
    assert!(
        stderr.contains("`_` arm"),
        "Expected violation message to identify `_` arm pattern, stderr=\n{}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// T1-9: verify_invariant_cell_coverage_double_partition (cell 13 / v6-2 part)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_invariant_double_partition_coverage() {
    let fixture = "tests/fixtures/i_d_main/positive/invariant_cell_coverage_inconsistent.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with INV claim inconsistency, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v6_2_count = count_violations_containing(&stderr, "v6-2 violation");
    assert!(
        v6_2_count >= 1,
        "Expected at least 1 'v6-2 violation' message, got {} (stderr=\n{})",
        v6_2_count,
        stderr
    );
    assert!(
        stderr.contains("'全 5'") && stderr.contains("actual matrix has 3"),
        "Expected violation message to identify claim/actual mismatch, stderr=\n{}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// T1-11: verify_pending_verdict_severity_default (cell 20 / v11-8)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_pending_verdict_severity_default() {
    let fixture = "tests/fixtures/i_d_main/positive/pending_verdict_severity_missing.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with pending verdict but no severity \
         declaration, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v11_8_count = count_violations_containing(&stderr, "v11-8 violation");
    assert!(
        v11_8_count >= 1,
        "Expected at least 1 'v11-8 violation' message, got {} (stderr=\n{})",
        v11_8_count,
        stderr
    );
    assert!(
        stderr.contains("severity") && stderr.contains("Critical"),
        "Expected violation message to mention severity Critical default, stderr=\n{}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// T1-12: verify_completion_criteria_probe_pattern (cell 26 / v13-1 audit part)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_completion_criteria_probe_pattern() {
    let fixture = "tests/fixtures/i_d_main/positive/completion_criteria_no_probe.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with manual-only criteria, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v13_1_count = count_violations_containing(&stderr, "v13-1 violation");
    assert!(
        v13_1_count >= 2,
        "Expected at least 2 'v13-1 violation' messages (2 criteria w/o probe), \
         got {} (stderr=\n{})",
        v13_1_count,
        stderr
    );
    assert!(
        stderr.contains("manual cross-check"),
        "Expected violation message to mention manual cross-check, stderr=\n{}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// T1-14: verify_fixture_oracle_byte_consistency (cell 29 / v13-6 audit part)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_fixture_oracle_byte_consistency() {
    let fixture = "tests/fixtures/i_d_main/positive/oracle_fixture_missing.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 1,
        "Expected audit FAIL (exit=1) for fixture with missing oracle TS file, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let v13_6_count = count_violations_containing(&stderr, "v13-6 violation");
    assert!(
        v13_6_count >= 1,
        "Expected at least 1 'v13-6 violation' message, got {} (stderr=\n{})",
        v13_6_count,
        stderr
    );
    assert!(
        stderr.contains("nonexistent"),
        "Expected violation message to identify nonexistent fixture path, stderr=\n{}",
        stderr
    );
}

#[test]
fn test_audit_cartesian_completeness_passes_with_documented_gaps() {
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS (exit=0) for fixture with documented gap allow-list, \
         got exit={}, stderr=\n{}",
        exit, stderr
    );
    let cp_violation_count =
        count_violations_containing(&stderr, "Cartesian product completeness violation");
    assert_eq!(
        cp_violation_count, 0,
        "Expected 0 'Cartesian product completeness violation' messages, \
         got {} (stderr=\n{})",
        cp_violation_count, stderr
    );
}

// ---------------------------------------------------------------------------
// /check_job Round 1 Action Item #1 + Round 2 R2-3 (rename for semantic clarity):
// Function-specific early return PASS path tests using cartesian_complete.md
// (= Option α gate pass、function-specific section absent → early return PASS)
//
// 各 test は cartesian_complete.md (= `## Cell Numbering Convention` 含むため Option α
// gate を pass) を使用、ただし fixture 内に各 audit function の specific section
// (pseudocode / "全 N cells" wording / Pending verdict wording / Completion Criteria
// section / Oracle fixture path references) が **不在** のため audit function は
// **function-specific early return** で PASS。
//
// 注: T1-6 verify_dispatch_arm_mapping_table は cartesian_complete.md が Spec→Impl
// Mapping section を持つため **specific PASS path** (= mapping present + cells
// covered) を verify。boundary verify でなく semantic specific verify。
// ---------------------------------------------------------------------------

#[test]
fn test_audit_no_v4_1_false_positive_on_pseudocode_absent() {
    // T1-5 boundary: Rust pseudocode 不在 → early return PASS
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on pseudocode-absent fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v4-1 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v4-1 violation' messages on pseudocode-absent fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v4_3_false_positive_on_complete_mapping() {
    // T1-6 specific PASS path: Spec→Impl Mapping section present + matrix cells
    // fully covered in mapping table → audit logic run + no violation
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on complete-mapping fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v4-3 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v4-3 violation' messages on complete-mapping fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v6_1_false_positive_on_pseudocode_absent() {
    // T1-8 boundary: Rust pseudocode 不在 → early return PASS
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on pseudocode-absent fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v6-1 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v6-1 violation' messages on pseudocode-absent fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v6_2_false_positive_on_no_claim_wording() {
    // T1-9 boundary: INV body 内 "全 N cells/candidates/variants" claim 不在 →
    // claim_pattern 不 match → loop skip PASS
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on no-claim-wording fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v6-2 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v6-2 violation' messages on no-claim-wording fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v11_8_false_positive_on_no_pending_verdict() {
    // T1-11 boundary: Iteration entry "Pending verdict N>0" wording 不在 →
    // pv_pattern 不 match → loop skip PASS
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on no-pending-verdict fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v11-8 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v11-8 violation' messages on no-pending-verdict fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v13_1_false_positive_on_no_completion_criteria() {
    // T1-12 boundary: `## Completion Criteria` section 不在 → early return PASS
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on no-completion-criteria fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v13-1 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v13-1 violation' messages on no-completion-criteria fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v13_6_false_positive_on_no_fixture_paths() {
    // T1-14 boundary: Oracle Observations section 内 fixture path references 不在 →
    // fixture_path_pattern 不 match → loop skip PASS
    let fixture = "tests/fixtures/i_d_main/negative/cartesian_complete.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on no-fixture-paths fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v13-6 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v13-6 violation' messages on no-fixture-paths fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

// ---------------------------------------------------------------------------
// /check_job Round 2 Action Item R2-1: Specific PASS path tests with distinct
// fixtures (= section present + actual logic runs + no violation pattern)
//
// 7 distinct synthetic fixtures、各 audit function の actual logic execution
// path で no violation PASS を verify (= early return boundary とは別の
// **semantic logic correctness** coverage)。
// ---------------------------------------------------------------------------

#[test]
fn test_audit_no_v3_5_false_positive_on_distinct_arms() {
    // T1-3 specific PASS: pseudocode present + all match arms distinct (no duplicates)
    let fixture = "tests/fixtures/i_d_main/negative/dispatch_tree_no_duplicate_arms.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on distinct-arms fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v3-5 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v3-5 violation' messages on distinct-arms fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v4_1_false_positive_on_full_axis_coverage() {
    // T1-5 specific PASS: pseudocode present + all matrix axis-tuples covered by arms
    let fixture = "tests/fixtures/i_d_main/negative/dispatch_tree_axis_tuple_full_coverage.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on full-axis-coverage fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v4-1 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v4-1 violation' messages on full-axis-coverage fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v6_1_false_positive_on_no_underscore_arm() {
    // T1-8 specific PASS: pseudocode present + no `_` arm (exhaustive enumeration)
    let fixture = "tests/fixtures/i_d_main/negative/pseudocode_no_underscore_arm.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on no-underscore-arm fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v6-1 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v6-1 violation' messages on no-underscore-arm fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v6_2_false_positive_on_matching_claim() {
    // T1-9 specific PASS: INV "全 N cells" claim present + N matches actual matrix cells
    let fixture = "tests/fixtures/i_d_main/negative/invariant_matching_claim.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on matching-claim fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v6-2 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v6-2 violation' messages on matching-claim fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v11_8_false_positive_on_severity_declared() {
    // T1-11 specific PASS: Pending verdict N>0 present + severity Critical declaration present
    let fixture = "tests/fixtures/i_d_main/negative/pending_verdict_with_severity.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on severity-declared fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v11-8 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v11-8 violation' messages on severity-declared fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v13_1_false_positive_on_probe_present() {
    // T1-12 specific PASS: Completion Criteria section present + probe pattern in each criterion
    let fixture = "tests/fixtures/i_d_main/negative/completion_criteria_with_probe.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on probe-present fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v13-1 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v13-1 violation' messages on probe-present fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

#[test]
fn test_audit_no_v13_6_false_positive_on_existing_fixture() {
    // T1-14 specific PASS: Oracle Observations references existing TS fixture path
    let fixture = "tests/fixtures/i_d_main/negative/oracle_fixture_existing.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS on existing-fixture-path fixture, got exit={}, stderr=\n{}",
        exit, stderr
    );
    let count = count_violations_containing(&stderr, "v13-6 violation");
    assert_eq!(
        count, 0,
        "Expected 0 'v13-6 violation' messages on existing-fixture-path fixture, \
         got {} (stderr=\n{})",
        count, stderr
    );
}

// ---------------------------------------------------------------------------
// /check_job Action Item #2: Option α auto-detect gate direct test
// (= `## Cell Numbering Convention` section 不在 fixture で全 NEW audit
//   functions が gate で skip = audit out-of-scope 自動分類 を verify)
// ---------------------------------------------------------------------------

#[test]
fn test_audit_option_alpha_gate_skips_pre_compliance_prds() {
    let fixture = "tests/fixtures/i_d_main/negative/option_alpha_gate_skips_pre_compliance.md";
    let (exit, stderr) = run_audit(fixture);
    assert_eq!(
        exit, 0,
        "Expected audit PASS (exit=0) on pre-compliance fixture without \
         `## Cell Numbering Convention` section, got exit={}, stderr=\n{}",
        exit, stderr
    );
    // Verify NO new audit function fired (= 全 10 が Option α gate で skip)
    for violation_id in &[
        "Cartesian product completeness violation",
        "v3-4 violation",
        "v3-5 violation",
        "v4-1 violation",
        "v4-3 violation",
        "v6-1 violation",
        "v6-2 violation",
        "v11-8 violation",
        "v13-1 violation",
        "v13-6 violation",
    ] {
        let count = count_violations_containing(&stderr, violation_id);
        assert_eq!(
            count, 0,
            "Expected 0 '{}' on pre-compliance fixture (Option α gate skip), \
             got {} (stderr=\n{})",
            violation_id, count, stderr
        );
    }
}
