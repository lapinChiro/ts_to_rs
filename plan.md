# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-21)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 112/158 (70.9%) |
| Hono bench errors | 62 |
| cargo test (lib) | 2857 pass |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 112 pass + 2 i144 fixtures `#[ignore]` (phase 別 reason: T6-4 × 1 / T6-5 × 1。cell-14 / cell-c1 / cell-c2a / cell-c2b / cell-c2c / cell-t4d / cell-i024 / multifn-isolation + regression 5 + T6-3 regression t4c/t4e = 計 15 un-ignored + 既存 97) |
| clippy | 0 warnings |
| fmt | 0 diffs |

### 進行中作業

- **I-144 Implementation stage 進行中** — PRD: [`backlog/I-144-control-flow-narrowing-analyzer.md`](backlog/I-144-control-flow-narrowing-analyzer.md)、phase 分割: [`plan.t6.md`](plan.t6.md)
  - **完了 sub-phase** (〜2026-04-21、実装詳細は git log 参照):
    T0-T2 Spec stage (Spec v2.2 approved、Dual verdict framework 確立) /
    T3+T4 analyzer 基盤 + `NarrowEvent` enum / T5 narrow guard 検出を `narrowing_analyzer/guards.rs` に統合 /
    T6-1 pipeline wiring + interim scanner 完全削除 + `??=` EmissionHint dispatch /
    T6-2 `helpers/coerce_default` + closure-reassign narrow suppression + E2b stale read emission /
    **I-169 T6-2 follow-up**: closure-capture scope precision (`NarrowEvent::ClosureCapture.enclosing_fn_body` + position-aware accessors + candidate-limited shadow-tracking walker を `closure_captures.rs` に独立) /
    **T6-3** truthy predicate E10 (`helpers/truthy.rs` + `try_generate_option_truthy_complement_match` + `wrap_in_synthetic_union_variant` + return_wrap double-wrap guard + `ir_body_always_exits` Match 対応)、`/check_job` 2 round + `/check_problem` で 15 defect 全 structural 対応済 (Spec Revision Log + Sub-matrix T4a/T4c/T4d/T4e ✓ 反映 + H-3 mixed-union integration test lock-in + 発見した周辺 defect は I-050-c 拡充 / I-171 新規起票で track)
  - **Foundation 確立済** (T6-4 以降の前提): `??=` EmissionHint dispatch、closure-capture 検出 (14 variant 網羅)、multi-fn scope isolation、primitive/composite truthy predicate emission、Option<Union> call-arg coercion、14 per-cell E2E (cell-14 / c1 / c2a / c2b / c2c / t4d / i024 + multi-fn isolation + 5 baseline regression GREEN、残 2 phase 別 ignore)
  - **次 action**: **T6-4 compound OptChain narrow** — cell-t7 (`x?.v !== undefined` → x non-null narrow) GREEN 化。以降 T6-5 (multi-exit implicit None) / T6-6 (quality gate + PRD close) は `plan.t6.md` 参照
  - **吸収対象 defect** (I-144 完了で一括解消): I-024 / I-025 / I-142 Cell #14 / I-142 Step 4 C-1 / C-2a-c / C-3 / C-4 / D-1
  - **T1 empirical で別 PRD 分離**: I-161 (`&&=` / `||=` 基本 emission)、I-149 (try/catch narrow emission 崩壊)、I-050 (synthetic union coerce)

### 直近の完了作業

