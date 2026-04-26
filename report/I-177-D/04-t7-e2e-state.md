# I-177-D Investigation Report 04: T7 E2E Cell Post-Revert State

**Generated**: 2026-04-25
**Scope**: Verify T7-1〜T7-5 E2E cell state after T7 patch revert (2026-04-25). Cross-reference TODO entry I-177-D for consistency.

---

## Executive Summary

T7 patch revert (2026-04-25) は完了済。T7-1 / T7-2 / T7-4 / T7-5 は **GREEN** (active)、T7-3 は **IGNORED** (`I-177-D dependency` annotation 付与)。3 件の T7 lock-in unit test は削除済。1 件の non-closure-reassign lock-in test (`narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign`) は保持。

| Cell | Test path | Status | Annotation |
|------|-----------|--------|------------|
| T7-1 | `tests/e2e_test.rs::test_e2e_cell_i161_t7_1_and_narrow_f64` | GREEN | (active) |
| T7-2 | `tests/e2e_test.rs::test_e2e_cell_i161_t7_2_or_narrow_f64` | GREEN | (active) |
| T7-3 | `tests/e2e_test.rs::test_e2e_cell_i161_t7_3_and_closure_reassign` | **IGNORED** | I-177-D dependency (26-line annotation) |
| T7-4 | `tests/e2e_test.rs::test_e2e_cell_i161_t7_4_or_then_nc` | GREEN | (active) |
| T7-5 | `tests/e2e_test.rs::test_e2e_cell_i161_t7_5_and_narrow_union_rhs` | GREEN | (active) |

---

## Q1: T7-3 Current State and Annotation

**Path**: `tests/e2e_test.rs:1822-1845` (approx)

**State**: `#[ignore]` with 26-line detailed annotation.

**Annotation contents**:
1. Pre-revert state (2026-04-25): T7 patch workaround attempted but caused Scenario A regression
   (E0308 mismatch on `return x` without `??`)
2. Root cause: `FileTypeResolution::narrowed_type(var, position)` suppression scope is overbroad
   (covers entire fn body instead of just LET-WRAP scope)
3. Pre-T7 form expected errors: E0599 / E0282 / E0308 chain + E0506 closure-mutable-capture borrow conflict
4. Resolution path: I-177-D case C
   (suppress only in LET-WRAP scope, preserve narrow in cons-span)

---

## Q2: T7-1 / T7-2 / T7-4 / T7-5 Status Detail

| Cell | Status | Assertion expected | TS pattern (one-line digest) |
|------|--------|--------------------|------------------------------|
| T7-1 | Active | `output == "3"` | Narrow F64 with `&&=` (truthy → assign) |
| T7-2 | Active | `output == "5"` | Narrow F64 with `\|\|=` (truthy → no-op) |
| T7-4 | Active | `output == "5"` | Chained `\|\|=` and `??=` interaction |
| T7-5 | Active | `output == "result\nresult\nn:0"` | Union narrow with string RHS |

**Why GREEN**: 4 cells avoid closure-reassign patterns. The closure-reassign suppression scope issue (I-177-D root cause) does not activate for these cells, so they remain unaffected by the revert.

---

## Q3: TS Script Source Patterns

### T7-1 (`cell-t7-1-and-narrow-f64.ts`)
```typescript
// Pattern: Narrowed primitive × &&= assignment
function f(): number {
    let x: number | null = 5;
    if (x !== null) {
        x &&= 3;  // Narrow preserved, truthy number → assign 3
        return x;
    }
    return -1;
}
```

### T7-2 (`cell-t7-2-or-narrow-f64.ts`)
```typescript
// Pattern: Narrowed primitive × ||= no-op (already truthy)
function f(): number {
    let x: number | null = 5;
    if (x !== null) {
        x ||= 99;  // Narrow preserved, truthy number → no assign
        return x;
    }
    return -1;
}
```

### T7-3 (`cell-t7-3-and-closure-reassign.ts`)  ★ I-177-D dependency
```typescript
// Pattern: Narrowed primitive × &&= × closure reassign (RED)
function f(): number {
    let x: number | null = 5;
    const reset = () => { x = null; };  // Closure modifies x
    if (x !== null) {
        x &&= 3;  // narrow-suppressed path
        reset();  // x becomes null
        return x ?? -1;  // null → -1
    }
    return -1;
}
```

