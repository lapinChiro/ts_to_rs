# Synthetic PRD Fixture: Oracle Fixture Existing (Negative, specific PASS path)

T1-14 audit `verify_fixture_oracle_byte_consistency` の **specific PASS path
negative test fixture**。Oracle Observations section に **既存** の TS fixture
path (`tests/e2e/scripts/arithmetic.ts` = repo 内 existing file) が referenced。
期待 audit result: T1-14 actual logic が run (= fixture_path_pattern match +
abs_path.exists() == True) + no v13-6 violation。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | impl_1 | 本 PRD |

## Oracle Observations

### Cell 1: synthetic with existing fixture reference

- TS fixture path: `tests/e2e/scripts/arithmetic.ts` (= repo 内 existing file)
- tsc / tsx output: synthetic placeholder
- Cell number: 1
- Ideal output rationale: synthetic reference for T1-14 negative test

## Goal

Synthetic specific PASS path fixture for T1-14 (existing fixture path reference)。

## Scope

### In Scope

cell 1 (C-1)。

### Out of Scope

なし。

### Tier 2 honest error reclassify

N/A。

## Invariants

### INV-1: synthetic (`test_invariant_1_synthetic`)

placeholder.

## Impact Area Audit Findings

N/A (synthetic fixture).

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (synthetic, 1 candidate)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Spec→Impl Mapping

| Cell # | Candidate | Implementation Task | Test contract path |
|--------|-----------|---------------------|--------------------|
| 1 | C-1 | T1-synthetic | `tests/synthetic_test.rs::test_cell_1` |

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Test Plan

- cell 1: covered by `test_cell_1` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic specific PASS path fixture for T1-14 (existing fixture path)
