# Synthetic PRD Fixture: Pending Verdict with Severity (Negative, specific PASS path)

T1-11 audit `verify_pending_verdict_severity_default` の **specific PASS path
negative test fixture**。Iteration entry に "Pending verdict 2" wording (N>0) +
"severity default = Critical" declaration が **両方存在**。期待 audit result:
T1-11 actual logic が run (= pv_pattern match + severity_decl_pattern match) +
no v11-8 violation (= severity declared で violation skip)。

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

Synthetic specific PASS path fixture for T1-11 (Pending verdict + severity declared)。

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

### Iteration v1 (synthetic with Pending verdict + severity Critical default declared)

- **Findings count**: Critical 0 / High 0 / Pending verdict 2
- **Findings detail**: 2 pending items; severity default = Critical 適用 (Rule 13 (v11-8) compliant)
- **Resolution**: severity default = Critical default 適用済 = Spec stage 移行 block 解除
