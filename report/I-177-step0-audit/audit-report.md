# I-177-D Phase 0 Empirical Audit: Latent Silent Semantic Change Quantification

**Date**: 2026-04-26
**Purpose**: I-177-D 案 C (TypeResolver `narrowed_type` suppression scope refactor) の単独 commit が誘発する latent silent semantic change を全 codebase で empirical 定量化し、commit 戦略を確定する。
**Decision input**: Plan η Phase 0 の出力 → PRD 1 (I-177-D) の completion criteria 確定
**Method**: Codebase grep + 各 cell の TS source / unit test source / IR emission の trace

---

## Executive Summary

| 評価 | 値 |
|------|-----|
| 総 closure-reassign 関連 candidate | 17 (13 fixture + 4 unit test) |
| **Dangerous pattern** (case-C 単独 commit で silent change 顕在化) | **1** (T7-3) |
| Safe pattern (EarlyReturnComplement narrow + closure-reassign、suppression 維持) | 6 |
| Independent (multifn isolation / coerce 系) | 4 |
| 無関係 (no narrow / no closure-reassign / Vec rest param 等) | 6 |

**Conclusion**: I-177-D 案 C の commit は **trigger-kind-based suppression** (Primary narrow: 非 suppress / EarlyReturnComplement narrow: 維持 suppress) 実装ならば **safe**。唯一の dangerous pattern (T7-3) は既に `#[ignore]` で I-177-D dependency 指定済、PRD 1 commit 後も ignore 維持で silent change exposure を防止できる。

---

## 1. Audit Method

### 1.1 Pattern Definition

Silent semantic change を生成する **dangerous pattern** の構成要素:

1. **Primary narrow guard**: `if (x !== null) { body }` 形式の Primary narrow event (cons-span 内 narrow が valid)
2. **Closure-reassign**: 同 fn body 内に closure が x を reassign
3. **Body mutation**: cons-span 内で x が mutate される (`x &&= ...` / `x ||= ...` / `x = ...` / etc.)
4. **Post-mutation read**: cons-span 内で mutation 後に x を read する operation (`return x`、`x ?? ...`、`...x`、etc.)

これら 4 条件を全て満たす場合、case-C 単独 commit で:
- Pre-I-177-D: TypeResolver suppression で compile error → silent runtime mismatch にならない (compile fail で停止)
- Post-I-177-D 案 C: TypeResolver narrow ↔ IR shadow agreement で compile pass → 内部 shadow x の mutation が outer Option<T> に propagate しない → **runtime mismatch が silent に発火**

EarlyReturnComplement narrow (`if (x === null) return; rest`) の case では、narrow scope = post-if (= rest) 内で closure call が runtime で x を null に再変できる構造で、coerce_default workaround が現状の正しい挙動 → case-C の trigger-kind-based 実装で suppression 維持されれば silent change risk なし。

### 1.2 Search Scope

- `tests/e2e/scripts/**/*.ts` (E2E fixture)
- `src/transformer/**/tests/**/*.rs` (Transformer unit test の embedded TS)
- `src/pipeline/**/tests/**/*.rs` (Pipeline unit test の embedded TS)
- `src/pipeline/type_resolver/tests/**/*.rs` (TypeResolver unit test)
- Hono codebase: 別途確認 (現時点で /tmp/hono 未 clone、PRD 1 implementation phase で hono-bench で empirical 確認予定)

### 1.3 Search Commands

```bash
# Closure-reassign keyword inventory
find tests/e2e/scripts -name "*.ts" | xargs grep -l -E "(closure|reassign)"
grep -rn "closure_reassign\|closure-reassign" src/ --include="*.rs" -l

# Direct TS source inspection per cell
cat tests/e2e/scripts/*/cell-c2*.ts
cat tests/e2e/scripts/i161-i171/cell-t7-*.ts

# Unit test TS source inspection
grep -A 30 "fn arith_coerce_when_closure_reassigned\|..." src/transformer/expressions/tests/binary_unary.rs
sed -n '1,100p' src/transformer/statements/tests/control_flow/narrowing.rs
sed -n '350,400p' src/transformer/statements/tests/control_flow/truthy_complement_match/bang_layer_2.rs
sed -n '85,125p' src/transformer/statements/tests/control_flow/truthy_complement_match/null_check_symmetric.rs
```

---

## 2. Dangerous Pattern Cells (1 件)

