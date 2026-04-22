# I-161 + I-171 Batch T1: per-cell E2E fixture red-state confirmation

**Date**: 2026-04-22 (v6 — /check_problem round gap closure)
**PRD**: [`backlog/I-161-I-171-truthy-emission-batch.md`](../backlog/I-161-I-171-truthy-emission-batch.md) Task T1
**SDCDF stage**: Spec stage — Adversarial Review Checklist #5 (E2E readiness) green 化
**Revise history**:
- v1 (initial 19 fixture)
- v2 (+27 fixture, Matrix A/B/C gap closure)
- v3 (in-matrix adversarial fixes: Tier 2 SimpleAssignTarget NA + Ref(T) reclassify + a4/a7/c16 fixture correction)
- v4 (Critical + Important gap closure: C-16b OptChain base narrow in-scope / T3/T4 unit test enumerate / +14 E2E fixtures / T7 regression expansion / I-175 + I-176 TODO 起票)
- v5 (/check_job round 2: O-14 matrix symmetric / T5→T6 循環依存解消 / T7-6 unit test 明記 / TsSatisfies NA 頻度根拠除去)
- **v6 (/check_problem round: T3-TR TypeResolver propagation empirical 確認 + fix design / T8 ||= expr-context semantic 修正 / peek-through unit test enumerate 7 case 拡張)**

## v6 variance (2026-04-22 /check_problem round)

Commit 前 /check_problem round で Critical 1 件 + Minor 2 件を追加発見、全対応完了。

### Critical: TypeResolver AndAssign/OrAssign expected propagation 不在 (empirical 確認)

- **発見**: `src/pipeline/type_resolver/expressions.rs` L126-148 が `??=` のみ expected propagate、`&&=`/`||=` は skip (コメント: "other compound ops ... do not read the LHS type from expr_types")
- **Empirical verify (2026-04-22)**: 同一 `interface P { a: number }` + `{ a: 99 }` object literal RHS で:
  - plain `=`: `p = P { a: 99.0 };` (✓ Named 経路、L94-125 propagate 経由)
  - `&&=`: `x = x && _TypeLit0 { a: 99.0 };` (✗ synthetic auto-generate、L405-407 `expected_types.insert(obj_span, _TypeLit0)` 経由)
- **対応**: PRD T3 を T3-TR sub-task で拡張し、`type_resolver/expressions.rs` L126-148 に AndAssign/OrAssign 条件追加を明記。Impact Area に `src/pipeline/type_resolver/expressions.rs` を追加。I-175 TODO を refine し compound logical assign subset が T3-TR で indirectly 解消される関係を明記。残 scope を他 compound assign (`+=`/`-=` 等) に narrow
- **cell-a7 fixture**: T3-TR 完了後に `mkP(a)` workaround 除去を empirical 検証する完了条件を T3 に追加

### Minor: T8 `||=` expr-context semantic 修正

- **発見**: PRD pseudo-code で `||=` T8 always-truthy expr-context が `Expr::Unit` (`()`) を返す設計だったが、JS semantic では原 x を返すべき
- **対応**: pseudo-code を (op × context × Copy-ness) 2x2x2 cross で明示化。`||=` expr-context T8 は `x.clone()` (non-Copy) / `x` (Copy) を返すよう修正

### Minor: peek-through unit test enumeration 不足

- **発見**: T2 test plan の peek-through 3 case (TsAs/TsNonNull/Paren) で TsTypeAssertion / TsConstAssertion / nested / post-peek double-neg が未列挙
- **対応**: peek-through test を 7 case に拡張、Matrix A unit test を 117 case に refine (80 primary + 12 Tier 2 error-path + 24 A.5 + 1 T7-6 error-path)

## v5 variance (2026-04-22 /check_job round 2)

Round 2 /check_job adversarial review で発見した v4 に対する 4 件の issue を解消:

