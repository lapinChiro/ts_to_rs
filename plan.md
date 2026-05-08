# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-05-09)

**進行中**: 案 γ Phase 0 = **PRD I-D (Framework rule integration cohesive batch) 着手予定**。

**最新の完了**: I-224 (B2 fn main mechanism、Option β cohesive batch) close (2026-05-09)。詳細 = § 直近の完了作業 + git log `[CLOSE] I-224 PRD 完了` commit + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section (= 16 sub-sections、12 度 v12-2 pattern recurrence chain evidence、9 framework 改善 candidates table embed)。

**開発順序の見直し (2026-05-09 user 確定)**: 旧案 β (= I-225 → I-162 → I-205 T14-T16 → I-D) を **案 γ (= I-D first → I-225 → I-162 → I-205 T14-T16)** に再設計。Rationale = framework quality first principle (= "PRD作成 / ワークフロー品質 を上げる対応から着手")、4 度連続 v12-2 pattern empirical lock-in を踏まえ I-D framework 整備を I-225/I-162/I-205 T14-T16 の prerequisite に位置付け、後続 PRDs の spec stage iteration cost を構造的削減。

**次着手**: PRD I-D = Framework rule integration cohesive batch (32 candidates / 14 rounds adversarial review 累積、`doc/handoff/design-decisions.md` framework lesson archive section + TODO `[I-D]` entry を primary reference)。

### Quality Gate (post I-224 PRD close、2026-05-09)

| 指標 | 値 |
|------|-----|
| cargo test | lib **3546** / e2e_test **201 active + 80 ignored** / i224_invariants 7 / i224_helper 5 / i205_invariants 2 + 5 ignored / i205_helper 4 / i399_isolation_test 2 active + 3 ignored / 全 green |
| cargo clippy / fmt / file-line src | 0 warnings / 0 diffs / 全 src/.rs file < 1000 行 (注: `tests/i224_invariants_test.rs` 1502 行 + `tests/e2e_test.rs` 3022 行 = project-wide policy 違反、scripts/check-file-lines.sh は src/ scope = auto detect なし、I-176 entry 拡張済 = 案 γ Phase 0 後 test layout split refactor として fix 予定) |
| audit-prd-rule10 / audit-no-pub-fn-init / audit-no-init-call-site | PASS / exit=0 (INV-4 + INV-7 CI merge gate 維持) |
| Hono bench | clean **107** / errors **72** at SHA-pinned 027e3df (Preservation classification 維持) |

**bench 非決定性**: ±1 clean / ±2 errors の noise variance を [I-172] として記録 (test/bench infra defect、別 PRD)。

---

## /start 再開時の手順

