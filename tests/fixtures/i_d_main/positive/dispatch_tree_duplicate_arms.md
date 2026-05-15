# Synthetic PRD Fixture: Dispatch Tree Duplicate Match Arms (Positive)

T1-3 audit `verify_dispatch_tree_pseudocode_syntactic` の **positive test fixture**。
`## Design` section 内 Rust pseudocode (= ```rust fenced code block) に duplicate
match arms (= 同 pattern が複数 arm で repeat、comment-only disambiguation で
隠れる pattern を含む)。期待 audit result: violation 検出。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | foo | 本 PRD |

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

## Design

### Dispatch tree pseudocode (duplicate match arms hidden by comment-only disambiguation)

```rust
match (axis_a, axis_b) {
    (Foo, Bar) => emit_cell_1(),
    (Foo, Bar) /* + lit init disambiguation */ => emit_cell_2(),
    (Baz, Qux) => emit_cell_3(),
    _ => unreachable!(),
}
```

Above pseudocode contains a duplicate `(Foo, Bar)` match arm = audit fail expected.

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic positive fixture for dispatch tree duplicate match arms detection