1. **Issue A (O-14 matrix symmetric)**: v4 で A-14 を 6 個別行に enumerate したが O-14 は 1 行 collapsed で非対称 → O-14a / O-14b / O-14c / O-14d / O-14e / O-14f の 6 個別行に展開、A-14 と symmetric
2. **Issue B (T5 → T6 循環依存)**: v4 で T5 Depends on を `T2, T4, T6` に変更 → T6 Depends on は `T2, T3, T4, T5` のまま → 循環。T5 を `T2, T4` に戻し、C-16b は T6 P3b (guards.rs OptChain Bang 拡張) で narrow event push → 既存 `narrowed_type` query で自動 consume される設計を明記
3. **Issue C (T7-6 unit vs E2E 不整合)**: T7-6 は incompatible RHS type の error-path verification だが「E2E 全 green」と誤記 → unit test として明記、T7 完了条件を T7-1〜T7-5 E2E + T7-6 unit test に分離
4. **Issue D (TsSatisfies NA 理由に頻度根拠残存)**: B.1.37h に「Hono で 0 件」の頻度論残存 → I-115 unsupported + convert_expr 構造的依存 + peek_through_type_assertions arm 追加のみで自動 extent 可能だが I-115 依存 と spec-traceable に refine

## v4 variance (2026-04-22 round 1 adversarial review gap closure)

### Critical gaps resolved

1. **C-16n dependency classification** (v3 まで「🔒 I-143-a + I-165」、誤):
   - 実調査結果: `narrowing_patterns.rs::extract_optchain_base_ident` (既存 pub(crate) helper) で `!x?.v` の base Ident を取得可能、I-165 / I-143-a 非依存
   - **resolution**: Matrix C-16 を split — **C-16a** (Layer 1 always-exit、本 PRD scope) / **C-16b** (OptChain base narrow、本 PRD T6 P3b 拡張で scope in) / **C-16n** (field-path narrow のみ I-165 scope out)
   - guards.rs Bang arm に OptChain case 追加 (~10 LOC、I-144 T6-4 と symmetric)

2. **T3/T4 unit test plan enumerate 不足**:
   - **resolution**: T3 を明示列挙 (Matrix A primary 52 case + O primary 52 case + A.5 cross product 24 case + Tier 2 NA error-path 12 case = 140 case)、T4 を明示列挙 (Matrix B.2 type 16 case + B.1 shape ~48 case + Tier 2 NA 10 case + TempBinder/const-fold/is_always_truthy 21 case = ~95 case)
   - Tuple / Ref(T) を T8 equivalence class member として明示、test plan に `T1-T8 + T12 Tuple + T12 Ref(T)` 表記に更新

### Important gaps resolved

3. **Matrix B 欠落 shape E2E fixture 8 件追加** (v4):
   - cell-b-bang-nc (B.1.28 NullishCoalescing)
   - cell-b-bang-cond (B.1.30 Cond)
   - cell-b-bang-await (B.1.32 Await)
   - cell-b-bang-assign (B.1.33 Assign)
   - cell-b-bang-this (B.1.35 This)
   - cell-b-bang-update (B.1.36 Update)
   - cell-b-bang-tstypeassertion (B.1.37g)
   - cell-b-bang-tsconstassertion (B.1.37i)
   - 他 trivial shape (Unary non-Bang / Bin Comparison/InstanceOf/In / Bin Bitwise / New / Array/Object/Tpl/Arrow/Fn individual) は T4 unit test plan の明示 enumerate で cover

4. **Matrix A.5 expr-context cross product test plan refine**:
   - **resolution**: T3 unit test に A.5 12 cell (A-1x〜A-12dx 個別) + O 側対称 12 cell = 24 case を明示列挙
   - Copy/non-Copy dispatch (`.clone()` 挿入有無) を cell 単位で検証

5. **T7 classifier interaction scope 拡張**:
   - **resolution**: T7 を R4 単一 cell から 6 cell (T7-1〜T7-6) に拡張
     - T7-1: `&&=` on narrowed F64 (R4 再生成)
     - T7-2: `||=` on narrowed F64
     - T7-3: `&&=` + closure reassign interaction
     - T7-4: `||=` then `??=` chain
     - T7-5: `&&=` on narrowed union + string RHS
     - T7-6: narrow reset via `&&=` with incompatible RHS type
   - E2E fixture 5 件追加 (cell-t7-1〜t7-5)、T7-6 は unit test (type error 化の expected error-path test)

### Document entries (TODO に別途起票)

6. **Pre-existing defect I-175**: object literal RHS が LHS Named 型に推論されず `_TypeLit0` で emit される課題を TODO 起票 (cell-a7 workaround で発覚、Phase B RC-11 の部分集合)

7. **Infra defect I-176**: `tests/e2e_test.rs` 1393 LOC (1000 超) を TODO 起票、別 refactor PRD 候補

## 目的

