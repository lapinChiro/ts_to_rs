# Synthetic PRD: T1-pre-1 pending verdict clean (negative fixture)

**Status**: Spec stage Iteration v1 (synthetic fixture for T1-pre-1 audit script extension)

PRD I-D-pre Phase 3 T1-pre-1 `verify_pending_verdict_findings_consistency` の negative
case fixture。Status field に post-v15 wording 不在 = pending verdict clean state =
audit pass。

All axes (pending verdict / cross-reference / cell numbering) clean.

## Background

T1-pre-1 audit を **negative case** (= no violation) で verify する目的。

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
- **Cell number reference**: matrix # 1
- **Ideal output rationale**: stub fixture, no real conversion semantics

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Axis A: stub (1 variant)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

### Cross-axis orthogonal direction detail

Single-cell synthetic fixture, no real orthogonality.

## Impact Area Audit Findings

### Pre-draft ast-variant audit (Rule 11 (d-5) compliance)

N/A — synthetic fixture, no real source file modification scope.

### Adapted Impact Area Review

| File | Size | LOC | Last modified | Audit |
|------|------|-----|---------------|-------|
| (synthetic fixture, no real files) | — | — | — | N/A |

## Goal

Verify T1-pre-1 audit passes when no violation present.

### Verifiable success criteria

`python3 scripts/audit-prd-rule10-compliance.py <this-fixture>` exits zero (no
"T1-pre-1 violation" substring in stderr).

## Scope

### In Scope

- Cell 1: stub (本 PRD).

### Out of Scope

- (nothing).

### Tier 2 honest error reclassify

- (nothing).

## Invariants

### INV-1: stub invariant

- **(a) Property statement**: stub fixture preserves stub semantics.
- **(b) Justification**: synthetic fixture only.
- **(c) Verification method**: `test_invariant_1_stub` (placeholder, not implemented).
- **(d) Failure detectability**: N/A.

Coverage: cell 1.

## Cell Numbering Convention

本 synthetic fixture の cell # canonical identifier = Problem Space matrix # 1。
Single-source-of-truth principle 適用、Spec→Impl Mapping table cell # と 1-to-1 sync。

## Design

### Spec→Impl Dispatch Arm Mapping

| Cell # | Implementation Stage Task |
|--------|---------------------------|
| 1 | T1: stub |

## Spec Stage Tasks

### TS-pre-1: synthetic spec author

- **Work**: Author this synthetic fixture.
- **Completion criteria**: File written.
- **Status**: COMPLETE.

### TS-pre-3: Self-applied audit script verify run

- **Work**: Run audit on the PRD doc.
- **Completion criteria**: audit detects no violation.
- **Status**: COMPLETE.

### TS-pre-4: Impact Area audit findings record

- **Work**: Record findings.
- **Completion criteria**: section embedded.
- **Status**: COMPLETE.

## Implementation Stage Tasks

### T1: stub

- **Work**: stub.
- **Completion criteria**: stub.

## Test Plan

### Test category 1: stub tests

Covers cell 1.

## Completion Criteria

1. **Matrix completeness**: cell 1 covered by stub test (verify via `cargo test --tests`).

### Tier-transition compliance

Synthetic fixture, no Hono bench impact.

### Impact estimates

Synthetic fixture, no real impact.

## Spec Review Iteration Log

### Iteration v1 (synthetic、本 draft 初版)

- **Source state**: synthetic fixture creation.
- **Findings count**: Critical 0 / High 0.
- **Convergence criterion application**: synthetic, n/a.
- **Spec stage 完了判定**: stub.

## 🔗 Cross-references

- PRD I-D-pre T1-pre-1 (= 本 fixture が trigger する audit function、negative case).
