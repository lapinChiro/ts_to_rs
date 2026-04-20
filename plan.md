# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-20)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 112/158 (70.9%) |
| Hono bench errors | 62 |
| cargo test (lib) | 2787 pass |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 97 pass + 14 i144 fixtures (`#[ignore]`、9 RED ✗ + 5 GREEN ✓ regression lock-in) |
| clippy | 0 warnings |
| fmt | 0 diffs |

### 進行中作業

- **I-144 Implementation stage 進行中** (T3/T4/T5 完了、2026-04-20): `backlog/I-144-control-flow-narrowing-analyzer.md`
  - **T3 (NarrowingAnalyzer 基盤実装) ✅**: `src/pipeline/narrowing_analyzer/` 新設
    (events.rs 360 + classifier.rs 908 + mod.rs 227 + tests 2253 行、計 3748 行)。
    scope-aware classifier (VarDecl L-to-R / closure param / block decl shadow) +
    branch merge (`merge_branches`) / sequential merge (`merge_sequential`) combinators +
    peel-aware wrapper handling (Paren + 6 TS wrappers) + unreachable stmt pruning
    (`stmt_always_exits`) + closure/fn/class/object-method descent (outer ident → `ClosureReassign`)
  - **T4 (NarrowingEvent → NarrowEvent enum migration) ✅**:
    `NarrowingEvent` struct 廃止、`NarrowEvent::{Narrow, Reset, ClosureCapture}` enum に migrate、
    `FileTypeResolution::narrow_events` rename、`NarrowEventRef` borrowed view + `as_narrow()` /
    `var_name()` accessor、`PrimaryTrigger` + `NarrowTrigger` 2-layer 型で nested
    `EarlyReturnComplement` を構造的排除。全 consumer (`type_resolver/narrowing.rs`, `visitors.rs`,
    Transformer 各所) を borrowed view 経由に統一
  - **T5 (narrowing.rs → CFG analyzer 統合) ✅** (2026-04-20):
    `type_resolver/narrowing.rs` (524 行) を削除、narrow guard 検出
    (typeof / instanceof / null check / truthy + early-return complement) を
    `src/pipeline/narrowing_analyzer/guards.rs` に single source of truth として集約。
    `NarrowTypeContext` trait 新設 (`type_context.rs`) で registry access (lookup_var /
    synthetic_enum_variants / register_sub_union) + event push を抽象化、
    `TypeResolver` が trait 実装 (`narrow_context.rs`)。visitor は
    `narrowing_analyzer::detect_narrowing_guard` / `detect_early_return_narrowing`
    free fn を直接呼出し。trait boundary 専用 unit test 19 件 (typeof 4 / null 3 /
    truthy 2 / instanceof 1 / compound 1 / unresolved 1 / NullCheckKind decision
    table 6 / typeof-object synthetic enum 1 / early-return 4) を
    `narrowing_analyzer/tests/guards.rs` に追加 (MockNarrowTypeContext)。既存
    `type_resolver/tests/narrowing/` 統合 test は無変更 pass (regression 0)。
    dead code 除去: T3/T4 で残された `NarrowingAnalyzer` struct / `var_types` / `new()` /
    `Default` / `AnalysisResult.events` を削除、`??=` 分析を free function
    (`analyze_function` / `analyze_stmt_list` / ...) に統一 (guards.rs の free fn style と整合)。
    Hono bench non-regression empirical 確認 (clean 112/158 変動なし、errors 62 変動なし)
  - **DRY**: `block_always_exits` (type_resolver/narrowing.rs) を削除し `stmt_always_exits`
    (narrowing_patterns.rs) を single source of truth に統合。narrowing_patterns.rs に
    共通 peel 関数 + 22 unit test 集約
  - **Test split (cohesion 基軸)**: narrowing_analyzer/tests/ を
    types_and_combinators / hints_flat / hints_nested / scope_and_exprs / closures /
    **guards** の 6 file に責務別分割、type_resolver/tests/narrowing/ を
    legacy_events / trigger_completeness の 2 file に分割
  - **Review rounds**: `/check_job` × 4 round (deep / deep deep × 3) + `/check_problem`
    で計 42 defect 発見 → 全解消 (構造 bug / spec gap / 情報精度 / code hygiene / DRY /
    ファイル分割 / doc intra-doc link)
  - 次 action: **T6 interim scanner 短絡 + Transformer emission 連動** (Phase 3、
    `??=` EmissionHint 消費、`coerce_default` helper 新設、C-2a/b/c empirical re-green 化)