I-161 + I-171 PRD の Problem Space Matrix (Matrix A / B / C) の各 in-scope cell に
per-cell E2E fixture を作成し、現行 `cargo build --release` + TS runtime (tsx) で
以下を empirical に確認する:

1. **✗ cell** — 現行 emission が red 状態 (compile fail or runtime mismatch)
2. **✓ regression / GREEN cell** — 現行 emission が既に GREEN (post-PRD 実装で runtime 非後退を lock-in)
3. **Runtime GREEN but emission-consistency ✗ cell** — 現行 emission が semantically 正しいが structural emission 形が ideal と乖離 (例: `bool && bool`)。E2E は runtime 等価性を lock-in、emission shape 検証は unit/snapshot test で対応

## Fixture inventory

配置: `tests/e2e/scripts/i161-i171/`、oracle (`*.expected`) は `scripts/record-cell-oracle.sh --all`
で tsx runtime stdout から記録済 (2026-04-22 v2)。

### Matrix A — `&&=` primary cells (9 fixture)

| Fixture | Matrix cell | Status | 期待 ideal emission (post-T3) |
|---------|-------------|--------|-------------------------------|
| `cell-a1-and-bool.ts` | A-1 (Bool) | **Runtime GREEN** | `if x { x = rhs; }` (現行 `x = x && rhs` も runtime 等価、T3 で structural 統一) |
| `cell-a2-and-f64-narrow.ts` | A-2 (F64 narrow alive) | RED | `if let Some(x) = x { if x != 0.0 && !x.is_nan() { x = 3.0; } return Some(x); }` |
| `cell-a3-and-string-empty.ts` | A-3 (String non-null) | RED | `if !x.is_empty() { x = "world".to_string(); }` |
| `cell-a4-and-int.ts` | A-4 (Primitive int) | RED | `if len != 0 { len = 99; }` (現 emission の bit-NOT runtime mismatch) |
| `cell-a5-and-option-f64.ts` | A-5 (Option<F64> no narrow) | RED | `if x.is_some_and(\|v\| *v != 0.0 && !v.is_nan()) { x = Some(3.0); }` |
| `cell-a5s-and-option-string.ts` | A-5s (Option<String>) | RED | `if x.as_ref().is_some_and(\|v\| !v.is_empty()) { x = Some("world".to_string()); }` |
| `cell-a6-and-option-union.ts` | A-6 (Option<synthetic union>) | RED | per-variant match with guard |
| `cell-a7-and-option-named.ts` | A-7 (Option<Named other>) | RED | `if x.is_some() { x = Some(y); }` |
| `cell-a8-and-always-truthy.ts` | A-8 (always-truthy Named) | RED | const-fold: `p = y;` |

### Matrix O — `||=` primary cells (8 fixture)

| Fixture | Matrix cell | Status | 期待 ideal emission (post-T3) |
|---------|-------------|--------|-------------------------------|
| `cell-o1-or-bool.ts` | O-1 (Bool) | **Runtime GREEN** | `if !x { x = rhs; }` (現行 `x = x \|\| rhs` も runtime 等価) |
| `cell-o2-or-f64.ts` | O-2 (F64) | RED | `if x == 0.0 \|\| x.is_nan() { x = 99.0; }` |
| `cell-o3-or-string.ts` | O-3 (String) | RED | `if x.is_empty() { x = "default".to_string(); }` |
| `cell-o5-or-option-f64.ts` | O-5 (Option<F64>) | RED | `if x.map_or(true, \|v\| *v == 0.0 \|\| v.is_nan()) { x = Some(3.0); }` |
| `cell-o5s-or-option-string.ts` | O-5s (Option<String>) | RED | `if x.as_ref().map_or(true, \|v\| v.is_empty()) { x = Some("default".to_string()); }` |
| `cell-o6-or-option-union.ts` | O-6 (Option<synthetic union>) | RED | per-variant match with falsy guards |
| `cell-o7-or-option-named.ts` | O-7 (Option<Named other>) | RED | `if x.is_none() { x = Some(y); }` |
| `cell-o8-or-always-truthy.ts` | O-8 (always-truthy) | RED | const-fold: no-op (assign 発動せず) |

### Matrix A supplementary — Member LHS + expr context (2 fixture)

| Fixture | Matrix cell | Status |
|---------|-------------|--------|
| `cell-a-member-and.ts` | A-{Member LHS} | RED |
| `cell-a-expr-context.ts` | A-{expr context} | RED |

