# Synthetic PRD Fixture: Option α Gate Skips Pre-Compliance (Negative / Gate)

T1 phase audit functions の **Option α gate direct test fixture**。
本 fixture は cartesian_implicit_omission.md と **同一の violation pattern**
(= yaml `Cartesian product completeness: { Expected: 5, gaps: [] }` + matrix
{1,2,4,5} で cell 3 implicit omission) を含むが、**`## Cell Numbering
Convention` section が不在** = retroactive compliance pending state。

期待 audit result: 全 NEW audit functions が Option α auto-detect gate で
**early-return = audit skip** = exit 0 / no violations。本 fixture は gate
correctness の direct verify として機能、I-D-205 等 retroactive compliance
pending PRDs に対する **audit out-of-scope 自動分類** が structurally lock-in
されていることを empirical 証明する。

**Status**: Draft

## Problem Space

### 組合せマトリクス (synthetic、5 cells expected、cell 3 implicit omission、ただし gate で skip)

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | foo | 本 PRD |
| 2 | C-2 | bar | 本 PRD |
| 4 | C-4 | qux | 本 PRD |
| 5 | C-5 | quux | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic Option α gate direct verify fixture。

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
  - Primary Axis A (synthetic, 5 candidates, NO Cell Numbering Convention section = pre-compliance)
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
- **Findings detail**: synthetic Option α gate direct verify fixture (= pre-compliance state、no `## Cell Numbering Convention` section、全 NEW audit functions が gate で skip 期待)
