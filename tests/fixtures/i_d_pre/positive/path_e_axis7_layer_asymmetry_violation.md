# Synthetic PRD: Axis 7 cross-cutting Layer asymmetry violation (positive fixture)

Layer 1 cross-cutting wording claims "cell 5 = Layer 1+2" but cell 5 NOT in Layer 1's main cells (only in Layer 2).
Also self-reference inconsistency: Layer 1's claim "cell 5 = Layer 2+3" doesn't include Layer 1.
→ Axis 7 drifts expected.

## Problem Space

### 組合せマトリクス (5 cells)

| # | Candidate | Ideal output |
|---|-----------|--------------|
| 1 | A | x |
| 2 | B | y |
| 3 | C | z |
| 4 | D | w |
| 5 | E | v |

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: 5 candidates
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Scope

In scope:

- **Layer 1: Audit script** (cells 1, 2 = **2 cell-slots**): work A. Cross-cutting cells: 5 = Layer 1+2 dual-slot
- **Layer 2: Rule wording** (cells 3, 4 = **2 cell-slots**): work B. Cross-cutting cells: 5 = Layer 2+3 dual-slot
- **Layer 3: Procedure** (cells 5 = **1 cell-slots**): work C
- **Layer 4: Skill workflow** (cells = **0 cell-slots**): work D

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
| 5 | T5 |
