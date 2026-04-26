# I-177-F: `resolve_arrow_expr` / `resolve_fn_expr` / class constructor / class method body traversal cohesion fix

**Plan η Step 2.5 (PRD 2.5)** — TypeResolver fn body traversal cohesion 完成のための structural fix。

## Scope expansion (2026-04-26 post-/check_job audit)

初版 PRD scope は `resolve_arrow_expr` / `resolve_fn_expr` の 2 method のみ。`/check_job deep deep` (2026-04-26) で **class constructor body (`visit_class_decl` 内の `ast::ClassMember::Constructor` arm) と class method body (`visit_method_function`) も同じ traversal bug を持つ** ことが Layer 3 Structural cross-axis verification で発覚 (初版 PRD の 直交軸 enumerate 表では "Method (class method body) 要 audit (本 PRD scope 外)" と note して deferred されていたが、Hono framework が class を多用する事実 + 「妥協絶対禁止」要請により本 PRD scope に編入)。最終 fix scope:

- `resolve_arrow_expr` body (fn_exprs.rs:201-206)
- `resolve_fn_expr` body (fn_exprs.rs:257-262)
- `visit_class_decl` constructor body (visitors.rs:480-486)
- `visit_method_function` class method body (visitors.rs:537-541)

4 site 全てで `for stmt in &body.stmts { self.visit_stmt(stmt); }` を `self.visit_block_stmt(body);` に置換。`visit_fn_decl` (visitors.rs:129) と完全 symmetric。

## Background

`TypeResolver::visit_fn_decl` (`src/pipeline/type_resolver/visitors.rs:125-129`) は function declaration body を `self.visit_block_stmt(body)` 経由で walk し、`current_block_end` を正しく set する。一方:

- **`resolve_arrow_expr`** (`src/pipeline/type_resolver/fn_exprs.rs:197-202`)
- **`resolve_fn_expr`** (`src/pipeline/type_resolver/fn_exprs.rs:252-259`)

の 2 method は arrow / function expression body を `for stmt in &block.stmts { self.visit_stmt(stmt); }` で **直接 iterate** し、`visit_block_stmt` を呼ばない。結果 `current_block_end` が None のまま、`visit_if_stmt` (`visitors.rs:735-740`) の `detect_early_return_narrowing` が:

```rust
if then_exits && !else_exits {
    if let Some(block_end) = self.current_block_end {  // ← None なので skip
        let if_end = if_stmt.cons.span().hi.0;
        detect_early_return_narrowing(&if_stmt.test, if_end, block_end, self);
    }
}
```

で None match → `EarlyReturnComplement` narrow event を post-if scope に push しない。

### Empirical defect (2026-04-26 reproduced via I-177-B unit test)

```ts
interface H { (x: string | number): string | number }
const h: H = (x: string | number): string | number => {
    if (typeof x === "string") return 0;
    else { console.log("ne"); }
    return x;
};
```

`narrow_events` 観測:
- ✓ Primary(TypeofGuard) String at then-branch [155, 164)
- ✓ Primary(TypeofGuard) F64 at else-block [186, 208)
- ✗ **EarlyReturnComplement(TypeofGuard) F64 at post-if scope [208, 232) が不在**

同じ source の declaration form (`function h(...)`) では 3 event 全て push されるが、callable interface arrow form では 2 event のみ。**root cause = `current_block_end` 未 set**。

I-177-B PRD で empirical 発見、`collect_leaves_typeof_narrow_post_if_return` test を `#[ignore]` 化して本 PRD prerequisite とした (annotation: "depends on resolve_arrow_expr setting current_block_end")。

### 関連 PRD

- **I-177-D / I-177-E / I-177-B (完了 2026-04-26)**: TypeResolver suppression / synthetic fork inheritance / leaf type cohesion。本 PRD と並んで narrow framework cohesion を完成させる。
- **後続 (Plan η)**: PRD 3 (mutation propagation 本体) → PRD 4 (I-177-A IR shadow) → ...

## Problem Space

本 PRD は **non-matrix-driven** (~5 LOC structural fix)。代替として **method × body shape × narrow trigger** matrix で完全 coverage を problem space とする。

### 入力次元 (Dimensions)

| 次元 | Variant 列挙 |
|------|------------|
| Resolver method | (a) `visit_fn_decl` / (b) `resolve_arrow_expr` / (c) `resolve_fn_expr` |
| Body shape | (i) BlockStmt with stmts / (ii) Expr (arrow only) |
| Inner if-stmt narrow trigger | typeof / instanceof / nullcheck / OptChain / Bang / Truthy |
| `then_exits && !else_exits` の発火 | (T) 該当 / (F) 非該当 |

