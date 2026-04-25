# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-25 post I-178+I-183 batch close)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 111/158 (70.3%) |
| Hono bench errors | 63 |
| cargo test (lib) | 3121 pass / 0 fail |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 155 pass + 28 `#[ignore]` |
| clippy | 0 warnings |
| fmt | 0 diffs |

**bench 非決定性**: ±1 clean / ±2 errors の noise variance を [I-172] として記録 (test/bench infra defect、別 PRD)。

### 進行中作業

なし。次の作業は本 file「次の作業」section 参照。

### 直近の完了作業

実装詳細は git log、設計判断は [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD | 日付 | 残課題 / 後続への影響 |
|-----|------|---------------------|
| **I-178 + I-183 + Rule corpus optimization batch** | 2026-04-25 | matrix-driven PRD framework (10-rule checklist + 4-layer review + 5-category defect classification) を整備、`.claude/rules/` 21 file + `.claude/skills/` 18 skill + `.claude/commands/` 9 command + CLAUDE.md に reference graph を確立。**次の I-177-D / I-177 で新 framework を初適用**。Tier 3-4 deferral として [I-184]〜[I-193] (10 件) を TODO 起票 |
| **I-161 + I-171 batch (`&&=`/`||=` desugar + Bang truthy emission)** | 2026-04-22〜04-25 | narrow-related compile error の structural fix。**T7 で `narrowed_type` suppression scope の architectural cohesion gap を発見** → I-177-D PRD に委譲。narrow-scope mutation propagation 欠陥が runtime 誤動作として顕在化 → I-177 (Tier 0 L1) として umbrella 化、3 sub-item (A/B/C) 集約 |

---

## 次の作業

**優先順位は [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md) (L1 > L2 > L3 > L4) および [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) (silent semantic change を最優先) に従う。**

### 実行順序 (prerequisite chain)

```
[I-177-D Tier 1 architectural (TypeResolver suppression scope refactor、案 C)]
       │
       ▼
[I-177 Tier 0 (L1 silent semantic change、mutation propagation 本体 + sub-items A/B/C)]
       │
       ▼
I-162 → Phase A Step 5 → I-015 → I-158+I-159 → Phase A Step 6 → ...
```

**I-177-D を I-177 mutation 本体より先行**: I-177-D は I-177 sub-items A/B/C の architectural root cause も同時解消する可能性があるため、structural fix を I-177-D で先行確立後、残 work を I-177 mutation 本体で実施。

### 着手順の導出原則

1. I-144 Dual verdict framework で `TS ✓ / Rust ✗` として分離された narrow-related compile error は I-144 context が fresh なうちに優先 (I-177-D / I-177)
2. Phase A roadmap (Step 5 → Step 6 → Step 7) で compile_test skip 直接削減
3. Phase B (RC-11 OBJECT_LITERAL_NO_TYPE 28件 = Hono 全 error の 45%) は Phase A 完了後
4. L4 latent items (runtime 同一 / reachability なし) は notes 欄に退避

### 着手順 table

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| **0a (Tier 0)** | **L1** | **I-177-D (architectural prerequisite for I-177、TypeResolver suppression scope refactor、案 C)** | `narrowed_type` の closure-reassign suppression scope を enclosing fn body 全体 → post-if 限定に refactor。T7-3 cohesion gap (IR shadow form と TypeResolver Option<T> view の不整合) の structural 解消。I-177 sub-items A/B/C の architectural root cause も同時解消候補 | I-161 T7 で empirical 発見の architectural defect、I-177 起票前の prerequisite。新 framework (`spec-stage-adversarial-checklist.md` 10-rule + `check-job-review-layers.md` 4-layer) の最初の trial 兼任 |
| **0b (Tier 0)** | **L1** | **I-177 (narrow emission v2 umbrella、L1 promoted 2026-04-24)** | I-144 T6-3 inherited の shadow-mutation-propagation 欠陥を structural fix。silent runtime 誤動作 (Tier 0)。**集約 sub-item 3 件**: I-177-A (typeof/instanceof/OptChain × `then_exit + else_non_exit` × post-narrow) / I-177-B (`collect_expr_leaf_types` query 順序 inconsistency) / I-177-C (`!== null` + (F, T) symmetric / Truthy `if (x)` symmetric) | I-161 T3 実装で latent defect が **runtime 誤動作** として顕在化、`conversion-correctness-priority.md` Tier 1 silent semantic change 該当 → L1 promote (旧 L2)。**I-177-D 完了後に sub-items A/B/C の自然解消有無を再評価して残 work を実施** |
| 1 | L3 | **I-162** | class without explicit constructor → `Self::new()` 自動合成 | I-144 T2 instanceof narrow の Rust 側 E2E lock-in が本 defect で block。`class Dog {}` → `struct Dog {}` 止まりで `Dog::new()` 不在で E0599 |
| 2 | L3 | **Phase A Step 5** (I-026 / I-029 / I-030) | 型 assertion / null as any / any-narrowing enum 変換 | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip (3 fixture 直接削減) |
| 3 | L3 | **I-015** | Hono types.rs `Input['out']` indexed access 解決失敗 (E0405) | `src/ts_type_info/resolve/indexed_access.rs:271`。Hono types.rs で 1 件だが dir compile blocker |
| 4 | L3 | **I-158 + I-159 batch** | Non-loop labeled stmt + 内部 emission 変数 user namespace hygiene | I-154 変数版 + I-153 labeled block 対応。I-158 が I-153 emission と interaction のため I-158 先行推奨 |
| 5 | L3 | **Phase A Step 6** (I-028 / I-033 / I-034) | intersection 未使用型パラメータ (E0091) + charAt/repeat/toFixed method 変換 | `string-methods`, `intersection-empty-object`, `type-narrowing` unskip |
| 6 | L3 | **I-143 meta-PRD** | `??` 演算子の問題空間完全マトリクス + 8 未解決セル (a〜h) | I-143-a〜h 未着手。I-143-b (`any ?? T`) は I-050 依存、他は独立 |
| 7 | L3 | **I-142 Step 4 C-5 / C-6 + Phase A Step 7 (I-071)** | I-144 非吸収の small cleanup (C-7 は I-050 依存) + `instanceof-builtin` unskip 用 builtin 型 impl 生成 | C-5/C-6 は test quality 改善 (handoff doc)、I-071 は Phase A 最終 step (1 fixture unskip) |
| 8 | L3 | **Phase B (RC-11)** (I-003 / I-004 / I-005 / I-006) | expected type 伝播の不完全性 (OBJECT_LITERAL_NO_TYPE 28件) | Hono 全 error の 45%、Phase A 完了後の最大インパクト category |

**注**: 各 PRD で `prd-template` skill + `.claude/rules/problem-space-analysis.md` + `.claude/rules/spec-first-prd.md` + `.claude/rules/spec-stage-adversarial-checklist.md` (10-rule) + `.claude/rules/check-job-review-layers.md` (4-layer) を適用する。

### 次点 / L4 deferred (上記 table 外)

- **I-013 + I-014 batch** (L3、RC-5 abstract class 変換パス欠陥) — class inheritance 系、抱え込み依存が強いため独立 PRD 着手時に整備
- **I-140** (L3、TypeDef::Alias variant 追加) — `type MaybeStr = string \| undefined` alias 経由の Option 認識。I-134 / I-056 と batch 可能
- **I-050 umbrella** (L3、Any coercion) — I-143-b + I-050-b + I-050-c が依存。structural 母体として設計維持
- **I-146** (L3、`return undefined` on void fn) — `keyword-types` unskip の残条件
- **I-048** (L3、所有権推論) — RC-2 根本解決、`closures` / `functions` unskip の残条件、修正規模大
- **I-074** (L4、`Item::StructInit` broken window) — pipeline-integrity 違反、PRD 化候補
- **I-160** (L4、Walker defense-in-depth Expr-embedded Stmt::Break) — 現時点 reachability なし
- **I-165 / I-166 / I-167 / I-170** (L4 narrow precision umbrella) — I-144 後の latent imprecision、runtime 動作同一、Rust 精度のみ向上
- **I-168** (L4、`NarrowEvent::Reset` event 未消費) — Hono で顕在化なし pre-existing imprecision
- **I-172** (L4、bench 非決定性) — test / bench infra、別 PRD

### Batching 検討

未着手 batch 候補 (上記 table 内 PRD 着手時に再検討):

- **I-158 + I-159**: namespace hygiene 系 (I-154 と同系)。I-158 先行推奨 (I-153 emission との interaction)
- **I-143 + I-050-b + I-050-c**: `??` / Any / Synthetic union coercion が共通 `resolve_expr` / `propagate_expected` 基盤を持つ
- **I-140 + I-134 + I-056**: type alias 関連、`TypeDef::Alias` variant 新設で DRY 可能
- **I-013 + I-014**: abstract class 変換パス (強依存、`generate_child_of_abstract()` 拡張)
- **I-165 / I-166 / I-167 / I-170**: narrow precision umbrella (`VarId` binding identity + CFG analysis の基盤を共有)

---

## 次の PRD 着手前の参照ポイント

新規 PRD 着手時は `prd-template` skill + 関連 rule (`problem-space-analysis.md` / `spec-first-prd.md` / `spec-stage-adversarial-checklist.md` / `check-job-review-layers.md`) を適用する。

特定 PRD 用の handoff doc:

- **Phase A Step 5 / 6 / 7**: 下記「開発ロードマップ」section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **I-144 設計判断 (archive)**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) (CFG narrowing analyzer / NarrowTypeContext trait / EmissionHint dispatch / coerce_default table / closure reassign Policy A)
- **I-142 Step 4 残余 (C-5〜C-9)**: [`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md)
- **I-158 / I-159**: TODO 参照
- **I-143 meta-PRD (`??` 完全仕様)**: TODO I-143 本体 + a〜h 未解決セル

---

## 設計判断の引継ぎ

後続 PRD 向けの設計判断アーカイブは [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。新規 PRD 着手時は関連 section を事前レビュー、実装が設計判断と乖離していたら該当 section を最新化 (削除は禁止 — 過去の設計判断は reference として保持)。

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済 (Step 0〜4 + I-153/I-154 + pre-Step-3)**: 詳細は git log + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) 参照。

**永続 skip (設計制約 4 件)**:

- `callable-interface-generic-arity-mismatch` — 意図的 error-case (INV-4)
- `indexed-access-type` — マルチファイル用 (`test_multi_file_fixtures_compile` でカバー)
- `vec-method-expected-type` — no-builtins mode 限定の設計制約
- `external-type-struct` — no-builtins mode 限定の設計制約 (with-builtins 側は Step 1 で解消済)

**effective residual (10 fixture)**: trait-coercion, any-type-narrowing, type-narrowing, instanceof-builtin, intersection-empty-object, closures, functions, keyword-types, string-methods, type-assertion

#### 次の Step

```
Step 5 (type conversion + null)       I-026 + I-029 + I-030
  ↓ I-158 / I-159 (hygiene follow-ups、並行可能)
Step 6 (string + intersection)        I-028 + I-033 + I-034
  ↓
Step 7 (builtin impl)                 I-071
```

#### Step 5-7 詳細 (未着手)

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

#### 残 fixture × 解消依存

| fixture | 解消 Step / 依存 | メモ |
|---------|-----------------|------|
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

Phase A 完了後、Hono ベンチマーク最大カテゴリ (全エラーの 45%) に着手。I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback) を対象とする。

---

## リファレンス

- 最上位原則: [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md)
- 優先度ルール: [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md)
- TODO 記載標準: [`.claude/rules/todo-entry-standards.md`](.claude/rules/todo-entry-standards.md)
- PRD workflow: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md)
- Spec stage 完了 verification: [`.claude/rules/spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) (10-rule)
- Implementation stage 完了 verification: [`.claude/rules/check-job-review-layers.md`](.claude/rules/check-job-review-layers.md) (4-layer)
- 設計整合性: [`.claude/rules/design-integrity.md`](.claude/rules/design-integrity.md) + [`.claude/rules/prd-design-review.md`](.claude/rules/prd-design-review.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- PRD handoff: `doc/handoff/*.md`
- Grammar reference: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- TODO 全体: [`TODO`](TODO)
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
- 実装調査 report: `report/*.md`
