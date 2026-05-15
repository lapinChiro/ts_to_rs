# Synthetic PRD: Axis 7 cross-cutting Layer symmetry clean (negative fixture)

Layer cross-cutting wording symmetric with main cells = each pairing's cells appear in both claimed Layers.

## Problem Space

### 組合せマトリクス (4 cells)

| # | Candidate | Ideal output |
|---|-----------|--------------|
| 1 | A | x |
| 2 | B | y |
| 3 | C | z |
| 4 | D | w |

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: 4 candidates
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Scope

In scope:

- **Layer 1: Audit script** (cells 1, 2, 3 = **3 cell-slots**): work A. Cross-cutting cells: 3 = Layer 1+2 dual-slot
- **Layer 2: Rule wording** (cells 3, 4 = **2 cell-slots**): work B. Cross-cutting cells: 3 = Layer 1+2 dual-slot
- **Layer 3: Procedure** (cells = **0 cell-slots**): N/A
- **Layer 4: Skill workflow** (cells = **0 cell-slots**): N/A

## Invariants

### INV-1 stub

Coverage: all cells.

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Task |
|--------|------|
| 1 | T1 |
| 2 | T2 |
| 3 | T3 |
| 4 | T4 |
