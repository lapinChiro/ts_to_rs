# I-161 + I-171 T7 — Classifier × Emission Cohesion (Verification Report)

**Date**: 2026-04-25 (initial), **2026-04-25 revert** (final state)
**Stage**: T7 of I-161 + I-171 batch PRD
**Goal**: Empirically verify that the new `&&=` / `||=` emission path
preserves narrow semantics across the matrix of `(narrow trigger) ×
(logical assign op) × (RHS shape)` cells, and locate any classifier ↔
emission cohesion gaps before closing the PRD.

**Final state (2026-04-25)**: cohesion gap **発見・documented** + I-177-D
PRD 起票で architectural fix を委譲。T7 内での fix attempt (workaround
patch) は deep deep `/check_job` で発見した Scenario A regression を回避
するため **revert**。本 report は cohesion gap の verification 成果と
revert 経緯の最終 record。

## Executive summary

T7 は当初の目的「classifier × emission cohesion verification」を達成。
T7-3 cell で **IR / TypeResolver lane coherence の architectural gap** を
empirically 発見し、その root cause として `FileTypeResolution::narrowed_type`
の closure-reassign suppression scope が enclosing fn body 全体に broad
すぎる事実を trace 特定。

T7 review iteration 中に試行した workaround patch (`try_generate_narrowing_match`
に NonNullish !== suppression branch 追加) は deep deep `/check_job`
adversarial review で **Scenario A regression** (`return x` body without
`??` × closure-reassign で E0308 mismatch) を導入していると判明し、
`ideal-implementation-primacy.md` の interim patch 条件を満たさない
**structural fix の patch 降格** と評価。**revert** 実施し、architectural
fix を **I-177-D** PRD に委譲。

PRD T7 cells T7-1 / T7-2 / T7-4 / T7-5 は un-ignored 状態を維持
(GREEN、本 cell は closure-reassign 不在で T7 fix 不発火、revert 影響なし)。
T7-3 は pre-T7 RED 状態に戻り、annotation を I-177-D dependency 反映に
更新。T7-6 unit test (narrow × incompatible RHS error-path) は維持
(dispatch lane separation の structural lock-in、T7 fix と独立)。

## Cell-by-cell verification results

| Cell | Trigger × Op | LHS type | Pre-T7 (= 現状) | Notes |
|------|-------------|----------|----------------|-------|
| T7-1 | `&&=` on narrowed F64 (R4 re-host from I-144) | `number \| null` → `number` | ✓ GREEN | closure-reassign 不在、`if let Some(mut x) = x { ... }` shadow + narrow event 正常消費。R4 lock-in。 |
| T7-2 | `\|\|=` on narrowed F64 | `number \| null` → `number` | ✓ GREEN | 同上、truthy `x` で `\|\|=` は no-op、narrow 維持。 |
| T7-3 | `&&=` + closure reassign of outer var | `number \| null` (closure-reassigned) | ⚠ RED, **I-177-D dependency** | T7 で **architectural cohesion gap** を発見・documented。fix は I-177-D で structural に解消予定。 |
| T7-4 | `\|\|=` then `??=` chain on narrowed | `number \| null` | ✓ GREEN | mixed-op chain composes correctly。 |
| T7-5 | `&&=` on narrowed synthetic union with `string` RHS | `number \| string \| null` → `number \| string` | ✓ GREEN | closure-reassign 不在、synthetic-union coercion 正常。 |

T7-6 unit tests (`src/transformer/statements/tests/compound_logical_assign/error_path.rs`):

- `narrow_incompatible_rhs_f64_and_string_does_not_intercept`
- `narrow_incompatible_rhs_f64_or_string_does_not_intercept`

これら 2 件は dispatch lane separation の structural contract (narrow ×
incompatible RHS が TypeResolver lane を侵さず desugar IR を well-formed
emit、Tier 2 compile fail を rustc に委譲、Tier 1 silent miscompile 防止)
を lock-in。T7 fix と独立で **維持**。

## T7-3 architectural cohesion gap の trace

### Pre-T7 / post-revert emission

```rust
fn f() -> f64 {
    let mut x: Option<f64> = Some(5.0);
    let mut reset = || { x = None; };
    if let Some(mut x) = x {                                       // ← shadow x: T = f64
        if x.is_some_and(|v| v != 0.0 && !v.is_nan()) {            // ← Option method on f64 shadow
            x = Some(3.0);                                         // ← Option<f64> RHS to f64 LHS
        }
        reset();
        return x.unwrap_or_else(|| -1.0);                          // ← Option method on f64 shadow
    }
    -1.0
}
```

Compile errors:

- E0599: `is_some_and` not found on `f64`
- E0282: closure type inference failure
- E0308: mismatched types (`Option<f64>` to `f64`)
- E0599: `unwrap_or_else` not found on `f64`
- E0506: `let mut reset = || { x = None; }` mutably borrows `x`,
  conflicting with subsequent `if let Some(mut x) = x` read

### Root cause (architectural)

```rust
// src/pipeline/type_resolution.rs::narrowed_type
pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
    if self.is_var_closure_reassigned(var_name, position) {
        return None;  // ← cons-span / post-if scope 区別なく一律 suppress
    }
    // ...
}
```

