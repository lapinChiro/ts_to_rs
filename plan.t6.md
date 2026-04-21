# I-144 T6 実装計画

**対象 PRD**: `backlog/I-144-control-flow-narrowing-analyzer.md`
**前提**: T0〜T5 完了 (2026-04-20)、Spec stage approved、T1 per-cell E2E fixture 14 種が
red/green 状態で lock-in 済
**起票日**: 2026-04-20
**Framework**: SDCDF Beta (matrix-driven PRD)、`.claude/rules/spec-first-prd.md` 準拠

---

## Goal (T6 完了条件 = I-144 PRD 完了条件の抜粋)

1. **Matrix ✗ cell 9 種の E2E fixture 全 GREEN**: Cell #14 / C-1 / C-2a / C-2b / C-2c /
   I-024 / I-025 / T4d / T7
2. Interim scanner (`pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts` +
   `stmt_has_reset` + `expr_has_reset`) **完全削除** (call site 8 + 関数本体 4)
3. `try_convert_nullish_assign_stmt` が `EmissionHint` 参照で dispatch
4. JS `coerce_default` helper が JS coerce table 準拠で実装 + unit test 完備
5. Matrix ✓ cell の regression 0 (既存 narrowing 動作維持)
6. `cargo test` (lib/integration/compile/E2E) / `cargo clippy` 0 warn / `cargo fmt` 0 diff
7. Hono bench 非後退 (clean 112/158、errors 62 維持以上)
8. `/check_job` Implementation Stage review で Spec gap = 0 + Implementation gap = 0

## 先行調査結果 (batching / 先行対応要否)

`.claude/rules/todo-prioritization.md` Step 0 Uncertainty Check:

| 項目 | 結論 | 根拠 |
|------|------|------|
| 先行する INV がないか | **無し** (全解消済) | INV-Step4-1 ✅ / INV-Step4-2 ✅ / I-153 ✅ |
| T6 と batch すべき issue | **無し** (全 batch 済) | I-142 Step 4 C-1/C-2/C-3/C-4/D-1 は I-144 吸収対象、Cell #14 も吸収対象 |
| T6 に先行する L1/L2 issue | **無し** | I-153 完了で L1 silent 該当なし、L2 foundation は I-144 自体 |
| I-050 依存 cell | **T6 scope 外** | C-2d (signature widen) / Cell #5/#9 (pure Any ??=) は I-050 umbrella に委譲、T6 で触れない |
| I-142 Step 4 C-5〜C-7 残余 | **T6 scope 外** | test quality / matrix gap の hygiene 系。完了条件に含まれず、priority 3 の別枠 |
| 新 PRD 別枠化済 cell | **確認済** | I-149 (try body narrow 崩壊) / I-161 (`&&=` 基本 emission) / I-050 (synthetic union coerce) は T1 時点で別 PRD scope に分離済、本 PRD の ✗ cell に含まれず |

**結論**: T6 単独で着手可能。先行対応を要する issue は無し。

## PRD Spec vs Implementation の scope ambiguity 解消

PRD Task section の T6 "Work" item (Step 6-1 scanner 短絡 + Step 6-2 emission 連動) は
`ShadowLet` / `GetOrInsertWith` / `E2b UnwrapOrCoerced` / `IfLetSome` の 4 種 dispatch を
明記する一方、T6 "Completion criteria" は 9 cell 全 GREEN を要求する。残り 4 cell
(I-024 / I-025 / T4d / T7) の実装 work item は Spec 明記欠落。

**対応**: 実装 phase を以下の 6 sub-phase に分割し、Spec 欠落分は sub-phase 3-5 で吸収する。
各 sub-phase 完了時に **cohesive な commit** を作成、`.claude/rules/incremental-commit.md`
準拠。

---

## Phase 構造 (6 sub-phase)

```
T6-1 ✅: Pipeline wiring + scanner retirement + ??= dispatch (3 cell GREEN)
   ↓
T6-2 ✅: coerce_default helper + E2b stale read emission (2 cell GREEN)
   ↓
T6-3 ✅: Truthy predicate E10 (primitive NaN + composite Option<Union>) (2 cell GREEN)
   ↓
T6-4 ✅: Compound OptChain narrow detection (1 cell GREEN)
   ↓
T6-5 ✅: Multi-exit Option return implicit None emission (1 cell GREEN)
   ↓
T6-6: Quality gate + regression lock-in + /check_job review + PRD close
```

各 phase は 1 commit を目安、単体で cargo test pass + clippy 0 warn で green-before-next。

---

## Phase T6-1: Pipeline wiring + scanner retirement + ??= EmissionHint dispatch

**目的**: `narrowing_analyzer::analyze_function` を pipeline に組込み、Transformer が
`EmissionHint` で `??=` dispatch する構造を確立。interim scanner を完全に retire する。