### 直近の完了作業

完了 PRD は「1〜3 行サマリ + PRD ファイル/git history への link」で記載。実装詳細と設計判断の
引継ぎは以下の専用ドキュメントで管理:

- **設計判断アーカイブ**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
  (label hygiene / optional param / switch emission 等の convention/idiom)
- **PRD history**: `backlog/` (close 後 git history に archive) + git log

| PRD | 日付 | サマリ |
|-----|------|--------|
| **I-144 T5 (既存 narrowing.rs → CFG analyzer 統合)** | 2026-04-20 | `type_resolver/narrowing.rs` (524 行) 削除、narrow guard 検出 (typeof / instanceof / null check / truthy + early-return complement) を `narrowing_analyzer/guards.rs` に集約。`NarrowTypeContext` trait (`type_context.rs`) で registry access を抽象化、`TypeResolver` が impl (`narrow_context.rs`)。visitor は `narrowing_analyzer::detect_narrowing_guard` / `detect_early_return_narrowing` free fn を直接呼出し。trait boundary 専用 unit test 19 件を `narrowing_analyzer/tests/guards.rs` に追加 (MockNarrowTypeContext、NullCheckKind decision table + typeof 反転 dispatch + early-return typeof/instanceof 含む)。dead code 除去: T3/T4 残置の `NarrowingAnalyzer` struct / `var_types` / `new()` / `AnalysisResult.events` を削除、`??=` 分析を free fn に統一。Hono bench non-regression (clean 112/158、errors 62 不変)、lib 2787 pass (+16 from 2771)、integration/compile/E2E 全 pass、clippy 0 / fmt 0 diff。次: T6 (emission 連動) |
| **I-144 T3 + T4 (CFG narrowing analyzer 基盤 + NarrowEvent migration)** | 2026-04-19 | `pipeline/narrowing_analyzer/` 新設 (events.rs / classifier.rs / mod.rs + 5 分割 test file、計 3748 行)。scope-aware classifier (VarDecl L-to-R / closure param / block decl shadow) + branch/sequential merge combinator + peel-aware wrapper + unreachable prune + closure descent。`NarrowingEvent` struct を `NarrowEvent` enum (`Narrow`/`Reset`/`ClosureCapture`) に migrate + `NarrowEventRef` borrowed view + `PrimaryTrigger`/`NarrowTrigger` 2-layer 型。`block_always_exits` 削除 → `stmt_always_exits` (narrowing_patterns.rs) を single source of truth 化。4 round `/check_job` (deep/deep deep × 3) + `/check_problem` で 42 defect 発見 → 全解消。test: lib 2771 pass (+179 from 2592)、clippy 0 / fmt 0 diff。次: T5 (narrowing.rs 統合) |
| **I-145 / I-142 Step 4 C-8 / I-150 batch (pre-I-144 cleanup)** | 2026-04-19 | (1) `tests/compile-check/src/lib.rs` を `.gitignore` 追加して artifact tracking 解消、(2) TODO I-048 entry に Cell #10 `.clone()` INTERIM の removal criterion 追加、(3) `resolve_new_expr` 未登録 class else branch に args visit loop 追加 (`resolve_call_expr` と symmetric 化)。unit test +1、integration fixture +1。`keyword-types` snapshot 更新 (副産物: `"..." + x` concat → `format!("{}{}", ..., x)` emission 改善)。詳細は各 PRD が git history で archive |
| **I-153 + I-154 batch** | 2026-04-19 | switch case body 内 nested bare `break` silent redirect の structural 解消 + 4 internal label (`switch/try_block/do_while/do_while_loop`) を `__ts_` prefix に統一 + 3-entry lint + A-fix (`ast::Stmt::Block` support)。empirical verify: TSX stdout `50/550/55` = Rust stdout `50/550/55` (pre-fix Rust=`0/550/55`)。追加 test: walker unit 19 / block 3 / lint 4 / per-cell E2E i153 13 + i154 3。report: [`report/i153-switch-nested-break-empirical.md`](report/i153-switch-nested-break-empirical.md) |
| **INV-Step4-1** | 2026-04-19 | I-142 Step 4 C-2 empirical 再分類: rustc E0308 検知の L3 Tier 2 compile error と確認 (L1 silent ではない)。I-144 CFG narrowing で structural 解消予定。report: [`report/i142-step4-inv1-closure-compile.md`](report/i142-step4-inv1-closure-compile.md) |
| **Phase A Step 4: I-023 + I-021** | 2026-04-17 | `async-await` / `discriminated-union` fixture unskip。try body `!`-typed detection + DU walker single source of truth 化 + scope-aware shadowing。Follow-up I-149〜I-157 を TODO に登録 |
| **I-SDCDF (Spec-Driven Conversion Dev Framework)** | 2026-04-17 | implementation-first → specification-first への process 転換。Beta 昇格、全 matrix-driven PRD に必須適用。rule: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md)、reference: `doc/grammar/` |
| **I-050-a: primitive Lit → Value coercion** | 2026-04-17 | SDCDF Pilot、Spec gap = 0 達成 |
| **I-142: `??=` NullishAssign Ident LHS** | 2026-04-15 | shadow-let + fusion + `get_or_insert_with`。Step 4 follow-up は [`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md) |
| **Phase A Step 3: Box wrap + implicit None** | 2026-04-17 | I-020 部分解消 + I-025 解消、`void-type` unskip |
| **I-142-b+c: FieldAccess/Index `??=`** | 2026-04-17 | `if is_none/get_or_insert_with` + HashMap `entry/or_insert_with` emission |
| **以前の完了** | — | I-022 (`??`), I-138 (Vec index Option), I-040 (optional param), I-392 (callable interface) |

### 次の作業 (empirical 再評価 2026-04-19、spec-first workflow 適用)

**優先順位は `.claude/rules/todo-prioritization.md` (L1 > L2 > L3 > L4) および
`.claude/rules/ideal-implementation-primacy.md` (silent semantic change を最優先) に従う。**

**Tier 0 (L1 silent) 該当なし** (I-153 完了により解消)。次の最優先は L2 structural foundation の I-144。

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| 1 | **L2 Struct** | **I-144** umbrella (Spec v2.2 approved、**T3/T4/T5 完了** 2026-04-20、T6 着手可能) | control-flow narrowing analyzer (I-024 / I-025 / I-142 Cell #14 / C-1 / C-2a-c / C-3 / C-4 / D-1 吸収) | T5 narrow guard 検出を narrowing_analyzer/guards.rs に集約、NarrowTypeContext trait 経由で single source of truth 確立。次 T6 で Transformer emission 連動 (EmissionHint 消費 + coerce_default helper)、T7 で interim scanner 廃止 |
| 2 | L3 | **Phase A Step 5** (I-026 / I-029 / I-030) | 型 assertion / null as any / any-narrowing enum 変換 | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip (3 fixture 直接削減) |
| 3 | L3 | I-142 Step 4 C-5〜C-7 残余 | I-144 非吸収の small cleanup (C-8 は 2026-04-19 完了済、C-9 は regression 消失で close、他は `doc/handoff/I-142-step4-followup.md` 参照) | — |
| 4 | L3 | **I-158** | Non-loop labeled stmt (`L: { ... }` / `L: switch(...)`) support | TS valid syntax の gap。I-153 完了により emission model 安定、依存解消済 |
| 5 | L3 | **I-159** | 内部 emission 変数の user namespace 衝突 (I-154 の variable 版) | `_try_result` / `_fall` / `_try_break` 等を `__ts_` prefix に統一 + 変数宣言 lint |
| 6 | L3 | I-143 meta-PRD | `??` 演算子の問題空間完全マトリクス + 8 未解決セル | I-143-a〜h 未着手。I-144 後の topology で一部 (I-143-b any ?? T) は I-050 依存 |
| 7 | L3 | I-140 | TypeDef::Alias variant 追加 | `type MaybeStr = string \| undefined` alias 経由 Option 認識失敗 |
| 8 | L3 | I-050-b | Ident → Value coercion | TypeResolver expr_type と IR 型乖離解消が前提 |
| 9 | L4 | I-160 | Walker defense-in-depth (Expr-embedded Stmt::Break) | 現時点 reachability なし |

**注**: 本テーブルは着手順。各 PRD で `prd-template` skill + `.claude/rules/problem-space-analysis.md`
+ `.claude/rules/spec-first-prd.md` を適用する。

### Batching 検討 (2026-04-19)

- I-144 + I-142 Step 4 C-1〜C-4+D-1: ✅ batch 推奨 (I-144 CFG narrowing が scanner を structural に置換)
- I-158 + I-159: batch 検討 (namespace hygiene 系、I-154 と同系。ただし I-158 が I-153 emission model と interaction するため I-158 先行推奨)
- I-143 + I-050-b: batch 検討 (I-143-b は I-050 Any coercion 依存)
- I-140 + I-134: type alias 関連、DRY 可能性
- **I-050 umbrella** (`backlog/I-050-any-coercion-umbrella.md`) は design 母体として存続

### INV 状態

- INV-Step4-1: ✅ 完了 (`report/i142-step4-inv1-closure-compile.md`)
- INV-Step4-2: ✅ **消失確認で close** (2026-04-19、observation 対象だった `utils/concurrent.ts:12` の OBJECT_LITERAL_NO_TYPE regression が現 bench で検出されず。bisection 不要、`doc/handoff/I-142-step4-followup.md` C-9 section に empirical 解消記録)
- I-153 問題空間: ✅ 完了 (`report/i153-switch-nested-break-empirical.md`)

---

## 次のタスクの先行調査まとめ

「次の作業」テーブル priority 1 (I-144) の着手前調査事項。詳細な問題空間と修正方針は
PRD 起票時に SDCDF spec stage で確定する。

### I-144 umbrella: control-flow narrowing analyzer (Spec v2.2 approved、T3/T4/T5 完了、T6 着手可能)

**Status**: Implementation stage 進行中 — PRD `backlog/I-144-control-flow-narrowing-analyzer.md`、
T3 analyzer 基盤 + T4 NarrowEvent enum migration + T5 narrow guard 検出統合 完了 (2026-04-20)。
次 action は T6 (interim scanner 短絡 + Transformer emission を `EmissionHint` / `RcContext` 連動化、
`coerce_default` helper 実装、C-2a/b/c empirical 再現を green 化)。

**v2 → v2.2 revise の要旨** (2026-04-19):
- v2: E 次元を Rust AST pattern に純化、RC (Read Context) 次元新設 (RC1-RC8、`emission-contexts.md` 準拠)、
  T 次元拡張 (T3c/T9/T10/T11/T12)、JS coerce_default table 追加、C-2 を a/b/c/d に sub-category 化、Sub-matrix 5 新設
- v2.1: T2 adversarial review で D1-D7 発見、D3 (E4 match 矛盾 → E5a/b 分割) + D4 (Closure Reassign Policy 未 pin) を主要解消
- v2.2: 2 回目 review で R1-R5 発見、E5a/b を単一 E5 に rollback (CFG dominator 収束)、
  E10 を composite `Option<Union<T,U>>` matches! guard に拡張、Policy A に NLL borrow lifetime 要件明記
- **T1 empirical** (Dual verdict framework 化): R4 (`&&=`) / F6 (try body) が observation ✓ だったが Rust emission RED → I-161 / I-149 別 PRD scope に再分類

**吸収対象** (いずれも I-144 完了で解消):

- I-024 (`if (x)` complex truthy narrowing)
- I-025 (Option return 暗黙 None の complex case)
- I-142 Cell #14 (narrowing-reset: linear `x = null`)
- I-142 Step 4 C-1 (compound op false-positive reset)
- I-142 Step 4 C-2a/b/c (closure body reassign shadow-let 不整合)
- I-142 Step 4 C-3 + C-4 (scanner test coverage、廃止により moot)
- I-142 Step 4 D-1 (scanner call site DRY、廃止により moot)

**別 PRD scope に分離** (T1 empirical):

- **I-161** `&&=` / `||=` 基本 emission 欠陥 (R4 cell)
- **I-149** try/catch narrow + reassign emission 崩壊 (F6 cell)
- **I-050** synthetic union coercion at call sites (typeof/instanceof regression fixture 不能)

**Task list** (T0-T5 完了、T6-T10 pending):
- T0 Discovery ✅ — 26 fixture、要調査 0 件
- T1 Per-cell E2E fixture (red state) ✅ — 14 fixture (9 RED ✗ + 5 GREEN ✓ regression)、report: [`report/i144-t1-red-state.md`](report/i144-t1-red-state.md)
- T2 Spec-Stage Review Checklist ✅ — v2.2 revise (E5 単一化 + E10 composite 拡張 + Policy A NLL)
- **T3 NarrowingAnalyzer 基盤実装 ✅** (2026-04-19) — scope-aware classifier + branch/sequential merge + peel-aware wrapper + unreachable prune + closure descent、events.rs / classifier.rs / mod.rs + 5 test file
- **T4 NarrowEvent enum migration ✅** (2026-04-19) — `NarrowingEvent` struct 廃止、`NarrowEvent::{Narrow, Reset, ClosureCapture}` + `NarrowEventRef` borrowed view + `PrimaryTrigger`/`NarrowTrigger` 2-layer 型、全 consumer 統一
- **T5 narrow guard 検出統合 ✅** (2026-04-20) — `type_resolver/narrowing.rs` 削除 + `narrowing_analyzer/guards.rs` に集約、`NarrowTypeContext` trait で registry access 抽象化、trait boundary 専用 unit test 19 件追加、T3/T4 残置 dead code (`NarrowingAnalyzer` struct / `var_types` / `AnalysisResult.events`) を削除して free fn 統一、Hono bench non-regression 確認
- T6 Transformer emission 連動 + interim scanner 短絡 ← **次 action**
- T7 Interim scanner 完全削除
- T8-T10 吸収対象 defect regression lock-in + quality gate + Implementation stage review

**実装規模 (残)**: ~200-400 行。T6 で Transformer emission (E1/E2a/E2b/E3) 連動 +
`coerce_default` helper (`src/transformer/helpers/coerce_default.rs`) 実装、T7 で interim scanner 廃止。

### I-142 Step 4 残余 (優先度 3、C-5〜C-9)

I-144 に吸収されない小規模 follow-up。[`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md) 参照。
I-144 完了後に 1 session で纏めて処理可能。

