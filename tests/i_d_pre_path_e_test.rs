//! PRD I-D-pre Path E utility tests (= scripts/verify_prd_self_audits.py own behavior
//! auto-verify、Cells 1+2+5 audit utility part、4 axes + F6/F7 fix + Axis 3 extension、
//! framework v1.9 Axes 5/6/7 拡張 PRD I-D-main Iteration v20 (= wording staleness
//! detection class structural absorption)。
//!
//! **Functional regression test lock-in state (post 2026-05-15 PRD I-D-main /check_job
//! L1-1 fix)**: 11 functional tests active / 0 ignored = Axes 1-4 既存 functional tests
//! (= I-D-pre close で functional 化済) + Axes 5/6/7 framework v1.9 functional tests
//! (= PRD I-D-main Iteration v20 cohesive batch で implementation、本 /check_job L1-1
//! fix で stub `#[ignore]` → functional 置換 + synthetic fixtures 追加完了) + 1 metadata
//! header verify test = 11 total。
//!
//! Test structure: utility own behavior auto-verify (= regression-protected lock-in)
//! - **7 axes × (positive + negative) = 14 fixture-based tests** + 1 metadata test = 15 total
//!   - Axes 1-4 = I-D-pre Cells 1+2+5 (formal lock-in、各 axis 1 positive+negative pair test)
//!   - Axes 5-7 = framework v1.9 (PRD I-D-main Iteration v20 = wording staleness
//!     detection structural prevention、9-round recurring class absorption、各 axis
//!     1 positive+negative pair test)
//!   - Note: 各 axis test fn 内で positive + negative 両 fixture を run (= 1 #[test] per axis、
//!     合計 7 + 4 = 11 implemented active)
//! - **F6 fix verify**: Axis 1 allow-list 動作 (= Scope partition exception flag suppress
//!   + その他 missing cells flag)
//! - **F7 fix verify**: Axis 2 post-v15 wording presence 検出 (= TS-X heading 内でも
//!   v15+ wording なら flag = blanket exclude 解消)
//! - **Axis 3 extension verify**: cell-slot vocabulary fork drift detection
//! - **Axis 5 verify (framework v1.9)**: matrix count claim consistency (= "N cells /
//!   candidates / variants / rows" wording vs actual matrix size + historical allowance)
//! - **Axis 6 verify (framework v1.9)**: baseline LOC claim cross-section consistency
//!   (= Design section "<file>: N 行" wording vs actual wc -l)
//! - **Axis 7 verify (framework v1.9)**: cross-cutting Layer symmetry (= Layer 1/2/3/4
//!   cross-cutting cells enumeration vs computed Layer membership graph)
//!
//! **Empirical validation lock-in (Iteration v20→v22)**: framework v1.9 Axes 5/6/7 を PRD
//! I-D-main に対し pre-fix (15 drifts detected = F1/F2/F5 v19 finding direct cover) →
//! post-fix (0 drifts on all 7 axes) で empirical 動作 confirm 済 + Iteration v22 で
//! Axes 5/6/7 refinement (= F4 dual-form regex / F5 Japanese variants / F7 annotated
//! heading regex) で false-negative class 排除。**本 /check_job L1-1 fix で functional
//! test contract 完成** = future Path E utility modification で silent regression 即時 detect。
//!
//! Synthetic fixtures (= `tests/fixtures/i_d_pre/{positive,negative}/path_e_*.md`):
//! - Axes 1-4: I-D-pre close で existing
//! - **Axes 5/6/7: 本 /check_job L1-1 fix で新設**:
//!   * `tests/fixtures/i_d_pre/positive/path_e_axis{5,6,7}_*_violation.md`
//!   * `tests/fixtures/i_d_pre/negative/path_e_axis{5,6,7}_clean.md`
//!   * `tests/fixtures/i_d_pre/axis6_loc_reference.md` (= Axis 6 LOC reference stable 5 行 file)

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
        // L1-9 fix (Round 2 /check_problem): framework v1.9 era markers
        // (= PRD I-D-main Iteration v20 cohesive batch で Axes 5/6/7 absorbed、本 metadata
        // が silent regression で削除されない structural lock-in)
        "framework v1.9",
        "Axes 5/6/7",
        "wording staleness",
        "PRD I-D-main Iteration v20",
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

