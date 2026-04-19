# I-144 Spec Stage — tsc Observation Results

**Base commit**: `5490ed4` (uncommitted: `backlog/I-144-control-flow-narrowing-analyzer.md` v2 + `tests/observations/i144/*` 26 fixtures)
**Observation date**: 2026-04-19 (v1 initial + v2 addendum)
**Fixtures**: `tests/observations/i144/*.ts` (26 total: 18 initial + 4 verify + 4 CR4)
**Observation tool**: `scripts/observe-tsc.sh`
**TS compiler**: `tools/extract-types/node_modules/.bin/tsc` (default strict config)
**Revise 履歴**:
- v1 (initial): 18 fixture、要調査 cell (T3b/T4b-f/T7/R4/R5/R6/F4/F6/Closure×Loop) empirical 解消
- v2 addendum: 4 verify fixture (null-coercion, complement-narrow, t7-narrow-vs-value, closure-reassign-emission) + 4 CR4 fixture (rc-validation, l11-typevar, l17-stdcollection, compound-condition-narrow) — PRD レビューで発見した C1/C2/C3/M1/M3 gap の empirical 解消

---

## Purpose

PRD I-144 (`backlog/I-144-control-flow-narrowing-analyzer.md`) の Spec stage T0 task
"Discovery - tsc observation" の成果物。問題空間マトリクスの **要調査 cell** に対し
empirical に tsc の narrow 挙動を観測し、ideal Rust 出力を確定する。

---

## Sub-matrix 1: Trigger × LHS type (truthy narrow and `===undefined`)

### T4b — `if (x)` truthy on `any`

**Fixture**: `tests/observations/i144/t4b-truthy-any.ts`

```typescript
function f(x: any): any {
    if (x) { return x.v ?? "no-v"; }
    return "falsy";
}
```

**tsc**: no error (type stays `any` inside branch).
**Runtime**:
| input | stdout |
|-------|--------|
| `{ v: "ok" }` | `ok` |
| `0` | `falsy` |
| `""` | `falsy` |
| `null` | `falsy` |
| `undefined` | `falsy` |

**Finding**: TS does NOT narrow `any` type on truthy check (stays `any`), but runtime
behavior matches JS truthiness (0, "", null, undefined, NaN → falsy).

**Ideal Rust output**: Any-enum (`Value`) 経路既存のまま。`if value_is_truthy(&x)` 相当
の predicate emission で JS truthiness を完全 preserve する。**本 PRD では narrow
emission 追加なし** (I-030 scope)。判定: ✓ regression lock-in のみ。

---

### T4c — `if (x)` truthy on `string`

**Fixture**: `tests/observations/i144/t4c-truthy-string.ts`

**tsc**: no error. Type stays `string` in then-branch (no narrow variant).
**Runtime**:
| input | output |
|-------|--------|
| `"hello"` | `5` |
| `""` | `-1` |
| `"a"` | `1` |

**Finding**: TS は `string` 型を変えない (narrow variant を作らない) が、
runtime 挙動は非空 `string` のみ truthy。`""` は falsy。

**Ideal Rust output**:
```rust
fn f(x: String) -> f64 {
    if !x.is_empty() {
        return x.len() as f64;
    }
    return -1.0;
}
```

Type 変化なし (内側も `String`)、predicate は `!x.is_empty()`。
判定: ✓ 既存挙動の可能性高 (要実装確認)。narrow emission は不要。

---

### T4d — `if (x)` truthy on `number`

**Fixture**: `tests/observations/i144/t4d-truthy-number.ts`

**tsc**: no error.
**Runtime**:
| input | output |
|-------|--------|
| `5` | `nonzero: 5` |
| `0` | `zero-or-nan` |
| `-1` | `nonzero: -1` |
| `NaN` | `zero-or-nan` |

**Finding**: TS は `number` 型を変えない。runtime は `0` かつ `NaN` のみ falsy。

**Ideal Rust output**:
```rust
fn f(x: f64) -> String {
    if x != 0.0 && !x.is_nan() {
        return format!("nonzero: {}", x);
    }
    return String::from("zero-or-nan");
}
```

Predicate は `x != 0.0 && !x.is_nan()` (JS truthy 完全等価)。narrow emission 不要。
判定: ✓ 既存 predicate (`x != 0.0`) の強化 — **NaN check 追加要否** は別 issue で
確認必須 (観測結果: **追加必要**)。

---

### T4f — `if (x)` truthy on `string[]`

**Fixture**: `tests/observations/i144/t4f-truthy-array.ts`

