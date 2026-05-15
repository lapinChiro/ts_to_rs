# Synthetic PRD Fixture: Pseudocode Underscore Arm (Positive)

T1-8 audit `verify_pseudocode_underscore_arm_self_applied` の **positive test
fixture**。Design Rust pseudocode 内に `_ =>` arm が含まれる (= Rule 11 (11-1)
`_` arm 全廃 prohibition 違反)。期待 audit result: violation 検出。

Matrix 2 cells と explicit arms 2 + `_` arm = T1-3 PASS (no duplicate) /
T1-5 PASS (active==explicit) / T1-8 violation のみ trigger する isolated 設計。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)

## Problem Space

### 組合せマトリクス

| # | Candidate | Axis A | Ideal output | Scope |
|---|-----------|--------|--------------|-------|
| 1 | C-1 | Foo | impl_foo | 本 PRD |
| 2 | C-2 | Bar | impl_bar | 本 PRD |

## Oracle Observations

(skip)

## Goal

Synthetic.

## Scope

### In Scope

C-1, C-2。

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
  - Primary Axis A (synthetic, 2 candidates)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

## Design

### Predicate dispatch pseudocode (Rule 11 (11-1) violation = `_` arm prohibited)

```rust
match axis_a {
    Foo => impl_foo(),
    Bar => impl_bar(),
    _ => unreachable!("never reached but Rule 11 (11-1) prohibits `_` arm in spec pseudocode"),
}
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
- **Findings detail**: synthetic positive fixture for Rule 11 (11-1) `_` arm self-applied compliance violation