### Matrix B — `!<expr>` Layer 1 cells (12 fixture)

| Fixture | Matrix cell | Status | 期待 ideal emission (post-T4) |
|---------|-------------|--------|-------------------------------|
| `cell-b-bang-f64-in-ret.ts` | B-T2 (F64) | RED | `x == 0.0 \|\| x.is_nan()` |
| `cell-b-bang-string-in-ret.ts` | B-T3 (String) | RED | `x.is_empty()` |
| `cell-b-bang-int.ts` | B-T4 (Primitive int) | RED | `x == 0` (現 `!usize as f64` は runtime mismatch) |
| `cell-b-bang-option-number-in-ret.ts` | B-T5 (Option<F64>) | RED | `!x.is_some_and(\|v\| *v != 0.0 && !v.is_nan())` |
| `cell-b-bang-option-union.ts` | B-T6 (Option<synthetic union>) | RED | per-variant match |
| `cell-b-bang-option-named.ts` | B-T7 (Option<Named>) | RED | `x.is_none()` |
| `cell-b-bang-named.ts` | B-T8 Named | RED | const-fold `false` |
| `cell-b-bang-vec.ts` | B-T8 Vec | RED | const-fold `false` |
| `cell-b-bang-bin-expr.ts` | B.1.21 (BinExpr) | RED | `{ let _tmp = x + 1.0; _tmp == 0.0 \|\| _tmp.is_nan() }` |
| `cell-b-bang-double-option.ts` | B.1.19 (double neg) | RED | `x.is_some_and(...)` (truthy fold) |
| `cell-b-bang-logical-and.ts` | B.1.23 (LogicalAnd) | RED | `<x falsy> \|\| <y falsy>` (De Morgan) |
| `cell-b-bang-tsas.ts` | B.1.17 (TsAs peek) | RED | peek-through + falsy predicate |

### Matrix C — if-stmt narrow cells (14 fixture)

| Fixture | Matrix cell | Status | 期待 ideal emission (post-T5) |
|---------|-------------|--------|-------------------------------|
| `cell-c4-if-bang-non-exit.ts` | C-4 (non-exit body) | RED | `if <x falsy> { side_effect; }` (narrow 材料化なし) |
| `cell-c5-if-bang-else.ts` | C-5 (else branch) | RED | consolidated match: `match x { Some(v) if truthy => { else }, _ => { then } }` |
| `cell-c7-const-fold-null.ts` | C-7 (const-fold `!null`) | RED | body 直挿入 |
| `cell-c11-peek-paren.ts` | C-11 (Paren peek) | RED | unwrap + recurse |
| `cell-c12-peek-tsas.ts` | C-12 (TsAs peek) | RED | peek-through + recurse |
| `cell-c13-peek-nonnull.ts` | C-13 (TsNonNull peek) | RED | peek-through + recurse |
| `cell-c14-peek-unary.ts` | C-14 (`!!x` double neg) | RED | truthy fold |
| `cell-c15-if-bang-member-exit.ts` | C-15 (Member LHS Layer 1 only) | RED | `if u.v.is_none() { exit }` |
| `cell-c16-if-bang-optchain.ts` | C-16 (OptChain Layer 1 only) | RED | `if x.is_none() \|\| <v falsy> { exit }` |
| `cell-c17-if-bang-bin-arith.ts` | C-17 (Bin arith) | RED | tmp-bind + falsy |
| `cell-c18-if-bang-logical-and.ts` | C-18 (LogicalAnd) | RED | De Morgan |
| `cell-c19-if-bang-call.ts` | C-19 (Call/Cond/Await/New) | RED | tmp-bind + falsy |
| `cell-c23-if-bang-logical-or.ts` | C-23 (LogicalOr) | RED | De Morgan (falsy && falsy) |
| `cell-c24-if-bang-always-truthy.ts` | C-24 (always-truthy operand) | RED | const-fold `!<truthy> = false` → else-branch 選択 |

### Regression ✓ lock-in (1 fixture)

| Fixture | Matrix cell | Status |
|---------|-------------|--------|
| `cell-regression-t6-3-ident-option.ts` | C-1 (T6-3 既存) | **GREEN** |

### 総計 (v4 cumulative)