**tsc**: no error.
**Runtime**:
| input | output |
|-------|--------|
| `["a","b"]` | `2` |
| `[]` | `0` |

**Finding**: Empty array は truthy (`if (x)` 通る)。Array 自体 `null` になり得ない型
(`string[]`) なので TS 常に truthy; `if/else` 分岐が無意味。

**Ideal Rust output**:
```rust
fn f(x: Vec<String>) -> f64 {
    // `if (x)` on non-nullable Vec is always true.
    // Either: emit `return x.len() as f64;` directly (DCE the else),
    // or: preserve control flow with `if true` (compiler optimizes).
    return x.len() as f64;
}
```

判定: NA (TS 常に truthy、narrow event 発生なし)。**変換挙動**: `if (x)` の
predicate を `true` とする (or DCE 適用)。**本 PRD では現状維持** (narrow analyzer の
対象外、const-fold 最適化は別 PRD 候補)。

---

### T3b — `x === undefined` on Option (`number | undefined`)

**Fixture**: `tests/observations/i144/t3b-eq-undefined-option.ts`

**Runtime**:
| input | output |
|-------|--------|
| `f(5)` | `6` |
| `f(undefined)` | `-1` |
| `g(5)` | `10` |
| `g(undefined)` | `0` |

**Finding**: `if (x === undefined) return; /* x: number */` が正しく narrow。
`if (x !== undefined) { /* x: number */ }` も narrow。

**Ideal Rust output (既存 Option 経路 と同等)**:
```rust
fn f(x: Option<f64>) -> f64 {
    if x.is_none() { return -1.0; }
    let x = x.unwrap();  // E1 shadow-let
    return x + 1.0;
}
```

判定: ✓ 既存挙動 (`==null` / `!==null` と同じ narrow event 発火)。本 PRD で lock-in。

---

### T3b on Union — `number | string | undefined`

**Fixture**: `tests/observations/i144/t3b-eq-undefined-union.ts`

**Runtime**:
| input | output |
|-------|--------|
| `f(5)` | `number:5` |
| `f("ab")` | `string:ab` |
| `f(undefined)` | `undef` |

**Finding**: `x === undefined` → narrow to `number | string`。else branch では
`typeof x` で更に narrow 可能。

**Ideal Rust output**: Union enum の undefined variant を `is_none` 判定し、
残余を `number | string` union variant binding で narrow (E8 既存経路)。
判定: ✓ 既存挙動継続。

---

### T3b on any — `x === undefined` when `x: any`

**Fixture**: `tests/observations/i144/t3b-eq-undefined-any.ts`

**Runtime**:
| input | output |
|-------|--------|
| `f(5)` | `defined:5` |
| `f(undefined)` | `undef` |
| `f(null)` | `defined:null` |
| `f({a:1})` | `defined:[object Object]` |

**Finding**: `null !== undefined` (JS/TS)。`any` でも `=== undefined` は
`undefined` のみ区別。

**Ideal Rust output**: Any-enum (`Value`) で `is_undefined` 判定。既存 any-enum
経路に dispatch。判定: ✓ (要 any-enum narrowing 確認、I-030 scope と連動)。

---

### T7 — OptChain `x?.v !== undefined`

**Fixture**: `tests/observations/i144/t7-optchain.ts`

**Runtime**:
| input | output |
|-------|--------|
| `f({v:10})` | `10` |
| `f(null)` | `undefined` |
| `g({v:10})` | `20` |
| `g(null)` | `-1` |

**Finding**: `if (x?.v !== undefined) { ...x.v... }` — TS narrows `x` to non-null
inside then-branch. `x.v` access compiles (implies x is narrowed).

**Ideal Rust output**:
```rust
fn g(x: Option<HashMap<String, f64>>) -> f64 {
    if x.as_ref().and_then(|o| o.get("v").copied()).is_some() {
        return x.unwrap().get("v").copied().unwrap() * 2.0;
    }
    return -1.0;
}
```

判定: 要確認 (TS narrow propagation 経由で `x` が `Option::Some(_)` narrow される
べき)。本 PRD の CFG analyzer で compound narrow 判定を追加する要あり → Sub-matrix 1 に
追記。

---

## Sub-matrix 2: LHS type × Reset cause (compound / mutation)

### R4 — `x &&= y` on narrowed

**Fixture**: `tests/observations/i144/r4-and-assign.ts`

**Runtime**:
| call | output |
|------|--------|
| `f()` (narrowed x=5, `x &&= 3`) | `3` |
| `g()` (x=null→??=10 then &&= 5) | `5` |