### 組合せマトリクス (method × body shape × narrow trigger × control-flow exit)

| # | Method | Body shape | trigger | exit cond | Pre-fix narrow events 数 | Post-fix (ideal) | 判定 |
|---|--------|-----------|---------|-----------|-------------------------|-----------------|------|
| 1 | visit_fn_decl | (i) BlockStmt | typeof | T | 3 (Primary then + Primary else + EarlyReturnComplement post-if) | 3 同 | ✓ no change |
| 2 | visit_fn_decl | (i) BlockStmt | instanceof | T | 3 同 | 3 同 | ✓ |
| 3 | visit_fn_decl | (i) BlockStmt | nullcheck | T | 3 同 | 3 同 | ✓ |
| 4 | visit_fn_decl | (i) BlockStmt | OptChain | T | 3 同 | 3 同 | ✓ |
| 5 | visit_fn_decl | (i) BlockStmt | Bang | T | 3 同 (OptionTruthyShape::EarlyReturnFromExitWithElse) | 3 同 | ✓ |
| 6 | visit_fn_decl | (i) BlockStmt | Truthy | T | 2 (post-if narrow なし、Truthy direction useful narrow なし) | 2 同 | ✓ |
| 7 | visit_fn_decl | (i) BlockStmt | any | F (then 非 exit) | Primary のみ | 同 | ✓ no EarlyReturn 該当 |
| 8 | resolve_arrow_expr | (i) BlockStmt | typeof | T | **2 (BUG: post-if narrow 不在)** | 3 (post-fix で発火) | ✗ **本 PRD 修正対象** |
| 9 | resolve_arrow_expr | (i) BlockStmt | instanceof | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 10 | resolve_arrow_expr | (i) BlockStmt | nullcheck | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 11 | resolve_arrow_expr | (i) BlockStmt | OptChain | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 12 | resolve_arrow_expr | (i) BlockStmt | Bang | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 13 | resolve_arrow_expr | (i) BlockStmt | Truthy | T | 2 (Truthy direction useful narrow なし) | 2 同 | ✓ no Bug (useful narrow なし) |
| 14 | resolve_arrow_expr | (ii) Expr body | n/a (no if-stmt in expression body) | n/a | n/a | n/a | NA (expression body には if-stmt が表現できない、conditional expr のみ) |
| 15 | resolve_arrow_expr | (i) BlockStmt | any | F | Primary のみ | 同 | ✓ no EarlyReturn 該当 |
| 16 | resolve_fn_expr | (i) BlockStmt | typeof | T | **2 (BUG、symmetric)** | 3 | ✗ **本 PRD 修正対象** |
| 17 | resolve_fn_expr | (i) BlockStmt | instanceof | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 18 | resolve_fn_expr | (i) BlockStmt | nullcheck | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 19 | resolve_fn_expr | (i) BlockStmt | OptChain | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 20 | resolve_fn_expr | (i) BlockStmt | Bang | T | **2 (BUG)** | 3 | ✗ **本 PRD 修正対象** |
| 21 | resolve_fn_expr | (i) BlockStmt | Truthy | T | 2 (Truthy direction useful narrow なし) | 2 同 | ✓ no Bug |
| 22 | resolve_fn_expr | (i) BlockStmt | any | F | Primary のみ | 同 | ✓ no EarlyReturn 該当 |

### 直交軸 (cross-axis)

| 軸 | 検討 | 結論 |
|----|------|------|
| nested 関数 (fn 内 arrow / arrow 内 fn etc.) | 本 PRD 修正で nested context でも block_end が正しく set される (Vec<u32> ではなく Option で stack-saved) | ✓ visit_block_stmt 内の `prev_block_end` save/restore で再帰 OK |
| async / generator function | TypeScript の async arrow / async fn-expr は body 構造同じ、本 PRD 修正で同様に効く | ✓ no special handling needed |
| arrow body = expression (non-block) | if-stmt 不在のため narrow event の post-if 概念が NA | ✓ Cell #14 = NA |
| Method (class method body) | TypeScript class method は `visit_fn_decl` 経由ではない別 path (visit_class_decl 内) | 要 audit (本 PRD scope 外、別 entry で TODO 起票候補) |

### Cross-cutting Invariants

