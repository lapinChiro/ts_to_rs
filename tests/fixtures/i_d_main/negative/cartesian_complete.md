# Synthetic PRD Fixture: Cartesian Product Complete (Negative)

T1 phase audit functions の **shared negative test fixture**。
yaml `Cartesian product completeness:` で Expected cell count = 5、Documented
gaps = [3] declare、matrix table には cell # 1 / 2 / 4 / 5 のみ (cell 3 は
documented gap で active scope 外)。`## Cell Numbering Convention` section 含む
ため Option α auto-detect gate を pass、各 NEW audit function の "section
presence + no violation pattern" PASS path (= true negative coverage) を
verify する shared fixture として機能する。

期待 audit result: violation なし (= documented gap allow-list で T1-1 absorb、
他 T1 functions も section content に violation pattern 不在で全 PASS、
I-D-pre 完成 audit (T1-pre-2 cross-reference) も全 cross-reference context
sections で cells 1/2/4/5 enumerate 済で PASS)。

**Status**: Draft

## Cell Numbering Convention

(synthetic test fixture: matrix # canonical identifier, single-source-of-truth)。
本 fixture は Option α auto-detect gate を pass し、各 NEW audit function の
"section presence + no violation pattern" PASS path を test するための shared
negative fixture として機能する。

## Problem Space

### 組合せマトリクス (synthetic、5 cells expected with documented gap {3}、4 active cells)

| # | Candidate | Ideal output | Scope |
|---|-----------|--------------|-------|
| 1 | C-1 | foo | 本 PRD |
| 2 | C-2 | bar | 本 PRD |
| 4 | C-4 | qux | 本 PRD |
| 5 | C-5 | quux | 本 PRD |

## Oracle Observations

(skip for synthetic fixture)

## Goal

Synthetic shared negative fixture for T1 phase audit functions。

## Scope

### In Scope

cell 1 (C-1) / cell 2 (C-2) / cell 4 (C-4) / cell 5 (C-5) = 4 active cells。

### Out of Scope

なし。

### Tier 2 honest error reclassify

N/A。

## Invariants

### INV-1: synthetic

(synthetic placeholder、`test_invariant_1_synthetic` reference)

## Impact Area Audit Findings

N/A (synthetic fixture).

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (synthetic, 5 candidates with documented gap {3})
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
Cartesian product completeness:
  Expected cell count: 5
  Documented gaps: [3]
```

## Design

### Spec→Impl Mapping (= dispatch tree comments)

| Cell # | Candidate | Implementation Task | Test contract path |
|--------|-----------|---------------------|--------------------|
| 1 | C-1 | T1-synthetic | `tests/synthetic_test.rs::test_cell_1` |
| 2 | C-2 | T1-synthetic | `tests/synthetic_test.rs::test_cell_2` |
| 4 | C-4 | T1-synthetic | `tests/synthetic_test.rs::test_cell_4` |
| 5 | C-5 | T1-synthetic | `tests/synthetic_test.rs::test_cell_5` |

## Spec Stage Tasks

### TS-0: synthetic

placeholder.

## Implementation Stage Tasks

### T1: synthetic

placeholder.

## Test Plan (= test category partition)

各 cell に対する test coverage:
- cell 1: covered by `test_cell_1` (verify via `cargo test --tests`)
- cell 2: covered by `test_cell_2` (verify via `cargo test --tests`)
- cell 4: covered by `test_cell_4` (verify via `cargo test --tests`)
- cell 5: covered by `test_cell_5` (verify via `cargo test --tests`)

## Spec Review Iteration Log

### Iteration v1 (synthetic)

- **Findings count**: 0
- **Findings detail**: synthetic shared negative fixture for T1 phase audit functions
