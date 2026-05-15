# Synthetic PRD Fixture: Pending Verdict Severity Missing (Positive)

T1-11 audit `verify_pending_verdict_severity_default` の **positive test fixture**。
Spec Review Iteration Log entry に "Pending verdict 3" 含むが "severity Critical
default" declaration が不在 = Rule 13 (v11-8) sub-rule violation。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | impl_1 | 本 PRD |

## Oracle Observations

(skip)

## Goal

Synthetic.

## Scope

### In Scope

C-1。

### Out of Scope

なし。

### Tier 2 honest error reclassify

N/A。

## Invariants

### INV-1: synthetic (`test_invariant_1_synthetic`)

placeholder.

## Impact Area Audit Findings

N/A.

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (synthetic, 1 candidate)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Spec Review Iteration Log

### Iteration v1 (synthetic, missing severity default)

- **Findings count**: Critical 0 / High 0 / Pending verdict 3
- **Findings detail**: 3 pending items with no severity default declaration
- **Resolution**: synthetic

### Iteration v2 (synthetic, no pending verdict)

- **Findings count**: 0
- **Findings detail**: clean iteration
