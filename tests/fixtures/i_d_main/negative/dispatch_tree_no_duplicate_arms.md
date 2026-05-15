# Synthetic PRD Fixture: Dispatch Tree No Duplicate Arms (Negative, specific PASS path)

T1-3 audit `verify_dispatch_tree_pseudocode_syntactic` の **specific PASS path
negative test fixture**。Design Rust pseudocode block が **存在** し、match arms
がすべて **distinct** (= no duplicate patterns、no comment-only disambiguation
hiding)。期待 audit result: T1-3 actual logic が run + no v3-5 violation。

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

Synthetic specific PASS path fixture for T1-3 (no duplicate arms in pseudocode)。

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

### Dispatch tree pseudocode (distinct arms = no v3-5 violation expected)

```rust
match axis_a {
    Foo => impl_foo(),
    Bar => impl_bar(),
}
```

### Spec→Impl Mapping (= dispatch tree comments)

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

## Test Plan (= test category partition)

- cell 1: covered by `test_cell_1` (verify via `cargo test --tests`)
- cell 2: covered by `test_cell_2` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic specific PASS path fixture for T1-3
