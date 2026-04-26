# I-177-B: `collect_expr_leaf_types` query 順序 fix + leaf type resolution cohesion

**Plan η Step 2 (PRD 2)** — narrow framework cohesion 完成のための DRY 違反解消。

## Status (2026-04-26 close、post I-177-F batch)

- **T1〜T3 完了**: canonical primitive 実装 + 3 site 統一 (DRY violation 解消)
- **5 canonical helper unit test GREEN** + **3 collect_leaves narrow scenario test GREEN** (declaration form + arrow form + fn-expression form 全 cover、I-177-F batch close で `#[ignore]` 解除済)
- **T4 E2E**: I-177-E + I-177-F E2E fixtures が canonical helper 経由の post-narrow type resolution を end-to-end で検証 (declaration form は I-177-E E2E、arrow form は I-177-F E2E が cover)
- **T5 verification**: cargo test 0 fail (lib 3142 + e2e 157 + integration 122 + compile 3、**0 ignored**)、clippy 0 warning、fmt clean、Hono bench 0 regression (clean 111 / errors 63 unchanged)。
- **`/check_job` 4-layer review (2026-04-26、batch 全体) findings 反映済**:
  - L1-1 (test naming): 全 3 test に `test_` prefix 統一 (`test_collect_leaves_typeof_narrow_post_if_return_*`)
  - L1-2 (`apply_substitutions_to_items` doc-impl mismatch): call site comment 強化で正規化 (defense-in-depth concern を **TODO [I-177-G]** 起票)
  - L3 (Cell #5 AnyEnum 要検証): I-177-E PRD で **NA** に確定 (any-enum は per-file file_any_synthetic 経由、fork query path に乗らない)
  - L4 (Spec gap × 3 件 framework signal): cross-axis enumeration を non-matrix-driven PRD でも適用する framework 改善を **TODO [I-198]** 起票
  - L4-T4 (callable arrow form scope reduction): **I-177-F batch close で完全解消** (`#[ignore]` 解除、test GREEN)、scope reduction 違反なし

## Plan η Step 1.5 (I-177-E) 起票経緯

PRD 当初の T4 (E2E lock-in) で empirical defect 解消が確認できなかったため逐次 dbg trace を実施し、`SyntheticTypeRegistry::fork_dedup_state` の `union_dedup` 継承 + `types: BTreeMap::new()` 設計が builtin pre-registered union 型を fork から query 不能にする pre-existing latent bug を発見 (`backlog/I-177-E-synthetic-fork-types-inheritance.md` 参照、同 2026-04-26 close)。本 PRD の canonical helper は I-177-E が prerequisite として close された前提で initial design 通り効力を発揮する。

---

## Background

I-144 / I-161 / I-171 / I-177-D で確立した narrow framework は、変数 Ident の型 lookup
において「**narrowed_type 優先 → expr_type fallback**」を canonical precedence とする。
この knowledge は production code 内 3 箇所に encode されている:

1. `Transformer::get_type_for_var` (`src/transformer/expressions/type_resolution.rs:20-32`)
   — name + span lookup、**正順 (narrow → expr)**
2. `Transformer::get_expr_type` (`src/transformer/expressions/type_resolution.rs:39-58`)
   — Expr lookup、**正順 (narrow → expr)**
3. `collect_expr_leaf_types` (`src/transformer/return_wrap.rs:391-432`)
   — return-wrap path leaf lookup、**逆順 (expr → narrow fallback only on Unknown)**

3 箇所が同一 knowledge を独立 encode する **DRY violation** であり、3 番目だけ
precedence が反転していることが defect の根本原因。`expr_type(span)` は Ident に対し
declared union type (例: `F64OrString`) を `ResolvedType::Known` で常に返すため、
narrowed_type は永久に query されず、return-wrap は declared union を見て variant
解決に失敗する。

### Empirical defect (2026-04-26 reproduce confirmed)

```ts
function h(x: string | number): string | number {
    if (typeof x === "string") return 0;
    else { console.log("ne"); }
    return x;
}
```

- **Plain function declaration form**: ts_to_rs が hard error 終了
  ```
  Error: unsupported syntax: cannot determine return variant at byte 135..136
  for union F64OrString
  (expr: Ident("x"), type: Some(Named { name: "F64OrString", type_args: [] }))
  ```
  `wrap_leaf` step 5 (`return_wrap.rs:206-210`) を踏み、`F64OrString` enum 自身は
  `variant_for(F64OrString)` で None を返す + `non_option_variants.len() = 2` で
  fallback step 4 も skip → hard error。

- **Callable interface arrow form** (`interface H { (x: ...): ...; } const h: H = (x) => {...}`):
  conversion は succeed するが、生成 Rust の trailing tail expr が `x` のまま
  (variant wrap 欠落)。Rust scope の偶然 (F64 inner = `Copy`、A arm 早期 return)
  により compile + runtime は accidentally correct。これは silent semantic risk:
  body shape が変われば post-match `x` は moved-out で E0382、または別 variant
  選択で wrong runtime value 返却の可能性。

### 関連 PRD

- **I-177-D 完了 (2026-04-26)**: TypeResolver `narrowed_type` の suppression scope を
  trigger-kind-based dispatch (Primary 非 suppress / EarlyReturnComplement suppress) に
  refactor。本 PRD はその cohesion を **return-wrap leaf path** にも拡張し、Plan η
  framework を最初の 2 PRD で完成させる。
- **後続 (Plan η)**: PRD 3 (I-177 mutation propagation 本体) → PRD 4 (I-177-A) → ...
- **無関係**: I-194 (typeof if-block elision、I-177-A scope 拡張候補)、I-048 (closure ownership)。

## Problem Space

本 PRD は **non-matrix-driven** (Plan η 確定: ~10 LOC、light spec)。AST shape / TS type /
emission context の直積 matrix を持たない **structural refactor** であり、
`spec-first-prd.md` の 2-stage workflow 対象外。

代替として、**call site enumeration による完全 coverage** を problem space とする。

### 入力次元 (Dimensions)

| 次元 | Variant 列挙 |
|------|------------|
| Call site location | (1) `Transformer::get_type_for_var` / (2) `Transformer::get_expr_type` / (3) `collect_expr_leaf_types` / (4) future addition |
| Lookup input shape | (a) name + swc Span / (b) `&ast::Expr` (Ident or non-Ident) / (c) name + position only |
| Variable narrow state | (i) declared 型のみ / (ii) narrowed_type が active / (iii) suppression が active (closure-reassign × EarlyReturnComplement) |
| Return type | `Option<&'a RustType>` (borrow) / `Option<RustType>` (owned, ReturnLeafType 用) |

### 組合せマトリクス (call site × narrow state)

| # | Call site | Lookup input | Narrow state | Pre-fix 出力 | Post-fix 出力 (ideal) | 判定 |
|---|-----------|------------|------------|-----------|-----------------------|------|
| 1 | get_type_for_var | name + span | (i) declared | declared | declared | ✓ no change |
| 2 | get_type_for_var | name + span | (ii) narrowed | narrowed | narrowed | ✓ no change |
| 3 | get_type_for_var | name + span | (iii) suppressed | declared | declared | ✓ no change |
| 4 | get_expr_type | Ident | (i) | declared | declared | ✓ no change |
| 5 | get_expr_type | Ident | (ii) | narrowed | narrowed | ✓ no change |
| 6 | get_expr_type | Ident | (iii) | declared | declared | ✓ no change |
| 7 | get_expr_type | non-Ident | — | expr_type | expr_type | ✓ no change |
| 8 | collect_expr_leaf_types | Ident | (i) | declared | declared | ✓ |
| 9 | collect_expr_leaf_types | Ident | (ii) | **declared (BUG)** | narrowed | ✗ **本 PRD 修正対象** |
| 10 | collect_expr_leaf_types | Ident | (iii) | declared | declared | ✓ post-fix consistent |
| 11 | collect_expr_leaf_types | non-Ident | — | expr_type | expr_type | ✓ no change |
| 12 | future addition | any | any | — | canonical helper 経由で自動的に正順 | NA (defense-in-depth) |

### 直交軸 (cross-axis)

| 軸 | 検討 | 結論 |
|----|------|------|
| Mutation propagation 関与 | F1/F3 narrow body mutation の case で leaf type lookup が誤動作するか | **無関係**: mutation propagation は emission 側 (PRD 3 scope)、type lookup precedence と独立 |
| TypeResolver suppression dispatch | I-177-D の trigger-kind-based suppression が leaf path に正しく effect するか | **automatic**: `narrowed_type()` 内部で suppression 判定済、本 PRD は呼び出し順序のみ修正 |
| Borrow vs owned return | get_expr_type は `&RustType`、collect_expr_leaf_types は `Option<RustType>` (clone 必要) | **canonical helper の signature**: 共通 primitive を `Option<&RustType>` で返し、呼び出し側で必要なら `.cloned()` |
| 並列 narrow path | `OptionTruthyShape` / `EarlyReturnComplement` / `Primary` 各 trigger の narrow 状態 | **automatic**: `narrowed_type()` 内部で判定、本 PRD は呼び出し順序のみ |

### Cross-cutting Invariants

| ID | Invariant | Verification |
|----|-----------|--------------|
| INV-CB-1 | **Canonical leaf type lookup precedence**: 任意の AST 位置における型 lookup は「Ident なら narrowed_type 優先 → expr_type fallback、非 Ident なら expr_type のみ」の単一 contract を持つ | Production code 内で `narrowed_type` と `expr_type` を別々に呼ぶ pattern が `FileTypeResolution::resolve_*_type` 以外に存在しないこと (grep verification) |
| INV-CB-2 | **3 site uniformity**: `get_type_for_var` / `get_expr_type` / `collect_expr_leaf_types` の挙動が同一 narrow state input に対し同一 type を返す | 同一 TS source に対し 3 site を呼んだとき返り値が一致 (unit test で 3 site cohesion 確認) |

## Goal

`FileTypeResolution` に canonical primitive `resolve_var_type(name, span)` /
`resolve_expr_type(expr)` を追加し、3 production site を canonical 経由に統一する。
これにより:

1. **Empirical defect 解消**: `function h(x: string | number)` の typeof narrow + 後続
   `return x` パターンで return-wrap が narrowed type (`F64`) を見て variant を正しく
   解決する。declaration form の hard error が消え、callable interface form も
   structurally correct な variant wrap を emit する。
2. **DRY violation 解消**: 「narrowed_type 優先 → expr_type fallback」という knowledge が
   `FileTypeResolution` 1 箇所に集約され、future addition でも canonical を呼ぶ限り
   precedence ずれが構造的に発生不可能になる。
3. **回帰ゼロ**: I-144 / I-161 / I-171 / I-177-D で確立した既存 narrow 動作 (3131 lib
   test + 122 integration + 155 E2E pass) を全て保持。

## Scope

### In Scope

- `FileTypeResolution::resolve_var_type(&self, name: &str, span: swc_common::Span) -> Option<&RustType>` 追加
- `FileTypeResolution::resolve_expr_type(&self, expr: &ast::Expr) -> Option<&RustType>` 追加
- `Transformer::get_type_for_var` を `resolve_var_type` の thin wrapper に変更
- `Transformer::get_expr_type` を `resolve_expr_type` の thin wrapper に変更
- `collect_expr_leaf_types` を `resolve_expr_type(...).cloned()` 経由に変更
- Unit test: `resolve_expr_type` / `resolve_var_type` が Ident narrow scenario で narrowed
  type を返すこと、suppression scenario で declared を返すこと、non-Ident で expr_type を
  返すこと
- Unit test: `collect_expr_leaf_types` が typeof narrow + post-narrow Ident return で
  narrowed type を含む `ReturnLeafType` を返すこと
- E2E fixture: `tests/e2e/scripts/i177-b-leaf-narrow-cohesion.ts` (declaration form +
  callable interface form 両方) で runtime stdout が tsc/tsx と byte-exact 一致

### Out of Scope

- I-177 mutation propagation 本体 (PRD 3、case body mutation の outer Option 反映)
- I-177-A else_block_pattern Let-wrap (PRD 4、emission 側の cohesion gap)
- I-177-C symmetric XOR early-return (PRD 5)
- I-048 closure ownership inference (PRD 6)
- I-194 typeof if-block elision (Tier 3-4 deferral、I-177-A scope 拡張候補)

## Design

### Technical Approach

**Step 1 — Canonical primitive を `FileTypeResolution` に追加**

```rust
// src/pipeline/type_resolution.rs
impl FileTypeResolution {
    /// Resolves the type of a variable at a given byte position.
    ///
    /// Canonical precedence:
    /// 1. If a [`NarrowEvent::Narrow`] applies at `position`, returns the narrowed type.
    /// 2. Otherwise, returns the resolved expression type from `expr_type(span)`.
    /// 3. Returns `None` if neither is known.
    ///
    /// Suppression dispatch (closure-reassign × `EarlyReturnComplement`) is
    /// internal to [`narrowed_type`], so callers never need to compose
    /// `narrowed_type` and `expr_type` manually. Composing them in reverse
    /// order (e.g., `expr_type` first → `narrowed_type` only on `Unknown`)
    /// silently drops narrowing because `expr_type` returns `Known(declared)`
    /// for any Ident with a declared type.
    pub fn resolve_var_type(
        &self,
        name: &str,
        span: swc_common::Span,
    ) -> Option<&RustType> {
        if let Some(narrowed) = self.narrowed_type(name, span.lo.0) {
            return Some(narrowed);
        }
        match self.expr_type(Span::from_swc(span)) {
            ResolvedType::Known(ty) => Some(ty),
            ResolvedType::Unknown => None,
        }
    }

    /// Resolves the type of an arbitrary expression.
    ///
    /// For an `Ident` expression, delegates to [`resolve_var_type`] so the
    /// canonical narrow precedence is preserved. For all other expressions,
    /// returns the resolved type from `expr_type(expr.span())`, since
    /// non-Ident expressions are not subject to per-variable narrowing.
    pub fn resolve_expr_type(&self, expr: &ast::Expr) -> Option<&RustType> {
        if let ast::Expr::Ident(ident) = expr {
            return self.resolve_var_type(ident.sym.as_ref(), ident.span);
        }
        match self.expr_type(Span::from_swc(expr.span())) {
            ResolvedType::Known(ty) => Some(ty),
            ResolvedType::Unknown => None,
        }
    }
}
```

`use swc_ecma_ast as ast;` と `use swc_common::Spanned;` を必要に応じて追加。

**Step 2 — Transformer 既存 method を thin wrapper 化**

```rust
// src/transformer/expressions/type_resolution.rs
impl<'a> Transformer<'a> {
    pub(crate) fn get_type_for_var(
        &self,
        name: &str,
        span: swc_common::Span,
    ) -> Option<&'a RustType> {
        self.tctx.type_resolution.resolve_var_type(name, span)
    }

    pub(crate) fn get_expr_type(&self, expr: &ast::Expr) -> Option<&'a RustType> {
        self.tctx.type_resolution.resolve_expr_type(expr)
    }
}
```

doc comment は canonical 側に集約 (DRY)、wrapper 側は最小コメント。

**Step 3 — `collect_expr_leaf_types` を canonical 経由に切替**

```rust
// src/transformer/return_wrap.rs
fn collect_expr_leaf_types(
    expr: &ast::Expr,
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    match expr {
        ast::Expr::Cond(cond) => {
            collect_expr_leaf_types(&cond.cons, type_resolution, out);
            collect_expr_leaf_types(&cond.alt, type_resolution, out);
        }
        ast::Expr::Paren(paren) => {
            collect_expr_leaf_types(&paren.expr, type_resolution, out);
        }
        leaf => {
            let swc_span = leaf.span();
            let ty = type_resolution.resolve_expr_type(leaf).cloned();
            out.push(ReturnLeafType {
                ty,
                span: (swc_span.lo.0, swc_span.hi.0),
            });
        }
    }
}
```

`Span::from_swc` import 不要、`narrowed_type` の直接参照削除、`ResolvedType` import 削除
(canonical helper 内に閉じる)。

### Design Integrity Review

per `.claude/rules/design-integrity.md`:

- **Higher-level consistency**: `FileTypeResolution` は immutable resolution data を提供
  する API surface。`narrowed_type` / `expr_type` / `expected_type` / `is_mutable` 等の
  query method 群と並び、`resolve_*_type` という abstraction を加える。同 module 内の
  既存 query method と同 abstraction level (✓ 一貫)。
- **DRY**: 3 site の knowledge duplication を canonical helper 1 箇所に集約。
  Future addition でも canonical を呼ぶ限り precedence 不一致が発生不可能 (broken-window
  resilience)。
- **Orthogonality**: `resolve_var_type` は「name + span → narrow-aware type」、
  `resolve_expr_type` は「Expr → narrow-aware type (Ident なら resolve_var_type 経由)」と
  単一責務。`narrowed_type` / `expr_type` の primitive query は変更せず、composite
  layer のみ追加 (orthogonal)。
- **Coupling**: `FileTypeResolution` が `swc_ecma_ast` 依存を新規導入。既に
  `swc_common::Span` 経由で SWC 依存はあり、`ast::Expr` 追加は同 dependency family
  への自然な拡張。pipeline-integrity.md の Transformer → IR ← Generator 方向に逆流
  しない (TypeResolver は Transformer の上流、Transformer は本 helper を consume)。

**Broken windows 検出**: なし。

**Verified, no design issues.**

### Impact Area

- **新規追加**: `src/pipeline/type_resolution.rs` (canonical helper 2 method)
- **変更**: `src/transformer/expressions/type_resolution.rs` (2 wrapper 簡略化)
- **変更**: `src/transformer/return_wrap.rs` (`collect_expr_leaf_types` 1 関数の query
  precedence 修正 + import cleanup)

LOC 推定: 新規 ~40 LOC (doc 込み)、変更 ~20 LOC、削除 ~15 LOC = 正味 +45 LOC
(structural fix としては DRY 解消の対価で adequate、Plan η 概算 ~10 LOC は単独
inversion fix の boundary lower estimate)。

### Semantic Safety Analysis

per `.claude/rules/type-fallback-safety.md`:

本 PRD は型 fallback (`Any` / wider union / HashMap) の導入なし。**型 lookup
precedence の修正のみ** (既存 fallback 経路を変えず、Ident 時の narrow 優先を
復元)。3-step analysis:

1. **Identify all usage sites**: `collect_expr_leaf_types` の出力 = `ReturnLeafType.ty`
   (Option<RustType>)。consumer は `wrap_leaf` の step 3 (variant lookup)。
2. **Classify each usage site**:
   - **Pre-fix Ident with narrow active**: `expr_type=declared` を見て `variant_for(declared)`
     が None、step 4 で fallback 失敗 → hard error または bare `x` emission (silent risk)
   - **Post-fix Ident with narrow active**: `narrowed_type=narrowed` (例: `F64`) を見て
     `variant_for(F64)` がマッチ、`F64OrString::F64(x)` で wrap → **TS と semantic 一致**
   - **Ident without narrow active** (Tier 0 invariant): pre/post-fix 共に declared を返す
     → no behavioral change
3. **Verdict**: **Safe** — pre-fix では (a) hard error または (b) silently broken
   (non-wrapped tail expr) のいずれか。post-fix では variant wrap が正しく挿入され
   TS runtime と semantic 一致する。silent regression は一切発生しない (pre-fix で
   "正しく動いていた" cell は narrow 非 active であり post-fix も declared 返却で同一
   出力)。

## Task List

TDD 順序: T1 (RED test) → T2 (GREEN canonical helper) → T3 (REFACTOR 3 site 切替) →
T4 (E2E lock-in)。

### T1: RED — `resolve_expr_type` / `collect_expr_leaf_types` narrow scenario test 追加

- **Work**:
  - `src/pipeline/type_resolution.rs` の `#[cfg(test)] mod tests` に以下を追加:
    - `test_resolve_var_type_returns_narrowed_when_active`: `narrow_events` に Narrow event を
      仕込み、`resolve_var_type("x", span_inside_scope)` が narrowed type を返すこと
    - `test_resolve_var_type_returns_declared_when_outside_scope`: `resolve_var_type("x",
      span_outside_scope)` が declared `expr_type` を返すこと
    - `test_resolve_var_type_returns_declared_when_suppressed`: closure-reassign +
      `EarlyReturnComplement` の suppression scenario で declared を返すこと
    - `test_resolve_expr_type_delegates_to_var_type_for_ident`: Ident expr で narrow が
      active なら narrowed を返すこと (canonical fall-through)
    - `test_resolve_expr_type_uses_expr_type_for_non_ident`: 非 Ident (e.g., Lit, Member)
      では `expr_type` を返すこと
  - `src/transformer/return_wrap.rs` の tests module に以下を追加:
    - `collect_leaves_typeof_narrow_post_if_return`: 既存 `collect_leaves_for_callable`
      helper を流用して typeof narrow + post-if `return x` シナリオで `leaves[N].ty`
      が narrowed type を含むこと
- **Completion criteria**: 上記 6 test を追加し、**全て RED** (canonical helper 未存在 +
  `collect_expr_leaf_types` が declared を返すため expected != actual)
- **Depends on**: なし
- **Prerequisites**: I-177-D 完了 (✓ 2026-04-26)

### T2: GREEN — `FileTypeResolution::resolve_var_type` / `resolve_expr_type` 実装

- **Work**:
  - `src/pipeline/type_resolution.rs` に Design section の Step 1 コードを追加
  - `swc_ecma_ast` import 追加 (file 先頭)
  - `swc_common::Spanned` trait import 追加 (`expr.span()` 用)
- **Completion criteria**:
  - T1 で追加した `resolve_var_type_*` / `resolve_expr_type_*` の 5 unit test が GREEN
  - `cargo test --lib pipeline::type_resolution::tests` 0 fail
  - `cargo build --release` 警告 0
- **Depends on**: T1
- **Prerequisites**: なし

### T3: REFACTOR — 3 site を canonical 経由に切替

- **Work**:
  - `src/transformer/expressions/type_resolution.rs`: `get_type_for_var` / `get_expr_type`
    を Design section Step 2 の thin wrapper に変更。doc comment は canonical 側に集約、
    wrapper 側は最小コメント (delegating note)。`Span::from_swc` import が不要なら削除、
    `ResolvedType` import が他で使われていなければ削除
  - `src/transformer/return_wrap.rs`: `collect_expr_leaf_types` を Design section Step 3
    の form に変更。`Span::from_swc` import 削除、`type_resolution.expr_type` /
    `narrowed_type` の直接呼び出しを `resolve_expr_type` に置換
- **Completion criteria**:
  - T1 で追加した `collect_leaves_typeof_narrow_post_if_return` test が GREEN
  - 既存 lib test (3131) / integration test (122) / compile test (3) が回帰 0
  - `cargo test --lib` 0 fail
  - `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
  - `cargo fmt --all --check` 0 diff
- **Depends on**: T2
- **Prerequisites**: なし

### T4: E2E lock-in — empirical scenario fixture

- **Work**:
  - `tests/e2e/scripts/i177-b-leaf-narrow-cohesion.ts` を作成。declaration form と
    callable interface form 両方をカバー (1 fixture file 内に複数関数で OK):
    ```ts
    // declaration form (TODO empirical 2026-04-24 reproduce)
    function h(x: string | number): string | number {
        if (typeof x === "string") return 0;
        else { console.log("ne"); }
        return x;
    }
    // callable interface form (silent risk)
    interface I { (x: string | number): string | number }
    const i: I = (x): string | number => {
        if (typeof x === "string") return 0;
        else { console.log("ne"); }
        return x;
    };
    console.log(h(42));
    console.log(h("a"));
    console.log(i(42));
    console.log(i("a"));
    ```
  - `tests/e2e_test.rs` に該当 entry 追加 (既存 fixture と同じ pattern で stdout 一致 assert)
  - `npx tsx <fixture>` で expected stdout を取得、E2E framework に oracle として記録
- **Completion criteria**:
  - `cargo test --test e2e_test i177_b_leaf_narrow_cohesion` GREEN
  - 生成 Rust 内で `return x` (declaration) と trailing `x` (callable) の両方が
    `F64OrString::F64(x)` で variant wrap されていること (生成出力を `--show-output` で
    目視確認)
  - tsc / tsx の runtime stdout (`ne / 42 / 0 / ne / 42 / 0`) と Rust 実行結果が
    byte-exact 一致
- **Depends on**: T3
- **Prerequisites**: なし

### T5: 回帰 verification + Hono benchmark

- **Work**:
  - `cargo test` (lib + integration + compile + e2e) 全 pass を確認
  - `./scripts/hono-bench.sh` 実行、pre/post で `clean files` / `error instances` の
    diff を測定。**期待**: 回帰 0、potentially clean files +N (declaration form の
    typeof narrow + return が Hono に存在すれば clean に転じる可能性あり)
  - `bench-history.jsonl` に新行追加 (自動)
- **Completion criteria**:
  - `cargo test` 0 fail
  - Hono bench で回帰 0 (clean files / error instances 共に worsening なし)
  - non-deterministic variance ±1 / ±2 範囲内なら GREEN
- **Depends on**: T4
- **Prerequisites**: なし

## Test Plan

### Unit tests (新規 6 件)

1. `pipeline::type_resolution::tests::test_resolve_var_type_returns_narrowed_when_active`
2. `pipeline::type_resolution::tests::test_resolve_var_type_returns_declared_when_outside_scope`
3. `pipeline::type_resolution::tests::test_resolve_var_type_returns_declared_when_suppressed`
4. `pipeline::type_resolution::tests::test_resolve_expr_type_delegates_to_var_type_for_ident`
5. `pipeline::type_resolution::tests::test_resolve_expr_type_uses_expr_type_for_non_ident`
6. `transformer::return_wrap::tests::collect_leaves_typeof_narrow_post_if_return`

### Test coverage gap analysis (Step 3b)

`.claude/rules/testing.md` の C1 branch coverage / equivalence partition technique 適用:

| Gap | Missing pattern | Technique | Severity |
|-----|----------------|-----------|----------|
| G1 | `collect_expr_leaf_types` の Ident leaf × narrow active (本 PRD core defect) | C1 branch coverage (Ident match arm) | High |
| G2 | `resolve_expr_type` の Ident vs non-Ident dispatch | Equivalence partition | High |
| G3 | `resolve_var_type` の suppression dispatch (closure-reassign × EarlyReturnComplement) | C1 branch coverage | Medium (既存 `narrowed_type` test が cover、本 PRD で composite として再 verify) |

全 gap を T1 unit test で cover、High severity を E2E (T4) で lock-in。

### E2E (新規 1 fixture)

- `tests/e2e/scripts/i177-b-leaf-narrow-cohesion.ts` (declaration form + callable
  interface form 両方、各々 number / string 入力で typeof narrow path を完全 cover)

### Regression protection (既存 test)

- 既存 lib test 3131 / integration test 122 / compile test 3 / E2E test 155 が全 pass
- Hono benchmark で `clean files` / `error instances` 回帰 0

## Completion Criteria

`.claude/rules/prd-completion.md` 準拠:

- [ ] T1〜T5 全 task の Completion criteria 達成
- [ ] Problem Space matrix の全 cell (#1〜#12) に対し post-fix 出力が ideal 仕様と一致
- [ ] 全 cell に lock-in test が存在 (cell #1〜#7 = 既存 narrow / get_expr_type test が cover、
      cell #8〜#11 = T1〜T4 で新規追加、cell #12 = future addition の defense-in-depth、
      canonical helper の存在自体が verification)
- [ ] `cargo test` 全 pass (lib + integration + compile + e2e + 新規 6 unit + 1 E2E)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `./scripts/check-file-lines.sh` 0 violation (`return_wrap.rs` / `type_resolution.rs`
      共に 1000 LOC 以下を維持)
- [ ] Hono benchmark で `clean files` / `error instances` 回帰 0
- [ ] 生成 Rust の empirical verification: declaration form / callable interface form
      両方で `return x` / trailing `x` が `F64OrString::F64(x)` variant wrap されていること
- [ ] `/check_job` で **Layer 1 (Mechanical)** + **Layer 4 (Adversarial trade-off)** を実施
      (本 PRD は non-matrix-driven のため Layer 2-3 は optional、ただし 3 site cohesion の
      cross-axis 検証として Layer 3 軽量実施推奨)
- [ ] CLAUDE.md / plan.md / TODO 更新 (PRD close 時、I-177-B 該当 entry 削除 + 直近完了
      作業 section 追記 + Plan η 進捗を Step 3 に進める)

### Impact estimate verification (3 instance trace)

本 PRD は Hono error reduction を直接ターゲットにしないが、empirical defect が
Hono codebase 内に存在する場合 clean files +N の可能性。3 instance trace を T5 で実施:

1. **Instance 1**: `/tmp/i177b-fn.ts` (declaration form、ts_to_rs hard error → post-fix 解消)
2. **Instance 2**: `/tmp/i177b-repro.ts` (callable interface form、silent broken tail →
   post-fix structurally correct)
3. **Instance 3**: Hono codebase 内で typeof narrow + post-narrow Ident return パターンを
   `scripts/inspect-errors.py --kind RETURN_VARIANT` 等で抽出し、本 PRD 修正後 clean に
   転じるか確認 (該当パターンが Hono に存在しない場合は instance 3 を別 synthetic
   fixture で代替)

## Spec Review

本 PRD は **non-matrix-driven** のため `spec-stage-adversarial-checklist.md` 10-rule の
全項目検証は不要。代替として以下の 5-point check を実施 (PRD 起票時 self-check):

- [x] **Call site enumeration completeness**: production code 内で `narrowed_type` /
      `expr_type` を直接 compose する全 site を grep enumerate (3 site 確認、Tests / context.rs
      の `is_empty()` check は除外)
- [x] **Defect empirical reproduction**: declaration form の hard error と callable
      interface form の silent broken tail を両方再現 (2026-04-26 confirm)
- [x] **Canonical helper signature 妥当性**: `Option<&RustType>` を返す primitive design
      で borrow / owned 両方の caller を統一 cover
- [x] **Suppression compatibility**: `narrowed_type` 内部の I-177-D suppression dispatch が
      canonical helper 経由でも自動的に効くこと (本 PRD は呼び出し順序のみ修正、
      suppression logic 不変)
- [x] **Regression scope**: 3 site の post-fix output が pre-fix の no-narrow / suppressed
      ケースで identical (matrix #1, #3, #4, #6, #8, #10, #11) であることを matrix で確認

---

## 参考 (関連ファイル)

- `src/transformer/return_wrap.rs:391-432` (collect_expr_leaf_types — 修正対象)
- `src/transformer/expressions/type_resolution.rs:20-58` (get_type_for_var / get_expr_type — wrapper 化)
- `src/pipeline/type_resolution.rs:163-250` (expr_type / narrowed_type primitives)
- `tests/e2e/scripts/` (E2E fixture 配置先)
- `plan.md` Plan η Step 2
- `TODO` I-177-B (本 PRD 起票後、PRD 完了時に削除)
