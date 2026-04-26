# I-177-D Closure-Reassign Unit Test Inventory

**Date**: 2026-04-25  
**PRD**: I-177-D (`narrowed_type` suppression scope refactor)  
**Purpose**: Impact analysis of architectural refactor on closure-reassign suppression behavior  
**Search scope**: Thorough (all closure-reassign related unit tests + E2E cells + deleted tests verification)

---

## Executive Summary

**Total closure-reassign tests identified**: 6 unit tests + 3 E2E cells + 3 deleted T7 tests (CONFIRMED)

### Test Status Classification

| Category | Count | Status |
|----------|-------|--------|
| **Active unit tests** | 6 | GREEN (current expected output) |
| **Ignored E2E cells** | 1 | IGNORED (T7-3 pending I-177-D) |
| **Other E2E cells** | 2 | GREEN (closure-reassign path) |
| **Deleted T7 lock-in tests** | 3 | DELETED (pre-revert 2026-04-25) |

### Impact Analysis

**Tests needing expected output updates post-I-177-D**: 1 (likely)
- `narrowing_match_suppressed_when_closure_reassign_present`: suppression scope narrowing from "fn body" → "post-if scope" may change emission form

**Tests unaffected by I-177-D**: 5
- All other tests either test non-closure-reassign paths, or non-suppression behavior

---

## Unit Tests Inventory

### 1. Control Flow Narrowing Tests

#### File: `src/transformer/statements/tests/control_flow/narrowing.rs`

| Test Name | Line | Guard Type | Body Shape | Current Expected | Closure-Reassign Behavior | I-177-D Impact |
|-----------|------|-----------|-----------|-------------------|---------------------------|----------------|
| `narrowing_match_suppressed_when_closure_reassign_present` | 14 | `=== null` | early-return | `if x.is_none() { return }` predicate form | **YES** - suppression active | **LIKELY CHANGE** - suppression scope narrowing may emit let-match instead |
| `narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign` | 92 | `!== null` | early-return | `if let Some(x) = x { }` shadow form (if-let) | **NO** - closure-reassign absent | **NO CHANGE** - narrow not suppressed when absent |

**Line count**: File is 133 total lines (post-T7 revert, added 2nd test on 2026-04-25)

**Test characteristics**:
- Both tests use `TctxFixture::from_source()` + pipeline (TypeResolver + Transformer)
- First test: verifies suppression emits predicate `is_none()` NOT match-shadow
- Second test: verifies when closure-reassign absent, standard if-let shadow is emitted

---

### 2. Bang `!x` Layer 2 Lowering Tests

#### File: `src/transformer/statements/tests/control_flow/truthy_complement_match/bang_layer_2.rs`

| Test Name | Line | Guard Type | Body Shape | Current Expected | Closure-Reassign Behavior | I-177-D Impact |
|-----------|------|-----------|-----------|-------------------|---------------------------|----------------|
| `bang_with_closure_reassign_falls_through_to_predicate_form` | 360 | Bang `!x` | early-return + non-exit | `if x.is_some_and(...)` predicate fallthrough (Layer 1) | **YES** - suppression active | **NO CHANGE** - predicate fallthrough is suppression intent |

**Test context** (lines 351-396):
- I-171 T5 P1 lock-in for Bang Layer 2
- Verifies that when closure-reassign is present, Layer 2 let-match suppressed
- Falls through to Layer 1 predicate form `if !x.is_some_and(truthy) { ... }`
- Function body includes auto-inserted `let mut x = x;` rebinding for closure-capture
- Searches for first If or Let-Match after rebinding

**Code path**: 
```
closure_reassign present
  ↓
Layer 2 returns None (suppresses)
  ↓
Layer 1 emits predicate `if !x.is_some_and(guard) { body }`
```

**Line range**: 359-396 (38 lines)

---

### 3. Null Check Symmetric Let-Wrap Tests

#### File: `src/transformer/statements/tests/control_flow/truthy_complement_match/null_check_symmetric.rs`

| Test Name | Line | Guard Type | Body Shape | Current Expected | Closure-Reassign Behavior | I-177-D Impact |
|-----------|------|-----------|-----------|-------------------|---------------------------|----------------|
| `null_check_then_exit_else_non_exit_with_closure_reassign_falls_through` | 89 | `=== null` | then-exit + else-non-exit | `if x.is_none() { exit }` predicate form | **YES** - suppression active | **NO CHANGE** - predicate fallthrough is suppression intent |