`is_var_closure_reassigned` は `enclosing_fn_body` (fn body 全 span) で
position match すれば true 返却。closure-reassign suppression の本来の
意図は「**LET-WRAP shadow** が closure の outer Option<T> reassign を
破壊するのを防ぐ」(`let x = match x { Some(x) => x, _ => exit; };` の
outer x 上書き回避) だが、suppression scope は「fn 全体」になっており、
本来 narrow 保持が valid な if-body (cons-span) 内も含めて narrow を
抑制している。

T7-3 の symptom はこの broad suppression が cons-span 内に作用した結果:

- Narrow event は detect_narrowing_guard で正しく push される (cons-span
  scope で x: T)。
- IR 側 `if let Some(mut x) = x { ... }` shadow は cons-span 内で x: T を
  bind する。
- TypeResolver 側 `narrowed_type(x, cons-span position)` query は
  closure-reassigned suppression で None 返却 → declared type Option<T>
  fallback。
- 下流 lowering (`convert_assign_expr` for `x &&= 3` / `??` desugar /
  `format!`) が TypeResolver query で Option<T> を取得 → Option-shape
  emission を生成。
- IR shadow (T) と TypeResolver-driven Option-shape emission の
  mismatch で E0599 chain。

## T7 review iteration の経緯と revert 判断

### Workaround patch attempt (T7 内、deep / deep deep `/check_job` で iteration)

T7 review iteration で以下の patch を試行:

1. **Initial fix**: `try_generate_narrowing_match` に
   `complement_is_none && !is_swap && closure_reassigned` 条件下で
   `if x.is_some() { body }` predicate form を emit する branch を追加。
2. **Initial /check_job**: Truthy guard 誤発火 (silent semantic change
   risk for `Some(0.0)` etc.) を発見、guard variant pattern-match
   `NarrowingGuard::NonNullish { is_neq: true, .. }` で structural restrict。
3. **Deep /check_job**: INV-2 path 3 symmetric coverage 欠落
   (`!== null + with-else + then-exit + non-exit-else + closure-reassign`)
   を発見、sub-case (b) 追加 (`if x.is_some() { body+exit; } rest;`)。
4. **Deep deep /check_job**: 以下 2 件発見:
   - sub-case (b) test の inline-after-if structural property 不完全
     (empty else で false-positive)、`marker_call()` 追加で test 強化。
   - **Scenario A regression**: empirical probe で `if (x !== null) {
     return x; }` + closure later (narrow-T-shape body without `??`)
     が post-T7 patch で `if x.is_some() { return x; }` → `Option<f64>`
     for `f64` return → E0308 mismatch を発見。pre-T7 shadow form は
     ✓ だった。

### Body shape × emission form の構造的 trade-off matrix

| Body shape × closure-reassign | Pre-T7 (shadow form) | Post-T7 patch (predicate form) |
|------------------------------|----------------------|------------------------------|
| narrow-T-shape (`return x` w/o `??`) | ✓ shadow x: T → return T | ✗ x: Option<T>, return Option<T> for T → E0308 |
| Option-shape (`x &&= 3`, `?? default`) | ✗ TypeResolver Option<T> vs IR shadow T → E0599 etc. | ✓ consistent Option<T> view |

**どちらの form でも一部の body shape が破綻する**。私の T7 patch は
Option-shape を ✓ にする workaround patch で、Scenario A (narrow-T-shape)
を犠牲にしている。`ideal-implementation-primacy.md` の interim patch
許容条件 (1) PRD 起票 / (2) `// INTERIM: <id>` コメント / (3) silent
semantic change なし / (4) removal criteria の 4 条件のうち (2)(4) が
未充足、加えて T7 patch は **structural fix の patch 降格** に該当
(architectural root cause = TypeResolver suppression scope の broadness、
fix で対処すべき)。

### User judgement (2026-04-25)

俯瞰分析の結果、user 判断で **選択 A: T7 patch を revert** + **Tier 2/3
framework rule 追加** を採用:

1. T7 patch revert: `ideal-implementation-primacy.md` 完全準拠、Scenario A
   regression を knowingly commit せず、git history を clean に保つ。
2. I-177-D 起票 (Tier 1 architectural): `narrowed_type` suppression scope
   refactor で root cause 解消。
3. I-178 Rule 10 追加 (Tier 2 design framework): Cross-axis matrix
   completeness rule で spec stage 側の defect 前置発見。
4. I-183 起票 (Tier 3 process): `/check_job` review プロセス 4 層化で
   implementation stage 側の defect 前置発見。

## Revert scope (実施内容、2026-04-25)

### 削除 (production code + test)

- `src/transformer/statements/control_flow.rs`:
  - `try_generate_narrowing_match` 内の T7 closure-reassign suppression
    branch (NonNullish !== sub-case (a) `if x.is_some() { body+exit; }` +
    sub-case (b) `if x.is_some() { body+exit; } rest;`、~80 LOC) を削除。
  - `then_exits` / `else_exits` の hoisting を path 3 元位置に戻す。

