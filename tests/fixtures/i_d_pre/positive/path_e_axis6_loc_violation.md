# Synthetic PRD: Axis 6 baseline LOC claim violation (positive fixture)

Design section claims wrong LOC for stable reference file → drift expected.

## Problem Space

### 組合せマトリクス (1 cells)

| # | Candidate | Ideal output |
|---|-----------|--------------|
| 1 | A | x |

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: 1 candidates
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Scope

In scope: Cell 1.

## Invariants

### INV-1 stub

Coverage: Cell 1.

## Design

### File structure changes

- `tests/fixtures/i_d_pre/axis6_loc_reference.md` (999 行) ← WRONG, actual is 5 行 = Axis 6 drift expected

### Spec→Impl Dispatch Arm Mapping

| Cell # | Task |
|--------|------|
| 1 | T1 |