**Test context** (lines 88-123):
- I-171 T5 P2 lock-in for deep-deep-deep-fix
- Tests `=== null` symmetric to Bang Layer 2
- Verifies `try_generate_narrowing_match` 4th branch (Deep-Deep-Deep-Fix-1) also honors closure-reassign suppression
- Searches for first If or Let-Match in body
- Expected: `Stmt::If` (not `Stmt::Let { init: Some(Expr::Match) }`)

**Body shape note**: 
- then-exit: `return -1;`
- else-non-exit: empty (fallthrough)
- Normal form (no suppression): would be Let-wrap with narrow-tail

**Line range**: 88-123 (36 lines)

---

### 4. Binary/Unary Arithmetic Coerce Tests

#### File: `src/transformer/expressions/tests/binary_unary.rs`

| Test Name | Line | Guard Type | Body Shape | Current Expected | Closure-Reassign Behavior | I-177-D Impact |
|-----------|------|-----------|-----------|-------------------|---------------------------|----------------|
| `arith_coerce_when_closure_reassigned_option_lhs` | 550 | `=== null` (narrow guard) | early-return | `x.unwrap_or(0.0) + 1.0` coerce form | **YES** - narrow suppressed | **NO CHANGE** - coerce logic independent of narrow form |
| `string_concat_coerce_when_closure_reassigned_option` | 587 | `=== null` (narrow guard) | early-return | `format!("{}{}", "v=", x.map(..).unwrap_or_else(..))` | **YES** - narrow suppressed | **NO CHANGE** - coerce logic independent of narrow form |
| `arith_coerce_does_not_fire_for_non_option_closure_reassigned_var` | 637 | N/A (Vec<T>, not Option) | early-return | `Ident("xs")` plain (no coerce) | **YES** - but filtered by type guard | **NO CHANGE** - type guard filtering logic unchanged |

**Test characteristics**:
- These tests exercise the `coerce_default` logic in `Transformer::maybe_coerce_for_arith`
- They verify that closure-reassigned `Option<T>` variables are wrapped with `unwrap_or(default)`
- The third test is a negative-path lock-in: `Vec<T>` (non-Option) is NOT coerced
- None of these tests directly verify the narrow suppression mechanism itself
- They verify the **consequence** of suppression: since narrow is suppressed, variable stays `Option<T>`, so coerce applies

**I-177-D dependency**: If I-177-D changes narrow suppression scope:
- Tests 1-2 should remain valid (coerce applies to Option<T> variables)
- Test 3 remains valid (Vec<T> is never Option, filter still applies)
- However, if emission form changes (predicate vs let-match), the narrow event may differ, affecting coerce detection site

**Line ranges**: 549-584 (36 lines), 586-634 (49 lines), 636-672 (37 lines)

---

## E2E Test Cells Inventory

### Ignored Cell: `i161-i171 / cell-t7-3-and-closure-reassign`

**File**: `tests/e2e_test.rs::test_e2e_cell_i161_t7_3_and_closure_reassign`  
**Status**: `#[ignore]`  
**Line**: 1822-1848  

**Ignore annotation** (lines 1825-1845):
```
I-161 T7-3 RED — narrow × `&&=` × closure-reassign の architectural
IR/TypeResolver cohesion gap、I-177-D 完了で GREEN 化見込み。
T7 review iteration (2026-04-25) で `try_generate_narrowing_match`
に NonNullish !== closure-reassign suppression branch を追加する
workaround patch を試行したが、deep deep /check_job adversarial
review で Scenario A regression (`return x` body without `??` で
E0308 mismatch) が判明し、`ideal-implementation-primacy.md` の
interim patch 条件未充足 + structural fix を patch に降格していると
判断し revert (2026-04-25)。

Root cause: `FileTypeResolution::narrowed_type(var, position)` の
closure-reassign suppression scope が enclosing fn body 全体で broad
すぎ、cons-span 内 (if-body 内、narrow が valid な scope) も含めて
suppress すること。

Resolution: I-177-D (案 C) で cons-span 内 narrow 保持 + LET-WRAP
scope のみ suppress → IR shadow form と TypeResolver narrow が agree
→ E0599/E0282/E0308 chain 解消。
```

**Script**: `tests/e2e/scripts/i161-i171/cell-t7-3-and-closure-reassign.ts`

