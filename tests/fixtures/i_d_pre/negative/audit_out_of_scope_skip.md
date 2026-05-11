# Synthetic PRD: Option α audit out-of-scope skip (I-205-like fixture)

**Status**: Spec stage Iteration v1 (synthetic fixture for Option α auto-detect verify)

PRD I-D-pre Phase 3 helper `has_cell_numbering_convention_section()` の **False branch
(= early-return)** を verify する I-205-like fixture。`## Cell Numbering Convention`
section が **不在** = 3 NEW verify functions (T1-pre-1/T1-pre-2/T1-pre-4) は全て
early-return で skip されるべき。

本 fixture は **3 violation patterns を意図的に含む** (= もし audit が誤って scope 内に
分類した場合、3 functions 全てが flag するべき pattern)。Option α auto-detect が正しく
動作すれば、これら全 violations は skip される = audit script PASS (exit 0)。

This is the symmetric counterpart of L1-2 / L4 trade-off finding: ensuring helper
False branch isn't dead code.

## Background

Helper False branch (= I-205-like PRD pattern) で 3 NEW verify functions が skip される
こと自体を test する。本 fixture は I-205 PRD doc の structural similarity を持つ:
- `## Cell Numbering Convention` section 不在 (= Option α auto-detect で out-of-scope)
- documented gaps を持つ可能性 (= synthetic では 1 cell scope に simplify)
- 案 γ Phase 2 T15 で retroactive update 候補 (= TODO `[I-205-retroactive-cell-numbering-section]`)

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

### Cell 1: stub oracle (with intentional cell-slot vocabulary fork = T1-pre-4 violation candidate, expected to be skipped)

- **TS fixture path**: `n/a (synthetic)`
- **tsc output**: stdout=`stub`, stderr=``, exit_code=0
- **Cell number reference**: matrix # 1 (equivalently cell-slot 1 in legacy vocabulary,
  if audit were in-scope this would trigger T1-pre-4 violation)
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

Verify Option α auto-detect skips 3 NEW verify functions on I-205-like PRD pattern
(= `## Cell Numbering Convention` section absent).

### Verifiable success criteria

audit script exits zero (no "T1-pre-1" / "T1-pre-2" / "T1-pre-4" violation in stderr)
despite fixture containing all 3 violation patterns.

## Scope

### In Scope

(NOTE: matrix cell intentionally omitted from Scope full enumeration = if audit were
in-scope this would trigger T1-pre-2 violation; expected to be skipped by Option α.)

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

### TS-pre-3: Self-applied audit script verify run (intentional T1-pre-1 violation pattern)

- **Work**: Run audit on the PRD doc.
- **Completion criteria**: audit detects no violation despite the post-v15 wording below.
- **Status**: IN PROGRESS (Iteration v17 期待 で完成、v18 convergence; if audit were in-scope
  this would trigger T1-pre-1 violation, expected to be skipped by Option α auto-detect.)

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

- PRD I-D-pre Phase 3 helper `has_cell_numbering_convention_section()` False branch test.
- TODO `[I-205-retroactive-cell-numbering-section]` (= 案 γ Phase 2 T15 で retroactive update
  対象 PRD pattern、本 fixture が I-205 structural similarity を持つ).
