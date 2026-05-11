//! PRD I-D-pre Audit script extensions tests (= scripts/audit-prd-rule10-compliance.py
//! 新 verify functions の synthetic PRD doc fixture-based positive + negative tests)。
//!
//! **Stub state (2026-05-11 post Spec stage v1)**: Implementation Phase 3 (T1-pre-1 +
//! T1-pre-2 + T1-pre-4) で fill in 予定、`#[ignore]` 状態維持。
//!
//! Test structure: 各 audit function に対し
//! - **Positive test**: synthetic PRD doc fixture (= 故意に違反 pattern 含む) で
//!   audit function が detect する (= violation list 非空)
//! - **Negative test**: synthetic PRD doc fixture (= 違反 pattern なし) で
//!   audit function PASS (= violation list 空)
//!
//! Synthetic fixtures = `tests/fixtures/i_d_pre/{positive,negative}/*.md`
//!
//! 各 test fn name は backlog/I-D-pre-audit-mechanism-bootstrap.md
//! `## Spec→Impl Dispatch Arm Mapping` table の Test contract path と 1-to-1 sync。

use std::process::Command;

/// Helper: run audit-prd-rule10-compliance.py on a fixture path, return (exit_code, stderr).
fn run_audit(fixture: &str) -> (i32, String) {
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

/// Helper: count audit violations for a specific T1-pre task ID in stderr.
fn count_violations(stderr: &str, t_id: &str) -> usize {
    let pattern = format!("{} violation", t_id);
    stderr.matches(&pattern).count()
}

/// Helper: run Path E utility on a fixture, return total drift count from stdout.
/// Used for dual-verify tests confirming fixture violation pattern presence.
fn path_e_total_drifts(fixture: &str) -> usize {
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
    // Parse line: "Total drifts (...): N"
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("Total drifts") {
            // Extract trailing integer after final ":"
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

/// Dual verify (Option α skip correctness empirical proof): `audit_out_of_scope_skip`
/// fixture が "3 violation patterns 含む" claim を **Path E utility 経由で direct verify**。
/// `test_audit_out_of_scope_skip_on_missing_cell_numbering_convention` (= audit script
/// 経由 0 violations) と組み合わせて **dual verify** が成立、Option α auto-detect skip
/// の correctness が empirical 証拠で 2 方向から lock-in される。
///
/// **設計意図 (/check_job deep deep review 由来)**: 単体 audit script 経由 PASS test だけでは
/// fixture が "本当に violation patterns を含むか" の direct evidence が弱い (= fixture
/// authoring mistake で patterns が欠落していたら、Option α skip の証拠が circular = silent
/// miss possible)。Path E utility 経由で fixture が flag されることを別途 verify することで、
/// Option α auto-detect の correctness が 2 independent observation で structurally locked。
///
/// **Expected**: Path E utility で 3 drifts (Axis 1 cross-reference + Axis 2 pending
/// verdict + Axis 3 cell-slot vocabulary fork) が flag される。Axis 4 (external file
/// drift) は fixture が synthetic (= 実 file reference なし) のため 0 drifts。
#[test]
fn test_out_of_scope_fixture_violation_patterns_present_via_path_e() {
    let total = path_e_total_drifts("tests/fixtures/i_d_pre/negative/audit_out_of_scope_skip.md");
    assert_eq!(
        total, 3,
        "audit_out_of_scope_skip fixture must trigger exactly 3 Path E drifts \
         (Axis 1 cross-reference + Axis 2 pending verdict + Axis 3 cell-slot vocabulary fork) \
         = empirical proof that fixture contains 3 violation patterns; got {} drifts",
        total
    );
}

/// Option α auto-detect helper test: I-205-like PRD pattern (= `## Cell Numbering
/// Convention` section 不在) で 3 NEW verify functions が全て early-return = skip
/// される動作を verify (= `has_cell_numbering_convention_section() == False` branch
/// の C1 branch coverage)。
///
/// **Fixture design**: `audit_out_of_scope_skip.md` は **3 violation patterns 全てを
/// 意図的に含む**:
/// - T1-pre-1 trigger candidate: TS-pre-3 Status field に post-v15 wording (v17/v18)
/// - T1-pre-2 trigger candidate: Scope full enumeration で matrix cell 1 omit
/// - T1-pre-4 trigger candidate: "cell-slot 1" identifier-level vocabulary fork
///
/// **Expected behavior**: `## Cell Numbering Convention` section が **不在** =
/// `has_cell_numbering_convention_section() == False` = 3 functions 全て早期 return
/// = audit script PASS (exit 0、0 T1-pre violations)。
///
/// **Why this test matters (testing.md C1 branch coverage compliance)**: 既存
/// positive/negative fixtures 6 件は全て `## Cell Numbering Convention` section を
/// **含む** = helper True branch のみ test。本 test 無しでは False branch が dead
/// code state で latent bug (= helper が常に True を返す regression) を silent miss。
///
/// **Symmetric counterpart**: I-205 PRD doc の retroactive update (= 案 γ Phase 2 T15、
/// TODO `[I-205-retroactive-cell-numbering-section]`) で section 追加 = audit scope 内
/// 自動 promote = future-proof design の structural lock-in test。
#[test]
fn test_audit_out_of_scope_skip_on_missing_cell_numbering_convention() {
    let (exit, stderr) = run_audit("tests/fixtures/i_d_pre/negative/audit_out_of_scope_skip.md");
    assert_eq!(
        exit, 0,
        "I-205-like fixture (no `## Cell Numbering Convention` section) must pass \
         audit despite containing 3 violation patterns (= Option α auto-detect skip); \
         stderr:\n{}",
        stderr
    );
    // All 3 NEW verify functions must early-return = no T1-pre-* violations emitted.
    assert_eq!(
        count_violations(&stderr, "T1-pre-1"),
        0,
        "T1-pre-1 must be skipped on missing `## Cell Numbering Convention` section; stderr:\n{}",
        stderr
    );
    assert_eq!(
        count_violations(&stderr, "T1-pre-2"),
        0,
        "T1-pre-2 must be skipped on missing `## Cell Numbering Convention` section; stderr:\n{}",
        stderr
    );
    assert_eq!(
        count_violations(&stderr, "T1-pre-4"),
        0,
        "T1-pre-4 must be skipped on missing `## Cell Numbering Convention` section; stderr:\n{}",
        stderr
    );
}

/// Cell 1 / v3-6+v4-2 / T1-pre-1: verify_pending_verdict_findings_consistency
/// consolidated audit function (= F7 fix integrated)
///
/// **Positive (violation pattern 検出)**:
/// - synthetic PRD `pending_verdict_violation.md` = TS-X heading 内 Status field に
///   post-v15 wording (= "Iteration v17 期待" / "post-v16 wording" 等) 残存 → flag detect
///
/// **Negative (violation 不在 PASS)**:
/// - synthetic PRD `pending_verdict_clean.md` = Status field に post-v15 wording 不在
#[test]
fn test_audit_pending_verdict_count_consistency() {
    let (pos_exit, pos_stderr) =
        run_audit("tests/fixtures/i_d_pre/positive/pending_verdict_violation.md");
    assert_eq!(
        pos_exit, 1,
        "positive fixture should fail audit; stderr:\n{}",
        pos_stderr
    );
    let pos_count = count_violations(&pos_stderr, "T1-pre-1");
    assert_eq!(
        pos_count, 2,
        "positive fixture should trigger exactly 2 T1-pre-1 violations \
         (TS-pre-3 v17 期待 + TS-pre-4 post-v16 wording); stderr:\n{}",
        pos_stderr
    );
    assert!(
        pos_stderr.contains("pending verdict"),
        "positive stderr must contain 'pending verdict' source label; stderr:\n{}",
        pos_stderr
    );

    let (neg_exit, neg_stderr) =
        run_audit("tests/fixtures/i_d_pre/negative/pending_verdict_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative fixture should pass audit; stderr:\n{}",
        neg_stderr
    );
    assert_eq!(
        count_violations(&neg_stderr, "T1-pre-1"),
        0,
        "negative fixture must trigger 0 T1-pre-1 violations; stderr:\n{}",
        neg_stderr
    );
}

/// Cell 1 / v4-2 / T1-pre-1 part 2: Critical=0 claim ↔ stale verdict consistency check
/// (= 同 consolidated function 内 sub-check)
///
/// **Positive**: TS-X heading 内 Status field に post-v15 wording (= late-stage stale
/// claim) 残存 = F7 fix integrated check で flag。Same fixture as primary check.
///
/// **Negative**: post-v15 wording 不在 = no flag.
#[test]
fn test_audit_critical0_claim_stale_verdict_inconsistency() {
    let (pos_exit, pos_stderr) =
        run_audit("tests/fixtures/i_d_pre/positive/pending_verdict_violation.md");
    assert_eq!(
        pos_exit, 1,
        "positive fixture should fail audit; stderr:\n{}",
        pos_stderr
    );
    // F7 fix integrated sub-check: stderr must explicitly cite "post-v15 wording"
    // marker (= TS-X late-stage stale claim detection signature).
    assert!(
        pos_stderr.contains("post-v15 wording"),
        "positive stderr must cite F7 fix 'post-v15 wording' marker for Critical=0 \
         stale verdict consistency; stderr:\n{}",
        pos_stderr
    );

    let (neg_exit, neg_stderr) =
        run_audit("tests/fixtures/i_d_pre/negative/pending_verdict_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative fixture should pass audit (no stale claim); stderr:\n{}",
        neg_stderr
    );
    assert!(
        !neg_stderr.contains("post-v15 wording"),
        "negative stderr must not contain 'post-v15 wording' marker; stderr:\n{}",
        neg_stderr
    );
}

/// Cell 2 / v5-1 / T1-pre-2: verify_cross_reference_cell_consistency audit function
/// (= F6 fix integrated = allow-list 置換)
///
/// **Positive**: Scope (policy=full) section omits matrix cell → audit script side mirror
///   flags missing cells → exit 1 with "T1-pre-2 violation (cross-reference cell)"
///
/// **Negative**: matrix と cross-ref contexts で 全 cells appearance consistency PASS
#[test]
fn test_audit_cross_reference_cell_appearance_consistency() {
    let (pos_exit, pos_stderr) =
        run_audit("tests/fixtures/i_d_pre/positive/cross_reference_violation.md");
    assert_eq!(
        pos_exit, 1,
        "positive fixture should fail audit; stderr:\n{}",
        pos_stderr
    );
    let pos_count = count_violations(&pos_stderr, "T1-pre-2");
    assert!(
        pos_count >= 1,
        "positive fixture should trigger >=1 T1-pre-2 violation; got {}, stderr:\n{}",
        pos_count,
        pos_stderr
    );
    assert!(
        pos_stderr.contains("cross-reference cell"),
        "positive stderr must contain 'cross-reference cell' source label; stderr:\n{}",
        pos_stderr
    );

    let (neg_exit, neg_stderr) =
        run_audit("tests/fixtures/i_d_pre/negative/cross_reference_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative fixture should pass audit; stderr:\n{}",
        neg_stderr
    );
    assert_eq!(
        count_violations(&neg_stderr, "T1-pre-2"),
        0,
        "negative fixture must trigger 0 T1-pre-2 violations; stderr:\n{}",
        neg_stderr
    );
}

/// Cell 5 / v13-5 / T1-pre-4: verify_cell_numbering_drift_detection audit function
///
/// **Positive**: cell-slot vocabulary fork (= "cell-slot N" identifier-level fork) 残存
///   → audit script side mirror flags namespace collision → exit 1 with "T1-pre-4
///   violation (cell numbering drift)"
///
/// **Negative**: vocabulary fork + namespace collision 不在 PASS
#[test]
fn test_audit_cell_numbering_drift_detection() {
    let (pos_exit, pos_stderr) =
        run_audit("tests/fixtures/i_d_pre/positive/cell_numbering_violation.md");
    assert_eq!(
        pos_exit, 1,
        "positive fixture should fail audit; stderr:\n{}",
        pos_stderr
    );
    let pos_count = count_violations(&pos_stderr, "T1-pre-4");
    assert!(
        pos_count >= 1,
        "positive fixture should trigger >=1 T1-pre-4 violation; got {}, stderr:\n{}",
        pos_count,
        pos_stderr
    );
    assert!(
        pos_stderr.contains("cell numbering drift"),
        "positive stderr must contain 'cell numbering drift' source label; stderr:\n{}",
        pos_stderr
    );

    let (neg_exit, neg_stderr) =
        run_audit("tests/fixtures/i_d_pre/negative/cell_numbering_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative fixture should pass audit; stderr:\n{}",
        neg_stderr
    );
    assert_eq!(
        count_violations(&neg_stderr, "T1-pre-4"),
        0,
        "negative fixture must trigger 0 T1-pre-4 violations; stderr:\n{}",
        neg_stderr
    );
}
