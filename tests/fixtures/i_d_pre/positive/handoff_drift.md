# Synthetic handoff doc fixture: line-ref drift (positive test fixture)

This fixture intentionally contains 6 line-ref drift patterns covering all
drift categories detected by `scripts/audit-handoff-doc-line-refs.py`
(PRD I-D-pre Cell 3 / v11-5 / T1-pre-3a)。

The 6 patterns combine the 4 drift categories (INVALID_RANGE / MISSING_FILE /
OUT_OF_BOUNDS / AMBIGUOUS) with 2 path-resolution paths (as-is / glob
fallback) and 2 line-spec forms (single / range)、providing **C1 branch
coverage** of `classify_ref` and **equivalence-partition coverage** of the
line-spec axis (single vs range vs backwards-range)。

## Drift 1: OUT_OF_BOUNDS via as-is path

`src/lib.rs:999999` claims line 999999 of a file with a few hundred lines。
Audit should flag as `OUT_OF_BOUNDS` with detail `as-is file has N lines, claim
ends at 999999`。

## Drift 2: MISSING_FILE

`src/nonexistent_handoff_audit_fixture.rs:1` references a file that does not
exist anywhere in the repo。Audit should flag as `MISSING_FILE`。

## Drift 3: AMBIGUOUS

`mod.rs:1` is a bare basename with many candidates in the repo (every Rust
module directory has its own `mod.rs`)。Audit should flag as `AMBIGUOUS`。

## Drift 4: OUT_OF_BOUNDS via glob fallback (L1-1 C1 branch coverage)

`audit-handoff-doc-line-refs.py:99999` is a bare basename that exists only at
`scripts/audit-handoff-doc-line-refs.py` (= 1 glob candidate)、but the file
has far fewer than 99999 lines so the candidate is filtered out by the OOB
check。Audit should flag as `OUT_OF_BOUNDS` with detail `all 1 glob
candidate(s) below line 99999 (e.g., scripts/audit-handoff-doc-line-refs.py
has N lines)`。

## Drift 5: OUT_OF_BOUNDS via range form (L1-2 equivalence partition coverage)

`src/lib.rs:99990-99999` is a range-form ref where the upper bound 99999
exceeds the file length。Audit should flag as `OUT_OF_BOUNDS` and the drift
output line should contain the range spec `99990-99999` literally。

## Drift 6: INVALID_RANGE backwards-range typo (/check_problem Issue #3 coverage)

`src/lib.rs:100-50` is a range-form ref with `start > end` (= author typo,
intended `50-100`)。Audit should flag as `INVALID_RANGE` and the drift detail
should explicitly cite "backwards range" + start/end values。