### T7-4 (`cell-t7-4-or-then-nc.ts`)
```typescript
// Pattern: Chained logical assigns with narrow carry-through
function f(): number {
    let x: number | null = null;
    x ||= 5;   // null ||= 5 → x = 5
    x ??= 99;  // x=5 narrowed; ??= no-op
    return x;
}
```

### T7-5 (`cell-t7-5-and-narrow-union-rhs.ts`)
```typescript
// Pattern: Union type narrow × &&= with string RHS
function f(init: number | string | null): string {
    let x: number | string | null = init;
    if (x !== null) {
        x &&= "result";
        return typeof x === "string" ? x : `n:${x}`;
    }
    return "null";
}
```

---

## Q4: T7-3 Compilation Errors (Pre-T7 / Post-Revert State)

**Expected error chain** (per ignore annotation):

1. **E0599 / E0282 / E0308 chain**: IR shadow form (`x: T`) vs TypeResolver view (`Option<T>`) mismatch
   - `x &&= 3` desugar queries Option-shape but sees T shadow
   - `?? -1` lowering queries Option-shape but sees T shadow
   - Type mismatch on `unwrap_or_else` / Option method calls
2. **E0506 borrow conflict**: Closure-mutable-capture on `let mut reset = || { x = null; }`
   - Closure captures mutable ref to outer x
   - Later operations in if-body conflict with closure capture

**Pre-T7 / Post-Revert emission form (sketch)**:
```rust
if let Some(mut x) = x {
    if x.is_some_and(...) { x = Some(3.0); }  // type confusion
    reset();
    return x.unwrap_or_else(...);              // type confusion
}
```

**Diagnosis**: IR shadows `x: f64` but TypeResolver sees `x: Option<f64>` at `&&=` and `??` positions due to overbroad suppression scope. I-177-D case C (suppression scope post-if 限定化) で IR / TypeResolver の view を coherent にする。

---

## Q5: I-161 Aggregate E2E Counts

- I-161 cells total (E2E): **25 tests**
  - Active GREEN: 7
  - Ignored RED: 18 (incl. T7-3)
- T7 subset: **5 cells** (T7-1〜T7-5)
  - Active: 4 (T7-1, T7-2, T7-4, T7-5)
  - Ignored: 1 (T7-3, I-177-D dependency)

(Note: Total E2E in repository = 177, of which 25 are I-161 series.)

---

## Q6: TODO Consistency Cross-Reference

### TODO I-177-D entry verified

| TODO claim | Actual state | Verdict |
|-----------|--------------|---------|
| T7 patch revert 2026-04-25 完了 | `control_flow.rs` no NonNullish closure-reassign predicate emission branch | ✓ |
| T7-3 ignore + "I-177-D dependency" annotation | annotation present (26 lines) | ✓ |
| 3 件の T7 lock-in test 削除済 | grep returns 0 hits | ✓ |
| `narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign` 保持 | test present in `narrowing.rs` | ✓ |
| T7-1 / T7-2 / T7-4 / T7-5 GREEN | all 4 active without `#[ignore]` | ✓ |
| Affected files for I-177-D listed | `type_resolution.rs` / `events.rs` / `closure_captures.rs` / etc. | ✓ |

**Discrepancies**: None.

---

## Conclusions

1. **Revert verified**: T7 patch successfully reverted on 2026-04-25.
2. **Cell state correct**: Closure-reassign 不在 cell (T7-1/2/4/5) が GREEN、closure-reassign 在 cell (T7-3) が IGNORED with I-177-D dependency。
3. **Annotation complete**: T7-3 annotation fully documents revert reason / root cause / expected errors / resolution path.
4. **No orphan tests**: Deleted T7-predicate tests properly removed; non-closure-reassign lock-in preserved.
5. **I-177-D readiness**: Structural prerequisites documented, T7-3 will GREEN-ify when case-C suppression scope refactor + ClosureCapture suppress_scopes 拡張 completes.

---

## References

- Test code: `tests/e2e_test.rs::test_e2e_cell_i161_t7_*`
- TS sources: `tests/e2e/scripts/cell-t7-*.ts`
- Production code (revert applied): `src/transformer/statements/control_flow.rs`
- Related TODO: `TODO` (`[I-177-D]` entry)
- Companion reports:
  - `01-typeresolver-narrowed-type.md` — suppression scope architecture
  - `02-narrow-emission-paths.md` — narrow emission dispatch
  - `03-test-inventory.md` — closure-reassign test inventory
  - `06-sub-items-analysis.md` — sub-item batching analysis
