# Synthetic PRD Fixture: Dispatch Tree Axis Tuple Mismatch (Positive)

T1-5 audit `verify_dispatch_tree_axis_tuple_consistency` の **positive test fixture**。
matrix table に 3 cells があり、Design Rust pseudocode に explicit match arms が
2 つしかない (= `_` wildcard fallback で 1 cell が unreachable!() に fall-through)。
期待 audit result: violation 検出。

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

(skip)

## Goal

Synthetic.

## Scope

### In Scope

C-1, C-2, C-3。

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
  - Primary Axis A (synthetic, 3 candidates)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Dispatch tree pseudocode (incomplete = cell 3 falls through to `_` arm)

```rust
match (axis_a, axis_b) {
    (Foo, Bar) => impl_foo_bar(),
    (Foo, Baz) => impl_foo_baz(),
    _ => unreachable!("cell 3 (Qux, Bar) falls through"),
}
```

Matrix has 3 cells but pseudocode covers only 2 explicit arms.

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic positive fixture for dispatch tree axis-tuple consistency violation detection
