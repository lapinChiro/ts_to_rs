# Synthetic handoff doc fixture: line-ref clean (negative test fixture)

This fixture contains only line-refs that the audit script verifies cleanly
(PRD I-D-pre Cell 3 / v11-5 / T1-pre-3a)。It exercises both line-spec forms
(single / range) and both path-resolution paths (as-is / glob fallback)。

## Stable single-line refs (as-is path)

- Repo-root manifest: `Cargo.toml:1` (file always exists with non-zero lines)。
- Repo-root readme: `README.md:1`。
- Self-reference: `tests/fixtures/i_d_pre/negative/handoff_clean.md:1` (this
  file itself)。

## Partial-path ref (single in-bounds candidate via glob fallback)

- `audit-handoff-doc-line-refs.py:1` is a bare basename, but only one file
  matches under common roots — `scripts/audit-handoff-doc-line-refs.py`。Audit
  treats this as VERIFIED via single-candidate glob fallback。

## Range-form ref (L1-2 equivalence partition coverage)

- `src/lib.rs:1-10` is a range-form ref well within the file's line count。
  Audit should treat range refs symmetrically to single-line refs and pass
  this case cleanly。

All 5 refs above resolve to exactly one in-bounds file = audit script reports
0 drifts。