- `src/transformer/statements/tests/control_flow/narrowing.rs`:
  - 3 件の T7 lock-in test を削除:
    - `narrowing_match_suppressed_for_neq_null_early_return_when_closure_reassign_present`
    - `narrowing_match_does_not_use_predicate_form_for_truthy_when_closure_reassign_present`
    - `narrowing_match_suppressed_for_neq_null_with_else_then_exit_when_closure_reassign_present`

### 維持 (pre-T7 状態の lock-in / T7 fix と独立な成果)

- `src/transformer/statements/tests/control_flow/narrowing.rs`:
  - `narrowing_match_suppressed_when_closure_reassign_present` (path 2
    既存 I-144 由来 test、I-177-D 完了で test 期待が変わる予定だが現
    post-revert 状態で still pass)。
  - `narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign`
    (closure-reassign 不在で `!== null + early-return` が if-let-Some
    shadow を emit する pre-T7 / pre-I-177-D positive boundary lock-in、
    architectural refactor 後も同 emission を維持すべき性質を lock-in)。

- `src/transformer/statements/tests/compound_logical_assign/error_path.rs`:
  - T7-6 unit tests (`narrow_incompatible_rhs_f64_and_string_does_not_intercept`
    + `narrow_incompatible_rhs_f64_or_string_does_not_intercept`) =
    dispatch lane separation の structural lock-in、T7 fix と独立。

- `tests/e2e_test.rs`:
  - T7-1 / T7-2 / T7-4 / T7-5 cell の un-ignore (T7 fix 不発火、
    pre-T7 状態で既に GREEN、un-ignore は valid)。

### Annotation 更新

- `tests/e2e_test.rs::test_e2e_cell_i161_t7_3_and_closure_reassign`
  ignore annotation を I-177-D dependency + revert 経緯記載に更新
  (現実装は pre-T7 shadow form、I-177-D 完了で GREEN 化見込み)。

## Quality gates (post-revert)

| Gate | Pre-revert | Post-revert | Delta |
|------|-----------|-------------|-------|
| `cargo test --lib` | 3124 | 3121 | -3 (T7 lock-in test 3 件削除) |
| `cargo test --test e2e_test` | 155 / 28 ignored | 155 / 28 ignored | unchanged |
| `cargo test --test integration` | 122 | 122 | unchanged |
| `cargo test --test compile` | 3 | 3 | unchanged |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings | 0 warnings | unchanged |
| `cargo fmt --all --check` | 0 diffs | 0 diffs | unchanged |
| `./scripts/check-file-lines.sh` | OK | OK | unchanged |
| Hono bench (clean / errors) | 112 / 62 | 112 / 62 | unchanged |

T7 fix は Hono bench に出現する pattern (`!== null + closure-reassign +
early-return`) を含まないため、revert 後も Hono bench に変動なし。

## I-177-D で実施予定の architectural fix (revert 後の resolution path)

**案 C** (TODO `[I-177-D]` 参照): `narrowed_type` suppression scope を
**post-if scope に限定**、cons-span 内では narrow 保持。pre-T7 shadow
form (revert で復活) と TypeResolver narrow が agree → IR/TypeResolver
lane coherent → `x &&= 3` (Option-shape body) と `return x` (narrow-T
body) 両方で works → T7-3 GREEN 化 (E0506 closure-mutable-capture は
別途 I-048 で対応)。

実装順序 (TODO `[I-177]` 「実装順序 (推奨更新 2026-04-25)」参照):

1. **I-177-D**: suppression scope refactor (本 cohesion gap の root cause
   解消、最架構)
2. I-177 mutation propagation 本体
3. I-177-B: `collect_expr_leaf_types` query 順序
4. I-177-A: typeof/instanceof/OptChain Let-wrap
5. I-177-C: 反対方向 narrow symmetric

## Lessons learned (framework rule 化)

T7 三度の `/check_job` iteration で発見された 4 件の defect は全て
「次元 A × 次元 B の直積 enumeration 不足」に帰着し、**review プロセスと
spec 設計の framework gap** が真の root cause:

| Iteration | 発見 defect | 漏らした defect | Root cause layer |
|-----------|-----------|---------------|----------------|
| Initial /check_job | 0 件 | Truthy 誤発火 / INV-2 / sub-case test / Scenario A | Process (Layer 1 mechanical only) |
| Deep /check_job | Truthy 誤発火 | INV-2 / sub-case test / Scenario A | Process (+ Layer 2 empirical) |
| Deep deep /check_job | INV-2 path 3 / sub-case test 強化 / Scenario A | (なし) | Process (+ Layer 3 structural / Layer 4 adversarial) |

→ Framework rule (TODO `[I-178-5]` Rule 10 + `[I-183]` 4 層化) で
**initial review で全 4 層を実施**することで構造的に解消。

## Conclusion

T7 は cohesion gap の verification (本来の PRD 目的) を達成し、cohesion
gap を documented finding として保持しつつ、architectural fix を I-177-D
に委譲。**workaround patch は revert** することで `ideal-implementation-primacy.md`
完全準拠、`PRD T7` の deliverable は本 report と TODO `[I-177-D]` 起票に
集約。