### I-158 / I-159 (優先度 4-5、I-153 完了後 derivable)

I-153 PRD から derive された hygiene 系 follow-up。詳細は TODO 参照。

### I-142 Cell 分類 (lock-in 状態)

```
I-142 Cell 分類
  ├── #1〜#4, #7, #8, #11, #13  — Step 1 で structural 解消
  ├── #6, #10, #12              — Step 2 で structural 解消
  ├── #5, #9                    — I-050 依存、compile-error lock-in test
  ├── #14 (narrowing-reset)     — I-144 依存、lock-in test
  └── FieldAccess / Index       — I-142-b+c で解消済 (2026-04-17)
```

---

## 設計判断の引継ぎ

後続 PRD 向けの設計判断アーカイブは **[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)** に集約。

含まれる topic (要約):

- **Type scope 管理**: `push_type_param_scope` の設計理由
- **Primitive type 9 variant YAGNI 例外**
- **Switch emission と label hygiene (I-153/I-154)**: `__ts_` prefix convention、walker 設計、conditional wrap、Block flatten、is_literal_match_pattern 微変化
- **Optional param 収束設計 (I-040)**: `wrap_if_optional` 単一ヘルパー、全 10 emission 経路
- **Conversion helpers (RC-2)**: remapped methods / `produces_option_result` / strictNullChecks / FieldAccess parens
- **Error handling emission**: TryBodyRewrite exhaustive capture / I-023 short-circuit / 協調 / union return 実行順序 (RC-13)
- **DU analysis (Phase A Step 4)**: walker single source of truth / Tpl children visit
- **Lock-in テスト (削除禁止)**: 保護対象テスト一覧
- **残存 broken window**: Item::StructInit 等

