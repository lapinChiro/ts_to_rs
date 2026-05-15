# Synthetic PRD: Axis 5 matrix count clean (negative fixture)

Matrix table has 3 active cells + 2 MIGRATED rows = 5 total enumerated.
Body claims match active count or use historical allowance context → no drift.

## Problem Space

### 組合せマトリクス (3 cells)

| # | Candidate | Ideal output |
|---|-----------|--------------|
| 1 | A | x |
| 2 | B | y |
| 3 | C | z |
| 4 | D | **MIGRATED to other PRD** |
| 5 | E | **MIGRATED to other PRD** |

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: 3 candidates
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Goal

This PRD covers 3 active candidates (= I-D parent 5 cells から 2 MIGRATED documented gaps reflected).

## Scope

In scope: Cell 1, Cell 2, Cell 3.

## Invariants

### INV-1 stub

Coverage: Cell 1, Cell 2, Cell 3.

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Task |
|--------|------|
| 1 | T1 |
| 2 | T2 |
| 3 | T3 |
