# Synthetic PRD: Axis 3 cell-slot used as identifier (positive fixture)

cell-slot N pattern (= identifier-level fork) = Axis 3 extension should flag.
Other axes clean.

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output |
|---|-----------|--------------|
| 1 | A | x |

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: 1 variant
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Scope

- Cell 1 included

## Invariants

### INV-1 stub

Coverage: Cell 1.

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Task |
|--------|------|
| 1 | T1 |

(Note in design: The cell-slot 1 is implemented in module X. The cell-slot #2 needs review.)

## Implementation Stage Tasks

### T1: stub

Covers cell 1.

## Test Plan

Tests for cell 1.
