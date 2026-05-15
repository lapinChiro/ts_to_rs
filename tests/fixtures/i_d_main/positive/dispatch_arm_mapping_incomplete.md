# Synthetic PRD Fixture: Dispatch Arm Mapping Incomplete (Positive)

T1-6 audit `verify_dispatch_arm_mapping_table` の **positive test fixture**。
matrix table に 3 cells (1, 2, 3) があるが、Spec→Impl Dispatch Arm Mapping
table には cell 1, 2 のみで cell 3 が missing。期待 audit result: violation
検出 (= 1-to-1 mapping incomplete)。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | impl_1 | 本 PRD |
| 2 | C-2 | impl_2 | 本 PRD |
| 3 | C-3 | impl_3 | 本 PRD |

## Oracle Observations

(skip)

## Goal

Synthetic.

## Scope

### In Scope

C-1, C-2, C-3。

### Out of Scope

なし。

### Tier 2 honest error reclassify

N/A。

## Invariants

### INV-1: synthetic (`test_invariant_1_synthetic`)

placeholder.

## Impact Area Audit Findings

N/A.

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (synthetic, 3 candidates)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Spec→Impl Dispatch Arm Mapping (incomplete = cell 3 missing)

| Cell # | Candidate | Implementation Task | Test contract path |
|--------|-----------|---------------------|--------------------|
| 1 | C-1 | T1-1 | `tests/synthetic_test.rs::test_cell_1` |
| 2 | C-2 | T1-2 | `tests/synthetic_test.rs::test_cell_2` |

Cell 3 is missing from this mapping table.

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic positive fixture for dispatch arm mapping completeness violation