新規 PRD 着手時は関連 section を事前レビュー。実装が設計判断と乖離していたら該当 section を
最新化 (削除は禁止 — 過去の設計判断は reference として保持)。

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。
skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済み:**

- Step 0: `basic-types` unskip
- Step 1 (RC-13): `union-fallback`, `ternary`, `ternary-union` unskip + `external-type-struct` (with-builtins) unskip
- Step 2: `array-builtin-methods` unskip + `closures` の I-011 filter 参照セマンティクス解消
- **Pre-Step-3**: I-138 (Vec index Option) + I-022 (`??`) + I-142 (`??=` Ident LHS) — Tier 1 silent bug を pre-Step として解消、`nullish-coalescing` fixture unskip
- **Step 3** (2026-04-17): I-020 部分 + I-025、`void-type` unskip
- **Step 4** (2026-04-17): I-023 + I-021、`async-await` + `discriminated-union` unskip
- **I-153 + I-154 batch** (2026-04-19): switch case body silent redirect + label hygiene structural fix + A-fix (Block stmt support)

**永続 skip (設計制約 4件):**

- `callable-interface-generic-arity-mismatch` — 意図的 error-case (INV-4)
- `indexed-access-type` — マルチファイル用 (`test_multi_file_fixtures_compile` でカバー)
- `vec-method-expected-type` — no-builtins mode 限定の設計制約
- `external-type-struct` — no-builtins mode 限定の設計制約 (with-builtins 側は Step 1 で解消済)