// ============================================================================
// framework v1.9 Axes 5/6/7 functional regression tests
// (PRD I-D-main Iteration v20 cohesive batch で implementation 完成、本 /check_job L1-1
// findings 由来 Iteration v22+ で stub → functional 置換 = `tests/fixtures/i_d_pre/
// {positive,negative}/path_e_axis{5,6,7}_*.md` synthetic fixtures で regression
// lock-in。Empirical lock-in 完了 = future Path E utility modification で silent
// regression 即時 detection mechanism active)
// ============================================================================

/// Axis 5 (framework v1.9) positive test: matrix count claim drift detection
/// Synthetic PRD has matrix table heading `(5 cells)` + body wording "5 candidates"
/// (in 2 distinct lines) but actual active cells = 3 (with 2 MIGRATED rows) + no
/// historical allowance context
/// → Expected: Axis 5 detects exactly 3 drifts (heading + 2 body claims)
#[test]
fn test_path_e_axis5_matrix_count_claim_drift_detected() {
    let (exit, stdout) =
        run_path_e("tests/fixtures/i_d_pre/positive/path_e_axis5_matrix_count_violation.md");
    let count = axis_drift_count(&stdout, 5);
    // L1-8 fix (Round 2 /check_problem): tighten from `count >= 1` to exact count
    // to lock-in the empirical detection behavior (= heading drift + 2 body drifts = 3 total)
    assert_eq!(
        count, 3,
        "Axis 5 expected exactly 3 drifts on positive fixture (heading + 2 body wording \
         claiming '5 cells/candidates' without historical allowance), got {}; \
         exit={}, stdout:\n{}",
        count, exit, stdout
    );
    // Verify specific drift content: heading-specific drift + body drifts
    assert!(
        stdout.contains("heading): line") && stdout.contains("'(5 cells)'"),
        "Axis 5 positive should detect heading drift '(5 cells)' specifically, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("'5 candidates'") && stdout.contains("active cells"),
        "Axis 5 positive should detect body drift '5 candidates' with active cells \
         reference, got:\n{}",
        stdout
    );
    // Also verify exit code reflects drift detection (= total > 0 → exit 1)
    assert_eq!(
        exit, 1,
        "expected exit code 1 (drifts detected), got {}",
        exit
    );
}

/// Axis 5 (framework v1.9) negative test: matching active count + historical allowance = no drift
/// Synthetic PRD has matrix heading `(3 cells)` matching active count + body uses
/// "3 candidates" (matches) + "I-D parent 5 cells から ... migration" (= legitimate
/// historical allowance context for residual 5 references)
/// → Expected: Axis 5 detects 0 drifts
#[test]
fn test_path_e_axis5_historical_allowance_no_drift() {
    let (_exit, stdout) = run_path_e("tests/fixtures/i_d_pre/negative/path_e_axis5_clean.md");
    let count = axis_drift_count(&stdout, 5);
    assert_eq!(
        count, 0,
        "Axis 5 expected 0 drifts on negative fixture (matching active count + \
         historical allowance keywords for I-D parent total), got {}; stdout:\n{}",
        count, stdout
    );
}