実装詳細は git log / `backlog/` (close 後 archive)、設計判断は
[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD | 日付 | サマリ (1-3 行) |
|-----|------|-----------------|
| **I-144 T6-3 (truthy predicate E10 + Option<Union> call-arg coercion + review 2 round 全対応)** | 2026-04-21 | `helpers/truthy.rs` で F64 NaN 含む全 primitive truthy/falsy predicate を中央化 (`x != 0.0 && !x.is_nan()`)、`convert_if_stmt` fallback で primitive の `if (x)` / `if (!x)` を predicate wrap、`!x` early-return on Option<T> を `try_generate_option_truthy_complement_match` で consolidated match 化 (composite Union variant: `match x { Some(V(v)) if <v truthy> => V(v), _ => exit }`、non-primitive variant は guard なし always-truthy arm)、`wrap_in_synthetic_union_variant` で call-site primitive literal 引数を Union variant に自動 wrap (`f("hi")` → `f(Some(F64OrString::String(...)))`)、`return_wrap::wrap_leaf` に同一 enum double-wrap guard 追加、`ir_body_always_exits` に Stmt::Match 全 arm exit 判定追加。`/check_job` 2 round + `/check_problem` で 15 defect (H-1 Paren / H-2 naming / H-3 non-primitive variant / M-1 Spec Revision Log / M-2 wrap unit tests / M-3 return_wrap regression / M-4 truthy exhaustive None / M-5 T4c/T4e E2E / L-1/L-2 review insight / R2-C1 Sub-matrix / R2-C2 Spec Log / R2-C3 H-3 integration / R2-I1 Match exits / R2-I2 命名統一 / R2-I3 unused param) 全 structural 対応。周辺 defect は I-050-c 拡充 / I-171 新規起票で track。cell-t4d / cell-i024 GREEN + cell-regression-t4c/t4e regression lock-in + integration test H-3 mixed-union。lib +19 test (2857)、Hono bench 非後退 (112/158, 62) |
| **I-144 T6-2 follow-up (I-169 closure-capture scope precision)** | 2026-04-20 | `NarrowEvent::ClosureCapture.enclosing_fn_body` で multi-fn scope isolation、`analyze_function(body, params)` に拡張して param-as-candidate 対応、`closure_captures.rs` を独立 module 化 (candidate-limited + shadow-tracking walker)。14 closure boundary variant 網羅 + 27 matrix cell 全判定。/check_job 連鎖 review で全 defect 解消、I-170 (hoisting) は future TODO |
| **I-144 T6-2 (coerce_default helper + narrow-stale emission)** | 2026-04-20 | `helpers/coerce_default.rs` で JS coerce table ((F64, RC1Arith)→0.0 / (F64, RC6StringInterp)→"null") を T6-2 scope 限定実装。narrow guard suppress + arith/string-concat coerce wrap で cell-c2b / cell-c2c GREEN |
| **I-144 T6-1 (pipeline wiring + scanner retirement + ??= EmissionHint dispatch)** | 2026-04-20 | `FileTypeResolution.emission_hints` + 5 entry point wiring、`try_convert_nullish_assign_stmt` を EmissionHint dispatch に書換、interim scanner 完全削除 (-440 行)。cell-14 / cell-c1 / cell-c2a GREEN |
| **I-144 T5 (narrow guard 検出を narrowing_analyzer に統合)** | 2026-04-20 | `type_resolver/narrowing.rs` 削除、`narrowing_analyzer/guards.rs` に集約、`NarrowTypeContext` trait で registry access 抽象化 |
| **I-144 T3+T4 (CFG narrowing analyzer 基盤 + NarrowEvent migration)** | 2026-04-19 | `pipeline/narrowing_analyzer/` 新設、`NarrowingEvent` struct を `NarrowEvent` enum に migrate、scope-aware classifier + branch/sequential merge combinator |
| **I-153 + I-154 batch** | 2026-04-19 | switch case body nested `break` silent redirect の structural 解消 + internal label を `__ts_` prefix に統一。report: [`report/i153-switch-nested-break-empirical.md`](report/i153-switch-nested-break-empirical.md) |
| **以前の完了 (< 2026-04-19)** | — | I-SDCDF (spec-first framework、beta)、I-050-a (SDCDF Pilot)、Phase A Step 3/4 (I-020 部分/I-023/I-021)、I-145 / I-150 batch、INV-Step4-1、I-142 (`??=`) / I-142-b+c、I-022 (`??`) / I-138 / I-040 / I-392 ほか。いずれも git log で参照可能 |

### 次の作業 (empirical 再評価 2026-04-19、spec-first workflow 適用)

**優先順位は `.claude/rules/todo-prioritization.md` (L1 > L2 > L3 > L4) および
`.claude/rules/ideal-implementation-primacy.md` (silent semantic change を最優先) に従う。**

**Tier 0 (L1 silent) 該当なし** (I-153 完了により解消)。次の最優先は L2 structural foundation の I-144。

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| 1 | **L2 Struct** | **I-144** umbrella (Spec v2.2 approved、**T3/T4/T5/T6-1/T6-2/T6-3 完了** 2026-04-20、T6-4 着手可能) | control-flow narrowing analyzer (I-024 / I-025 / I-142 Cell #14 / C-1 / C-2a-c / C-3 / C-4 / D-1 吸収) | T6-3 で truthy predicate E10 (`helpers/truthy.rs` + `try_generate_option_truthy_complement_match` + `wrap_in_synthetic_union_variant`) を確立、cell-t4d (F64 NaN) / cell-i024 (composite Option<Union> via consolidated match + call-arg coercion) GREEN。次 T6-4 で compound OptChain narrow (cell-t7)、続いて T6-5 〜 T6-6 で残 1 cell + quality gate (詳細: `plan.t6.md`) |
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

## 次の PRD 着手前の参照ポイント

優先度 2 以降の PRD に着手する際、以下を参照:

- **I-144 (進行中)**: 本 file「進行中作業」section + [`backlog/I-144-...md`](backlog/I-144-control-flow-narrowing-analyzer.md) + [`plan.t6.md`](plan.t6.md)
- **Phase A Step 5 / 6 / 7**: 下記「開発ロードマップ」 section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **I-142 Step 4 残余 (C-5〜C-9)**: [`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md)
- **I-158 / I-159 (hygiene follow-ups)**: TODO 参照
- **I-143 meta-PRD (`??` 完全仕様)**: TODO I-143 本体 + a〜h 未解決セル

新規 PRD 着手時は `prd-template` skill + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md) + [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) を適用する。

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
I-144 (L2 struct、CF narrowing)      現 進行中。T6-3 〜 T6-6 残 (詳細: plan.t6.md)
  ↓                                   I-142 Step 4 C-1/C-2/C-3/C-4/D-1 を吸収
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
