# Synthetic PRD: Axis 2 post-v15 wording violation (positive fixture)

TS-pre-X heading 内 で post-v15 wording 残存 = F7 fix should flag.
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

## Implementation Stage Tasks

### T1: stub

Covers cell 1.

## Test Plan

Tests for cell 1.

## Spec Stage Tasks

### TS-pre-3: Self-applied audit script verify run

- **Work**: Run audit on the PRD doc
- **Status**: IN PROGRESS (v17 期待 で完成、Iteration v18 で convergence)

### TS-pre-4: Impact Area audit findings record

- **Work**: Record findings
- **Status**: PENDING (post-v16 wording で flag されるべき)
