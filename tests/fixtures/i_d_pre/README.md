# PRD I-D-pre Test Fixtures

Synthetic PRD doc fixtures for PRD I-D-pre audit function + utility tests.

## Directory structure

- `positive/` — PRD doc fixtures intentionally containing violation pattern (= audit/utility がdetect 想定)
- `negative/` — PRD doc fixtures without violation pattern (= audit/utility PASS 想定)

## Naming convention

Per-test fixture pattern: `<test_module>_<scenario>.md`

- `pending_verdict_violation.md` / `pending_verdict_clean.md` — Cell 1 / T1-pre-1 (`tests/i_d_pre_audit_extensions_test.rs`)
- `cross_reference_violation.md` / `cross_reference_clean.md` — Cell 2 / T1-pre-2
- `cell_numbering_violation.md` / `cell_numbering_clean.md` — Cell 5 / T1-pre-4
- `method_a_drift.md` / `method_a_clean.md` — Cell 4 / T1-pre-5 (`tests/i_d_pre_method_a_test.rs`)
- `path_e_axis1_*.md` / `path_e_axis2_*.md` / `path_e_axis3_*.md` / `path_e_axis4_*.md` — Cells 1+2+5 / T1-pre-6 (`tests/i_d_pre_path_e_test.rs`)
- `handoff_drift.md` / `handoff_clean.md` — Cell 3 / T1-pre-3a (`tests/i_d_pre_handoff_audit_test.rs`)
  - positive 3 refs cover all drift categories (OUT_OF_BOUNDS / MISSING_FILE / AMBIGUOUS)
  - negative refs use only repo-stable paths (`Cargo.toml` / `README.md` / self-reference / single-candidate glob)

Each Implementation Phase (2/3/4) creates its required fixtures as part of its task scope.

## Reference

PRD doc: [`backlog/I-D-pre-audit-mechanism-bootstrap.md`](../../../backlog/I-D-pre-audit-mechanism-bootstrap.md) `## Test Plan` section