| ID | Invariant | Verification |
|----|-----------|--------------|
| INV-CF-1 | **Function body block_end uniformity**: 任意の TypeScript function 形式 (declaration / arrow / function-expression / class method) の body block で `current_block_end` が body block の `.hi` に set される | 全 method の body 走査が `visit_block_stmt` 経由であることを grep で検証 |
| INV-CF-2 | **Nested scope safety**: nested function context で `current_block_end` が outer / inner で混乱せず、stack-like に正しく save/restore される | visit_block_stmt 内の `prev_block_end` save/restore で保証、本 PRD では既存 mechanism を利用 |

## Goal

`resolve_arrow_expr` / `resolve_fn_expr` の body traversal を `visit_block_stmt` 経由に統一し、`current_block_end` を arrow / fn-expr body 内の `detect_early_return_narrowing` で正しく利用可能にする。これにより:

1. **Empirical defect 解消**: callable interface arrow form / fn-expr form の typeof / instanceof / nullcheck / OptChain / Bang narrow guard with post-if scope が `EarlyReturnComplement` narrow event を正しく push する。
2. **I-177-B `#[ignore]` 解除**: `collect_leaves_typeof_narrow_post_if_return` test (callable interface arrow form) が GREEN に。Plan η Step 2 の callable arrow form coverage が完成。
3. **回帰ゼロ**: 既存 lib test 3140 + integration 122 + e2e 156 を全 pass。

## Scope

### In Scope

- `resolve_arrow_expr` (fn_exprs.rs:196-203) の `BlockStmtOrExpr::BlockStmt(block)` arm の `for stmt in &block.stmts { self.visit_stmt(stmt); }` を `self.visit_block_stmt(block);` に変更
- `resolve_fn_expr` (fn_exprs.rs:252-259) の同パターンを同様に修正 (symmetric cohesion)
- Unit test: arrow / fn_expr body 内の typeof + post-if return scenario で narrow event 3 件 (Primary then + Primary else + EarlyReturnComplement post-if) が push されることを verify
- I-177-B `collect_leaves_typeof_narrow_post_if_return` test の `#[ignore]` 解除 + GREEN 確認
- E2E fixture: arrow form / fn-expr form の typeof + post-if return パターンを runtime stdout 一致で lock-in

### Out of Scope

- TypeScript class method の body traversal cohesion (Cross-axis 検討で audit 候補だが別 entry)
- I-177-A / I-177-C / I-177 mutation propagation 本体 / I-048 (Plan η 後続)

## Design

### Technical Approach

**Step 1 — `resolve_arrow_expr` の body traversal 修正**

```rust
// src/pipeline/type_resolver/fn_exprs.rs:196-203
match &*arrow.body {
    ast::BlockStmtOrExpr::BlockStmt(block) => {
        let param_pats: Vec<&ast::Pat> = arrow.params.iter().collect();
        self.collect_emission_hints(block, &param_pats);
        self.visit_block_stmt(block);
    }
    ast::BlockStmtOrExpr::Expr(expr) => { /* unchanged */ }
}
```

**Step 2 — `resolve_fn_expr` の body traversal 修正 (symmetric)**

```rust
// src/pipeline/type_resolver/fn_exprs.rs:252-259
if let Some(body) = &fn_expr.function.body {
    let param_pats: Vec<&ast::Pat> =
        fn_expr.function.params.iter().map(|p| &p.pat).collect();
    self.collect_emission_hints(body, &param_pats);
    self.visit_block_stmt(body);
}
```

両 method とも既に function-level の `enter_scope` を行っており (line 129 / line 230)、`visit_block_stmt` 内で nested block scope が enter されるが、これは `visit_fn_decl` の pattern と完全 symmetric (visit_fn_decl も function-level + body-level の 2 重 scope)。

### Design Integrity Review

per `.claude/rules/design-integrity.md`:

- **Higher-level consistency**: `visit_fn_decl` と `resolve_arrow_expr` / `resolve_fn_expr` は **同じ "function body traversal" abstraction** に属する。本 PRD で 3 method が同 traversal mechanism (`visit_block_stmt`) を共有することで abstraction level の cohesion が完成。
- **DRY**: 「function body は visit_block_stmt 経由で walk」という invariant が 1 箇所に集約 (visit_block_stmt method)。3 caller が同じ knowledge を独立 encode する duplication を解消。
- **Orthogonality**: visit_block_stmt は「block stmt traversal + scope + block_end tracking」の単一責務、各 caller は「function-level scope + body 委譲」の責務を保ち、cross-cutting concern を明確に分離。
- **Coupling**: 変更により caller の visit_block_stmt 依存が顕在化するが、これは pipeline-internal な必須依存で couplingを抽象化する余地なし。