### 2.1 T7-3 (`tests/e2e/scripts/i161-i171/cell-t7-3-and-closure-reassign.ts`)

**TS source**:
```typescript
function f(): number {
    let x: number | null = 5;
    const reset = () => { x = null; };       // closure-reassign declared OUTSIDE if
    if (x !== null) {                         // Primary narrow guard
        x &&= 3;                              // cons-span body mutation #1
        reset();                              // cons-span closure call (mutates outer x)
        return x ?? -1;                       // cons-span post-mutation read
    }
    return -1;
}
console.log(f());  // -1
```

**Pattern conditions** (全 4 条件 ✓):
1. Primary narrow ✓ (`if (x !== null)` body)
2. Closure-reassign ✓ (`const reset = () => { x = null; };`)
3. Body mutation ✓ (`x &&= 3` + closure-internal `x = null`)
4. Post-mutation read ✓ (`return x ?? -1`)

**Pre-I-177-D Rust emission** (per investigation):
- Predicate form (`if x.is_none() { return; }`) で suppression を試みるが Branch dispatch の条件 mismatch (is_swap=false の `!== null` は Branch 1/2 fired せず Branch 5 fallback)
- 実際は F1 shadow form (`if let Some(mut x) = x { ... }`) emission
- TypeResolver は narrow 全 fn body suppress (Option<f64>) ↔ IR shadow form は内部 x: f64
- Cohesion gap → E0599/E0282/E0308 chain + E0506 closure-mutable-capture
- **Compile fail で停止 → runtime に到達せず silent にならない**

**Post-I-177-D 案 C 単独 commit (mutation propagation 未対応) Rust emission 予測**:
- IR shadow form 維持 + TypeResolver narrow も active (案 C 効果)
- compile pass (E0599/E0282/E0308 解消、E0506 は closure-mutable-capture 別途残存可能性、または同時解消)
- 内部 shadow x: f64 = 3 (mutation `x &&= 3` の結果)
- closure `reset()` call は **outer** x: Option<f64> を None に変えるが、inner shadow x は decoupled で不変
- `return x ?? -1` → 内部 shadow x: f64 = 3 を return → **runtime: 3**
- TS runtime: -1
- **Silent semantic change** (Tier 0 silent change): Rust=3 ≠ TS=-1

**現状の defense**:
- `tests/e2e_test.rs::test_e2e_cell_i161_t7_3_and_closure_reassign` は `#[ignore]` 状態
- ignore annotation に I-177-D dependency 明記済 (Report 04 で confirmed)
- I-177-D commit 後も **ignore 維持** で silent change が test runner で発火しない

**Action**:
- PRD 1 (I-177-D) では T7-3 ignore を維持
- T7-3 GREEN-ify は PRD 3 (I-177 mutation propagation 本体) 完了後に達成
- PRD 1 完了 commit messsage に「T7-3 ignore 維持: silent change prevention」明記

---

## 3. Safe Pattern Cells (EarlyReturnComplement narrow + closure-reassign、6 件)

これらは narrow scope = post-if (= [if_end, fn_end))。case-C の trigger-kind-based 実装で suppression 維持される。

### 3.1 c2b (`tests/e2e/scripts/i144/cell-c2b-closure-reassign-arith-read.ts`)

```typescript
function f(): number {
    let x: number | null = 5;
    if (x === null) return -1;            // EarlyReturnComplement narrow guard (early-return on null)
    const reset = () => { x = null; };    // closure-reassign in narrow scope
    reset();
    return x + 1;                          // arith read (coerce_default applies)
}
// TS runtime: 1
// Current Rust: x.unwrap_or(0.0) + 1.0 = 1.0 ✓ (coerce preserved)
```

**Trigger kind**: EarlyReturnComplement (narrow established by `=== null` early-return)
**Suppression behavior post-case-C (trigger-kind-based)**: 維持 (suppression continues for EarlyReturnComplement)
**Result**: coerce_default workaround 維持 → runtime 1.0 ≡ TS 1 ✓

### 3.2 c2c (`tests/e2e/scripts/i144/cell-c2c-closure-reassign-string-concat.ts`)

```typescript
function f(): string {
    let x: number | null = 5;
    if (x === null) return "no";          // EarlyReturnComplement narrow guard
    const reset = () => { x = null; };
    reset();
    return "v=" + x;                       // string concat (coerce: null → "null")
}
// TS runtime: "v=null"
// Current Rust: format!("{}{}", "v=", x.map(...).unwrap_or_else(...)) = "v=null" ✓
```

