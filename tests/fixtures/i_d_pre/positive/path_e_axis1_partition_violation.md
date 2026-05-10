# Synthetic PRD: Axis 1 partition violation (positive fixture)

Scope (policy=full) omits matrix cell. Other sections complete.

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output |
|---|-----------|--------------|
| 1 | A | x |
| 2 | B | y |
| 3 | C | z |

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: 3 variants
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Scope

In scope: Cell 1 and Cell 2 only.

(Note: third matrix cell omitted intentionally to trigger Axis 1 policy=full violation.)

## Invariants

### INV-1 stub

Coverage: Cell 1, Cell 2, Cell 3.

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Task |
|--------|------|
| 1 | T1 |
| 2 | T1 |
| 3 | T1 |

## Implementation Stage Tasks

### T1: stub

Covers cells 1, 2, 3.

## Test Plan

Tests for cells 1, 2, 3 (partition_ok).