**Broken windows 検出**:
- `resolve_arrow_expr` / `resolve_fn_expr` が visit_block_stmt を skip していたのは broken window pattern (visit_fn_decl と semantic divergence)。本 PRD で正規化。

**Verified, no design issues remaining post-fix.**

### Impact Area

- **変更**: `src/pipeline/type_resolver/fn_exprs.rs` (2 箇所、各 1 行 + 削除 2 行 = 正味 -2 LOC)
- **変更**: `src/transformer/return_wrap.rs` (`#[ignore]` annotation 削除)
- **新規**: `tests/e2e/scripts/i177-f-arrow-fn-expr-narrow-cohesion.ts` (E2E fixture)
- **新規**: `tests/e2e_test.rs` (E2E entry)

LOC 推定: production code -2 LOC + 削除 1 ignore + E2E fixture ~30 行 + test entry ~10 行 = 正味 +40 LOC (うち test ~40 LOC)。**production change は 2 行のみ**。

### Semantic Safety Analysis

per `.claude/rules/type-fallback-safety.md`:

本 PRD は型 fallback 導入なし。**TypeResolver body traversal completeness の修正のみ**。3-step analysis:

1. **Identify all usage sites**: `resolve_arrow_expr` / `resolve_fn_expr` の body 内 if-stmt が `current_block_end` 経由で `detect_early_return_narrowing` を発火する path。
2. **Classify each usage site**:
   - **Pre-fix**: `current_block_end` None → `detect_early_return_narrowing` skip → post-if narrow event 不在 → silent type widening at narrow-stale read sites (post-fix の Tier 2 compile error / 隣接 fix が active な場合は Tier 1 silent semantic change のリスク)
   - **Post-fix**: `current_block_end` set → `detect_early_return_narrowing` 発火 → post-if narrow event push → narrow-aware type info propagation
3. **Verdict**: **Safe** — pre-fix は silent type widening (より wide な型) で、post-fix は narrow type を提供する。silent regression は不可能 (None → Some(narrower) は narrow event を **追加** するだけで既存 narrow event は変更しない)。

**E2E regression risk**: 既存 E2E test 156 件は pre-fix で何らかの narrow path を通過していた可能性。post-fix で追加 narrow event が発生する場合、IR emission が変わる可能性。**緩和策**: T5 で Hono benchmark + 全 E2E test 実行で回帰検出。

## Task List

TDD 順序: T1 (RED test) → T2 (GREEN production fix) → T3 (REFACTOR / ignore 解除) → T4 (E2E lock-in) → T5 (verification)。

### T1: RED — 既存 `#[ignore]` test と新規 fn_expr form unit test の RED 確認

- **Work**:
  - `src/transformer/return_wrap.rs` の `collect_leaves_typeof_narrow_post_if_return` (現在 `#[ignore]`) の `#[ignore]` annotation を削除
  - 新規 unit test `collect_leaves_typeof_narrow_post_if_return_fn_expr` を追加 (resolve_fn_expr 経路、`function (x: string | number): string | number { ... }` 形式)
- **Completion criteria**: 上記 2 test が **RED** (`current_block_end` 未 set のため narrow event 不足で leaves[1].ty = F64OrString のまま)
- **Depends on**: なし
- **Prerequisites**: I-177-B + I-177-E 完了 (✓ 2026-04-26)

### T2: GREEN — `resolve_arrow_expr` / `resolve_fn_expr` の body traversal を `visit_block_stmt` 経由に変更

- **Work**:
  - `src/pipeline/type_resolver/fn_exprs.rs:200-202` (resolve_arrow_expr) の `for stmt in &block.stmts { self.visit_stmt(stmt); }` を `self.visit_block_stmt(block);` に変更
  - `src/pipeline/type_resolver/fn_exprs.rs:256-258` (resolve_fn_expr) の同パターンを同様に変更
- **Completion criteria**:
  - T1 で追加した 2 test が GREEN
  - `cargo test --lib` 全 pass (回帰 0)
  - `cargo build --release` 警告 0
- **Depends on**: T1
- **Prerequisites**: なし

### T3: REFACTOR / verification — clippy / fmt + 既存 test 全 pass