| Category | 件数 |
|----------|------|
| Matrix A (&&=) | 9 + 2 supplementary = 11 |
| Matrix O (||=) | 8 |
| Matrix B (!<expr> Layer 1) | 12 + 8 v4 additions = 20 |
| Matrix C (if-stmt Layer 2) | 14 + 1 v4 C-16b = 15 |
| T7 classifier-interaction regression | 5 (v4 additions) |
| Regression | 1 |
| **合計** | **60 fixture** |

**Status 内訳**: 3 currently GREEN (regression + A-1 + O-1) / 57 RED `#[ignore]` (v4 で 14 件追加)

## Red-state empirical verification (代表例、v2 追加分)

### `cell-a8-and-always-truthy` (Matrix A-8)

```rust
struct P { a: f64 }

fn f() -> f64 {
    let mut p: P = P { a: 1.0 };
    p = p && P { a: 99.0 };  // ← E0369 (binary op `&&` on P)
    p.a
}
```

**Red confirmed**: `&&` が `P` に非対応。const-fold 適用後 `p = P { a: 99.0 };` で解消。

### `cell-o2-or-f64` (Matrix O-2)

```rust
fn f(init: f64) -> f64 {
    let mut x: f64 = init;
    x = x || 99.0;  // ← E0369 (binary op `||` on f64)
    x
}
```

**Red confirmed**: `||` は bool 専用、f64 非対応。

### `cell-c7-const-fold-null` (Matrix C-7)

```rust
fn f() -> String {
    if !None {  // ← E0600 (`!` on Option)
        return "ok".to_string();
    }
    "unreachable".to_string()
}
```

**Red confirmed**: `!None` 非対応。const-fold で `return "ok".to_string();` 直接 emit が ideal。

### `cell-c24-if-bang-always-truthy` (Matrix C-24)

```rust
fn f() -> String {
    let arr = vec![1.0, 2.0, 3.0];
    if !arr {  // ← E0600 (`!` on Vec)
        return "unreachable".to_string();
    }
    "truthy".to_string()
}
```

**Red confirmed**: `!Vec<f64>` 非対応。const-fold `!<always-truthy> = false` → else branch 選択で `"truthy"` emit。

### `cell-a1-and-bool` (Matrix A-1) — Runtime GREEN

```rust
fn f(init: bool, rhs: bool) -> bool {
    let mut x: bool = init;
    x = x && rhs;  // ← 現行 Rust bool 対応で valid
    x
}
```

**Runtime: GREEN (valid bool && bool)**。Rust の `bool && bool` は `if x { x = rhs; } else { x = false; }` に対応し、semantic 等価。T3 で `if x { x = rhs; }` に structural 統一するが runtime は不変。

### `cell-b-bang-int` (Matrix B-T4)

```rust
fn f(arr: Vec<f64>) -> bool {
    !arr.len() as f64  // ← (!<usize>) as f64 = bit-NOT (usize::MAX) → runtime mismatch
}
```

**Red confirmed (runtime mismatch)**: `!usize` は bit-NOT で巨大値を返す。JS `!0` = true は expected boolean。現 emission は Rust として compile するが TS runtime と等価でない。

## Test harness 組み込み (v2)

`tests/e2e_test.rs` に計 46 test function を登録:

- **3 active (`#[test]` のみ)**: regression-t6-3-ident-option, a1-and-bool, o1-or-bool
- **43 ignored (`#[ignore = "... — unignore at T<N> (...)"]`)**: 全 RED cell

T3 (I-161) / T4 (I-171 Layer 1) / T5 (I-171 Layer 2) 完了時点で該当 group の `#[ignore]`
を外し、GREEN 化を Spec-Stage Review Checklist #5 の empirical 証左とする。

```bash
# Spec stage 現在時点:
$ cargo test --test e2e_test -- i161 i171 --test-threads=1
test result: ok. 3 passed; 0 failed; 43 ignored; 0 measured; ...
```

## Spec-Stage Adversarial Review Checklist (v2 状態)