**Finding**: `&&=` preserves narrow when RHS type is compatible
(`narrow x: number, x &&= number_val` keeps `x: number`)。

**Ideal Rust output**: CFG analyzer は `&&=` assign を narrow-compatible assign と
判定、reset event を **生成しない** (narrow preserved)。判定: ✓ (維持)、
本 PRD で lock-in test 追加。

---

### R5 — `??=` on already-narrowed

**Fixture**: `tests/observations/i144/r5-nullish-on-narrowed.ts`

**Runtime**:
| call | output |
|------|--------|
| `f()` (narrowed, `??=` no-op) | `5` |
| `g()` (null→=7→??=99) | `7` |

**Finding**: `??=` on narrowed var は runtime no-op。TS narrow 維持。

**Ideal Rust output**: CFG analyzer は narrowed var (type `T`, not `Option<T>`) への
`??=` を no-op 判定、**statement 自体を elide** してよい (I-142 Cell #14 の
structural な根本解決)。判定: E2 経路不要、narrow 維持のまま predicate 省略。

---

### R6 — Pass-by-mutation

**Fixture**: `tests/observations/i144/r6-pass-by-mutation.ts`

**Runtime**:
| call | output |
|------|--------|
| `f([1,2,3])` (mutate then use narrow) | `4` |
| `f(null)` | `-1` |
| `g({v:10})` (reset resets property to null; narrow stale) | `-99` |

**Finding**: TS は関数呼び出しで narrow を widen しない (unsound アクセス)。
property narrow (`o.v`) も同様に unsound 維持。

**Ideal Rust output**:
```rust
fn f(x: Option<Vec<f64>>) -> f64 {
    if x.is_none() { return -1.0; }
    let x = x.unwrap();  // E1 shadow-let (immutable binding)
    mutate(&x);
    return x.len() as f64;
}
```

判定: ✓ (narrow 維持)、property narrow (`o.v`) は **object-level narrow 非対応**
扱いで scope-out (struct field narrow は本 PRD 外 — I-144 scope は variable narrow)。
CFG analyzer は reset event を生成しない (関数呼び出しは narrow invalidation しない)。

---

## Sub-matrix 4: Flow context × narrow propagation

### F4 — Loop body narrow (per-iteration reset)

**Fixture (a)**: `tests/observations/i144/f4-loop-body-narrow.ts`

**Runtime**:
| call | output |
|------|--------|
| `f()` (for + if-narrow + reassign at i=1) | `10` |
| `g()` (narrow before loop, loop reads x) | `15` |

**Fixture (b)**: `tests/observations/i144/f4-loop-narrow-reset-via-assign.ts`
- `f()` → `:5:null|end`。narrow preserved at iter 0, widens on re-entry (`x=null` inside body → TS widens at loop head).

**Finding (key)**: TS は **loop body 内で変数が reassign される場合、loop head で
narrow を widen する** (fixpoint 再計算)。reassign がなければ narrow は preserved。

**Ideal Rust output (narrow preserved case, loop body no reassign)**:
```rust
fn g() -> f64 {
    let x: Option<f64> = Some(5.0);
    if x.is_none() { return -1.0; }
    let x = x.unwrap();  // E1 shadow-let (outside loop)
    let mut out = 0.0;
    for i in 0..3 { out += x; }
    return out;
}
```

**Ideal Rust output (narrow reset inside loop)**:
```rust
fn f() -> String {
    let mut x: Option<f64> = Some(5.0);
    if x.is_none() { return String::from("null"); }
    // CFG analyzer detects reassign inside loop → use E2 path
    // x stays as Option<f64>, access via get_or_insert_with or match
    let mut out = String::new();
    for i in 0..2 {
        out += &format!(":{}", match &x { Some(v) => v.to_string(), None => "null".into() });
        if i == 0 { x = None; }
    }
    return out + "|end";
}
```

判定: CFG analyzer は **loop body 内の reassign 有無** を判定し、E1/E2 経路を
切り替え。reassign なし → E1 shadow-let (ループ外)、reassign あり → E2 (`let mut Option`)。

---

### F6 — Try body narrow

**Fixture**: `tests/observations/i144/f6-try-narrow.ts`

**Runtime**:
| call | output |
|------|--------|
| `f()` (narrow in try, throw → catch) | `6` (try returns 5+1) |
| `g()` (narrow + reassign in try) | `10` (try returns new x) |

**Finding**: Try body 内の narrow は try scope 内で有効。Catch body では
narrow を widen (TS policy: try 中の任意 point で throw し得るため)。
assignment による narrow は try scope 末尾で observable だが、catch では widen。