**GREEN 化する cell (3)**:
- `cell-14-narrowing-reset-structural` (Cell #14): scanner の `UnsupportedSyntaxError` を
  E2a (`x.get_or_insert_with(|| d);`) emission に置換
- `cell-c1-compound-arith-preserves-narrow` (C-1): scanner false-positive (`x += 1` を
  reset 誤判定) を分類器 `ResetCause::CompoundArith` (invalidates_narrow = false) で解消
- `cell-c2a-nullish-assign-closure-capture` (C-2a): 分類器 `ResetCause::ClosureReassign`
  検出により `EmissionHint::GetOrInsertWith` 選択 (Option 保持、closure 内 `x = null`
  許容)

### 設計 (T6-1)

#### 1. `FileTypeResolution.emission_hints` field 追加

```rust
// src/pipeline/type_resolution.rs
pub struct FileTypeResolution {
    // 既存 field...
    pub narrow_events: Vec<NarrowEvent>,
    /// `??=` site の EmissionHint。key = `stmt.span.lo.0`。
    /// T6-1 で追加。T6-2 以降で更に E2b/IfLetSome 等が populate される。
    pub emission_hints: HashMap<u32, EmissionHint>,
}

impl FileTypeResolution {
    pub fn empty() -> Self {
        Self { /* ... */, emission_hints: HashMap::new() }
    }
    /// Returns the emission hint for a `??=` site keyed by its statement start position.
    pub fn emission_hint(&self, stmt_lo: u32) -> Option<EmissionHint> {
        self.emission_hints.get(&stmt_lo).copied()
    }
}
```

#### 2. TypeResolver による `analyze_function` の呼び出し

**設計判断**: 各関数 body 入口で 1 回呼び、結果 `AnalysisResult.emission_hints` を
`self.result.emission_hints` に merge する。5 entry point:

| Entry point | 場所 | 呼び出し前提 |
|-------------|------|--------------|
| `visit_fn_decl` | `visitors.rs:125` body visit 直前 | `fn_decl.function.body: Option<BlockStmt>` |
| `visit_method_function` | `visitors.rs:494` stmts iter 直前 | `function.body: Option<BlockStmt>` |
| Constructor body | `visitors.rs:460` body stmts iter 直前 | `ctor.body: Option<BlockStmt>` |
| `resolve_arrow_expr` BlockStmt 分岐 | `fn_exprs.rs:197` stmts iter 直前 | `ast::BlockStmtOrExpr::BlockStmt(block)` |
| `resolve_fn_expr` | `fn_exprs.rs:250` body stmts iter 直前 | `fn_expr.function.body: Option<BlockStmt>` |

**DRY**: TypeResolver に helper method `collect_emission_hints(&mut self, body: &BlockStmt)`
を新設 (`type_resolver/mod.rs` or 適切な file) し、5 entry point から call。

```rust
// src/pipeline/type_resolver/mod.rs (or new emission_hints.rs)
impl<'a> TypeResolver<'a> {
    pub(super) fn collect_emission_hints(&mut self, body: &ast::BlockStmt) {
        let result = crate::pipeline::narrowing_analyzer::analyze_function(body);
        self.result.emission_hints.extend(result.emission_hints);
    }
}
```

#### 3. `try_convert_nullish_assign_stmt` の dispatch 書換

**現状** (`statements/nullish_assign.rs:165-213`):
- Ident LHS + `NullishAssignStrategy::ShadowLet` → `build_option_unwrap_with_default` で
  shadow-let emit

**T6-1 後**:
- Ident LHS + `NullishAssignStrategy::ShadowLet` →
  `self.get_emission_hint(assign.span.lo.0)` で dispatch:
  - `Some(EmissionHint::GetOrInsertWith)` → `x.get_or_insert_with(|| d);` emit
  - `Some(EmissionHint::ShadowLet)` / `None` → 既存 shadow-let emit

**新規 IR helper**: `build_option_get_or_insert_with(target, default) -> Expr`
(`transformer/mod.rs` に `build_option_unwrap_with_default` と並置)。
出力: `Expr::MethodCall { object: target, method: "get_or_insert_with",
args: [Expr::Closure { body: default, ... }] }` (stmt 位置なので返り値を捨てる =
`Stmt::Expr(...)`)。

**Transformer accessor**: `get_emission_hint(&self, stmt_lo: u32) -> Option<EmissionHint>`
を `transformer/mod.rs` に新設 (既存 `get_type_for_var` / `get_expr_type` と同じ file)。
`self.tctx.type_resolution.emission_hint(stmt_lo)` を wrap。

#### 4. Scanner call site + 関数本体の削除

**Call sites (8)** を完全削除:
- `statements/mod.rs:242`
- `statements/switch.rs:239`
- `classes/members.rs:205`, `:334`, `:382`
- `expressions/functions.rs:129`, `:291`, `:310`

**関数削除** (`statements/nullish_assign.rs`):
- `Transformer::pre_check_narrowing_reset` (line 129〜151)
- 自由関数 `has_narrowing_reset_in_stmts` (438〜440)
- `stmt_has_reset` (443〜505)
- `expr_has_reset` (未確認だが同 file 内)

**設計判断**: PRD では T6 Step 6-1 で scanner を「無効化のみ」、T7 で完全削除としているが、
無効化後の関数本体は dead code となり broken-window 化する。`incremental-commit.md` +
`ideal-implementation-primacy.md` より、T6-1 で一括削除が ideal。T7 は phase として吸収。

`extract_nullish_assign_ident_stmt` (420〜434) は `classifier.rs` に equivalent が既存
なので削除候補。call 元が scanner のみなら削除。確認後に削除。

#### 5. テスト更新 (cell14_* 4 tests)

**現状** (`expressions/tests/nullish_assign.rs:404-499`):
- `cell14_narrowing_reset_surfaces_unsupported_blocked_by_i144` — linear reset → error surface
- `cell14_narrowing_reset_detects_inner_if_block` — if-conditional reset → error
- `cell14_narrowing_reset_detects_loop_body_reassign` — for-of body reset → error
- `cell14_closure_body_reassign_does_not_surface_reset` — closure body NOT reset (scanner 境界)

**T6-1 後の assertion 方針**:

| Test 名 | 旧 assertion | 新 assertion |
|---------|--------------|--------------|
| linear reset | UnsupportedSyntaxError 含む | Rust 出力が `x.get_or_insert_with(\|\| 0` を含む + error 無し |
| if-conditional reset | UnsupportedSyntaxError 含む | `x.get_or_insert_with` 含む + error 無し |
| for-of body reset | UnsupportedSyntaxError 含む | `x.get_or_insert_with` 含む + error 無し |
| closure body reassign | shadow-let 維持 + error 無し | **意味変更**: 新分類器では closure body は境界ではない (ClosureReassign 検出) → `x.get_or_insert_with` 含む + error 無し。既存 assertion の `shadow-let` 期待を `get_or_insert_with` に書換 |

test 関数名も rename: `cell14_narrowing_reset_*_emits_get_or_insert_with` / `cell14_closure_body_reassign_emits_get_or_insert_with`。

#### 6. E2E per-cell test 分割

**現状** (`tests/e2e_test.rs:960-967`): `test_e2e_cell_i144` 1 関数が `#[ignore]` 付きで
全 14 fixture を aggregate。

**T6-1 後**: 14 個の per-cell `#[test]` 関数に分割。`run_cell_e2e_test(prd_id, cell_id)`
既存 helper (line 421) を使用。

```rust
// tests/e2e_test.rs
#[test]
fn test_e2e_cell_i144_14_narrowing_reset_structural() {
    run_cell_e2e_test("i144", "cell-14-narrowing-reset-structural");
}
#[test]
fn test_e2e_cell_i144_c1_compound_arith_preserves_narrow() {
    run_cell_e2e_test("i144", "cell-c1-compound-arith-preserves-narrow");
}
// ... 14 関数
```

**T6-1 時点で un-ignore**:
- Baseline GREEN (5): closure-no-reassign-keeps-e1 / f4-loop-body-narrow-preserves /
  null-check-narrow / r5-nullish-on-narrowed-is-noop / rc-narrow-read-contexts
- Phase T6-1 で GREEN 化 (3): cell-14 / cell-c1 / cell-c2a

**T6-1 時点で `#[ignore = "I-144 T6-N"]`** (fix phase 別指定):
- `cell-c2b`, `cell-c2c`: `#[ignore = "I-144 T6-2: E2b stale read emission"]`
- `cell-i024`: `#[ignore = "I-144 T6-3: E10 composite truthy predicate"]`
- `cell-t4d`: `#[ignore = "I-144 T6-3: E10 NaN truthy predicate"]`
- `cell-t7`: `#[ignore = "I-144 T6-4: compound OptChain narrow"]`
- `cell-i025`: `#[ignore = "I-144 T6-5: multi-exit implicit None"]`

**集約 test** `test_e2e_cell_i144` は削除 (`run_cell_e2e_tests("i144")` は他 PRD では使用継続)。

### 影響範囲 (T6-1)

| File | 変更種別 | 推定 LOC |
|------|---------|---------|
| `src/pipeline/type_resolution.rs` | `emission_hints` field + accessor + empty() 更新 | +20 |
| `src/pipeline/type_resolver/mod.rs` (or new file) | `collect_emission_hints` helper | +15 |
| `src/pipeline/type_resolver/visitors.rs` | 3 entry point (`visit_fn_decl` / `visit_method_function` / constructor) に call 追加 | +6 |
| `src/pipeline/type_resolver/fn_exprs.rs` | 2 entry point (`resolve_arrow_expr` / `resolve_fn_expr`) に call 追加 | +4 |
| `src/transformer/mod.rs` | `get_emission_hint` accessor + `build_option_get_or_insert_with` helper | +30 |
| `src/transformer/statements/nullish_assign.rs` | `try_convert_nullish_assign_stmt` dispatch 書換 + scanner 関数削除 | -200 / +30 |
| `src/transformer/statements/mod.rs` | scanner call site 削除 | -5 |
| `src/transformer/statements/switch.rs` | scanner call site 削除 | -2 |
| `src/transformer/classes/members.rs` | scanner call site 削除 (3 箇所) | -6 |
| `src/transformer/expressions/functions.rs` | scanner call site 削除 (3 箇所) | -6 |
| `src/transformer/expressions/tests/nullish_assign.rs` | cell14_* 4 tests 書換 | -60 / +80 |
| `tests/e2e_test.rs` | per-cell 14 test 関数 + ignore reason 配分 | +50 / -10 |

**推定 total**: ±500 LOC

### 完了条件 (T6-1)

- [ ] `cargo test --lib` pass (全 narrow/nullish_assign 関連 regression 0)
- [ ] `cargo test --test e2e_test` で un-ignore した 8 cell (5 baseline + 3 Phase-1) PASS
- [ ] `cargo test --test compile_test` pass
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 warn
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `scripts/hono-bench.sh` 非後退 (clean 112/158, errors 62 維持)
- [ ] `pre_check_narrowing_reset` / `has_narrowing_reset_in_stmts` / `stmt_has_reset` /
      `expr_has_reset` が grep で 0 hits (source + test)
- [ ] `test_e2e_cell_i144` 集約関数が存在しない (grep 0 hits)
- [ ] plan.md 「進行中作業」を T6-1 完了に更新

### Commit message (draft)

```
I-144 T6-1 完了: analyzer pipeline + ??= EmissionHint dispatch + scanner 完全削除 (3 cell GREEN)

- FileTypeResolution に emission_hints field 追加、TypeResolver が
  narrowing_analyzer::analyze_function を 5 entry point (fn decl / method / ctor /
  arrow / fn expr) で call して populate
- try_convert_nullish_assign_stmt を EmissionHint dispatch に書換:
  ShadowLet → 既存 shadow-let / GetOrInsertWith → x.get_or_insert_with(|| d)
- Interim scanner 完全削除: pre_check_narrowing_reset + has_narrowing_reset_in_stmts
  + stmt_has_reset + expr_has_reset + 8 call site (statements/mod.rs, switch.rs,
  classes/members.rs × 3, expressions/functions.rs × 3)
- cell14_* 4 tests を error-surface assertion → structural emission assertion に書換
- test_e2e_cell_i144 集約関数を per-cell 14 関数に分割、un-ignore 8 cell
  (5 baseline GREEN + 3 T6-1 GREEN: Cell #14 / C-1 / C-2a)
- cell-c2b/c2c/i024/i025/t4d/t7 は phase 別 #[ignore] reason で後続 phase 明示
```

---

## Phase T6-2: coerce_default helper + E2b stale read emission

**目的**: Closure が narrow された outer var を reassign するケース (C-2b/c) で、
post-closure-read 時に `x.unwrap_or(coerce_default(T))` を emit する仕組みを構築。

**GREEN 化する cell (2)**:
- `cell-c2b-closure-reassign-arith-read` (C-2b): 早期 null-check narrow 後、closure 内
  `x = null` reassign → post-closure で `x + 1` を読む。ideal: `x.unwrap_or(0.0) + 1.0`
- `cell-c2c-closure-reassign-string-concat` (C-2c): 同構造、read が `"v=" + x`。
  ideal: `"v=".to_string() + &x.map(\|v\| v.to_string()).unwrap_or_else(\|\| "null".to_string())`

### 設計 (T6-2)

#### 1. `src/transformer/helpers/` 新設

現状 `src/transformer/statements/helpers.rs` は存在するが、coerce_default は statement
context に閉じず使用されるため、transformer 全体で共有する `helpers/` directory を新設。

```
src/transformer/helpers/
├── mod.rs              # re-export
└── coerce_default.rs   # JS coerce_default table 実装
```

`src/transformer/mod.rs` の module tree に `pub(crate) mod helpers;` を追加。

#### 2. `coerce_default` の signature + semantics

JS coerce_default は `(RustType, RcContext) → IR Expr` の 2 引数関数。PRD `v2 JS coerce_default
table` (Semantic Safety Analysis section) に従う:

| RC | LHS type | null coerce | undefined coerce |
|----|---------|-------------|------------------|
| RC1 arithmetic `+-*/%` | `F64` | `0.0` | `f64::NAN` |
| RC1 arithmetic | Primitive(int) | `0 as i*` | N/A |
| RC1 comparison `===` | `T` | 型別 sentinel | 同上 |
| RC4 truthy | `F64` / `String` / `Bool` | `false` | `false` |
| RC6 String concat `+` | `String` | `"null".to_string()` | `"undefined".to_string()` |
| RC6 Template interp | `String` | `"null".to_string()` | `"undefined".to_string()` |

**API draft**:
```rust
// src/transformer/helpers/coerce_default.rs
pub(crate) fn coerce_default(inner_ty: &RustType, rc: RcContext) -> Expr { /* ... */ }
```

**初期実装 scope (T6-2)**: RC1 arithmetic F64 + RC6 StringInterp の 2 RC のみ (C-2b/c
カバーに必要)。他 RC は phase 3-5 で逐次追加。YAGNI 準拠。

#### 3. analyzer の narrow stale 検出強化

現状 `narrowing_analyzer` は `??=` site 限定の `EmissionHint` のみ出力。C-2b/c は
**narrow scope 内の read site** (not ??=) で stale 判定が必要。

**設計判断 (重要)**: 2 候補

- **Option A**: read site に per-site EmissionHint を追加。analyzer が narrow scope ×
  closure reassign 検出で read position に `EmissionHint::UnwrapOrCoerced` を map。
- **Option B**: TypeResolver が narrow event の scope 内で closure reassign 検出時、
  narrow 後半を stale-subscope として「narrow 発火から closure call まで」で切断、
  post-closure を non-narrow にする。Transformer は naturally Option 型を見るので
  読み site で unwrap_or を emit (既存の Option → T coerce 経路を expected_type 経由
  で活用)。

**推奨**: Option B (TypeResolver scope 調整)。理由:
- narrow stale は本質的に **scope 問題**、read site 単位の hint は事後対応で設計が歪む
- TypeResolver 経由で narrow scope を調整すれば、downstream (Transformer) は既存の
  Option → T coerce 機構 (RC1 arithmetic で `x + 1` は `x.unwrap_or(0) + 1` を emit
  できれば) で自動対応できる可能性。ただし既存の unwrap_or emit は limited coverage

**T6-2 での検証 phase**: 
- 先に per-cell tsc で ideal 出力を再確認
- 既存 Transformer の Option → F64 coerce 経路が RC1 arithmetic で `unwrap_or(0)` を
  emit するか調査 (probe)
- 経路不足なら expected_type + coerce_default 明示呼び出し経路を追加

**DRY check**: C-2b/c のケース以外に `.unwrap_or` を call arg / return expr で emit
する既存経路 (I-142 で整備) があるため、同経路に合流するのが望ましい。

#### 4. 分類器拡張 (narrowing_analyzer/classifier.rs)

現状 `classify_reset_in_stmts` は stmt list に対する reset 分類。**narrow read sites
の enumerate** を担う新 API が必要:

```rust
// src/pipeline/narrowing_analyzer/classifier.rs (extension)
pub(super) fn classify_closure_capture_in_scope(
    stmts: &[ast::Stmt],
    narrow_var: &str,
) -> Option<Span /* closure span */>;
```

Closure reassign が narrow scope 内で検出されたとき、narrow scope を closure span 直前で
切断する情報を返す。TypeResolver が narrow event の `scope_end` を closure span 直前に
調整するか、別 event `NarrowEvent::ClosureCapture` を追加で発行して Transformer 側で
処理する。

**現状 events.rs** には既に `NarrowEvent::ClosureCapture { var_name, closure_span,
outer_narrow }` variant が定義済。**T6-2 でこの variant を initial populate** する。

### 影響範囲 (T6-2)

| File | 変更種別 | 推定 LOC |
|------|---------|---------|
| `src/transformer/helpers/mod.rs` | 新規 (module re-export) | +10 |
| `src/transformer/helpers/coerce_default.rs` | 新規 (coerce_default + unit tests) | +150 |
| `src/transformer/mod.rs` | helpers module 登録 | +2 |
| `src/pipeline/narrowing_analyzer/classifier.rs` | closure capture detection 拡張 | +50 |
| `src/pipeline/narrowing_analyzer/guards.rs` (または新 module) | NarrowEvent::ClosureCapture 発行 | +30 |
| `src/pipeline/type_resolver/visitors.rs` (or narrow_context.rs) | narrow scope 切断 or event populate | +20 |
| `src/transformer/expressions/*` (read sites) | coerce_default 呼び出し経路 | +40 |
| `tests/e2e_test.rs` | cell-c2b/c2c un-ignore | -4 |
| 新 unit test | coerce_default per (RustType, RcContext) 網羅 | +100 |

**推定 total**: ±400 LOC

### 完了条件 (T6-2)

- [ ] cell-c2b / cell-c2c E2E PASS
- [ ] `coerce_default` unit test が **T6-2 に必要な (RustType, RcContext) 全 cell**
      (RC1 F64, RC6 String) を網羅 + future RC の placeholder/TODO を持たない (YAGNI)
- [ ] regression 0 (T6-1 の 8 cell + baseline 全 pass)
- [ ] clippy 0 / fmt 0 / Hono bench 非後退
- [ ] plan.md 更新

### 調査タスク (T6-2 先行)

T6-2 着手時に以下を probe (実装着手前):
- **probe-1**: 現状 `let x: Option<f64>` に対する `x + 1` の Rust emission が何を
  出すか (E0277 か、expected_type 経路で unwrap_or emit か)。
- **probe-2**: 既存 expected_type 経路での coerce 実装箇所の特定 (expressions/binary.rs
  or similar)。coerce_default helper をどこから呼ぶかの接点確定。
- **probe-3**: narrow scope 切断の設計選択: NarrowEvent::ClosureCapture 発行 vs
  narrow event scope_end 調整。empirical test で semantic 一致確認。

---

## Phase T6-3: Truthy predicate E10 (primitive NaN + composite Option<Union>) ✅ 完了 (2026-04-21)

**完了サマリ** (詳細は backlog/I-144 T6-3 section 参照):
- 実装: `helpers/truthy.rs` 新設 + `try_generate_primitive_truthy_condition` + `try_generate_option_truthy_complement_match` (Primitive/Union/non-primitive always-truthy arm) + `wrap_in_synthetic_union_variant` (call-arg Literal coercion) + `return_wrap::wrap_leaf` priority 0 guard + `ir_body_always_exits` Match 対応
- 対応: `/check_job` × 2 round + `/check_problem` で 15 defect 全 structural 対応 (H-1〜M-5 + R2-C1〜R2-I3)
- PRD 更新: Sub-matrix T4a/T4c/T4d/T4e を ✓ T6-3 に反映 + Spec Revision Log 追加 (matches! → consolidated match 決定記録)
- テスト: lib +19 test (wrap_in_synthetic_union_variant 11 / truthy exhaustive + primitive int falsy / return_wrap priority 0 × 2 / ir_body_always_exits / H-3 integration)、E2E +2 (cell-regression-t4c / t4e) + 2 un-ignore (cell-t4d / cell-i024)
- 周辺 defect: I-050-c 拡充 (non-literal Union coercion) + I-171 新規起票 (`if (!x)` 全経路対応) で track

**旧原版**: 以下は Spec stage の着手前計画。完了後の実装採用形は backlog/I-144 T6-3 section。

**目的**: `if (x)` / `if (!x)` の truthy predicate emission を E10 対応 (primitive NaN
+ composite Option<Union>)。

**GREEN 化した cell (2)**:
- `cell-t4d-truthy-number-nan` (T4d): `if (x)` on `number` → 現状 `x != 0.0` のみ、
  ideal `x != 0.0 && !x.is_nan()`
- `cell-i024-truthy-option-complex` (I-024): `if (!x) return "none"` on
  `string | number | null` → early-return complement narrow + composite truthy 述語
  (E10) — 実装は consolidated match emission (`let x = match x { Some(V(v)) if
  <v truthy> => V(v), _ => return "none".to_string() }`) で truthy check + Option
  unwrap + Union narrow materialization を 1 match に集約

### 設計 (T6-3)

#### 1. 現状調査 phase (T6-3 冒頭)

- **probe-4**: 現状の truthy emission 箇所 (if_stmt.test が `Ident` で type が `F64` /
  `Option<T>` / `Union<T,U>` / `Option<Union<T,U>>` のとき何を emit するか) を
  probe。`src/transformer/expressions/*` 内。
- **probe-5**: composite Option<Union> 型の variant 列挙 API (`synthetic_registry` 内の
  Union 定義 lookup) 確認。
- **probe-6**: `if (!x) return;` の current emission と narrow scope 処理を確認
  (existing early-return complement との整合)。

#### 2. 実装設計

**primitive NaN (T4d)**:
- `emit_truthy_predicate(expr: Expr, ty: &RustType) -> Expr` を新 helper 化
  (推定 `src/transformer/helpers/truthy.rs`)。
- `RustType::F64` → `x != 0.0 && !x.is_nan()` (BinOp 合成)
- `RustType::Primitive(IntKind)` → `x != 0` 相当
- `RustType::String` → `!x.is_empty()` (既存)
- `RustType::Bool` → identity (既存)
- `RustType::Option(inner)` → composite via 下記
- `RustType::Named { name }` で synthetic Union variant → variant 毎に inner predicate
  合成 (`matches!(x, Union::V1(v) if <inner truthy>) || ...`)
- `RustType::Option(Union)` → Some(...) + inner variant 別 truthy

**composite Option<Union> (I-024)**:
- `Option<Union<T, U>>` の truthy は JS semantics で:
  - `None` → false
  - `Some(T_instance)` → T instance の truthy (e.g. string 非空 / f64 非 0 非 NaN)
- emission: `matches!(&x, Some(Union::T(v)) if <T truthy>) || matches!(&x, Some(Union::U(v)) if <U truthy>)`

**early-return complement の調整**:
- 現状 `detect_early_return_narrowing` が `if (!x) return;` を truthy complement として
  narrow を発火しているか確認 (probe-4/6)。発火していない場合は guards.rs に拡張を追加。
- 発火している場合: narrow scope 後続での x 使用が Option unwrap (Some(...)) を前提
  に構成されるか確認、必要なら emission 調整。

### 影響範囲 (T6-3)

| File | 変更種別 | 推定 LOC |
|------|---------|---------|
| `src/transformer/helpers/truthy.rs` | 新規 (predicate emission) | +150 |
| `src/transformer/expressions/*` (truthy 呼出点) | 既存 truthy emission を helper 経由に統一 | ±80 |
| `src/pipeline/narrowing_analyzer/guards.rs` | complement narrow 拡張 (必要なら) | +30 |
| Unit test (truthy per RustType) | `helpers/truthy.rs` 内 | +120 |
| `tests/e2e_test.rs` | cell-t4d / cell-i024 un-ignore | -4 |

**推定 total**: ±400 LOC

### 完了条件 (T6-3) ✅ 全達成

- [x] cell-t4d / cell-i024 E2E PASS
- [x] truthy predicate unit test が全 RustType variant × truthy cell を網羅 (primitive
      positive 値 assert + non-primitive exhaustive None 返し)
- [x] regression 0 (T6-1/T6-2 cell + baseline 全 pass、i144 cell 13 pass)
- [x] clippy 0 / fmt 0 / Hono bench 非後退 (clean 112/158, errors 62)
- [x] review 2 round + check_problem で 15 defect structural 対応、ad-hoc patch 0 件

---

## Phase T6-4: Compound OptChain narrow (`x?.v !== undefined` → x non-null) ✅ 完了 (2026-04-21)

**完了サマリ** (詳細は backlog/I-144 T6-4 section 参照):
- 実装: `narrowing_patterns.rs` に `extract_optchain_base_ident` (DRY 共有ヘルパー)、`guards.rs` に `extract_optchain_null_check_narrowing` + `extract_non_nullish_side` / `unwrap_option_type` (DRY helper 抽出)。`detect_narrowing_guard` + `detect_early_return_narrowing` に OptChain パス追加。`transformer/expressions/patterns.rs::extract_narrowing_guard` に OptChain LHS 対応追加。`PrimaryTrigger::OptChainInvariant` doc 更新。
- テスト: unit +22 (narrowing_patterns 6 / guards 11 / patterns 6)、E2E cell-t7 GREEN + #[ignore] 解除
- Hono bench: clean 113/158 (+1)、errors 60 (-2)
- `/check_job` deep review: H-1〜H-5 (doc comment / DRY / dead code / PRD drift) + M-1〜M-2 全修正、Spec gap = 0 / Implementation gap = 0
- `/check_problem`: bench OBJECT_LITERAL_NO_TYPE +1 は改善副産物 (net -2)、PRD Completion Criteria item 7/12 を ⏳ に修正 (I-025 pending T6-5)

**旧原版**: 以下は着手前計画。完了後の実装採用形は backlog/I-144 T6-4 section。

**目的**: `x?.prop !== undefined` pattern を narrow trigger として検出し、x を non-null に
narrow。guards.rs 拡張のみで emission 変更はほぼ不要の想定。

**GREEN 化した cell (1)**:
- `cell-t7-optchain-compound-narrow` (T7)

### 設計 (T6-4)

現状 `guards.rs::extract_null_check_narrowing` は直接 ident の null/undefined check を
検出。OptChain (`x?.prop`) の non-null check は未対応。

**拡張**: `extract_null_check_narrowing` に OptChain LHS 対応を追加:
- `BinExpr(NotEqEqUndefined, OptChainExpr { base: x, chain: .prop }, Undefined)` を
  検出 → narrow target を OptChain の **base** (x) として非 null narrow 発行

### 影響範囲 (T6-4)

| File | 変更種別 | 推定 LOC |
|------|---------|---------|
| `src/pipeline/narrowing_analyzer/guards.rs` | OptChain null check 対応 | +50 |
| Unit test | narrowing_analyzer/tests/guards.rs (既存) | +40 |
| `tests/e2e_test.rs` | cell-t7 un-ignore | -2 |

**推定 total**: ±100 LOC

### 完了条件 (T6-4) ✅ 全達成

- [x] cell-t7 E2E PASS
- [x] guards.rs OptChain null check unit test 追加 (10 tests: neq/eq/reversed/non-option/deep-chain/null-rhs/precedence/early-return/compound-and)
- [x] narrowing_patterns.rs extract_optchain_base_ident unit test 追加 (6 tests)
- [x] patterns.rs extract_narrowing_guard OptChain test 追加 (6 tests)
- [x] regression 0 (lib 2877 / integration 122 / compile 3 / E2E 113)
- [x] clippy 0 / fmt 0
- [x] Hono bench 改善 (113/158 clean +1, errors 60 -2)

---

## Phase T6-5: Multi-exit Option return implicit None emission (I-025 complex) ✅ 完了 (2026-04-21)

**完了サマリ**: `append_implicit_none_if_needed` をパターンマッチ heuristic (if-no-else / while / for の 4 variant 限定) から `ir_body_always_exits` + `TailExpr` 判定に構造的書き換え。`ir_body_always_exits` を `pub(crate)` に昇格 (DRY: functions/ からも使用可能に)。unit test +9。cell-i025 GREEN → I-144 全 9 matrix ✗ cell GREEN 達成。

**旧原版**: 以下は着手前計画。

**目的**: 複数 exit path を持つ Option 返り値関数で、全 fall-off path に implicit `None`
を tail injection。

**GREEN 化した cell (1)**:
- `cell-i025-option-return-implicit-none-complex` (I-025)

### 設計 (T6-5)

現状 `transformer` は simple case (tail fall-off) の implicit None は emit できる想定
(要 probe-7)。複数 exit の場合、分岐毎の fall-off を認識して tail injection が必要。

**probe-7 (T6-5 冒頭)**: 現状の Option return emit 処理箇所の特定。CFG reachability 的な
tail 認識がどこで行われているか。

**実装設計 (probe-7 後確定)**: 想定では `convert_stmt_list` の末尾処理か `convert_function`
で return type が Option のとき、最終 tail が fall-off なら `Stmt::Expr(Expr::None)` を
inject。複数 branch の場合、各 branch の末尾で同処理が必要。

### 影響範囲 (T6-5)

| File | 変更種別 | 推定 LOC |
|------|---------|---------|
| `src/transformer/statements/*` (return 処理) | tail injection 拡張 | +80 |
| Unit test / snapshot test | regression lock-in | +60 |
| `tests/e2e_test.rs` | cell-i025 un-ignore | -2 |

**推定 total**: ±150 LOC

### 完了条件 (T6-5) ✅ 全達成

- [x] cell-i025 E2E PASS
- [x] regression 0 (lib 2887 / integration 122 / compile 3 / E2E 114 + 0 ignored)
- [x] clippy 0 / fmt 0
- [x] Hono bench 非後退 (113/158, 60 errors, 変動 0)

---

## Phase T6-6: Quality gate + regression lock-in + /check_job review + PRD close

**目的**: PRD 完了条件 (全 9 cell GREEN + 既存 12 完了条件) の最終 verify。PRD close
処理。

### Work items (T6-6)

1. **T8 regression lock-in (PRD spec 由来)**:
   - I-024 / I-025 / C-1 / C-2 / Cell #14 snapshot test 追加 (`tests/fixtures/`)
   - `functions` compile_test fixture の narrow 関連部分 verify、unskip 可能性確認
   - 吸収対象 (I-024 / I-025 / I-142 Cell #14 / C-1 / C-2a-c / C-3 / C-4 / D-1) の
     TODO/plan.md entry 削除

2. **T9 Quality gate**:
   - `cargo test` 全 pass
   - `cargo clippy --all-targets --all-features -- -D warnings` 0 warn
   - `cargo fmt --all --check` 0 diff
   - `./scripts/hono-bench.sh` 実測、非後退 (errors 62 以下)

3. **T10 /check_job Implementation Stage review**:
   - Defect 分類 (Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight)
   - Spec gap = 0 + Implementation gap = 0 目標
   - 発見 defect は本 phase 内 fix または別 TODO 化

4. **PRD close**:
   - `backlog/I-144-control-flow-narrowing-analyzer.md` に完了記録、削除
   - plan.md の「進行中作業」を完了済に移行、「次の作業」table から priority 1 を
     priority 2 に昇格
   - 設計判断の引継ぎ事項を `doc/handoff/design-decisions.md` に追記
     (CFG analyzer / NarrowTypeContext trait / EmissionHint dispatch / coerce_default
     table / closure reassign Policy A の設計理由等)

### 完了条件 (T6-6 = T6 全体 = I-144 PRD 全体)

PRD `Completion Criteria` 全 13 項目を充足:

- [ ] ✅ Spec Stage checklist 5 項目全 [x]、`/check_job` Spec Stage defect 0 (既達)
- [ ] ✅ `pipeline/narrowing_analyzer.rs` 実装完了、Unit test 全 pass (既達)
- [ ] ✅ 既存 `type_resolver/narrowing.rs` が CFG analyzer 経由に統合、regression 0 (既達 T5)
- [ ] Transformer emission が CFG analyzer + RC context 連動、E1/E2a/E2b/E3/E5/E6/E7/E8/E9/E10 等選択可能
- [ ] `coerce_default` helper が JS coerce table 準拠、unit test 全 RustType × 関連 RC
- [ ] Interim scanner 廃止 (T6-1)
- [ ] Matrix ✗ cell 全 9 E2E GREEN
- [ ] Matrix ✓ cell regression 0
- [ ] `cargo test` 全 pass
- [ ] `cargo clippy` 0 warn / `cargo fmt` 0 diff
- [ ] Hono bench 非後退
- [ ] 吸収対象の TODO/plan.md entry 削除
- [ ] `/check_job` Implementation Stage で Spec gap = 0 + Implementation gap = 0

---

## Risk / Known Unknowns

| Risk | Phase | 影響 | 対応 |
|------|-------|------|------|
| T6-2 Option B (TypeResolver scope 切断) で既存 narrow 依存 code に regression | T6-2 | 中 | probe-1/2/3 で既存経路を先行確認、切断点を empirical verify |
| T6-3 composite Option<Union> truthy で synthetic union variant lookup が必要 | T6-3 | 中 | synthetic_registry API の事前確認 (probe-5) |
| T6-5 multi-exit implicit None の emit 箇所が `convert_function` 外で複雑 | T6-5 | 中 | probe-7 で tail 認識箇所の事前特定 |
| T6 の scope 不明瞭 (PRD Work vs Completion) | 全体 | 低 | 本 plan で 6 phase に分割、PRD 更新を T6-6 で同時実施 |
| Hono bench で narrow 精度向上による silent improvement/regression | 全 phase | 低 | 各 phase 末の bench 比較、category 別分析 |

## 先行 probe 項目まとめ

各 phase 着手前に実施すべき probe:

| Probe | Phase | 目的 |
|-------|-------|------|
| probe-1 | T6-2 | Option<f64> + f64 演算の現状 Rust emission 確認 |
| probe-2 | T6-2 | 既存 expected_type → coerce 経路の実装箇所特定 |
| probe-3 | T6-2 | narrow scope 切断の設計選択 (NarrowEvent::ClosureCapture vs scope_end 調整) |
| probe-4 | T6-3 | 現状 truthy emission の場所 + behavior |
| probe-5 | T6-3 | synthetic Union variant 列挙 API 確認 |
| probe-6 | T6-3 | early-return complement narrow の現状動作 |
| probe-7 | T6-5 | Option return tail injection の現状実装箇所 |

---

## Summary Table: phase 毎の cell / 主要 file / 推定 LOC

| Phase | GREEN cell | 主要 file | 推定 LOC |
|-------|-----------|----------|---------|
| T6-1 | cell-14, c1, c2a | `pipeline/type_resolution.rs` / `type_resolver/visitors.rs` / `transformer/statements/nullish_assign.rs` + scanner call sites | ±500 |
| T6-2 | c2b, c2c | `transformer/helpers/coerce_default.rs` (新) / `narrowing_analyzer/classifier.rs` | ±400 |
| T6-3 | t4d, i024 | `transformer/helpers/truthy.rs` (新) / `narrowing_analyzer/guards.rs` | ±400 |
| T6-4 | t7 | `narrowing_analyzer/guards.rs` | ±100 |
| T6-5 | i025 | `transformer/statements/*` (return 周辺) | ±150 |
| T6-6 | — (quality) | snapshot/E2E/regression tests + plan.md | ±200 |

**Total 推定**: ±1750 LOC (production + test)

---

## 参照

- PRD: `backlog/I-144-control-flow-narrowing-analyzer.md`
- SDCDF rule: `.claude/rules/spec-first-prd.md`
- Problem Space rule: `.claude/rules/problem-space-analysis.md`
- Ideal implementation rule: `.claude/rules/ideal-implementation-primacy.md`
- Incremental commit rule: `.claude/rules/incremental-commit.md`
- T1 E2E red-state report: `report/i144-t1-red-state.md`
- I-142 Step 4 handoff: `doc/handoff/I-142-step4-followup.md`
- Design decisions archive: `doc/handoff/design-decisions.md`
