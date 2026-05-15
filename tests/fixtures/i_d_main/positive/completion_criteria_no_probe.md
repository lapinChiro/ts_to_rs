# Synthetic PRD Fixture: Completion Criteria No Probe Pattern (Positive)

T1-12 audit `verify_completion_criteria_probe_pattern` の **positive test
fixture**。Completion Criteria section の criterion に **empirical probe
pattern が不在** (= manual review wording のみ)、structural enforcement 不在
= v13-1 violation。

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

### Iteration v1 (synthetic)

- **Findings count**: 0

## Completion Criteria

1. **Matrix completeness (manual review only)**: 全 cell が ideal output 通り実装され、manual review で目視 verify 完了。
2. **Test coverage (manual review only)**: developer が test 結果を manual で確認、適切な coverage と判断する。
