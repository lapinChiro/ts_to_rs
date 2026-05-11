# Synthetic PRD: T1-pre-2 cross-reference cell violation (positive fixture)

**Status**: Spec stage Iteration v1 (synthetic fixture for T1-pre-2 audit script extension)

PRD I-D-pre Phase 3 T1-pre-2 `verify_cross_reference_cell_consistency` を audit script
側で trigger するための synthetic fixture。Scope (policy=full) section が matrix cell 2 を
omit = SECTION_COVERAGE_POLICY allow-list で flag されるべき。

Other axes (pending verdict / cell numbering drift) は clean.

## Background

T1-pre-2 violation を **positive case** で確実に trigger する目的。

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

Trigger T1-pre-2 cross-reference violation.

### Verifiable success criteria

audit script exits non-zero with "T1-pre-2 violation (cross-reference cell)" in stderr.

## Scope

### In Scope

- variant_a entry (本 PRD).

(NOTE: second matrix entry intentionally omitted from Scope full enumeration =
T1-pre-2 violation trigger; downstream cross-reference contexts still list both.)

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

- PRD I-D-pre T1-pre-2.
