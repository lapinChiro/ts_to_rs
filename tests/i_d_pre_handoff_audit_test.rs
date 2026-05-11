//! PRD I-D-pre Handoff audit script tests (= scripts/audit-handoff-doc-line-refs.py
//! NEW own behavior auto-verify、Cell 3 / v11-5)。
//!
//! **Filled-in state (2026-05-11 post Implementation Phase 4 T1-pre-3a)**: 2 tests
//! enabled, both PASS。Phase 3 で確立した subprocess invocation helper pattern
//! (= `tests/i_d_pre_audit_extensions_test.rs::run_audit` / `path_e_total_drifts`)
//! と同 pattern を `run_handoff_audit` helper として適用。
//!
//! Test structure:
//! - **Positive test (`test_audit_handoff_doc_line_refs_drift_detection`)**:
//!   `tests/fixtures/i_d_pre/positive/handoff_drift.md` で 5 drift patterns
//!   (= 3 drift categories × 2 path-resolution paths × 2 line-spec forms の
//!   dispatch combinations、L1-1 OOB-via-glob C1 coverage + L1-2 range partition
//!   coverage 含む) を全 detect、exit 1。
//!   `tests/fixtures/i_d_pre/negative/handoff_clean.md` で exit 0 + 0 drifts、
//!   5 refs (single + range form 両方含む = L1-2 negative side coverage)。
//! - **Standalone baseline (`test_audit_handoff_doc_line_refs_standalone_baseline`)**:
//!   `doc/handoff/` directory に対し audit run、PRD I-D-pre 完了時点での
//!   handoff doc baseline (= 5 ambiguous refs structural fix 後の **0 drifts** state)
//!   を frozen baseline として lock-in。Regression detection = future PR で
//!   handoff doc に drift 混入されたら本 test fail。
//!
//! Synthetic fixtures = `tests/fixtures/i_d_pre/{positive,negative}/handoff_*.md`

use std::process::Command;

/// Helper: run audit-handoff-doc-line-refs.py on a fixture path, return
/// (exit_code, stdout, stderr).
fn run_handoff_audit(fixture: &str) -> (i32, String, String) {
    let output = Command::new("python3")
        .arg("scripts/audit-handoff-doc-line-refs.py")
        .arg(fixture)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "python3 audit-handoff-doc-line-refs.py failed for {}: {}",
                fixture, e
            )
        });
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit = output.status.code().expect("exit code present");
    (exit, stdout, stderr)
}

/// Helper: parse "Total drifts: N" line from stdout.
fn parse_total_drifts(stdout: &str) -> usize {
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("Total drifts:") {
            if let Ok(n) = rest.trim().parse::<usize>() {
                return n;
            }
        }
    }
    panic!("could not parse 'Total drifts' line from stdout:\n{stdout}")
}

