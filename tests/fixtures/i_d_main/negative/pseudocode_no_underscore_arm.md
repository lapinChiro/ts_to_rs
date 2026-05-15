# Synthetic PRD Fixture: Pseudocode No Underscore Arm (Negative, specific PASS path)

T1-8 audit `verify_pseudocode_underscore_arm_self_applied` の **specific PASS
path negative test fixture**。Design Rust pseudocode は **NO `_` arm** で
exhaustive enumeration (= Rule 11 (11-1) compliant)。期待 audit result: T1-8
actual logic が run + no v6-1 violation。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | impl_foo | 本 PRD |
| 2 | C-2 | impl_bar | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic specific PASS path fixture for T1-8 (no `_` arm, Rule 11 (11-1) compliant)。

## Scope

### In Scope

cell 1 (C-1) / cell 2 (C-2)。

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
  - Primary Axis A (synthetic, 2 candidates)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Dispatch tree pseudocode (= exhaustive enumeration, NO `_` arm)

```rust
match axis_a {
    Foo => impl_foo(),
    Bar => impl_bar(),
}
```

### Spec→Impl Mapping

| Cell # | Candidate | Implementation Task | Test contract path |
|--------|-----------|---------------------|--------------------|
| 1 | C-1 | T1-synthetic | `tests/synthetic_test.rs::test_cell_1` |
| 2 | C-2 | T1-synthetic | `tests/synthetic_test.rs::test_cell_2` |

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Test Plan

- cell 1: covered by `test_cell_1` (verify via `cargo test --tests`)
- cell 2: covered by `test_cell_2` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic specific PASS path fixture for T1-8 (no `_` arm)