**Ideal Rust output**: CFG analyzer は `try` body を独立 block として narrow state
計算、`catch` block には widen した entry state を渡す。Try 内 reassign 経由 narrow は
try block 内でのみ有効 (E1 shadow-let で局所化)。

判定: 本 PRD scope。I-149 (try/catch emission) 完了と連動した catch scope narrow 定義。

---

## Sub-matrix 4: Closure × narrow

### Closure capture of outer-narrowed (read-only)

**Fixture**: `tests/observations/i144/cl1-closure-outer-narrow.ts`

**Runtime**:
| call | output |
|------|--------|
| `f(5)` | `5` |
| `f(null)` | `-1` |

**Finding**: `const getter = () => x;` — TS narrows `x` inside closure when closure is
a **non-reassigned read**. Compile 成功 → closure 内でも narrow 見える。

**Ideal Rust output**: CFG analyzer は closure capture を **read-only capture** と
判定、outer narrow を closure 内にも propagate。E1 shadow-let は closure body に
影響なし (shadow された binding が closure に capture される)。

判定: ✓ (既存 E1 shadow-let でカバー可)、lock-in test 追加。

---

### Closure in loop

**Fixture**: `tests/observations/i144/cl2-closure-in-loop.ts`

**Runtime**: `5,5,5`

**Finding**: Loop 内で closure 生成 + outer narrow 使用 → narrow が loop 全 iteration で
preserved (x is never reassigned)。

**Ideal Rust output**:
```rust
fn f() -> Vec<f64> {
    let x: Option<f64> = Some(5.0);
    let mut fns: Vec<Box<dyn Fn() -> f64>> = Vec::new();
    for i in 0..3 {
        if let Some(x_inner) = x {  // or E3 if-let narrow
            fns.push(Box::new(move || x_inner));
        }
    }
    return fns.iter().map(|fn_| fn_()).collect();
}
```

判定: E3 (if-let Some) で closure capture できる narrow emission を選択。本 PRD で
emission 選択 logic 追加。

---

### Closure reassigning outer (I-142 C-2 scenario)

**Fixture**: `tests/observations/i144/cl3-closure-reassign-outer.ts`
**Fixture (unsound verify)**: `tests/observations/i144/cl3b-ts-narrow-unsound.ts`

**Runtime**:
| call | output |
|------|--------|
| `cl3 f()` | `-99` (reset set x=null) |
| `cl3b f()` | `1` (null + 1 = 1 due to JS coercion; TS compiled `x + 1` unsound) |

**Finding**: TS は closure reassign 後も narrow を preserve (**unsound**)。
TS compile succeeds with `const r = x + 1` where `x` was reset to null via closure.
Rust straight translation → E0308 (C-2 issue)。

**Ideal Rust output (I-142 C-2 structural fix)**:
```rust
fn f() -> String {
    let mut x: Option<f64> = Some(5.0);
    if x.is_none() { return String::from("null"); }
    let reset = || { x = None; };  // closure reassigns Option
    reset();
    let r = *x.get_or_insert_with(|| f64::NAN) + 1.0;  // E2 path
    return r.to_string();
}
```

判定: E2 経路 (`let mut Option` + `get_or_insert_with`) で closure capture に対応。
本 PRD の核心。CFG analyzer が closure capture + reassign を検出し E1→E2 経路切替。

---

## 要調査 Cell 解消サマリ (PRD matrix 更新対象)