**effective residual (10 fixture):**

trait-coercion, any-type-narrowing, type-narrowing, instanceof-builtin,
intersection-empty-object, closures, functions, keyword-types, string-methods, type-assertion

#### 次の Step

```
~~(L1 silent) I-153 + I-154 batch~~ — ✅ 完了 (2026-04-19)
  ↓
(L2 struct) I-144 (CF narrowing) ← I-142 Step 4 C-1/C-2/C-3/C-4/D-1 吸収
  ↓
Step 5 (type conversion + null)       I-142 Step 4 C-5〜C-9 残余処理 (並行可能)
  ↓ I-158 / I-159 (hygiene follow-ups、並行可能)
Step 6 (string + intersection)        type-narrowing は Step 1 + 6 で完全解消
  ↓
Step 7 (builtin impl)
```

#### Step 5-7 の予定 (未着手)

**Step 5: 型変換 + null セマンティクス** — Tier 2、型変換パイプライン

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-026 | 型 assertion 変換 | `as unknown as T` の中間 `unknown` を消去して直接キャスト |
| I-029 | null/any 変換 | `null as any` → `None` が `Box<dyn Trait>` 文脈で型不一致 |
| I-030 | `build_any_enum_variants()` (`any_narrowing.rs:85`) | any-narrowing enum の値代入で型強制 |

