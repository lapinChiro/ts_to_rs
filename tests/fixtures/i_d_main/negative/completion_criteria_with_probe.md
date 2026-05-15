# Synthetic PRD Fixture: Completion Criteria with Probe (Negative, specific PASS path)

T1-12 audit `verify_completion_criteria_probe_pattern` の **specific PASS path
negative test fixture**。Completion Criteria section が **存在** し、各 numbered
criterion の body 内に **empirical probe pattern** (= `cargo test` / `python3
scripts/` / `verify_<name>` reference 等) が embed されている。期待 audit
result: T1-12 actual logic が run (= criterion_pattern match + probe_regex
match) + no v13-1 violation。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | impl_1 | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic specific PASS path fixture for T1-12 (probe pattern in each criterion)。

## Scope

### In Scope

cell 1 (C-1)。

### Out of Scope

なし。

### Tier 2 honest error reclassify

N/A。

## Invariants

### INV-1: synthetic (`test_invariant_1_synthetic`)

placeholder.

## Impact Area Audit Findings

N/A (synthetic fixture).

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (synthetic, 1 candidate)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Spec→Impl Mapping

| Cell # | Candidate | Implementation Task | Test contract path |
|--------|-----------|---------------------|--------------------|
| 1 | C-1 | T1-synthetic | `tests/synthetic_test.rs::test_cell_1` |

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Test Plan

- cell 1: covered by `test_cell_1` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0

## Completion Criteria

1. **Matrix completeness**: cell 1 covered (verify via `cargo test --tests`)。
2. **Audit compliance**: `python3 scripts/audit-prd-rule10-compliance.py` で exit code 0 達成。
