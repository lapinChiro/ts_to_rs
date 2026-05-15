# Synthetic PRD Fixture: Duplicate Top-Level Matrix (Positive)

T1-2 audit `verify_no_duplicate_top_level_matrix` の **positive test fixture**。
`## Problem Space` section 内に 2 つの `### 組合せマトリクス` sub-section が共存
(= iteration 移行時の旧 matrix 残存 pattern)。期待 audit result: violation 検出。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 入力次元

- Axis A: discrete

### 組合せマトリクス (旧、Iteration v1 由来、cleanup 忘れ)

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | foo | 本 PRD |
| 2 | C-2 | bar | 本 PRD |

### 組合せマトリクス (新、Iteration v2 で再構築)

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | foo-new | 本 PRD |
| 2 | C-2 | bar-new | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic for positive test.

## Scope

### In Scope

C-1, C-2。

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
  - Primary Axis A (synthetic, 2 candidates)
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
- **Findings detail**: synthetic positive fixture for duplicate top-level matrix detection
