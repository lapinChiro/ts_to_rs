# Synthetic PRD Fixture: Invariant Cell Coverage Inconsistent (Positive)

T1-9 audit `verify_invariant_cell_coverage_double_partition` の **positive test
fixture**。INV-N entry が "全 5 cells" claim を含むが、matrix table の actual
active cells は 3 cells = cross-reference inconsistency。期待 audit result:
violation 検出。

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

### INV-1: synthetic over-claim

- **(a) Property statement**: 全 5 cells 全 PASS (= but matrix only has 3 cells = inconsistent claim)
- **(b) Justification**: synthetic
- **(c) Verification method**: `test_invariant_1_synthetic` 集約 entry
- **(d) Failure detectability**: compile error

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

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic positive fixture for INV cell coverage cross-reference inconsistency