**Code pattern**:
```typescript
function f(): number {
    let x: number | null = 5;
    const reset = () => { x = null; };
    if (x !== null) {
        x &&= 3;  // narrow-suppressed path (closure reset exists)
        reset();
        return x ?? -1;
    }
    return -1;
}
```

**Current blockers** (pre-I-177-D):
1. IR emits shadow form: `if let Some(mut x) = x { ... x = Some(3.0); ... }`
2. TypeResolver sees narrow-suppressed Option<T> (broad scope suppression)
3. `x &&= 3` desugars expecting Option context, but IR shadow made x: T
4. `x ?? -1` lowering queries Option shape → suppressed None return → mismatch
5. Closure-mutable-capture E0506 borrow conflict on `let mut reset = || { x = None; }`

**I-177-D expected resolution**: cons-span narrow preservation + scope-limited suppression → coherent IR/TypeResolver view

---

### Green Cells: Closure-Reassign E2E Tests

#### Cell: `i144 / cell-c2b-closure-reassign-arith-read`
**File**: `tests/e2e_test.rs::test_e2e_cell_i144_c2b_closure_reassign_arith_read` (line 1337)  
**Status**: GREEN  
**Script**: `tests/e2e/scripts/i144/cell-c2b-closure-reassign-arith-read.ts`  
**Comment**: "T6-2 GREEN (2026-04-20): closure-reassign suppresses narrow shadow-let, arith-read coerce applies"

#### Cell: `i144 / cell-c2c-closure-reassign-string-concat`
**File**: `tests/e2e_test.rs::test_e2e_cell_i144_c2c_closure-reassign-string-concat` (line 1341)  
**Status**: GREEN  
**Script**: `tests/e2e/scripts/i144/cell-c2c-closure-reassign-string-concat.ts`  
**Comment**: "T6-2 GREEN: closure-reassign suppresses narrow shadow-let, string-concat coerce applies"

**Characteristics**:
- Both cells test the coerce behavior under closure-reassign suppression
- Both are NOT checking narrow suppression per se, but the coerce consequence
- Should remain GREEN post-I-177-D if coerce logic unchanged

---

## Deleted Tests Verification

### Three T7 Lock-in Tests (2026-04-25 Revert)

**Revert commit**: `74baa62` [CLOSE] I-161 + I-171 batch  
**Reason**: T7 patch (predicate form) reverted due to Scenario A regression

#### Deleted Test 1: `narrowing_match_suppressed_for_neq_null_early_return_when_closure_reassign_present`

**Status**: NOT FOUND in current codebase (confirmed deleted)

**What it tested**:
- `if (x !== null) { body; return; }` (early-return) + closure-reassign present
- T7 patch attempted: emit `if x.is_some() { ... }` predicate form
- Pre-T7 form: Let-wrap shadow `let x = match x { Some(x) => body; _ => exit }`

**Reason for deletion**: T7 patch was reverted; this test asserted T7 predicate emission which no longer applies

---

#### Deleted Test 2: `narrowing_match_suppressed_for_neq_null_with_else_then_exit_when_closure_reassign_present`

**Status**: NOT FOUND in current codebase (confirmed deleted)

**What it tested**:
- `if (x !== null) { body } else { ... return }` (then-exit with else-non-exit) + closure-reassign
- T7 patch code path for non-early-return form
- Pre-T7 form: Let-wrap

**Reason for deletion**: T7 patch was reverted

---

#### Deleted Test 3: `narrowing_match_does_not_use_predicate_form_for_truthy_when_closure_reassign_present`

**Status**: NOT FOUND in current codebase (confirmed deleted)

**What it tested**:
- Broader category: "truthy" guard (generic, not just `!== null`)
- T7 patch assertion: predicate form under closure-reassign
- Negative path lock-in: verify predicate form is used (not Let-wrap)

**Reason for deletion**: T7 patch was reverted; predicate form is no longer the expected emission

---

### Verification Method

**Git search command**:
```bash
grep -r "narrowing_match_suppressed_for_neq_null_early_return_when_closure_reassign_present" /home/kyohei/ts_to_rs --include="*.rs"
```

**Result**: No matches (CONFIRMED DELETED)

**Git history**:
- `git log --all -S "narrowing_match_suppressed_for_neq_null_early_return_when_closure_reassign_present" --oneline`
- Last appearance: commit `5877e76` ([CLOSE] I-178 + I-183 batch)
- Previous to that: commit `74baa62` (T7 revert commit)
- Pre-T7: tests were present (would be in earlier commits)

