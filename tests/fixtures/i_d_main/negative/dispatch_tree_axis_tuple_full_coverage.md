# Synthetic PRD Fixture: Dispatch Tree Axis Tuple Full Coverage (Negative, specific PASS path)

T1-5 audit `verify_dispatch_tree_axis_tuple_consistency` の **specific PASS path
negative test fixture**。Matrix に Axis columns + 3 cells、Design Rust pseudocode
は **exhaustive enumeration** (= 3 explicit arms で全 matrix axis-tuples を cover
+ `_` wildcard 不在 = Rule 11 (11-1) compliance preserve)。期待 audit result:
T1-5 actual logic が run + no v4-1 violation (= matrix axis-tuples set ⊆ arm
tuples set、has_wildcard False branch でも fall-through 不在)。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Axis A | Axis B | Ideal output | Scope |
|---|-----------|--------|--------|--------------|-------|
| 1 | C-1 | Foo | Bar | impl_foo_bar | 本 PRD |
| 2 | C-2 | Foo | Baz | impl_foo_baz | 本 PRD |
| 3 | C-3 | Qux | Bar | impl_qux_bar | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic specific PASS path fixture for T1-5 (full axis-tuple coverage)。

## Scope

### In Scope

cell 1 (C-1) / cell 2 (C-2) / cell 3 (C-3)。

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
  - Primary Axis A (synthetic, 3 candidates with Axis A / Axis B columns)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Dispatch tree pseudocode (= 全 axis-tuples を exhaustive enumeration、Rule 11 (11-1) compliance preserve)

```rust
match (axis_a, axis_b) {
    (Foo, Bar) => impl_foo_bar(),
    (Foo, Baz) => impl_foo_baz(),
    (Qux, Bar) => impl_qux_bar(),
}
```

### Spec→Impl Mapping (= dispatch tree comments)

| Cell # | Candidate | Implementation Task | Test contract path |
|--------|-----------|---------------------|--------------------|
| 1 | C-1 | T1-synthetic | `tests/synthetic_test.rs::test_cell_1` |
| 2 | C-2 | T1-synthetic | `tests/synthetic_test.rs::test_cell_2` |
| 3 | C-3 | T1-synthetic | `tests/synthetic_test.rs::test_cell_3` |

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Test Plan (= test category partition)

- cell 1: covered by `test_cell_1` (verify via `cargo test --tests`)
- cell 2: covered by `test_cell_2` (verify via `cargo test --tests`)
- cell 3: covered by `test_cell_3` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic specific PASS path fixture for T1-5 (all axis-tuples covered)
