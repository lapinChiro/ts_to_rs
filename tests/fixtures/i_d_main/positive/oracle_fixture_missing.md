# Synthetic PRD Fixture: Oracle Fixture Missing (Positive)

T1-14 audit `verify_fixture_oracle_byte_consistency` の **positive test fixture**。
Oracle Observations section に **存在しない** TS fixture path (`tests/e2e/scripts/nonexistent-prd/nonexistent-cell.ts`)
が referenced されている = Oracle re-grounding gap = v13-6 violation。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | impl_1 | 本 PRD |

## Oracle Observations

### Cell 1: synthetic

- TS fixture path: `tests/e2e/scripts/nonexistent-prd/nonexistent-cell.ts`
- tsc / tsx output: synthetic
- Cell number: 1
- Ideal output rationale: synthetic

## Goal

Synthetic.

## Scope

### In Scope

C-1。

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
  - Primary Axis A (synthetic, 1 candidate)
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
