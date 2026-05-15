# Synthetic PRD Fixture: Dispatch Tree Count-Based Fallback (Positive)

T1-5 audit `verify_dispatch_tree_axis_tuple_consistency` の **count-based
fallback path** positive test fixture (= /check_problem G-1 由来 2026-05-15)。
Matrix header に **Axis columns 不在** (= column 識別 heuristic で fail)、
semantic verify path が disabled、count-based fallback path で `active cells (3)
> explicit arms (2) + has_wildcard` で violation detect。

期待 audit result: T1-5 count-based fallback で v4-1 violation 検出 +
"count-based fallback" wording を violation message に含む。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス (Axis columns 不在の header = count-based fallback path 強制 trigger)

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | impl_1 | 本 PRD |
| 2 | C-2 | impl_2 | 本 PRD |
| 3 | C-3 | impl_3 | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic count-based fallback path positive verify fixture for T1-5。

## Scope

### In Scope

cell 1 (C-1) / cell 2 (C-2) / cell 3 (C-3) = 3 active cells。

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
  - Primary Axis A (synthetic, 3 candidates, no Axis columns in matrix header)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Dispatch tree pseudocode (= active cells > explicit arms + `_` wildcard、count-based fallback violation expected)

```rust
match input {
    A => impl_1(),
    B => impl_2(),
    _ => unreachable!("cell 3 falls through"),
}
```

### Spec→Impl Mapping

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

## Test Plan

- cell 1: `test_cell_1` (verify via `cargo test --tests`)
- cell 2: `test_cell_2` (verify via `cargo test --tests`)
- cell 3: `test_cell_3` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic count-based fallback path positive fixture for T1-5
