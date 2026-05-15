# Synthetic PRD Fixture: Invariant Matching Claim (Negative, specific PASS path)

T1-9 audit `verify_invariant_cell_coverage_double_partition` の **specific PASS
path negative test fixture**。INV-N entry の "全 N cells" claim が **actual
matrix active cells count と一致** (= claim と actual の cross-reference 整合)。
期待 audit result: T1-9 actual logic が run (= claim pattern match) + no v6-2
violation (= count match で violation skip)。

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
| 4 | C-4 | impl_4 | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic specific PASS path fixture for T1-9 (claim count matches actual)。

## Scope

### In Scope

cell 1 (C-1) / cell 2 (C-2) / cell 3 (C-3) / cell 4 (C-4)。

### Out of Scope

なし。

### Tier 2 honest error reclassify

N/A。

## Invariants

### INV-1: 全 4 cells PASS (= matrix 4 active cells と matching claim)

- **(a) Property statement**: 全 4 cells が `test_invariant_1_synthetic` で PASS する
- **(b) Justification**: synthetic claim matching actual matrix
- **(c) Verification method**: `test_invariant_1_synthetic` 集約 entry で 4 cells aggregate verify
- **(d) Failure detectability**: compile error

## Impact Area Audit Findings

N/A (synthetic fixture).

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (synthetic, 4 candidates)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Spec→Impl Mapping

| Cell # | Candidate | Implementation Task | Test contract path |
|--------|-----------|---------------------|--------------------|
| 1 | C-1 | T1-synthetic | `tests/synthetic_test.rs::test_cell_1` |
| 2 | C-2 | T1-synthetic | `tests/synthetic_test.rs::test_cell_2` |
| 3 | C-3 | T1-synthetic | `tests/synthetic_test.rs::test_cell_3` |
| 4 | C-4 | T1-synthetic | `tests/synthetic_test.rs::test_cell_4` |

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Test Plan

- cell 1〜4: covered by `test_cell_N` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic specific PASS path fixture for T1-9 (claim 全 4 cells matches matrix 4 cells)