/// Axis 6 (framework v1.9) positive test: Design section LOC claim drift detection
/// Synthetic PRD Design section claims
/// `tests/fixtures/i_d_pre/axis6_loc_reference.md (999 行)` but actual `wc -l` = 5
/// → Expected: Axis 6 detects 1 drift (= 999 != 5)
#[test]
fn test_path_e_axis6_baseline_loc_claim_drift_detected() {
    let (exit, stdout) =
        run_path_e("tests/fixtures/i_d_pre/positive/path_e_axis6_loc_violation.md");
    let count = axis_drift_count(&stdout, 6);
    assert!(
        count >= 1,
        "Axis 6 expected ≥1 drift on positive fixture (Design section claims wrong \
         LOC for stable reference file), got {}; exit={}, stdout:\n{}",
        count,
        exit,
        stdout
    );
    assert_eq!(
        exit, 1,
        "expected exit code 1 (drifts detected), got {}",
        exit
    );
    // Verify the specific drift message contains expected file + claim/actual values
    assert!(
        stdout.contains("axis6_loc_reference.md")
            && stdout.contains("claims 999")
            && stdout.contains("actual 5"),
        "expected drift message to reference axis6_loc_reference.md with claim/actual \
         LOC values, got:\n{}",
        stdout
    );
}

/// Axis 6 (framework v1.9) negative test: matching LOC = no drift
/// Synthetic PRD Design section claims
/// `tests/fixtures/i_d_pre/axis6_loc_reference.md (5 行)` matching actual `wc -l`
/// → Expected: Axis 6 detects 0 drifts
#[test]
fn test_path_e_axis6_matching_loc_no_drift() {
    let (_exit, stdout) = run_path_e("tests/fixtures/i_d_pre/negative/path_e_axis6_clean.md");
    let count = axis_drift_count(&stdout, 6);
    assert_eq!(
        count, 0,
        "Axis 6 expected 0 drifts on negative fixture (Design section claim matches \
         actual wc -l), got {}; stdout:\n{}",
        count, stdout
    );
}

/// Axis 7 (framework v1.9) positive test: cross-cutting Layer symmetry violation
/// Synthetic PRD has Layer 1 main_cells = {1, 2} claiming `cell 5 = Layer 1+2` cross-cutting,
/// but cell 5 not in Layer 1's main cells AND not in Layer 2's main cells.
/// Layer 2 main_cells = {3, 4} claiming `cell 5 = Layer 2+3` but cell 5 not in Layer 2.
/// → Expected: Axis 7 detects ≥2 drifts (cell-membership inconsistency)
#[test]
fn test_path_e_axis7_cross_cutting_layer_asymmetry_detected() {
    let (exit, stdout) =
        run_path_e("tests/fixtures/i_d_pre/positive/path_e_axis7_layer_asymmetry_violation.md");
    let count = axis_drift_count(&stdout, 7);
    assert!(
        count >= 2,
        "Axis 7 expected ≥2 drifts on positive fixture (cross-cutting cell 5 \
         claim mismatch with Layer main cells), got {}; exit={}, stdout:\n{}",
        count,
        exit,
        stdout
    );
    assert_eq!(
        exit, 1,
        "expected exit code 1 (drifts detected), got {}",
        exit
    );
    // Verify specific drift content references cell 5 + Layer membership inconsistency
    assert!(
        stdout.contains("cell 5") && stdout.contains("Layer"),
        "expected drift message to reference cell 5 + Layer membership issue, got:\n{}",
        stdout
    );
}

/// Axis 7 (framework v1.9) negative test: symmetric cross-cutting claims = no drift
/// Synthetic PRD has Layer 1 main_cells = {1, 2, 3} + Layer 2 main_cells = {3, 4}
/// claiming `cell 3 = Layer 1+2` cross-cutting. Cell 3 IS in both Layer 1 and Layer 2.
/// → Expected: Axis 7 detects 0 drifts
#[test]
fn test_path_e_axis7_symmetric_cross_cutting_no_drift() {
    let (_exit, stdout) = run_path_e("tests/fixtures/i_d_pre/negative/path_e_axis7_clean.md");
    let count = axis_drift_count(&stdout, 7);
    assert_eq!(
        count, 0,
        "Axis 7 expected 0 drifts on negative fixture (symmetric Layer cross-cutting \
         pairings with main cells), got {}; stdout:\n{}",
        count, stdout
    );
}

