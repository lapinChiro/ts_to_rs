# Synthetic PRD: T1-pre-2 cross-reference cell clean (negative fixture)

**Status**: Spec stage Iteration v1 (synthetic fixture for T1-pre-2 audit script extension)

PRD I-D-pre Phase 3 T1-pre-2 `verify_cross_reference_cell_consistency` の negative case
fixture。Scope (policy=full) section に matrix cells 全 enumerate + Spec→Impl Mapping も
1-to-1 alignment = audit pass.

All axes (pending verdict / cross-reference / cell numbering) clean.

## Background

T1-pre-2 audit を **negative case** (= no violation) で verify する目的。

## Problem Space

### Matrix-driven 判定 (Step 0a)

Matrix-driven: yes.

### 入力次元 (Dimensions)

#### Primary Axis A: stub (2 variants)

variant_a / variant_b.

### 組合せマトリクス (2 cells)

| # | Axis A | Ideal output | Scope |
|---|--------|--------------|-------|
| 1 | variant_a | preserved | 本 PRD |
| 2 | variant_b | preserved | 本 PRD |

### Spec-Stage Adversarial Review Checklist

13-rule self-applied verify (本 v1 draft 完了直後実施).

## Oracle Observations

### Cell 1: variant_a oracle

- **TS fixture path**: `n/a (synthetic)`
- **tsc output**: stdout=`stub`, stderr=``, exit_code=0
- **Cell number reference**: matrix # 1
- **Ideal output rationale**: stub.

### Cell 2: variant_b oracle

- **TS fixture path**: `n/a (synthetic)`
- **tsc output**: stdout=`stub`, stderr=``, exit_code=0
- **Cell number reference**: matrix # 2
- **Ideal output rationale**: stub.

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: variant_a / variant_b (2 variants)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

### Cross-axis orthogonal direction detail

2-cell synthetic fixture.

## Impact Area Audit Findings

### Pre-draft ast-variant audit (Rule 11 (d-5) compliance)

N/A — synthetic fixture.

### Adapted Impact Area Review

| File | Size | LOC | Last modified | Audit |
|------|------|-----|---------------|-------|
| (synthetic fixture) | — | — | — | N/A |

## Goal

Verify T1-pre-2 audit passes when matrix cells fully enumerated.

### Verifiable success criteria

audit script exits zero (no "T1-pre-2 violation" in stderr).

## Scope

### In Scope

- Cell 1: variant_a (本 PRD).
- Cell 2: variant_b (本 PRD).

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

Coverage: cells 1, 2.

## Cell Numbering Convention

Cell # canonical = Problem Space matrix # 1, 2.

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Implementation Stage Task |
|--------|---------------------------|
| 1 | T1 |
| 2 | T2 |

## Spec Stage Tasks

### TS-pre-1: synthetic spec author

- **Work**: Author this fixture.
- **Completion criteria**: File written.
- **Status**: COMPLETE.

## Implementation Stage Tasks

### T1: stub for cell 1

- **Work**: stub.
- **Completion criteria**: stub.

### T2: stub for cell 2

- **Work**: stub.
- **Completion criteria**: stub.

## Test Plan

Cells 1, 2 covered.

## Completion Criteria

1. Matrix completeness for cells 1, 2.

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

- PRD I-D-pre T1-pre-2 (= negative case).