| # | Checklist item | Status | 根拠 |
|---|----------------|--------|------|
| 1 | **Matrix completeness** | ✅ | Matrix A (A-1〜A-13 + A-12a〜f 全列挙) / O (O-1〜O-13 + O-12a〜f 全列挙) / B (B.1.1〜B.1.37l 個別展開) / C (C-1〜C-24) 全 cell に ideal output 記載、blocked は I-050/I-165/I-143-a link 明示、expr-context × Copy/non-Copy cross product (A.5 section) 追加 |
| 2 | **Oracle grounding** | ✅ | tsc observation 26 fixture (`tests/observations/i161-i171/` v1: a1-a7 + b1-b14 21 件 + v2: a8 and/or + a9 or-narrow + b15 bigint-regex + b16 logical-or 5 件)。全 ✗ cell に対応する observation で TS runtime 確認済 |
| 3 | **NA justification** | ✅ | A-11 (IR invariant `Never`)、A-12a〜f / O-12a〜f (IR invariant / emission path 不在 / ECMA syntax error)、A-13 / O-13 (PatternAssignTarget → ECMA-262 syntax error)、A-12e empirical trace で `_try_result` が唯一の Result local var であり `&&=` 対象外であることを確認、B.1.37a〜l (12 variant 個別 NA 理由、Tier 2 unsupported 背景 + peek-through 候補明示) |
| 4 | **Grammar consistency** | ✅ | 全 variant が `ast-variants.md` §1/§6/§7/§9, `rust-type-variants.md` §1 (18 全 variant map), `emission-contexts.md` §1-§8 と整合、未記載 variant なし |
| 5 | **E2E readiness** | ✅ | 46 fixture (3 GREEN + 43 RED `#[ignore]`) 作成済、test harness 登録完了。Matrix A 11 + Matrix O 8 + Matrix B 12 + Matrix C 14 + regression 1 |

**判定**: 全 5 項目 [x]。Implementation stage (T2) 移行可能。

## v3 adversarial review follow-up (2026-04-22 same day)

Commit 前 `/check_job` adversarial review で 5 件の追加発見、v3 で全解消:

1. **Spec gap (Tier 2 SimpleAssignTarget 未列挙)**: Matrix A/O に `SimpleAssignTarget::{SuperProp, Paren, OptChain, TsAs, TsSatisfies, Invalid}` の 6 variant を A-14a〜f / O-14a〜f として NA 行追加 (`convert_assign_expr` match arm が Ident/Member 限定で UnsupportedSyntaxError 返却する仕様を明記)
2. **誤分類 (Ref(T))**: A-12b / O-12b を「defer, I-048 依存」から「T8 const-fold (Rust `&T` 常に non-null)」に reclassify。Matrix 本体では in-scope 化
3. **Fixture-Matrix mismatch (A-4)**: cell-a4-and-int を `arr.length &&= 99` (emission は f64 LHS、実は A-2 相当) から bigint 版 `let x: bigint = init; x &&= 99n` に書き換え。i128 LHS = genuine Primitive(int) 確認 (emission `x = x && 99` on i128 → E0369 RED)
4. **Fixture narrow dependency (C-16)**: cell-c16-if-bang-optchain を post-if `x.v` 直接アクセスから `x?.v ?? ""` に書き換え。narrow 材料化なしで Layer 1 only fix との整合 確保
5. **Fixture synthetic type issue (A-7)**: cell-a7-and-option-named に `mkP(a)` explicit constructor helper 追加。object literal → synthetic `_TypeLit0` emission の pre-existing 課題 (本 PRD scope 外) を isolated fixture 設計で回避、A-7 structural fix の pure test に

## v2 variance vs v1

v1 (2026-04-22 initial) → v2 (2026-04-22 adversarial review gap closure):

| 変更 | 数量 |
|------|------|
| tsc observation 追加 | +5 (a8-and, a8-or, a9-or-narrow, b15-bigint-regex, b16-logical-or) |
| E2E fixture 追加 | +27 (A supplementary 4 + O 7 + B supplementary 5 + C supplementary 11) |
| test harness 追加 | +27 entries (2 active + 25 ignored) |
| Matrix A/O 行展開 | O-1 to O-13 individually + A-13 + O-13 PatternAssignTarget + expr-context A.5 section |
| Matrix B.1.37 展開 | 1 row → 12 rows (B.1.37a-l) |
| NA justification 更新 | A-12e empirical trace、TsTypeAssertion/TsConstAssertion → in-scope peek-through |

## Outstanding / Next Steps

- **Implementation stage (T2-T8)** 着手可能。T2 で helper 実装 (`truthy_predicate_for_expr` + `peek_through_type_assertions` + `TempBinder` + `try_constant_fold_bang`)。
- T3-T5 で各 matrix cell の ignore 解除を段階的に実施。
- T6 で broken window fix (P1-P4)、T7 で classifier / emission 相互検証 (R4 empirical)、T8 で quality gate。