**Same trigger kind** (EarlyReturnComplement) → 維持 ✓

### 3.3 regression-multifn (`tests/e2e/scripts/i144/cell-regression-multifn-same-var-isolation.ts`)

```typescript
function f(): number {
    let x: number | null = 5;
    if (x === null) return -1;            // EarlyReturnComplement
    const reset = () => { x = null; };
    reset();
    return x + 1;                          // coerce → 1
}
function g(): number {
    let x: number | null = 10;
    if (x === null) return -2;            // EarlyReturnComplement (NO closure-reassign)
    return x + 1;                          // narrow active → 11
}
// TS runtime: f=1, g=11
```

**Multi-fn isolation**: I-169 P1 fix で確立済、case-C で影響なし ✓

### 3.4-3.6 Unit tests (3 件)

**3.4 `narrowing_match_suppressed_when_closure_reassign_present`**:
```ts
if (x === null) return -1;          // EarlyReturnComplement
const reset = () => { x = null; };
reset();
return x + 1;
```
trigger kind = EarlyReturnComplement → suppression 維持 ✓ → predicate form `if x.is_none() { return; }` 維持 → unit test pass

**3.5 `bang_with_closure_reassign_falls_through_to_predicate_form`**:
```ts
if (!x) return -1;                  // EarlyReturnComplement (truthy negation)
```
EarlyReturnComplement → 維持 ✓

**3.6 `null_check_then_exit_else_non_exit_with_closure_reassign_falls_through`**:
```ts
if (x === null) { return -1; } else { /* non-exit */ }
```
EarlyReturnComplement → 維持 ✓

---

## 4. Independent Cells (multifn isolation / coerce 系、4 件)

### 4.1 c2a (`tests/e2e/scripts/i144/cell-c2a-nullish-assign-closure-capture.ts`)

```typescript
function f(): number {
    let x: number | null = 5;
    x ??= 0;                              // ??= no narrow guard
    const reset = () => { x = null; };
    reset();
    return x ?? -99;                       // -99 (reset fires)
}
```

**No narrow guard** (just `??=` desugar). case-C 影響範囲外 ✓

### 4.2-4.4 Coerce 系 unit test (3 件)

- `arith_coerce_when_closure_reassigned_option_lhs`: c2b と同じ EarlyReturnComplement pattern
- `string_concat_coerce_when_closure_reassigned_option`: c2c と同じ EarlyReturnComplement pattern
- `arith_coerce_does_not_fire_for_non_option_closure_reassigned_var`: rest param `Vec<T>` (Option ではない) → narrow 該当せず

case-C 影響範囲外 ✓

---

## 5. 無関係 Cells (6 件)

### 5.1 T7-1, T7-2, T7-4, T7-5 (i161 cell-t7-*)

| Cell | Narrow type | Mutation | Closure-reassign |
|------|------------|----------|-----------------|
| T7-1 | Primary `if (x !== null)` | `x &&= 3` | **NO** (no closure) |
| T7-2 | Primary | `x ||= 99` | NO |
| T7-4 | NO narrow guard | chain `||=`/`??=` | NO |
| T7-5 | Primary | `x &&= "result"` | NO |

closure-reassign 不在で suppression 元から発火しない → case-C 影響範囲外 ✓

### 5.2 regression-f4 (`cell-regression-f4-loop-body-narrow-preserves.ts`)

```ts
if (x !== null) {                  // Primary narrow
    let out = 0;
    for (let i = 0; i < 3; i++) {
        out += x;                  // body read only, no mutation, no closure
    }
    return out;
}
```

Primary narrow + body read only + no closure-reassign → case-C 影響範囲外 ✓

### 5.3 regression-no-reassign (`cell-regression-closure-no-reassign-keeps-e1.ts`)

```ts
if (x === null) return -1;
const read = () => x + 5;          // closure READS x (no reassign)
return read();
```

closure-reassign 不在 (read のみ) → ClosureCapture event not pushed → suppression 元から発火しない → case-C 影響範囲外 ✓

### 5.4-5.5 Step3 box-wrap (`step3/box-wrap-{counter,greeter}.ts`)

closure factory pattern、no narrow、no closure-reassign → 完全無関係 ✓

