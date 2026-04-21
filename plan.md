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
| cargo test (lib) | 2880 pass |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 114 pass + 0 i144 fixtures `#[ignore]` (I-144 全 17 cell un-ignored: cell-14 / c1 / c2a / c2b / c2c / t4d / i024 / t7 / i025 + multifn-isolation + regression 5 + T6-3 regression t4c/t4e + 既存 97) |
| clippy | 0 warnings |
| fmt | 0 diffs |

**Note (2026-04-21)**: T6-4/T6-5 commit message は Hono bench 113/158 clean / 60 errors を報告したが、T6-6 empirical 再測 (clean rebuild × 複数 run) では 112/158 / 62 errors が stable な値。同一 HEAD + 同一ソースで bench に ±1 clean / ±2 errors の non-deterministic variance が発生。**I-144 前後の stable 値 net change = 0 errors**。当初 HashMap iteration order を疑ったが empirical 調査で否定 (`expr_types.get(&span)` 等は lookup only で emission 非影響)。候補 root cause は `std::fs::read_dir` の platform-dependent order / bench script の `find | xargs cp` / `module_graph` の cross-module resolution のいずれか (要調査)。pre-existing 非決定性を I-172 として TODO 起票、I-144 scope 外で別 PRD 扱い。

### 進行中作業

**該当なし** (I-144 完了 2026-04-21)。

### 直近の完了作業