---

## Test Impact Matrix: I-177-D Expected Changes

### Refactor Scope: TypeResolver `narrowed_type()` Suppression Scope Narrowing

**Current behavior (pre-I-177-D)**:
```rust
pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
    if self.is_var_closure_reassigned(var_name, position) {
        return None;  // ← Broad: suppresses in ALL scopes (fn body)
    }
    // ... narrow event lookup
}
```

**Proposed behavior (I-177-D Case C)**:
```rust
pub fn narrowed_type(&self, var_name: &str, position: u32, narrow_scope: Span) -> Option<&RustType> {
    if self.is_var_closure_reassigned_in_scope(var_name, position, narrow_scope) {
        return None;  // ← Narrow: suppresses only in post-if scope
    }
    // ... narrow event lookup (cons-span inner retains narrow)
}
```

---

### Impact Prediction by Test

| Test | Suppression Scope Change | Expected Emission Form Change | Severity |
|------|-------------------------|-------------------------------|----------|
| `narrowing_match_suppressed_when_closure_reassign_present` | Yes (fn body → post-if) | **Possibly**: predicate `is_none()` → Let-wrap match | **HIGH** |
| `narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign` | No (not suppressed) | None | **NONE** |
| `bang_with_closure_reassign_falls_through_to_predicate_form` | No (fallthrough intent) | None (predicate form is suppression) | **NONE** |
| `null_check_then_exit_else_non_exit_with_closure_reassign_falls_through` | No (fallthrough intent) | None (predicate form is suppression) | **NONE** |
| `arith_coerce_when_closure_reassigned_option_lhs` | N/A (coerce logic) | None (coerce logic unchanged) | **NONE** |
| `string_concat_coerce_when_closure_reassigned_option` | N/A (coerce logic) | None (coerce logic unchanged) | **NONE** |
| `arith_coerce_does_not_fire_for_non_option_closure_reassigned_var` | N/A (Vec<T> filter) | None (filter logic unchanged) | **NONE** |

---

### Expected Test Update Requirement

**Question**: Will `narrowing_match_suppressed_when_closure_reassign_present` need its expected output updated?

**Answer**: **LIKELY YES** (depends on I-177-D implementation detail)

**Reasoning**:
1. Test name says "suppressed" — suppression still exists post-I-177-D
2. BUT suppression scope changes from "fn body" to "post-if scope"
3. Current expected: `if x.is_none() { return }` (predicate form = "narrow NOT materialized")
4. Post-I-177-D: If cons-span (`if` guard + body) retains narrow, then:
   - IR shadow form `let x = match x { None => return, Some(x) => x }` becomes possible
   - TypeResolver narrow query inside `if` body = not suppressed
   - Test may need: expected output = Let-wrap form instead of predicate form

**Verification needed at I-177-D implementation time**:
- Does Case C implementation materialize narrow in cons-span?
- If yes: this test must be updated to expect Let-wrap
- If no (only post-if): test remains unchanged

**Action item for I-177-D task list**: 
```
UPDATE EXPECTED: narrowing_match_suppressed_when_closure_reassign_present
- Current: expects `if x.is_none() { return }` predicate
- Verify post-refactor expected form
- If narrow materialized in cons-span: update to expect Let-wrap match
- If not: assertion remains valid, no update needed
- Confirm during implementation/testing phase
```

---

## Summary Table: Complete Closure-Reassign Test Registry