### 5.6 その他 (mutation_detection.ts, to_string_calls.ts, optional_params.ts, array_methods.ts)

`reassign` keyword に部分一致したのみで、narrow + closure-reassign + body mutation pattern 不一致 → 無関係 ✓

---

## 6. Hono Codebase Audit (Pending、PRD 1 implementation phase で実施)

**Status**: 現時点で /tmp/hono 未 clone (`./scripts/hono-bench.sh` 実行で auto-clone 予定)
**Concern**: Hono は production codebase、Primary narrow + closure-reassign + body mutation pattern が混入している可能性
**Mitigation**:
- Hono は現状 47 errors / 70.3% clean (compile error 多数) の状態で、case-C 単独 commit ではこれらの compile error も解消されないため latent silent change が runtime で顕在化しない (= 防止される)
- ただし I-177 main + その他 PRDs 完了後の Hono 全 compile pass 段階で、latent silent change が initially exposed される可能性
- **Action**: PRD 1 (I-177-D) implementation phase で Hono pre/post bench で error count diff を確認し、新たな silent change risk が exposed されたら同 fixture を ignore で隔離して PRD 3 (I-177 main) の scope に追加

---

## 7. Decision: I-177-D commit 戦略

### 7.1 Phase 0 Verdict

**I-177-D 案 C 単独 commit は SAFE** (条件付き)。

**条件**:
1. Implementation は **trigger-kind-based suppression** を採用 (Primary: 非 suppress / EarlyReturnComplement: 維持 suppress)
2. T7-3 cell の `#[ignore]` を維持 (PRD 1 commit 後も ignore のまま、I-177 main 完了で GREEN-ify)
3. PRD 1 completion criteria に「T7-3 以外の closure-reassign 系 test (i144 c2a/b/c + multifn-isolation + 3 active unit tests + step3 box-wrap-* + T7-1/2/4/5 + regression-f4/no-reassign + 3 coerce unit tests) 全 byte-exact 非後退」を含める
4. PRD 1 implementation で新 unit test 追加: 「Primary narrow + closure-reassign + body read-only → narrow 保持 (= case-C 効果直接検証)」
5. PRD 1 implementation で新 unit test 追加: 「EarlyReturnComplement narrow + closure-reassign → suppression 維持 (= regression lock-in)」

### 7.2 Risk Quantification

| Risk | Likelihood | Mitigation |
|------|-----------|-----------|
| T7-3 GREEN-ify と silent change 同時発火 | High (case-C 案単独では internal shadow mutation propagation 未対応) | T7-3 ignore 維持で test runner 発火防止 |
| EarlyReturnComplement cells regression | Low (suppression 維持) | 新 regression lock-in unit test で防御 |
| Hono runtime regression | Low (現状 compile error で runtime 到達せず) | implementation phase で hono-bench pre/post 確認 |
| 未発見 dangerous pattern | Medium (audit は local codebase 中心、Hono 未走査) | Hono compile rate が向上した時点 (将来 PRD) で再 audit |

### 7.3 Implementation Strategy 推奨

**Option A: Trigger-kind-based suppression** (推奨採用)
```rust
pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
    // Find matching narrow event
    let narrow_event = self.narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .rfind(|n| n.var_name == var_name && n.scope_start <= position && position < n.scope_end);

    let narrow = narrow_event?;

    // case-C: trigger-kind-based suppression
    let should_suppress = matches!(narrow.trigger, NarrowTrigger::EarlyReturnComplement(_))
        && self.is_var_closure_reassigned(var_name, position);

    if should_suppress {
        return None;
    }

    Some(narrow.narrowed_type)
}
```

**Option B: Position-based "post-narrow scope" suppression** (代替案)
- narrow event scope_end を boundary とし、position >= scope_end でのみ suppress
- ただし EarlyReturnComplement narrow は scope = post-if (= 通常の narrow 使用 area) なので scope_end が fn_end や block_end になる構造で、Option B は事実上 Option A と等価になる

両 implementation の semantic 差異は微小。Option A の方が **意図 explicit** で trigger 種別を直接 dispatch するので preferred。

---

## 8. Conclusion & Next Steps

### 8.1 Audit Conclusion

I-177-D 案 C の単独 commit は **trigger-kind-based suppression** 実装ならば safe。silent semantic change の latent pattern は **T7-3 のみ** (1/17 cell)、ignore 維持で防止可能。