- unskip: `type-assertion`, `trait-coercion`, `any-type-narrowing`

**Step 6: string メソッド + intersection** — Tier 2、独立した小修正群

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-033 | `methods.rs` | `charAt` → `chars().nth()`, `repeat` → `.repeat()` マッピング追加 |
| I-034 | `methods.rs` | `toFixed(n)` → `format!("{:.N}", v)` 変換 |
| I-028 | `intersections.rs:132-145` | mapped type の非 identity 値型で型パラメータ T が消失 (E0091) |

- unskip: `string-methods`, `intersection-empty-object`, `type-narrowing`

**Step 7: ビルトイン型 impl 生成** — Tier 2、大規模

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-071 | `external_struct_generator/` + generator | ビルトイン型（Date, RegExp 等）の impl ブロック生成 |

- unskip: `instanceof-builtin`

#### fixture × Step 解消マトリクス

| fixture | 解消 Step / 依存 | メモ |
|---------|-----------------|------|
| ~~basic-types~~ | ~~Step 0~~ | — |
| ~~union-fallback~~ / ~~ternary~~ / ~~ternary-union~~ | ~~Step 1~~ | — |
| ~~external-type-struct (with-builtins)~~ | ~~Step 1~~ | — |
| ~~array-builtin-methods~~ | ~~Step 2~~ | — |
| ~~void-type~~ | ~~Step 3~~ | — |
| ~~async-await~~ / ~~discriminated-union~~ | ~~Step 4~~ | — |
| ~~nullish-coalescing~~ | ~~pre-Step-3 (I-022 + I-142)~~ | — |
| closures | I-048 (所有権推論) | I-020 Box wrap 解消済、残: move/FnMut |
| keyword-types | I-146 | I-025 implicit None 解消済、残: `return undefined` on void |
| functions | I-319 (Vec index move) | I-020 Box wrap 解消済 |
| type-assertion / trait-coercion / any-type-narrowing | Step 5 | — |
| string-methods / intersection-empty-object | Step 6 | — |
| type-narrowing | Step 6 | Step 1 (I-007) 依存済 |
| instanceof-builtin | Step 7 | — |
| vec-method-expected-type | — | 設計制約 (永続 skip) |
| external-type-struct (no-builtins) | — | 設計制約 (永続 skip) |

### Phase B: RC-11 expected type 伝播 (OBJECT_LITERAL_NO_TYPE 28件)

Phase A 完了後、Hono ベンチマーク最大カテゴリ（全エラーの 45%）に着手。
I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback) を対象とする。
(件数: 2026-04-17 bench 実測 62 errors 中 28 件)

---

## リファレンス

- 最上位原則: [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md)
- 優先度ルール: [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md)
- TODO 記載標準: [`.claude/rules/todo-entry-standards.md`](.claude/rules/todo-entry-standards.md)
- PRD workflow: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md)
- 設計整合性: [`.claude/rules/design-integrity.md`](.claude/rules/design-integrity.md) + [`.claude/rules/prd-design-review.md`](.claude/rules/prd-design-review.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- PRD handoff: `doc/handoff/*.md` (I-142 Step 4 follow-up 等)
- Grammar reference: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- TODO 全体: [`TODO`](TODO)
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
- 実装調査 report: `report/*.md`