/// Cell 3 / v11-5 / T1-pre-3a: audit-handoff-doc-line-refs.py drift detection
/// (= positive fixture trigger 6 patterns + negative fixture clean、L1-1 + L1-2
/// coverage fix + /check_problem Issue #3 INVALID_RANGE integrated)
///
/// **Positive**: `tests/fixtures/i_d_pre/positive/handoff_drift.md` contains 6
/// intentional drift patterns (= 4 categories × dispatch combinations):
/// - Drift 1: OUT_OF_BOUNDS via as-is path (`src/lib.rs:999999`)
/// - Drift 2: MISSING_FILE (`src/nonexistent_handoff_audit_fixture.rs:1`)
/// - Drift 3: AMBIGUOUS (`mod.rs:1` multi-candidate bare basename)
/// - Drift 4 (L1-1 fix): OUT_OF_BOUNDS via glob fallback
///   (`audit-handoff-doc-line-refs.py:99999` = single glob candidate with
///   line < claim → exercise `if not in_bounds: True` branch)
/// - Drift 5 (L1-2 fix): OUT_OF_BOUNDS via range form
///   (`src/lib.rs:99990-99999` = range upper bound OOB)
/// - Drift 6 (/check_problem Issue #3 fix): INVALID_RANGE backwards-range typo
///   (`src/lib.rs:100-50` = start > end, silent failure mode → structural
///   detection via dedicated INVALID_RANGE category)
///
/// **Negative**: `tests/fixtures/i_d_pre/negative/handoff_clean.md` contains 5
/// refs all resolving to exactly one in-bounds file (Cargo.toml / README.md /
/// self-reference / single-candidate glob / range-form `src/lib.rs:1-10`) =
/// 0 drifts。
#[test]
fn test_audit_handoff_doc_line_refs_drift_detection() {
    let (pos_exit, pos_stdout, pos_stderr) =
        run_handoff_audit("tests/fixtures/i_d_pre/positive/handoff_drift.md");
    assert_eq!(
        pos_exit, 1,
        "positive fixture should fail audit (exit 1); stdout:\n{pos_stdout}\nstderr:\n{pos_stderr}"
    );
    assert_eq!(
        parse_total_drifts(&pos_stdout),
        6,
        "positive fixture must trigger exactly 6 drifts (= 4 categories × \
         path-resolution / line-spec sub-dispatches per L1-1 + L1-2 + \
         /check_problem #3 coverage); stdout:\n{pos_stdout}"
    );
    // Each drift category must appear in stderr (4 categories total).
    assert!(
        pos_stderr.contains("[OUT_OF_BOUNDS]"),
        "positive stderr must contain OUT_OF_BOUNDS category; stderr:\n{pos_stderr}"
    );
    assert!(
        pos_stderr.contains("[MISSING_FILE]"),
        "positive stderr must contain MISSING_FILE category; stderr:\n{pos_stderr}"
    );
    assert!(
        pos_stderr.contains("[AMBIGUOUS]"),
        "positive stderr must contain AMBIGUOUS category; stderr:\n{pos_stderr}"
    );
    assert!(
        pos_stderr.contains("[INVALID_RANGE]"),
        "/check_problem #3: positive stderr must contain INVALID_RANGE category; \
         stderr:\n{pos_stderr}"
    );
    // L1-1 OOB-via-glob C1 branch coverage: stderr must contain the "all N glob
    // candidate(s) below line M" wording specific to `if not in_bounds:` True
    // branch (= distinct from as-is OOB wording "as-is file has N lines")。
    assert!(
        pos_stderr.contains("glob candidate(s) below line"),
        "L1-1: positive stderr must contain OOB-via-glob branch detail \
         ('glob candidate(s) below line'); stderr:\n{pos_stderr}"
    );
    // L1-2 range-form partition coverage: stderr must contain range line_spec
    // notation `<start>-<end>` (= verifies regex range capture + classify_ref
    // line_spec format dispatch on `ref.end != ref.start`)。
    assert!(
        pos_stderr.contains(":99990-99999"),
        "L1-2: positive stderr must contain range-form line_spec '99990-99999'; \
         stderr:\n{pos_stderr}"
    );
    // /check_problem #3 INVALID_RANGE backwards-range detection: stderr must
    // cite "backwards range" + start/end values explicitly (= verifies
    // classify_ref invokes INVALID_RANGE branch before as_is path resolution)。
    assert!(
        pos_stderr.contains("backwards range"),
        "/check_problem #3: positive stderr must contain 'backwards range' \
         detail wording; stderr:\n{pos_stderr}"
    );

    let (neg_exit, neg_stdout, neg_stderr) =
        run_handoff_audit("tests/fixtures/i_d_pre/negative/handoff_clean.md");
    assert_eq!(
        neg_exit, 0,
        "negative fixture should pass audit (exit 0); stdout:\n{neg_stdout}\nstderr:\n{neg_stderr}"
    );
    assert_eq!(
        parse_total_drifts(&neg_stdout),
        0,
        "negative fixture must trigger 0 drifts; stdout:\n{neg_stdout}"
    );
    // L1-2 range-form negative coverage: negative fixture references 5 refs
    // including `src/lib.rs:1-10` range form。Sanity-check the count to
    // ensure range form ref is parsed (= regex capture works for negative path
    // resolution as well)。
    let line_refs = neg_stdout
        .lines()
        .find_map(|l| {
            l.strip_prefix("Line refs found:")
                .and_then(|r| r.trim().parse::<usize>().ok())
        })
        .expect("negative stdout must report 'Line refs found: N'");
    assert_eq!(
        line_refs, 5,
        "L1-2: negative fixture must contain exactly 5 refs (4 single + 1 range); \
         got {line_refs}; stdout:\n{neg_stdout}"
    );
}

/// Cell 3 / v11-5 / T1-pre-3a: standalone CLI invocation against `doc/handoff/`
/// directory baseline lock-in
///
/// **Verification**: PRD I-D-pre 完了時点で `doc/handoff/` 内全 `<file>:<line>`
/// cross-references は audit script で **0 drifts** PASS する state (= 5
/// pre-existing ambiguous refs を design-decisions.md で structural fix 済)。
/// Future PR が drift を混入したら本 test fail で regression detect。
///
/// **Frozen baseline (post-Phase 4 T1-pre-3a structural fix)**: exit 0、Total
/// drifts: 0、Line refs found: 30 (現状値、handoff doc に新 ref が追加されれば
/// 増える、減少 / 増加は本 assertion で flag されない = drift count のみ baseline)。
#[test]
fn test_audit_handoff_doc_line_refs_standalone_baseline() {
    let (exit, stdout, stderr) = run_handoff_audit("doc/handoff/");
    assert_eq!(
        exit, 0,
        "doc/handoff/ standalone baseline must be 0 drifts (PRD I-D-pre Phase 4 \
         T1-pre-3a structural fix lock-in); stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        parse_total_drifts(&stdout),
        0,
        "doc/handoff/ baseline must be 0 drifts; stdout:\n{stdout}"
    );
    // Sanity: at least one handoff doc was scanned (= directory walk works).
    assert!(
        stdout.contains("Handoff docs scanned:"),
        "stdout must report scanned doc count; stdout:\n{stdout}"
    );
    // Sanity: at least one line ref was found in real handoff docs.
    let line_refs = stdout
        .lines()
        .find_map(|l| {
            l.strip_prefix("Line refs found:")
                .and_then(|r| r.trim().parse::<usize>().ok())
        })
        .expect("stdout must report 'Line refs found: N'");
    assert!(
        line_refs >= 1,
        "doc/handoff/ must contain at least 1 line ref (sanity check); got {line_refs}"
    );
}
