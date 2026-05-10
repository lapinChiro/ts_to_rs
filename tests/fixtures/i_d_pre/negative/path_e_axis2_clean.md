# Synthetic PRD: Axis 2 clean - pre-v15 wording in TS-pre-N (negative fixture)

TS-pre-X heading 内 で pre-v15 wording のみ = legitimate early-stage = no flag.
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
- **Status**: IN PROGRESS (v1 draft で実施、findings → fix → v2 で convergence target)

### TS-pre-4: Impact Area audit findings record

- **Work**: Record findings
- **Status**: PENDING (Implementation Phase で fill-in 予定)