| Cell | Before | After observation | 判定 |
|------|--------|-------------------|------|
| T3b × L1 (Option) | 要確認 | ✓ narrow OK | Lock-in |
| T3b × L2 (Union) | 要調査 | ✓ union variant narrow | Lock-in |
| T3b × L3 (Any) | ? | ✓ any-enum `is_undefined` | Lock-in (I-030 連動) |
| T4b truthy Any | 要調査 | ✓ any-enum truthy (I-030 scope) | Out of I-144 scope |
| T4c truthy String | 要調査 | `!x.is_empty()` predicate | Lock-in (既存?) |
| T4d truthy Number | 要調査 | `x != 0.0 && !x.is_nan()` predicate | **Enhance (NaN 追加)** |
| T4f truthy Array | 要調査 | Always true (Vec non-null) | NA (const-fold 別 PRD) |
| T7 OptChain narrow | 要確認 | TS narrows `x` non-null via `x?.v !== undefined` | **Enhance (compound narrow)** |
| R4 `&&=` | 要調査 | narrow preserved if RHS compatible | Lock-in |
| R5 `??=` on narrowed | 要調査 | narrow preserved, runtime no-op | **Elide predicate** (Cell #14 解消) |
| R6 pass-by-mutation | 要調査 | narrow preserved (TS unsound) | Lock-in (var narrow only) |
| F4 Loop body | 要調査 | reassign → widen at loop head (fixpoint) | **E1/E2 経路切替** |
| F6 Try body | 要調査 | try 内 narrow、catch widen | Lock-in (I-149 連動) |
| Closure read narrow (cl1) | 要確認 | narrow propagates to closure | Lock-in (E1) |
| Closure reassign (cl3) | ✗ C-2 | TS unsound, need E2 path | **Core I-144 scope** |

**結論**: 全 要調査 cell 解消。残 action:
1. PRD matrix (Sub-matrix 1-4) の判定列を上記 After に更新
2. T4d NaN predicate 強化を本 PRD scope または TODO 化
3. T7 compound narrow (OptChain + `!==undefined`) を本 PRD scope に含める
4. R5 runtime no-op predicate elide を Cell #14 structural 解消として PRD の T4/T5 に反映

---

## Spec Stage 次 action (T1/T2)

1. **T1 (matrix refinement)**: 上記 observation 結果を反映し matrix cell を確定 (要調査 0 件達成)
2. **T2 (per-cell E2E fixture skeleton)**: `tests/e2e/scripts/i144/*.ts` を red 状態で作成
3. **Spec stage review**: `spec-first-prd.md` Checklist 5 項目検証

---

## v2 Addendum: レビューで発見した matrix structure gap と empirical 解消

PRD v1 完了後の self-review で以下の gap を発見し、追加 observation で解消:

### C1: E 次元の conflate (AST pattern vs 使用状況 cluster)

v1 の E2 は `??=` 文 (mutating) と closure-reassign 後 read (non-mutating) を同一 label で扱っていた。
**→ v2 で E2a (`get_or_insert_with`) / E2b (`unwrap_or(coerce_default)`) / E2c (Option 直接) に分割**。

### C2: JS coerce_default semantic 欠落

`verify-null-coercion.ts` で empirical 確認:
- `null + 1 = 1` (null → 0 for numeric)
- `undefined + 1 = NaN`
- `null + "x" = "nullx"` (null → "null" for concat)
- `!null = true` (null → false for boolean)

**→ v2 で PRD の Semantic Safety Analysis に `JS coerce_default table` 追加**。E2b 適用時は
type-specific coerce_default を使用。

### C3: C-2 sub-category 未分化

`verify-closure-reassign-emission.ts` / `cl3b-ts-narrow-unsound.ts` で empirical 確認:
- TS は closure reassign 後も narrow を保持 (unsound) — `const r = x + 1` を compile 通す
- runtime では null 到達 (`null + 1 = 1` via JS coerce)

**→ v2 で C-2 を C-2a (??= + closure capture) / C-2b (closure reassign + RC1 arith read) /
C-2c (closure reassign + RC6 string concat) / C-2d (closure reassign + RC1 return、scope out) に分化**。

### M1: T 次元 trigger 漏れ

`verify-complement-narrow.ts` で以下 pattern を確認:
- Negation `!(cond)` — narrow complement
- Compound `cond1 && cond2` — 両方 narrow
- Early-exit `if (x==null) throw;` — scope 後続で narrow

**→ v2 で T 次元に T9 (Negation), T10 (Compound), T11 (Early-throw), T12 (Short-circuit `x && x.v`) 追加**。

### CR4: RC 次元妥当性 + L11 TypeVar + L17 StdCollection

`rc-validation.ts` で RC1-RC8 全 context を empirical 確認 (runtime 出力一致)。
`l11-typevar.ts` で generic narrow 確認 (TS narrows generic type params)。
`l17-stdcollection.ts` で Record/Map truthy 確認 (empty でも truthy, T4f と同じ挙動)。
`compound-condition-narrow.ts` で short-circuit narrow 確認。

**→ v2 で Sub-matrix 5 (RC × narrow state × LHS → Emission) 新設**、L11/L17 を narrow 対象外の
justified NA として確定。

### 最終成果 (v2 時点)

- **Matrix completeness**: Sub-matrix 1-5 全 cell に ideal 出力記載、要調査 0 件
- **Oracle grounding**: 26 fixture (18 initial + 4 verify + 4 CR4) で全 ✗/要調査 cell empirical 解消
- **Spec Stage Review Checklist**: 5 項目中 4 項目 [✅]、#5 E2E readiness は T1 で実施
