# Synthetic PRD Fixture: Cartesian Product Implicit Omission (Positive)

T1-1 audit `verify_cartesian_product_completeness` の **positive test fixture**。
yaml `Cartesian product completeness:` で Expected cell count = 5、Documented gaps = []
declare、matrix table には cell # 1 / 2 / 4 / 5 のみ (cell # 3 implicit omission)
含む synthetic state。期待 audit result: violation 検出 (= "cells [3] expected
but absent")。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス (synthetic、5 cells expected、cell 3 implicit omission)

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | foo | 本 PRD |
| 2 | C-2 | bar | 本 PRD |
| 4 | C-4 | qux | 本 PRD |
| 5 | C-5 | quux | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic for positive test.

## Scope

### In Scope

C-1, C-2, C-4, C-5。

### Out of Scope

なし。

### Tier 2 honest error reclassify

N/A。

## Invariants

### INV-1: synthetic

(synthetic placeholder、`test_invariant_1_synthetic` reference)

## Impact Area Audit Findings

N/A (synthetic fixture).

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (synthetic, 5 candidates)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
Cartesian product completeness:
  Expected cell count: 5
  Documented gaps: []
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
- **Findings detail**: synthetic positive fixture for cartesian completeness violation detection