| # | Test Name | File | Line | Type | Status | Closure-Reassign | Expected Form | I-177-D Impact |
|---|-----------|------|------|------|--------|------------------|---------------|----------------|
| 1 | `narrowing_match_suppressed_when_closure_reassign_present` | narrowing.rs | 14 | Unit | ACTIVE | YES (suppressed) | predicate `is_none()` | **POSSIBLE UPDATE** |
| 2 | `narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign` | narrowing.rs | 92 | Unit | ACTIVE | NO (absent) | if-let shadow | NO CHANGE |
| 3 | `bang_with_closure_reassign_falls_through_to_predicate_form` | bang_layer_2.rs | 360 | Unit | ACTIVE | YES (suppression) | predicate fallthrough | NO CHANGE |
| 4 | `null_check_then_exit_else_non_exit_with_closure_reassign_falls_through` | null_check_symmetric.rs | 89 | Unit | ACTIVE | YES (suppression) | predicate fallthrough | NO CHANGE |
| 5 | `arith_coerce_when_closure_reassigned_option_lhs` | binary_unary.rs | 550 | Unit | ACTIVE | YES (context) | coerce unwrap_or | NO CHANGE |
| 6 | `string_concat_coerce_when_closure_reassigned_option` | binary_unary.rs | 587 | Unit | ACTIVE | YES (context) | coerce format! | NO CHANGE |
| 7 | `arith_coerce_does_not_fire_for_non_option_closure_reassigned_var` | binary_unary.rs | 637 | Unit | ACTIVE | YES (context) | plain ident | NO CHANGE |
| 8 | `test_e2e_cell_i144_c2b_closure_reassign_arith_read` | e2e_test.rs | 1337 | E2E | GREEN | YES (context) | coerce result | NO CHANGE |
| 9 | `test_e2e_cell_i144_c2c_closure_reassign_string_concat` | e2e_test.rs | 1341 | E2E | GREEN | YES (context) | coerce result | NO CHANGE |
| 10 | `test_e2e_cell_i161_t7_3_and_closure_reassign` | e2e_test.rs | 1846 | E2E | IGNORED (T7-3 RED) | YES (root cause) | IR/TypeResolver mismatch | **GREEN EXPECTED** |
| X1 | `narrowing_match_suppressed_for_neq_null_early_return_when_closure_reassign_present` | (deleted) | — | Unit | DELETED (T7 revert) | YES | predicate (T7) | DELETED |
| X2 | `narrowing_match_suppressed_for_neq_null_with_else_then_exit_when_closure_reassign_present` | (deleted) | — | Unit | DELETED (T7 revert) | YES | predicate (T7) | DELETED |
| X3 | `narrowing_match_does_not_use_predicate_form_for_truthy_when_closure_reassign_present` | (deleted) | — | Unit | DELETED (T7 revert) | YES | predicate (T7) | DELETED |

---

## Recommendations for I-177-D Test Management

### Phase 1: Pre-Implementation
- [ ] Document current expected output for test #1 in detail
- [ ] Confirm I-177-D Case C implementation plan for cons-span narrow scope
- [ ] Determine whether cons-span inner `if` body narrows are materialized

### Phase 2: Implementation
- [ ] Update test #1's assertion IF narrow materialization changes
- [ ] Verify tests #2-7 still pass with new narrow-query scope
- [ ] Monitor test #10 (t7-3) for GREEN transition after implementation

### Phase 3: Regression Verification
- [ ] Re-run all 7 active unit tests post-implementation
- [ ] Verify 3 deleted tests remain absent (no accidental resurrection)
- [ ] Confirm E2E GREEN cells remain GREEN
- [ ] Check whether test #10 (t7-3) can be unignored (may require I-048 follow-up)

### Test Count Summary
- **Total unique closure-reassign tests**: 10 (7 active + 1 ignored E2E + 2 other E2E)
- **Affected by I-177-D scope narrowing**: 1 unit test (high likelihood)
- **Estimated update effort**: ~20+ test assertions (per TODO entry), mostly in test #1 and possibly #10 E2E assertions

---

## Appendix: Test File Locations

```
src/transformer/statements/tests/control_flow/
├── narrowing.rs                                (2 tests)
└── truthy_complement_match/
    ├── mod.rs                                  (1 test: H-3 mixed-union)
    ├── bang_layer_2.rs                         (1 closure-reassign test + 5 other)
    ├── null_check_symmetric.rs                 (1 closure-reassign test + 1 other)
    └── synthetic_union.rs                      (union-specific tests)

src/transformer/expressions/tests/
└── binary_unary.rs                             (3 closure-reassign context tests + ~50 other)

tests/
├── e2e_test.rs                                 (1 ignored T7-3 + 2 GREEN cells)
└── e2e/scripts/
    ├── i144/cell-c2b-closure-reassign-arith-read.ts
    ├── i144/cell-c2c-closure-reassign-string-concat.ts
    └── i161-i171/cell-t7-3-and-closure-reassign.ts
```

---

**Report generated**: 2026-04-25  
**Codebase state**: Post-T7 revert, pre-I-177-D implementation  
**Git commits verified**: 74baa62, 5877e76, 0efe022  