// =========================================================================
// PRD I-D-c11: expand_cell_list filter upper-bound uniform `<= 99` consistency
//
// Resolution direction R2 採用: 全 4 Pattern (CELL_LIST_RE / CELL_STANDALONE_RE /
// CELL_BRACKET_LIST_RE / TABLE_FIRST_COL_NUM_RE) で filter `<= 99` uniform、regex
// `\d{1,2}` upper-bound (= 99) と integral consistency 確立。
//
// INV-1 (filter uniform), INV-3 (cells 1-30 preservation), INV-4 (cells 31-99
// P2/P4 detection = bug fix outcome), INV-5 (cells 100+ structural rejection),
// INV-6 (R5 negative pattern-asymmetric preservation) を lock-in。
// =========================================================================

/// Helper: invoke `expand_cell_list` function directly via Python inline script + stdin pipe.
/// Returns sorted Vec<u32> of detected cells.
fn invoke_expand_cell_list(input: &str) -> Vec<u32> {
    use std::io::Write;
    let python_script = r#"
import sys
sys.path.insert(0, 'scripts')
import importlib.util
spec = importlib.util.spec_from_file_location('vpsa', 'scripts/verify_prd_self_audits.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
text = sys.stdin.read()
cells = m.expand_cell_list(text)
print(','.join(str(n) for n in sorted(cells)))
"#;
    let mut cmd = Command::new("python3")
        .args(["-c", python_script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn python3 for expand_cell_list");
    {
        let stdin = cmd.stdin.as_mut().expect("stdin pipe");
        stdin
            .write_all(input.as_bytes())
            .expect("failed to write stdin");
    }
    let output = cmd.wait_with_output().expect("python3 wait failure");
    assert!(
        output.status.success(),
        "python3 expand_cell_list invocation failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("non-utf-8 stdout");
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    trimmed
        .split(',')
        .map(|s| s.parse::<u32>().expect("u32 parse"))
        .collect()
}

/// Helper: invoke a specific regex pattern from verify_prd_self_audits.py + filter `<= 99`.
/// Used for INV-6 individual regex test (P2 = CELL_STANDALONE_RE / P4 = TABLE_FIRST_COL_NUM_RE).
fn invoke_pattern_regex(pattern_name: &str, input: &str) -> Vec<u32> {
    use std::io::Write;
    let python_script = format!(
        r#"
import sys
sys.path.insert(0, 'scripts')
import importlib.util
spec = importlib.util.spec_from_file_location('vpsa', 'scripts/verify_prd_self_audits.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
pattern = getattr(m, '{}')
text = sys.stdin.read()
cells = set()
for match in pattern.finditer(text):
    try:
        n = int(match.group(1))
        if 1 <= n <= 99:
            cells.add(n)
    except (ValueError, IndexError):
        pass
print(','.join(str(n) for n in sorted(cells)))
"#,
        pattern_name
    );
    let mut cmd = Command::new("python3")
        .args(["-c", &python_script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn python3 for pattern regex test");
    {
        let stdin = cmd.stdin.as_mut().expect("stdin pipe");
        stdin
            .write_all(input.as_bytes())
            .expect("failed to write stdin");
    }
    let output = cmd.wait_with_output().expect("python3 wait failure");
    assert!(
        output.status.success(),
        "python3 pattern regex test failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("non-utf-8 stdout");
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    trimmed
        .split(',')
        .map(|s| s.parse::<u32>().expect("u32 parse"))
        .collect()
}

/// PRD I-D-c11 Cell # 1, # 2 (P1 × R1+R2 behavior preservation、INV-3)
/// Pattern 1 (CELL_LIST_RE = "cells N, M, ..." lowercase form) で cells 1-99 全 detect、
/// Pre/Post PRD で effective behavior 不変 (filter `<= 999` → `<= 99` uniform 化)。
#[test]
fn test_path_e_id_c11_p1_preserves_cells_1_to_99() {
    let text =
        fs::read_to_string("tests/fixtures/i_d_pre/positive/path_e_expand_cell_list_p1_n_gt_30.md")
            .expect("read p1 fixture");
    let cells = invoke_expand_cell_list(&text);
    assert_eq!(
        cells,
        vec![5, 30, 31, 38, 70, 99],
        "P1 fixture cells 1-99 detection failed: expected {{5, 30, 31, 38, 70, 99}}, got {:?}",
        cells
    );
}

/// PRD I-D-c11 Cell # 6 + Cell # 7 (P2 × R1+R2、primary intent = INV-4 bug fix outcome)
/// Pattern 2 (CELL_STANDALONE_RE = "Cell N" capitalized standalone form) で cells 31-99
/// detection (= Pre-PRD silent skip からの bug fix transition)。
///
/// **Test name rationale**: `_detects_cells_31_to_99` で **primary intent (= INV-4 bug fix
/// outcome、cells 31-99 newly detected)** を強調。Assertion 内に cells 5, 30 (= Cell # 6 = R1
/// partition behavior preservation = INV-3) も含み、本 test で R1 + R2 両 partition の coverage
/// を 1-to-1 fixture-based assertion で達成。`_31_to_99` 名は test scope の literal range
/// ではなく architectural concern (= bug fix outcome) の semantic label。
#[test]
fn test_path_e_id_c11_p2_detects_cells_31_to_99() {
    let text =
        fs::read_to_string("tests/fixtures/i_d_pre/positive/path_e_expand_cell_list_p2_n_gt_30.md")
            .expect("read p2 fixture");
    let cells = invoke_expand_cell_list(&text);
    assert_eq!(
        cells,
        vec![5, 30, 31, 38, 70, 99],
        "P2 fixture cells 31-99 detection failed (bug fix outcome): expected {{5, 30, 31, 38, 70, 99}}, got {:?}",
        cells
    );
    // INV-4 explicit assert: cells 31, 38, 70, 99 (= newly detected via bug fix) must be present
    for newly_detected in [31u32, 38, 70, 99] {
        assert!(
            cells.contains(&newly_detected),
            "INV-4 bug fix outcome violated: cell {} should be newly detected via P2 filter `<= 30` → `<= 99` uniform 化, got cells={:?}",
            newly_detected, cells
        );
    }
}

/// PRD I-D-c11 Cell # 11, # 12 (P3 × R1+R2 behavior preservation、INV-3)
#[test]
fn test_path_e_id_c11_p3_preserves_cells_1_to_99() {
    let text =
        fs::read_to_string("tests/fixtures/i_d_pre/positive/path_e_expand_cell_list_p3_n_gt_30.md")
            .expect("read p3 fixture");
    let cells = invoke_expand_cell_list(&text);
    assert_eq!(
        cells,
        vec![5, 30, 31, 38, 70, 99],
        "P3 fixture cells 1-99 detection failed: expected {{5, 30, 31, 38, 70, 99}}, got {:?}",
        cells
    );
}

/// PRD I-D-c11 Cell # 16 + Cell # 17 (P4 × R1+R2、primary intent = INV-4 bug fix outcome)
/// Pattern 4 (TABLE_FIRST_COL_NUM_RE = markdown table first column `| N |`) で cells 31-99
/// detection (= Pre-PRD silent skip からの bug fix transition)。
///
/// **Test name rationale**: `_detects_cells_31_to_99` で **primary intent (= INV-4 bug fix
/// outcome)** 強調。Assertion 内に cells 5, 30 (= Cell # 16 = R1 partition behavior preservation
/// = INV-3) も含み、R1 + R2 両 partition coverage を 1-to-1 fixture-based assertion で達成。
#[test]
fn test_path_e_id_c11_p4_detects_cells_31_to_99() {
    let text =
        fs::read_to_string("tests/fixtures/i_d_pre/positive/path_e_expand_cell_list_p4_n_gt_30.md")
            .expect("read p4 fixture");
    let cells = invoke_expand_cell_list(&text);
    assert_eq!(
        cells,
        vec![5, 30, 31, 38, 70, 99],
        "P4 fixture cells 31-99 detection failed (bug fix outcome): expected {{5, 30, 31, 38, 70, 99}}, got {:?}",
        cells
    );
    // INV-4 explicit assert: cells 31, 38, 70, 99 (= newly detected via bug fix) must be present
    for newly_detected in [31u32, 38, 70, 99] {
        assert!(
            cells.contains(&newly_detected),
            "INV-4 bug fix outcome violated: cell {} should be newly detected via P4 filter `<= 30` → `<= 99` uniform 化, got cells={:?}",
            newly_detected, cells
        );
    }
}

/// PRD I-D-c11 Cell # 3, # 8, # 13, # 18 (R3 cells 100+ structural rejection、INV-5)
/// 4 Pattern × R3 fixtures で empty set return (regex `\d{1,2}` word boundary fail)。
#[test]
fn test_path_e_id_c11_rejects_3digit_numbers() {
    let fixtures = [
        "tests/fixtures/i_d_pre/negative/path_e_expand_cell_list_p1_overflow_100.md",
        "tests/fixtures/i_d_pre/negative/path_e_expand_cell_list_p2_overflow_100.md",
        "tests/fixtures/i_d_pre/negative/path_e_expand_cell_list_p3_overflow_100.md",
        "tests/fixtures/i_d_pre/negative/path_e_expand_cell_list_p4_overflow_100.md",
    ];
    for fixture in fixtures {
        let text = fs::read_to_string(fixture).expect("read overflow fixture");
        let cells = invoke_expand_cell_list(&text);
        assert!(
            cells.is_empty(),
            "INV-5 violated: fixture {} should return empty set (cells 100+ structural reject), got cells={:?}",
            fixture, cells
        );
    }
}

/// PRD I-D-c11 Cell # 4, # 9, # 14, # 19 (R4 cell # 0 filter lower-bound reject)
/// 全 4 Pattern で cell 0 を reject (filter `1 <= n`)。
#[test]
fn test_path_e_id_c11_rejects_zero() {
    let inputs = [
        ("P1", "cells 0"),
        ("P2", "Cell 0"),
        ("P3", "{0}"),
        ("P4", "| 0 | invalid |"),
    ];
    for (label, input) in inputs {
        let cells = invoke_expand_cell_list(input);
        assert!(
            cells.is_empty(),
            "R4 violated for {}: input '{}' should return empty set (cell 0 filter lower-bound reject), got cells={:?}",
            label, input, cells
        );
    }
}

/// PRD I-D-c11 Cell # 10, # 20 (P2/P4 × R5 individual regex digit-only reject、INV-6)
/// Pattern 2 (CELL_STANDALONE_RE) + Pattern 4 (TABLE_FIRST_COL_NUM_RE) を **individual
/// regex level** で test、negative sign を digit-only regex で reject。
#[test]
fn test_path_e_id_c11_p2_p4_reject_negative_individual() {
    // P2 individual regex (CELL_STANDALONE_RE)
    let p2_cells = invoke_pattern_regex("CELL_STANDALONE_RE", "Cell -5");
    assert!(
        p2_cells.is_empty(),
        "INV-6 violated for P2 individual regex: input 'Cell -5' should match nothing via CELL_STANDALONE_RE (digit-only after \\s+), got cells={:?}",
        p2_cells
    );
    // P4 individual regex (TABLE_FIRST_COL_NUM_RE)
    let p4_cells = invoke_pattern_regex("TABLE_FIRST_COL_NUM_RE", "| -5 | invalid |");
    assert!(
        p4_cells.is_empty(),
        "INV-6 violated for P4 individual regex: input '| -5 |' should match nothing via TABLE_FIRST_COL_NUM_RE (digit-only after `|\\s*`), got cells={:?}",
        p4_cells
    );
}

/// PRD I-D-c11 Cell # 5, # 15 (P1/P3 × R5 latent absorb behavior preservation、INV-6)
/// Pattern 1 (CELL_LIST_RE) + Pattern 3 (CELL_BRACKET_LIST_RE) body regex set
/// `[\d,\s\-–/]` で sign を range delimiter として absorb、後続 digit を cell # extract。
/// 本 PRD で behavior 不変 (= INV-2 regex unchanged)、Future structural hardening は
/// 別 TODO `[I-D-future-audit-extensions-hardening]` C14 候補。
#[test]
fn test_path_e_id_c11_p1_p3_latent_absorb_negative_sign() {
    // P1 latent absorb: "cells -5" → cells={5} (sign absorbed in body range delimiter set)
    let p1_cells = invoke_expand_cell_list("cells -5");
    assert_eq!(
        p1_cells,
        vec![5],
        "INV-6 P1 latent absorb behavior violated: input 'cells -5' should return cells={{5}} (sign absorbed via CELL_LIST_RE body set), got {:?}",
        p1_cells
    );
    // P3 latent absorb: "{-5}" → cells={5} (sign absorbed in bracket-list body set)
    let p3_cells = invoke_expand_cell_list("{-5}");
    assert_eq!(
        p3_cells,
        vec![5],
        "INV-6 P3 latent absorb behavior violated: input '{{-5}}' should return cells={{5}} (sign absorbed via CELL_BRACKET_LIST_RE body set), got {:?}",
        p3_cells
    );
}

/// PRD I-D-c11 INV-1 syntactic lock-in: source file scan で expand_cell_list 内 全 filter
/// expression が `<= 99` uniform。`scripts/verify_prd_self_audits.py` の `expand_cell_list`
/// 関数 body (def expand_cell_list ... return cells) を抽出、filter upper-bound literal を
/// extract し、全 occurrences で literal == 99 assert。
#[test]
fn test_path_e_id_c11_filter_uniform_99() {
    let source = fs::read_to_string("scripts/verify_prd_self_audits.py").expect("read source");

    // Extract expand_cell_list function body (from "def expand_cell_list" to next top-level def or end)
    let func_start = source
        .find("def expand_cell_list(")
        .expect("expand_cell_list function not found");
    let after_func = &source[func_start..];
    // Find end: next top-level "def " or end of file
    let func_end_rel = after_func[1..]
        .find("\ndef ")
        .map(|i| i + 1)
        .unwrap_or(after_func.len());
    let func_body = &after_func[..func_end_rel];

    // Extract all `if 1 <= ... <= N:` filter literals via manual string parse
    // (= regex crate not added to dev-dependencies、test self-contained ideal-clean lens)
    let mut filter_literals: Vec<String> = Vec::new();
    for line in func_body.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("if 1 <=") {
            continue;
        }
        // Find colon position (end of filter expression)
        let colon_pos = match trimmed.find(':') {
            Some(p) => p,
            None => continue,
        };
        let before_colon = &trimmed[..colon_pos];
        // Find LAST "<=" before colon (handles "if 1 <= lo <= hi <= 99" + "if 1 <= n <= 99")
        let last_le = match before_colon.rfind("<=") {
            Some(p) => p,
            None => continue,
        };
        let after_le = before_colon[last_le + 2..].trim();
        if after_le.parse::<u32>().is_ok() {
            filter_literals.push(after_le.to_string());
        }
    }

    assert_eq!(
        filter_literals.len(),
        6,
        "expected 6 filter expressions in expand_cell_list (4 Pattern × {{range + individual / single}} = 6), got {} (literals: {:?})",
        filter_literals.len(), filter_literals
    );
    for literal in &filter_literals {
        assert_eq!(
            literal, "99",
            "INV-1 syntactic lock-in violated: expand_cell_list filter literal expected '99' uniform, got '{}' (all literals: {:?})",
            literal, filter_literals
        );
    }
}