### Step 1: 状態確認
1. **本 plan.md** を読む = 現在の状態 + 次着手 = **PRD I-D (Framework rule integration cohesive batch)** 着手 (案 γ Phase 0)
2. **TODO `[I-D]` entry** を読む = 32 framework 改善 candidates 全列挙 (R-1 + R-5 + 改善 v2-1〜v13-7、14 rounds adversarial review 累積)
3. **[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section** を読む = 16 sub-sections の cross-PRD applicable design patterns + 4 度連続 v12-2 pattern recurrence chain evidence + 9 candidates I-224-derived chain detailed table = I-D PRD 起票時の **primary lesson source**
4. **closed PRDs**: I-224 / I-399 / I-180 は `backlog/` から削除済、git log audit trail + design-decisions.md framework lesson archive から access

### Step 2: PRD I-D 着手 (次着手)

**Pre-requisite verify** (= I-224 完了で達成済):
- I-224 PRD close (= INV-1〜INV-7 全 GREEN structural lock-in、e2e empirical verification 信頼性 base 確立 by I-399)
- Quality gate: cargo test 全 pass / cargo clippy 0 warnings / cargo fmt 0 diffs / 全 audit PASS
- 32 framework candidates が 14 rounds adversarial review を経て累積、design-decisions.md に primary lesson source embed 済

**Work**: TODO `[I-D]` entry + design-decisions.md framework lesson archive を参照、Discovery → spec stage matrix (`prd-template` skill 適用) → cohesive batch boundary 確定 (= 32 candidates の sub-domain split or 単一 PRD の判断、spec stage で確定) → Implementation stage TDD。

**Completion criteria**:
- 32 candidates の rule target / resolution direction を spec で確定 (Spec stage)
- framework rule files (`spec-stage-adversarial-checklist.md` / `check-job-review-layers.md` / `spec-first-prd.md` / `prd-completion.md` 等) + audit scripts (`audit-prd-rule10-compliance.py` 等) の改修実装 (Implementation stage)
- self-applied integration: 本 PRD I-D 自身が新 framework rules で structural compliance verify (= v12-2 pattern 5 度連続再発防止 empirical lock-in)
- 後続 PRDs (I-225 / I-162 / I-205 T14-T16) prerequisite path clean

---

## 実行順序 (prerequisite chain、案 γ = Framework quality first + Universal infra leverage)

**案 γ 採用根拠 (2026-05-09 user 確定)**:
- Framework quality first (= PRD作成 / ワークフローそのものの品質を上げる対応から着手) = 全 future PRDs に leverage
- 4 度連続 v12-2 pattern empirical lock-in (= I-224 chain で確認) → I-D 未対処なら I-225 spec stage で 5 度連続再発の risk
- I-D 完了で audit scripts CI integration + framework rule strengthening = 後続 PRDs spec stage iteration cost 構造的削減 (= initial iteration で完成可能化)
- 旧案 β (I-225 → I-162 → I-205 T14-T16 → I-D) は scope-based ordering、framework leverage を後回しにする structural compromise

```
[完了] PRD 1〜2.7 (I-177-D / I-177-E / I-177-B / I-177-F / I-198+199+200 batch) — 2026-04-26〜27
   ↓
[完了] PRD 2.75 = I-205 (T1-T13 完了) — 2026-05-01
   T14-T16 は案 γ Phase 1 完了後に再開 (= I-225 / I-162 universal infra prerequisite block)
   ↓
[完了] PRD α-0 = **I-399 (E2E test isolation defect)** — 2026-05-08 (= e2e empirical verification 信頼性 base 構造的 lock-in)
   ↓
[完了] PRD α-1 = **I-224 (B2 fn main mechanism)** — 2026-05-09 (= top-level fn main mechanism + Option β cohesive batch、INV-1〜INV-7 structural lock-in)
   ↓
═════ 案 γ Phase 0: Framework quality integration (NEW、2026-05-09 順序入れ替え = 旧案 β I-D を Phase 1-C → Phase 0 へ前倒し) ═════
   ↓
[次] **PRD I-D = Framework rule integration cohesive batch** (= 32 candidates / 14 rounds adversarial review 累積、4 度連続 v12-2 pattern 構造的解消)。**起票時 primary reference** = TODO `[I-D]` entry + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section
   ↓
═════ 案 γ Phase 1: I-205 T14 prerequisite chain (旧案 β Phase 1-A、framework quality 後着手) ═════
   ↓
[次] PRD α-2 = I-225 (B3 class field literal-only initializer type inference)
   ↓
[次] PRD α-3 = I-162 (constructor synthesis `Self::new()` for no-explicit-constructor classes)
   ↓
═════ 案 γ Phase 2: I-205 close (旧案 β Phase 1-B) ═════
   ↓
[次] I-205 T14: E2E fixtures green-ify (B2/B3/I-162 verified end-to-end、34 cells)
   ↓
[次] I-205 T15: /check_job 4-layer review + 12-rule self-applied verify (= I-D で強化された framework 適用)
   ↓
[次] I-205 T16: Task-ID-based naming → semantic naming refactor + I-205 範囲内 unwrap() cleanup
   ↓
═════ 案 γ Phase 3: L1 Tier 0 priority (旧案 β Phase 2) ═════
   ↓
[次] PRD 3 = I-177 mutation propagation 本体 (Tier 0 silent semantic change、L1)
   ↓
═════ 案 γ Phase 4: Class dispatch group → L1 silent decorator (旧案 β Phase 3) ═════
   ↓
[次] PRD 2.76 = I-A (Method static-ness IR field propagation、元 I-205 T11-b)
   ↓
[次] PRD 2.77 = I-B (Class TypeName context detection unification + Static field associated const emission、元 I-205 T11-d/f + I-214 統合)
   ↓
[次] PRD 2.8 = I-201-A (AutoAccessor 単体 Tier 1 化、decorator なし subset)
   ↓
[次] PRD 2.9 = I-202 (Object literal Prop::Method/Getter/Setter Tier 1 化)
   ↓
[次] PRD 7 = I-201-B (Decorator framework 完全変換、TC39 Stage 3、L1 silent semantic change)
   ↓
═════ 案 γ Phase 5: Narrow refinements (旧案 β Phase 4、post-L1 cleanup) ═════
   ↓
[次] PRD 4 = I-177-A (else_block_pattern Let-wrap + I-194 typeof if-block elision 拡張可)
   ↓
[次] PRD 5 = I-177-C (symmetric XOR early-return detection)
   ↓
[次] PRD 6 = I-048 (closure ownership 推論、T7-3 完全 GREEN-ify)
   ↓
Phase A Step 5 → I-015 → I-158+I-159 → Phase A Step 6 → ...
```

**PRD 凝集度原則**: 凝集度高 + 適切な粒度。1 PRD = 1 architectural concern。各 PRD の architectural concern + 着手順 rationale は下記「次の作業 table」+ TODO 参照。

---

## 次の作業 table (priority order、案 γ 反映)

| 優先度 | レベル | PRD | architectural concern (= 1 PRD = 1 concern) |
|--------|-------|-----|---------------------------------------------|
| **次着手 (案 γ Phase 0)** | L4 (framework quality) | **PRD I-D Framework rule integration cohesive batch** | 32 framework 改善 candidates の cohesive integration (= R-1 + R-5 + 改善 v2-1〜v13-7、14 rounds adversarial review 累積)。**Canonical source**: TODO `[I-D]` entry (32 件全列挙) + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section (= 9 candidates I-224-derived chain detailed table + 12 度 v12-2 pattern recurrence chain evidence)。Cohesive batch boundary (= 単一 PRD or sub-domain split) は spec stage で確定 |
| 次優先 (案 γ Phase 1) | L3 | **I-225 (B3)** | Class field の literal-only initializer (annotation 無) で type inference 完成 |
| 次優先 (案 γ Phase 1) | L3 | **I-162** | Constructor synthesis `Self::new()` for no-explicit-constructor classes |
| 次優先 (案 γ Phase 2) | L2 | **I-205 T14〜T16** | Class member access dispatch with getter/setter framework 完了 (e2e green-ify + naming refactor) |
| L1 Tier 0 (案 γ Phase 3) | L1 | **PRD 3 (I-177 mutation propagation)** | F1/F3 narrow body 内 mutation の outer Option<T> propagation (silent semantic change 解消) |
| Class group (案 γ Phase 4) | L3 | **PRD 2.76 (I-A) + 2.77 (I-B) + 2.8 (I-201-A) + 2.9 (I-202)** | Method static-ness IR field / Class TypeName context detection / AutoAccessor / Object literal getter/setter |
| L1 silent (案 γ Phase 4) | L1 | **PRD 7 (I-201-B)** | Decorator framework 完全変換 (TC39 Stage 3) |
| Narrow refinements (案 γ Phase 5) | L3 | **PRD 4-6 (I-177-A / I-177-C / I-048)** | typeof Let-wrap / symmetric XOR / closure ownership 推論 |
| Phase A continuations | L3 | **I-162 → Step 5 → I-015 → I-158+I-159 → Step 6 → I-143 / Step 7 / Phase B** | compile_test skip 解消 chain |

詳細 architectural concern + 着手順 rationale + completion criteria は各 PRD の TODO entry / backlog/ doc 参照。

### 次点 / L4 deferred (上記 table 外)
- I-013 + I-014 batch (RC-5 abstract class 変換パス)、I-140 (TypeDef::Alias)、I-050 umbrella (Any coercion)、I-146 (`return undefined` on void fn)、I-074 / I-160 / I-165〜I-170 / I-168 / I-172 / I-177-G (= 各 L4 latent items、TODO 参照)
- **I-395** = Class expression conversion (anonymous class lifting、I-201-A / I-201-B 系 cohesive batch 候補)
- **I-396** = Module-level destructuring pattern proper conversion (I-016 silent drop family、5-axis matrix-driven PRD 候補)
- **I-397** = e2e harness `should_auto_append_main_call` detection edge cases (low priority infra)
- **I-400** = E2E runner mechanism defensive design improvements (I-399 T4 由来、test infra cluster sister)
- **I-401** = I-399 heavyweight invariants の CI scheduled workflow 配置 (週次 cron `--ignored` opt-in 自動実行、I-400 と batch 化候補)
- **I-402** = `tests/` 共有 module の subdirectory pattern migration (= project-wide test layout refactor、I-176 と batch 化候補)

---

## 直近の完了作業 (audit trail summary)

実装詳細は git log、設計判断 archive は [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD / Phase | 日付 | 後続への影響 |
|-------------|------|-------------|
| **[CLOSE] I-224 PRD 完了 (B2 fn main mechanism + Option β cohesive batch + Iteration v13 + post-close 2 round /check_job adversarial review)** | 2026-05-09 | top-level executable script の Rust emission `fn main()` 自動生成 mechanism 完成 (INV-1〜INV-7 structural lock-in)、Option β cohesive batch (top-await Tier 1 + ESM harness + collision detection) 完成、12 C1 cells e2e GREEN-ify + I-180 close 達成、Hono bench Preservation 107/72 維持 (production code 0 LOC change)。**4 度連続 v12-2 pattern empirical lock-in** = framework v12-1/v12-2 + v13-1〜v13-7 = **9 candidates** を I-D PRD batch 起票候補化 (計 **32 件 / 14 rounds adversarial review** 累積)。**詳細** = git log `[CLOSE] I-224 PRD 完了` commit + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section (= 16 sub-sections embed: Option β cohesive batch decision pattern + Axis E orthogonality merge + 25 NA cells unified mutual exclusion + 3-tuple dispatch tree + INV 4-item invariant pattern + 6-category test layout + R-2/R-4 audit methodology + 23 sub-commits decomposition + 9 framework 改善 candidates table + 12 度 v12-2 pattern recurrence chain evidence + Implementation-level structural fixes (Iteration v8〜v11) + structural lock-in artifact 一覧) |
| **audit-prd-rule10-compliance.py default mode structural fix (= I-224 T7 /check_problem 由来)** | 2026-05-08 | CI merge gate が真に I-205 + I-224 を audit 化 (`## Rule 10 Application` section presence による content-based auto-detect、命名 convention に依存しない future-proof な audit 対象選定)。詳細 = git log |
| **[CLOSE] I-399 PRD 完了 (E2E test isolation defect、universal infra prerequisite for I-224 T9)** | 2026-05-08 | 全 PRD review iteration の e2e empirical verification 信頼性 base が **structural lock-in** 化 = 偽陽性/偽陰性 source 構造的排除 (per-test content-hash-derived bin design、INV-T1/T2/T3 が `tests/i399_isolation_test.rs` lock-in)。Layer 2 empirical post-fix mean **128.77s vs baseline 165.72s = -22%**。詳細 = git log `[CLOSE] I-399 PRD 完了` commit |
| **I-205 T13** (B6/B7 corner cells Tier 2 reclassify lock-in + INV-5 Option B audit + 5 NEW integration tests) | 2026-05-01 | INV-5 visibility consistency (Option B、production 0 LOC change) |
| **I-205 T8〜T12** (Compound assign Member target setter dispatch + Logical compound + Internal `this.x` dispatch + Class Method Getter body C1 `.clone()` 自動挿入) | 2026-04-29〜05-01 | Decision Table A/B/C 完全 cover、INV-2/3/5 等 lock-in、TODO I-217〜I-223 起票 |
| **環境整備** (4 file 構造的分割 + DRY refactor、行数超過解消) | 2026-04-29 | 4 → 27 file split、TODO I-393 / I-394 起票 |
| **PRD 2.7 (I-198 + I-199 + I-200 batch)** framework Rule 改修 + TypeResolver coverage extension + ast-variants.md Prop section 追加 + audit scripts CI 化 | 2026-04-27 | framework Rule 3/4/10/11/12 拡張 |
| **I-184** (CI fresh-clone defect: stale gitignored template files post pool refactor) | 2026-04-27 | `.gitignore` asymmetric handling + Cargo.lock tracked |
| **I-177-E + I-177-B + I-177-F batch** (Plan η Step 1.5 + Step 2 + Step 2.5) | 2026-04-26 | `synthetic fork inheritance` fix + `FileTypeResolution` canonical primitive + arrow/fn-expr `visit_block_stmt` 統一 |
| **I-177-D** (TypeResolver `narrowed_type` suppression scope refactor、案 C、Plan η Step 1) | 2026-04-26 | trigger-kind-based dispatch refactor、Plan η framework 初実戦投入 |
| **I-178 + I-183 + Rule corpus optimization batch** | 2026-04-25 | matrix-driven PRD framework 整備 (12-rule checklist + 4-layer review + 5-category defect classification) |
| **I-161 + I-171 batch** (`&&=`/`\|\|=` desugar + Bang truthy emission) | 2026-04-22〜04-25 | I-177 umbrella 起票 (Tier 0 L1) |

---

## 次の PRD 着手前の参照ポイント

- **PRD I-D (Framework rule integration cohesive batch、次着手 = 案 γ Phase 0)**: I-205/I-225/I-162 PRD chain prerequisite、32 candidates / 14 rounds adversarial review 累積。**Canonical source** = TODO `[I-D]` entry (32 件全列挙)。**Primary lesson source** = [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section (= 16 sub-sections + 9 candidates I-224-derived chain detailed table + 12 度 v12-2 pattern recurrence chain evidence)。**Cohesive batch boundary** (= 単一 PRD or sub-domain split) は spec stage で確定
- **I-225 / I-162 (案 γ Phase 1、I-D 完了後着手)**: TODO 内 entry + 案 γ chain
- **I-205 T14-T16 (案 γ Phase 2、I-225/I-162 完了後着手)**: [`backlog/I-205-getter-setter-dispatch-framework.md`](backlog/I-205-getter-setter-dispatch-framework.md) の T11 削除 + 新 PRD I-A/I-B migration 注記
- **I-224 / I-399 / I-180 (closed PRDs)**: PRD doc は `backlog/` から削除済、git log audit trail (`[CLOSE] I-224 PRD 完了` / `[CLOSE] I-399 PRD 完了` commit) + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) framework lesson archive section から access
- **I-400 / I-401 / I-402 (新 PRD 起票候補)**: I-399 T4 /check_job Review insight 由来 follow-up improvements、test infra cluster sister として I-172 / I-397 と batch 化候補
- **PRD 3 (I-177 本体)**: matrix-driven、案 A (mutation-ref) vs 案 B (writeback) を spec stage で empirical 確定
- **Phase A Step 5/6/7**: 「開発ロードマップ」section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) (削除禁止 — 過去判断 + closed PRD lessons の primary reference、I-D PRD 等 framework integration PRD 起票時に **I-224 + I-399 lesson source** として参照必須)

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。

**完了済 (Step 0〜4 + I-153/I-154 + pre-Step-3)**: 詳細は git log + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) 参照。