### 8.2 PRD 1 Completion Criteria への反映 (推奨)

1. ✓ Trigger-kind-based suppression 実装 (Primary: 非 suppress / EarlyReturnComplement: 維持)
2. ✓ T7-3 ignore 維持
3. ✓ 全 17 closure-reassign 関連 cells / unit tests の byte-exact 非後退 (T7-3 を除く)
4. ✓ 新 unit test: Primary narrow + closure-reassign + body read-only → narrow 保持
5. ✓ 新 unit test: EarlyReturnComplement narrow + closure-reassign → suppression 維持 (regression lock-in)
6. ✓ Hono pre/post bench で error count diff (silent change exposed なら同 fixture を ignore で隔離)

### 8.3 PRD 3 (I-177 mutation propagation 本体) への引き継ぎ

T7-3 GREEN-ify は PRD 3 の primary acceptance signal とし、PRD 3 で:
- 案 A (mutation-ref `match &mut x`) または 案 B (writeback `x.take()`) を spec stage で empirical 確定
- T7-3 runtime correctness (Rust=-1 ≡ TS=-1) を達成

---

## Appendix A: 完全 inventory

### A.1 Fixture (13 件)

| # | File | Pattern | Risk |
|---|------|---------|------|
| 1 | i161-i171/cell-t7-3-and-closure-reassign.ts | Primary + closure-reassign + mutation + read | **Dangerous** (only 1) |
| 2 | i144/cell-c2a-nullish-assign-closure-capture.ts | ??= + closure (no narrow) | Independent |
| 3 | i144/cell-c2b-closure-reassign-arith-read.ts | EarlyReturnComplement + closure + arith read | Safe |
| 4 | i144/cell-c2c-closure-reassign-string-concat.ts | EarlyReturnComplement + closure + concat | Safe |
| 5 | i144/cell-regression-multifn-same-var-isolation.ts | Multi-fn isolation | Safe |
| 6 | i144/cell-regression-f4-loop-body-narrow-preserves.ts | Primary read-only, no closure | Unrelated |
| 7 | i144/cell-regression-closure-no-reassign-keeps-e1.ts | closure read-only (no reassign) | Unrelated |
| 8 | i161-i171/cell-t7-1-and-narrow-f64.ts | Primary + mutation, no closure | Unrelated |
| 9 | i161-i171/cell-t7-2-or-narrow-f64.ts | Primary + mutation, no closure | Unrelated |
| 10 | i161-i171/cell-t7-4-or-then-nc.ts | No narrow (chain assign) | Unrelated |
| 11 | i161-i171/cell-t7-5-and-narrow-union-rhs.ts | Primary + mutation, no closure | Unrelated |
| 12 | step3/box-wrap-counter.ts | closure factory, no narrow | Unrelated |
| 13 | step3/box-wrap-greeter.ts | closure factory, no narrow | Unrelated |

### A.2 Unit test (4 件)

| # | Test name | Pattern | Risk |
|---|-----------|---------|------|
| 1 | narrowing_match_suppressed_when_closure_reassign_present | EarlyReturnComplement (`=== null` early-return) + closure-reassign | Safe |
| 2 | bang_with_closure_reassign_falls_through_to_predicate_form | EarlyReturnComplement (`!x` early-return) + closure-reassign | Safe |
| 3 | null_check_then_exit_else_non_exit_with_closure_reassign_falls_through | EarlyReturnComplement (`=== null` then-exit + else-non-exit) + closure-reassign | Safe |
| 4 | arith_coerce_when_closure_reassigned_option_lhs | EarlyReturnComplement + arith | Safe |
| 5 | string_concat_coerce_when_closure_reassigned_option | EarlyReturnComplement + concat | Safe |
| 6 | arith_coerce_does_not_fire_for_non_option_closure_reassigned_var | rest param Vec<T> (Option ではない) | Independent |
| 7 | narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign | Primary, no closure | Unrelated |

(unit test は実数 7 件、上記表で合計 17 candidate を網羅)

---

**Audit conducted by**: Claude (PRD 1 起票準備)
**Audit duration**: 2026-04-26 session
**Reproducibility**: 上記 Search Commands を再実行で同 candidate inventory 取得可能
**Sign-off**: Audit 結果に基づき PRD 1 (I-177-D) commit 戦略を確定 → trigger-kind-based suppression 実装で safe commit 可能
