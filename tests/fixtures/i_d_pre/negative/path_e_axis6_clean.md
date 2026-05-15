# Synthetic PRD: Axis 6 baseline LOC clean (negative fixture)

Design section claims correct LOC for stable reference file → no drift.

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

- `tests/fixtures/i_d_pre/axis6_loc_reference.md` (5 行) ← matches actual `wc -l` = no Axis 6 drift

### Spec→Impl Dispatch Arm Mapping

| Cell # | Task |
|--------|------|
| 1 | T1 |