- **Work**:
  - `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
  - `cargo fmt --all --check` 0 diff
  - 既存 lib test 3140 + integration 122 + compile 3 + e2e 156 全 pass 確認
- **Completion criteria**:
  - 上記 verification 全 pass
- **Depends on**: T2
- **Prerequisites**: なし

### T4: E2E lock-in — arrow form / fn-expr form の empirical scenario fixture

- **Work**:
  - `tests/e2e/scripts/i177-f-arrow-fn-expr-narrow-cohesion.ts` を作成 (top-level として配置、既存 E2E pattern に従う):
    ```ts
    // arrow form (top-level const)
    const fArrow = (x: string | number): number => {
        if (typeof x === "number") return x * 2;
        return x.length;
    };
    // fn-expr form (top-level const)
    const fFnExpr = function(x: string | number): number {
        if (typeof x === "number") return x * 2;
        return x.length;
    };
    function main(): void {
        console.log(fArrow(10));
        console.log(fArrow("hello"));
        console.log(fFnExpr(10));
        console.log(fFnExpr("hello"));
    }
    ```
  - `tests/e2e_test.rs` に該当 entry 追加
  - 期待 stdout: `20\n5\n20\n5\n`
- **Completion criteria**:
  - `cargo test --test e2e_test test_e2e_i177_f_arrow_fn_expr_narrow_cohesion` GREEN
  - tsc / tsx の runtime stdout と Rust 実行結果が byte-exact 一致
- **Depends on**: T3
- **Prerequisites**: なし

### T5: 回帰 verification + Hono benchmark

- **Work**:
  - `cargo test` 全 pass を確認
  - `./scripts/hono-bench.sh` 実行、pre/post で `clean files` / `error instances` の diff を測定
  - **期待**: 回帰 0、potentially clean files +N (silent type widening 解消で Hono codebase 内の既 silent latent が顕在化 → improvement の可能性)
- **Completion criteria**:
  - `cargo test` 0 fail
  - Hono bench worsening 0
- **Depends on**: T4
- **Prerequisites**: なし

## Test Plan

### Unit tests (新規 1 件 + ignore 解除 1 件)

1. (`#[ignore]` 解除) `transformer::return_wrap::tests::collect_leaves_typeof_narrow_post_if_return` (callable interface arrow form)
2. (新規) `transformer::return_wrap::tests::collect_leaves_typeof_narrow_post_if_return_fn_expr` (function expression form)

### E2E (新規 1 fixture)

- `tests/e2e/scripts/i177-f-arrow-fn-expr-narrow-cohesion.ts` (arrow form + fn-expr form を 1 fixture に統合、各々 number / string 入力で typeof narrow path を完全 cover)

### Regression protection (既存 test)

- 既存 lib test 3140 / integration test 122 / compile test 3 / E2E test 156 が全 pass
- Hono benchmark で `clean files` / `error instances` 回帰 0

## Completion Criteria

`.claude/rules/prd-completion.md` 準拠:

- [ ] T1〜T5 全 task の Completion criteria 達成
- [ ] Problem Space matrix の全 cell (#1〜#22) に対し post-fix 出力が ideal 仕様と一致
- [ ] `cargo test` 全 pass
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
- [ ] `cargo fmt --all --check` 0 diff
- [ ] Hono benchmark で worsening 0
- [ ] **I-177-B integration**: I-177-B の `collect_leaves_typeof_narrow_post_if_return` test (`#[ignore]`) が本 PRD 完了で GREEN に転じることを T3 で確認
- [ ] CLAUDE.md / plan.md / TODO 更新 (PRD close 時、I-177-F 該当 entry 削除 + 直近完了作業 section 追記 + Plan η の chain で本 PRD を Step 2.5 として正規化)

---

## 参考 (関連ファイル)

- `src/pipeline/type_resolver/fn_exprs.rs:196-203` (resolve_arrow_expr — 修正対象)
- `src/pipeline/type_resolver/fn_exprs.rs:252-259` (resolve_fn_expr — 修正対象、symmetric cohesion)
- `src/pipeline/type_resolver/visitors.rs:81-135` (visit_fn_decl — reference pattern、visit_block_stmt 経由)
- `src/pipeline/type_resolver/visitors.rs:571-580` (visit_block_stmt — current_block_end set/restore)
- `src/pipeline/type_resolver/visitors.rs:735-740` (visit_if_stmt — detect_early_return_narrowing 呼び出し条件)
- `src/transformer/return_wrap.rs:931-...` (collect_leaves_typeof_narrow_post_if_return — `#[ignore]` 解除対象)
- `backlog/I-177-B-collect-expr-leaf-types-cohesion.md` (本 PRD prerequisite link、Status section で `#[ignore]` 解除を本 PRD 完了で確認)
- `plan.md` Plan η Step 2.5 (本 PRD)