**永続 skip (設計制約 4 件)**: `callable-interface-generic-arity-mismatch` / `indexed-access-type` / `vec-method-expected-type` / `external-type-struct`。

**effective residual (10 fixture)**: trait-coercion, any-type-narrowing, type-narrowing, instanceof-builtin, intersection-empty-object, closures, functions, keyword-types, string-methods, type-assertion。

**次の Step**:
```
Step 5 (型変換 + null)              I-026 + I-029 + I-030
Step 6 (string + intersection)     I-028 + I-033 + I-034
Step 7 (builtin impl)               I-071
```

| Step | 修正対象 | 主要 issue | unskip target |
|------|---------|-----------|---------------|
| 5 | 型 assertion / null/any 変換 / any-narrowing enum | I-026 / I-029 / I-030 | type-assertion, trait-coercion, any-type-narrowing |
| 6 | string method / intersection mapped type | I-028 / I-033 / I-034 | string-methods, intersection-empty-object, type-narrowing |
| 7 | builtin 型 impl 生成 (Date, RegExp 等) | I-071 | instanceof-builtin |

**残 fixture × 解消依存** (Step 経由不能): closures (I-048)、keyword-types (I-146)、functions (I-319)。

### Phase B: RC-11 expected type 伝播 (OBJECT_LITERAL_NO_TYPE)