実装詳細は git log / `backlog/` (close 後 archive)、設計判断は
[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD | 日付 | サマリ (1-3 行) |
|-----|------|-----------------|
| **File line-count reduction refactor (8 files)** | 2026-04-21 | 1000 LOC 超過 8 file を cohesion-driven split (21 files changed, +1964 / −8767 LOC net)。Phase 1 test files (build_registry 1123→6 / control_flow 1095→7 / generator/tests 1068→8 / switch 1028→7 / generator/expressions/tests 1019→8) + Phase 2 production files (registry/collection 1524→8 sub-dir with placeholder/decl/class/resolvers/type_literals/const_values/callable / ts_type_info/mod 1045→3 files helpers+tests / transformer/expressions/methods 1267→3 sub-dir mod+closures+tests)。visibility `pub(in crate::registry)` で original `pub(super)` scope を厳密保持。`check-file-lines.sh` OK、quality gate 全 pass、Hono bench 非後退。post-review で `map_method_call` 411 LOC 単一 match decomposition を I-174 として起票 (L4)。計画詳細は git log 参照 |
| **I-144 (control-flow narrowing analyzer umbrella)** | 2026-04-19〜04-21 | CFG-based narrowing analyzer PRD (umbrella: I-024 / I-025 / I-142 Cell #14 / C-1 / C-2a-c / C-3 / C-4 / D-1 吸収) を 9 sub-phase (T0-T6-6) で完了。T0-T2 SDCDF Spec stage (matrix-driven + Dual verdict framework) + T3-T5 analyzer 基盤 (`pipeline/narrowing_analyzer/` + `NarrowEvent` enum + `NarrowTypeContext` trait) + T6-1〜T6-5 emission 実装 (EmissionHint dispatch / coerce_default / truthy E10 / OptChain compound narrow / implicit None tail) + T6-6 close で 7 連鎖 review 11 structural fix (IMPL-1〜7 YAGNI dead variant/field 除去 + `transformer/mod.rs` 1117→718 LOC cohesion 分割)。matrix 全 9 ✗ cell GREEN。設計判断は `doc/handoff/design-decisions.md` section「Control-flow narrowing analyzer (I-144)」8-section archive、sub-phase 実装詳細は git log 参照 |
| **I-153 + I-154 batch + 以前の完了** | 2026-04-19 以前 | I-153 / I-154: switch case body nested `break` silent redirect の structural 解消 + internal label `__ts_` prefix 統一 (`report/i153-switch-nested-break-empirical.md`)。以前: I-SDCDF (spec-first framework、beta)、I-050-a (SDCDF Pilot)、Phase A Step 3/4 (I-020 部分/I-023/I-021)、I-145 / I-150 batch、INV-Step4-1、I-142 (`??=`) / I-142-b+c、I-022 (`??`) / I-138 / I-040 / I-392 ほか。git log で参照可能 |

### 次の作業 (I-144 完了後 2026-04-21、spec-first workflow 適用)

**優先順位は `.claude/rules/todo-prioritization.md` (L1 > L2 > L3 > L4) および
`.claude/rules/ideal-implementation-primacy.md` (silent semantic change を最優先) に従う。**

**Tier 0 (L1 silent) 該当なし**。**Tier 1 (L2 Struct) 該当なし** (I-144 完了で解消)。

**着手順の導出原則**:
1. I-144 Dual verdict framework で `TS ✓ / Rust ✗` として分離された narrow-related compile error は I-144 context が fresh なうちに優先 (I-161 / I-162 / I-171)
2. Phase A roadmap (Step 5 → Step 6 → Step 7) で compile_test skip 直接削減
3. Phase B (RC-11 OBJECT_LITERAL_NO_TYPE 28件 = Hono 全 error の 45%) は Phase A 完了後
4. L4 latent items (runtime 同一 / reachability なし) は notes 欄に退避

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| 1 | L3 | **I-161 + I-171 batch** | `&&=` / `\|\|=` compound logical assignment + `if (!x)` 汎用 truthy (non-Ident LHS / non-exit body / else branch、9 pattern) | I-144 T1 `cell-regression-r4` + T6-3 residual の compile error。両者とも `truthy_predicate_for_expr` 汎用 helper を共有、I-144 narrow context 温存の継続 |
| 2 | L3 | **I-162** | class without explicit constructor → `Self::new()` 自動合成 | I-144 T2 instanceof narrow の Rust 側 E2E lock-in が本 defect で block。`class Dog {}` → `struct Dog {}` 止まりで `Dog::new()` 不在で E0599 |
| 3 | L3 | **Phase A Step 5** (I-026 / I-029 / I-030) | 型 assertion / null as any / any-narrowing enum 変換 | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip (3 fixture 直接削減) |
| 4 | L3 | **I-015** | Hono types.rs `Input['out']` indexed access 解決失敗 (E0405) | `src/ts_type_info/resolve/indexed_access.rs:271`。Hono types.rs で 1 件だが dir compile blocker |
| 5 | L3 | **I-158 + I-159 batch** | Non-loop labeled stmt + 内部 emission 変数 user namespace hygiene | I-154 変数版 + I-153 labeled block 対応。I-158 が I-153 emission と interaction のため I-158 先行推奨 |
| 6 | L3 | **Phase A Step 6** (I-028 / I-033 / I-034) | intersection 未使用型パラメータ (E0091) + charAt/repeat/toFixed method 変換 | `string-methods`, `intersection-empty-object`, `type-narrowing` unskip |
| 7 | L3 | **I-143 meta-PRD** | `??` 演算子の問題空間完全マトリクス + 8 未解決セル (a〜h) | I-143-a〜h 未着手。I-143-b (`any ?? T`) は I-050 依存、他は独立 |
| 8 | L3 | **I-142 Step 4 C-5 / C-6 + Phase A Step 7 (I-071)** | I-144 非吸収の small cleanup (C-7 は I-050 依存) + `instanceof-builtin` unskip 用 builtin 型 impl 生成 | C-5/C-6 は test quality 改善 (handoff doc)、I-071 は Phase A 最終 step (1 fixture unskip) |
| 9 | L3 | **Phase B (RC-11)** (I-003 / I-004 / I-005 / I-006) | expected type 伝播の不完全性 (OBJECT_LITERAL_NO_TYPE 28件) | Hono 全 error の 45%、Phase A 完了後の最大インパクト category |

**注**: 本テーブルは着手順。各 PRD で `prd-template` skill + `.claude/rules/problem-space-analysis.md`
+ `.claude/rules/spec-first-prd.md` を適用する。

### 次点 / L4 deferred (上記 table 外)

table に入らなかった L3 / L4 items:

- **I-013 + I-014 batch** (L3、RC-5 abstract class 変換パス欠陥) — class inheritance 系、抱え込み依存が強いため独立 PRD 着手時に整備
- **I-140** (L3、TypeDef::Alias variant 追加) — `type MaybeStr = string \| undefined` alias 経由の Option 認識。I-134 / I-056 と batch 可能
- **I-050 umbrella** (L3、Any coercion) — I-143-b + I-050-b + I-050-c が依存。structural 母体として設計維持
- **I-146** (L3、`return undefined` on void fn) — `keyword-types` unskip の残条件
- **I-048** (L3、所有権推論) — RC-2 根本解決、`closures` / `functions` unskip の残条件、修正規模大
- **I-074** (L4、`Item::StructInit` broken window) — pipeline-integrity 違反、PRD 化候補
- **I-160** (L4、Walker defense-in-depth Expr-embedded Stmt::Break) — 現時点 reachability なし
- **I-165 / I-166 / I-167 / I-170** (L4 narrow precision umbrella) — I-144 後の latent imprecision、runtime 動作同一、Rust 精度のみ向上
- **I-168** (L4、`NarrowEvent::Reset` event 未消費) — Hono で顕在化なし pre-existing imprecision
- **I-172 / I-173** (L4、bench 非決定性 + E2E parallel flakiness) — test / bench infra、別 PRD

### Batching 検討 (2026-04-21)

- ✅ **完了**: I-144 + I-142 Step 4 C-1〜C-4+D-1 (I-144 で一括吸収)
- **I-161 + I-171**: narrow-related truthy compile error。`truthy_predicate_for_expr` 汎用 helper + `if (!x)` 経路拡張を共有基盤として構築 (新規 batch proposal)
- **I-158 + I-159**: namespace hygiene 系 (I-154 と同系)。I-158 先行推奨 (I-153 emission との interaction)
- **I-143 + I-050-b + I-050-c**: `??` / Any / Synthetic union coercion が共通 `resolve_expr` / `propagate_expected` 基盤を持つ
- **I-140 + I-134 + I-056**: type alias 関連、`TypeDef::Alias` variant 新設で DRY 可能
- **I-013 + I-014**: abstract class 変換パス (強依存、`generate_child_of_abstract()` 拡張)
- **I-165 / I-166 / I-167 / I-170**: narrow precision umbrella (`VarId` binding identity + CFG analysis の基盤を共有)
- **I-050 umbrella** (`backlog/I-050-any-coercion-umbrella.md`) は design 母体として存続

### INV 状態

- INV-Step4-1: ✅ 完了 (`report/i142-step4-inv1-closure-compile.md`)
- INV-Step4-2: ✅ **消失確認で close** (2026-04-19、observation 対象だった `utils/concurrent.ts:12` の OBJECT_LITERAL_NO_TYPE regression が現 bench で検出されず。bisection 不要、`doc/handoff/I-142-step4-followup.md` C-9 section に empirical 解消記録)
- I-153 問題空間: ✅ 完了 (`report/i153-switch-nested-break-empirical.md`)

---

## 次の PRD 着手前の参照ポイント

次期 PRD 着手時、以下を参照:

- **Phase A Step 5 / 6 / 7**: 下記「開発ロードマップ」 section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **I-144 設計判断 (archive)**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) の CFG narrowing analyzer / NarrowTypeContext trait / EmissionHint dispatch / coerce_default table / closure reassign Policy A section
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
- **Control-flow narrowing analyzer (I-144)**: 2-channel architecture (NarrowEvent via guards / EmissionHint dispatch / du_analysis) / `NarrowTypeContext` trait / 3-variant `NarrowEvent` enum + 2-layer `NarrowTrigger` / `coerce_default` table / closure reassign Policy A / Dual verdict framework / `ir_body_always_exits` / **YAGNI 厳守方針 (actually-populated のみ enum variant 化)** / `transformer/mod.rs` cohesion 分割 (helpers/option_builders / injections / ts_enum)
- **Lock-in テスト (削除禁止)**: 保護対象テスト一覧
- **残存 broken window**: Item::StructInit 等、`transformer/mod.rs` 以外の pre-existing file-size violation 8 件

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
I-144 (L2 struct、CF narrowing)      ✅ 完了 2026-04-21 (I-024/I-025/I-142 Cell #14/C-1〜C-4/D-1 吸収)
  ↓
Step 5 (type conversion + null)       I-142 Step 4 C-5〜C-7 残余処理 (C-8 / C-9 完了済、並行可能)
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
(件数: 2026-04-21 T6-6 後 bench 実測 62 errors 中 28 件、I-144 前後で変動なし)

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
