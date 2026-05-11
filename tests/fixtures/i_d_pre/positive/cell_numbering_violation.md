# Synthetic PRD: T1-pre-4 cell numbering drift violation (positive fixture)

**Status**: Spec stage Iteration v1 (synthetic fixture for T1-pre-4 audit script extension)

PRD I-D-pre Phase 3 T1-pre-4 `verify_cell_numbering_drift_detection` を audit script
側で trigger するための synthetic fixture。Cell-slot identifier-level fork pattern
("cell-slot 1" 等の数値 identifier 用法) を CURRENT spec section 内に含む = matrix cell
# canonical 違反として flag されるべき。

Other axes (pending verdict / cross-reference) は clean。

## Background

T1-pre-4 violation を **positive case** で確実に trigger する目的。

## Problem Space

### Matrix-driven 判定 (Step 0a)

Matrix-driven: yes.

### 入力次元 (Dimensions)

#### Primary Axis A: stub

Single variant.

### 組合せマトリクス (1 cell)

| # | Axis A | Ideal output | Scope |
|---|--------|--------------|-------|
| 1 | stub | preserved | 本 PRD |

### Spec-Stage Adversarial Review Checklist

13-rule self-applied verify (本 v1 draft 完了直後実施).

## Oracle Observations

### Cell 1: stub oracle

- **TS fixture path**: `n/a (synthetic)`
- **tsc output**: stdout=`stub`, stderr=``, exit_code=0
- **Cell number reference**: matrix # 1 (= equivalently cell-slot 1 in legacy vocabulary,
  vocabulary fork drift trigger for T1-pre-4 audit)
- **Ideal output rationale**: stub.

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: stub (1 variant)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

### Cross-axis orthogonal direction detail

Single-cell synthetic fixture.

## Impact Area Audit Findings

### Pre-draft ast-variant audit (Rule 11 (d-5) compliance)

N/A — synthetic fixture.

### Adapted Impact Area Review

| File | Size | LOC | Last modified | Audit |
|------|------|-----|---------------|-------|
| (synthetic fixture) | — | — | — | N/A |

## Goal

Trigger T1-pre-4 violation for cell-slot vocabulary fork detection.

### Verifiable success criteria

audit script exits non-zero with "T1-pre-4 violation (cell numbering drift)" in stderr.

## Scope

### In Scope

- Cell 1: stub (本 PRD).

### Out of Scope

- (nothing).

### Tier 2 honest error reclassify

- (nothing).

## Invariants

### INV-1: stub invariant

- **(a) Property statement**: stub.
- **(b) Justification**: synthetic.
- **(c) Verification method**: `test_invariant_1_stub` (placeholder).
- **(d) Failure detectability**: N/A.

Coverage: Cell 1.

## Cell Numbering Convention

Cell # canonical = Problem Space matrix # 1. (NOTE: equivalent legacy term cell-slot 1
is intentionally retained for T1-pre-4 fork drift detection trigger.)

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Implementation Stage Task |
|--------|---------------------------|
| 1 | T1 |

## Spec Stage Tasks

### TS-pre-1: synthetic spec author

- **Work**: Author this fixture.
- **Completion criteria**: File written.
- **Status**: COMPLETE.

## Implementation Stage Tasks

### T1: stub for cell 1

- **Work**: stub.
- **Completion criteria**: stub.

## Test Plan

Cell 1 covered.

## Completion Criteria

1. Matrix completeness for cell 1.

### Tier-transition compliance

Synthetic, no Hono bench impact.

### Impact estimates

Synthetic.

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: Critical 0 / High 0.
- **Convergence**: synthetic, n/a.
- **Spec stage 完了判定**: stub.

## 🔗 Cross-references

- PRD I-D-pre T1-pre-4.