Phase A 完了後、Hono ベンチマーク最大カテゴリ (全エラーの 45%) に着手。I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback)。

---

## リファレンス

- **最上位原則**: [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md)
- **優先度ルール**: [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md)
- **TODO 記載標準**: [`.claude/rules/todo-entry-standards.md`](.claude/rules/todo-entry-standards.md)
- **PRD workflow**: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md)
- **Spec stage 完了 verification**: [`.claude/rules/spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) (12-rule)
- **Implementation stage 完了 verification**: [`.claude/rules/check-job-review-layers.md`](.claude/rules/check-job-review-layers.md) (4-layer)
- **設計整合性**: [`.claude/rules/design-integrity.md`](.claude/rules/design-integrity.md) + [`.claude/rules/prd-design-review.md`](.claude/rules/prd-design-review.md)
- **完了基準**: [`.claude/rules/prd-completion.md`](.claude/rules/prd-completion.md) (Tier-transition compliance wording 含む)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) (削除禁止 — 過去判断 + closed PRD lessons の primary reference、I-D PRD 等 framework integration PRD 起票時に **I-224 + I-399 lesson source** として参照必須)
- **closed PRD lesson archive (I-224)**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section (= 16 sub-sections embed: Option β cohesive batch decision pattern + Axis E orthogonality merge + 25 NA cells unified mutual exclusion + 3-tuple dispatch tree + INV 4-item invariant pattern + 6-category test layout + R-2/R-4 audit methodology + 23 sub-commits decomposition + **9 framework 改善 candidates table** + **計 12 度 v12-2 pattern recurrence chain evidence** + Implementation-level structural fixes (v8〜v11) + structural lock-in artifact 一覧)。**PRD I-D 起票時 mandatory reference**
- **PRD handoff**: `doc/handoff/*.md`
- **Grammar reference**: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- **TODO 全体**: [`TODO`](TODO)
- **active PRD docs in `backlog/`**: [`backlog/I-205-getter-setter-dispatch-framework.md`](backlog/I-205-getter-setter-dispatch-framework.md) (= class member access dispatch with getter/setter framework、T14-T16 残、案 γ Phase 2 で再開) + [`backlog/I-050-any-coercion-umbrella.md`](backlog/I-050-any-coercion-umbrella.md) (= legacy partial-framework umbrella、deferred)
- **ベンチマーク履歴**: `bench-history.jsonl`
- **エラー分析**: `scripts/inspect-errors.py`
- **実装調査 report**: `report/*.md`
