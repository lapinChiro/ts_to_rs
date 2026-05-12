# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-05-12、**Rules 徹底レビュー + 改善 batch 完了** = 20 rule files + 5 cross-system files に 24 観点 適用、PRD I-D-main spec stage 再開 ready)

**進行中**: なし (案 γ Phase 0.5 完了)。**次着手** = 案 γ Phase 0 継続 = **PRD I-D-main spec stage 再開** (Iteration v19 で initial iteration convergence target、Hybrid 4-条件 final rule satisfy 到達なら Implementation stage 着手)。

**最新の完了** (2026-05-12): **Rules 徹底レビュー + 改善 batch 完了** = `rule_review_list.md` (8 group / 24 観点) を `.claude/rules/*.md` 全 file + cross-system files (`CLAUDE.md` / `commands/*.md` / `skills/*/SKILL.md` / `plan.md`) に適用。Cross-cutting violation matrix を構築 (24 観点 × 20 rule file = 480 cell evaluation)、cross-system inter-file relationship verify (Rule↔Rule / Rule↔Skill / Rule↔CLAUDE.md / Rule↔Command / Rule↔plan.md)。Phase 3a (Critical, "N-rule" stale reference drift system-wide fix) / Phase 3b (A2 Versioning section 削除) / Phase 3c (B1+B2 instance/temporal citation 抽象 pattern essence 化) / Phase 3d (H2 `paths` frontmatter 11 file 追加 = 15/22 coverage) / 終 (F1 terminology uniformity) 全完了 + /check_job Layer 1 + /check_problem Review insight TODO 起票完了。詳細 = § 直近の完了作業。

**前回完了** (2026-05-11): **PRD I-D-pre close 完了** = 5 cells matrix-driven、6 phases implementation、2 度の `/check_job` adversarial review、14 structural fixes / 0 patches、framework v1.8 self-applied integration 達成。詳細 = `doc/handoff/design-decisions.md` `## I-D-pre: Audit mechanism bootstrap` section。

**次着手** = **PRD I-D-main spec stage 再開** = Rules 改善 base (24 観点 全 file 適用済) で first third-party adversarial review (= Iteration v19) を実施、convergence target で Implementation stage 着手 ready。

**開発順序**: 案 γ (= **[完了] I-D-pre → I-D-main → I-225 → I-162 → I-205 T14-T16**、Path B split で I-D を 2 PRD serial sequence に展開、I-D-pre 完了で bootstrap utility 完成 base 確立)。詳細 = 下記「実行順序」section。

---

## /start 再開時の手順 (= PRD I-D-main spec stage 着手 ready)

### Step 1: Empirical sanity check (= post I-D-pre close state preservation verify)

```bash
# 1. INV-4 post-close baseline (3-tuple、I-D-pre は close で audit out-of-scope)
for prd in backlog/I-050-any-coercion-umbrella.md backlog/I-205-getter-setter-dispatch-framework.md backlog/I-D-main-framework-rule-integration-cohesive-batch.md; do
    python3 scripts/audit-prd-rule10-compliance.py "$prd" 2>&1 | head -1
done
# Expected: I-050 FAIL (preserve baseline) / I-205 PASS / I-D-main PASS

# 2. Path E 0 drifts (strict byte-exact mode)
python3 scripts/verify_prd_self_audits.py backlog/I-D-main-framework-rule-integration-cohesive-batch.md
# Expected: Total drifts: 0

# 3. Handoff audit 0 drifts
python3 scripts/audit-handoff-doc-line-refs.py doc/handoff/
# Expected: 0 drifts

# 4. cargo test / clippy / fmt clean
cargo test --tests --no-fail-fast 2>&1 | grep "test result" | tail -5
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

### Step 2: Primary references for I-D-main start

1. **[`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md)** = 24 cells PRD doc (Iteration v1-v17 + v18 Path B split entry preserved、I-D-pre 完了後 spec stage 再開 = WAITING state、`## Spec Review Iteration Log` 末尾 Iteration v18 = Path B split adoption record)
2. **[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)** `## I-D-pre: Audit mechanism bootstrap` section = bootstrap utility formal lock-in 完成 + framework v1.8 self-applied integration evidence + Path B split rationale empirical proof
3. **[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)** `## I-224: ...` section = 12 度 v12-2 pattern recurrence chain evidence + 9 framework 改善 candidates table
4. **TODO entries**:
   - `[I-D-main]` = 24 cells + iteration v1-v17 + v18 history
   - `[I-D-future-vocab-fork]` = broader vocabulary fork detection deferred (L4 latent、I-D-pre Cell 5 scope 分離由来)
   - `[I-D-future-audit-extensions-hardening]` = 6 candidate classes (C1-C6) cohesive batch (L4 latent、I-D-pre Phase 3/4 由来)
   - `[I-205-retroactive-cell-numbering-section]` = 案 γ Phase 2 T15 batch 化、I-205 PRD doc に `## Cell Numbering Convention` + `## Spec→Impl Mapping` section 追加 (= audit scope 内自動 promote)
5. **Bootstrap utilities (= I-D-pre 完成 lock-in、I-D-main で full leverage)**:
   - [`scripts/verify_line_refs.py`](scripts/verify_line_refs.py) (Method A、PRD doc heading-based line-ref drift detection)
   - [`scripts/verify_prd_self_audits.py`](scripts/verify_prd_self_audits.py) (Path E、4 axes + strict byte-exact、recursive self-audit structure)
   - [`scripts/audit-handoff-doc-line-refs.py`](scripts/audit-handoff-doc-line-refs.py) (handoff doc 4 drift categories、CI step PR merge gate active)
6. **Framework v1.8 (= I-D-pre Phase 5 完成 self-applied integration)**:
   - [`.claude/rules/check-job-review-layers.md`](.claude/rules/check-job-review-layers.md) Layer 1 sub-step (4) factual accuracy semantic check
   - [`.claude/rules/spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) Rule 9 sub-rule (d) + Rule 13 sub-rule (13-6) cell numbering convention

### Step 3: PRD I-D-main spec stage 再開 = 24 cells, Iteration v19 で initial iteration convergence target

1. I-D-main PRD doc 最新 spec state 確認 (= 1516 LOC、24 cells、Iteration v18 = Path B split entry)
2. First third-party adversarial review (= Iteration v19) 実施、Hybrid 4-条件 final rule (Critical 0 + High 0 + trajectory diminishing + meta ratio ≤ 50%) で convergence target
3. convergence 達成 = Implementation stage 着手、未達 = recursive iteration
4. **bootstrap leverage**: Iteration v19 review で framework v1.8 + 3 audit utilities full leverage、I-D-pre で structural absorbed defect class は再発しない (= bootstrap utility correctness ceiling 構造的解消の empirical 効果検証)

### Step 4: 後続 prerequisite chain (案 γ Phase 1〜)

PRD I-D-main close + Implementation stage 完了後、案 γ Phase 1 (= I-225 → I-162 → I-205 T14-T16) 着手。詳細 = 下記「実行順序」section + 「次の作業 table」。

---

## Quality Gate (latest baseline post I-D-pre close、2026-05-11)

| 指標 | 値 |
|------|-----|
| cargo test | lib **3546** / e2e_test **201 active + 80 ignored** / i_d_pre 17 PASS + 5 ignored / i224_invariants 7 / i205_invariants 2 + 5 ignored / i399_isolation 2 + 3 ignored / 全 green |
| cargo clippy / fmt | 0 warnings / 0 diffs |
| file-line src | 0 violations (注: `tests/i224_invariants_test.rs` + `tests/e2e_test.rs` の project-wide policy 違反は I-176 entry で fix 予定) |
| audit-prd-rule10 / verify_prd_self_audits / audit-handoff-doc-line-refs | INV-4 3-tuple baseline (I-050 FAIL preserve / I-205 PASS / I-D-main PASS) / 0 drifts on I-D-main strict mode / 0 drifts on doc/handoff/ |
| Hono bench | clean **107** / errors **72** (Preservation 維持、bench 非決定性 ±1/±2 noise = [I-172]) |

I-D parent Iteration v1-v17 trajectory、Path B split rationale、3 path options 評価、bootstrap utility correctness ceiling 構造的解消の詳細は [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-D-pre: Audit mechanism bootstrap` section にアーカイブ済。

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
═════ 案 γ Phase 0: Framework quality integration (NEW、2026-05-09 順序入れ替え = 旧案 β I-D を Phase 1-C → Phase 0 へ前倒し、2026-05-11 Path B split で 2 PRD serial sequence 化) ═════
   ↓
[完了] PRD I-D-pre = **Audit mechanism bootstrap** — 2026-05-11 (= 5 cells matrix-driven、6 phases implementation、2 度 `/check_job` adversarial review、14 structural fixes / 0 patches、framework v1.8 self-applied integration、bootstrap utility formal lock-in + bootstrapping circularity 構造的解消)。詳細 = `doc/handoff/design-decisions.md` `## I-D-pre: ...` section
   ↓
═════ 案 γ Phase 0.5: Rules 徹底レビュー + 改善 batch (完了 2026-05-12) ═════
   ↓
[完了] **spec-stage-adversarial-checklist.md 整理** — 2026-05-12 (= Versioning 削除 + PRD-agnostic 化 + sub-rule 命名 (N-N) 統一 + Rule 11 (d-6) flatten + Rule 8 restructure + paths frontmatter 追加、50.5KB → 35.1KB)
   ↓
[完了] **`rule_review_list.md` 新設** — 2026-05-12 (= 8 group / 24 観点の汎用 Claude Code rule review checklist、project root に top-level 配置)
   ↓
[完了] **Rules 徹底レビュー + 改善 batch** — 2026-05-12 (= 20 rule files + 5 cross-system files (CLAUDE.md / plan.md / commands / skills / handoff) に 24 観点 適用。Critical: "N-rule" stale reference (10/12) → 13-rule 統一 8 file。High: A2 Versioning section 削除 4 file / A3 Lesson source の pattern essence 化 / B1+B2 instance+temporal citations 抽象 pattern essence 化 7 file。Medium: F1 terminology uniformity (Recurring problem rationale 統一)。Low: H2 paths frontmatter 追加 11 file (15/22 coverage、残り 7 = foundational intentional eager)。/check_job Layer 1 で 2 件 Implementation gap 即時 fix + /check_problem で 18 SKILL.md violations を TODO 起票 (deferred batch、L4 latent)。Counter-pattern 残存ゼロ confirmed)
   ↓
═════ 案 γ Phase 0 (継続): I-D-main spec stage 再開 (Rules 改善 base で着手) ═════
   ↓
[次] **PRD I-D-main = Framework rule integration cohesive batch (Path B split 後、24 cells)** (= post-bootstrap framework full leverage + rule corpus 改善 base で initial iteration convergence target で再開、I-D-pre 完了 prerequisite satisfy)。**起票時 primary reference** = TODO `[I-D-main]` entry + [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md) (1516 LOC、Iteration v1-v17 + v18 Path B split entry preserved) + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-D-pre: ...` + `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` sections
   ↓
═════ 案 γ Phase 0.5: lib/CLI API + Web API runtime integration (NEW、2026-05-10 PRD I-D scope split 由来) ═════
   ↓
[次候補] **PRD I-E = lib/CLI API + Web API runtime integration cohesive batch** (= I-D scope 分離由来 v13-2 + v13-3 candidates、orthogonal concern なので Phase 1/2 と並行可能)。Priority L3、起票 timing は I-D close 後または案 γ Phase 1/2 並行
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
[次] I-205 T15: /check_job 4-layer review + 13-rule self-applied verify (= I-D で強化された framework 適用)
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
| **次着手 (案 γ Phase 0、I-D-pre close 完了 2026-05-11 = bootstrap utility 完成 base + framework v1.8 leverage 可能、Rules 改善 batch 完了 2026-05-12 = rule corpus 品質 base 確立)** | L3 (framework rule level structural compliance) | **PRD I-D-main Framework rule integration cohesive batch (Path B split 後、24 cells)** | 24 framework rule integration cells (= I-D parent matrix # 1, 2, 3, 4, 5, 7, 9, 11, 12, 13, 14, 15, 16, 18, 20, 21, 22, 23, 24, 25, 26, 27, 29, 30、original numbers preserved with documented gaps {6, 8, 10, 17, 19, 28}) の cohesive integration、I-D-pre 完成 bootstrap utilities full leverage + rule corpus 改善 base で post-bootstrap framework state 確立、initial iteration convergence target で再開。**次 action**: first third-party adversarial review = Iteration v19 で convergence target、Hybrid 4-条件 final rule satisfy 到達なら Implementation stage 着手。詳細 = [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md) Iteration v18 entry + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-D-pre: ...` + `## I-224: ...` sections |
| **次候補 (案 γ Phase 0.5、I-D scope split 由来 NEW)** | L3 | **PRD I-E lib/CLI API + Web API runtime integration cohesive batch** | v13-2 (Promise builtin runtime integration deficiency) + v13-3 (transpile lib API vs CLI binary builtin loading inconsistency) cohesive batch。Spec stage で 2 candidates の真 cohesion verify + 必要なら 2 PRD split 判断、orthogonal concern のため案 γ Phase 1/2 並行可能 |
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
- **I-skills-commands-quality-review-batch** = Skills + Commands body への rule_review_list.md 24 観点 適用 batch (2026-05-12 Rules batch 由来 Review insight、L4 latent、案 γ Phase 0 完了後再評価)

---

## 直近の完了作業 (audit trail summary)

実装詳細は git log、設計判断 archive は [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD / Phase | 日付 | 後続への影響 |
|-------------|------|-------------|
| **[REFACTOR] Rules 徹底レビュー + 改善 batch 完了 (24 観点 全 file 適用 + cross-system inter-file relationship verify)** | 2026-05-12 | `rule_review_list.md` 24 観点 (8 group: A normative content / B project-agnostic / C reference integrity / D structural design / E content quality / F self-consistency / G completeness / H loading) を 20 rule files + 5 cross-system files (`CLAUDE.md` / `plan.md` / `.claude/commands/*.md` / `.claude/skills/*/SKILL.md` / `doc/handoff/`) に適用。**観点-major approach** = 24 観点ごとに 20 rule file を cross-cutting scan、violation matrix 構築 (= file-major approach より同種違反 coordinated fix が可能)。**Critical fix**: "N-rule" stale reference drift system-wide (現 13-rule に対し CLAUDE.md / commands/start.md "10-rule" / commands/check_job.md / skills/prd-template / skills/hono-cycle / rules/spec-first-prd / plan.md "12-rule" = 計 8 file × 13 reference) → 全 reference を current 13-rule + 正確な sub-rule label (= Rule 11 (11-1..11-6) / Rule 12 (12-1..12-8) / Rule 13 (13-1..13-6)) で再構築。**High fix**: A2 Versioning section 削除 4 file (check-job-review-layers.md / spec-first-prd.md / post-implementation-defect-classification.md / file-size-resolution.md) + A3 Lesson source / Worked Example の pattern essence 化 (project-specific incident citation 除去、transferable pattern は Recurring problem rationale として 抽象保全) + B1+B2 instance/temporal citation 全除去 7 file (check-job-review-layers.md / spec-first-prd.md / prd-completion.md / problem-space-analysis.md / file-size-resolution.md / pipeline-integrity.md / testing.md)。**Medium fix**: F1 terminology uniformity (`Recurring problem rationale` 統一)。**Low fix**: H2 paths frontmatter 追加 9 file (PRD-context: prd-design-review / post-implementation-defect-classification / prd-completion / spec-first-prd / problem-space-analysis / check-job-review-layers / TODO/Plan-context: todo-entry-standards / todo-prioritization / Src-code-context: file-size-resolution)。**Final state**: 24 観点 counter-pattern 残存ゼロ confirmed (A2/A3/B1/B2 grep 全 0、N-rule reference drift 全 fix、D1 sub-rule scheme (N-N) 統一済、G1/G3/G4 全 file presence ✓)。詳細 = git log `[REFACTOR] Rules 徹底レビュー + 改善 batch 完了` commit |
| **[REFACTOR] `.claude/rules/spec-stage-adversarial-checklist.md` 大幅整理 + `rule_review_list.md` 新設** | 2026-05-12 | spec-stage-adversarial-checklist.md (50,544 → 35,068 bytes、-31% / 603 → 449 行) = 5 段階 cleanup の cumulative result: (1) Versioning section 削除 (v1.0〜v1.8 change log、git history へ delegation) (2) PRD-agnostic 化 (I-205 / PRD 2.7 / Lesson source / Finding ID / 日付 / iteration marker 等の citation 全除去、transferable pattern は `Recurring problem rationale` / `Failure pattern` / `Rationale` として abstract 保全) (3) Tier 1+2+4 fixes (grammar fix、equivalence chain readability、placeholder 明示化、cross-reference 修正、Rule 7 sub-rule structure 追加、orphan prefix cross-ref semantic 化) (4) Sub-rule 命名 scheme `(N-N)` numeric 全 rule 統一 (Rule 8 mixed (a-d)+(8-5) restructure / Rule 9 (a-d) → (9-1〜4) / Rule 11 (d-N) → (11-N) / Rule 12 (e-N) → (12-N))、Rule 11 (d-6) triple-nesting → 2-level flatten (5) `paths` frontmatter 追加で on-demand load (PRD work 時のみ load)。**外部 file への coordinated rename** (5 files): `.claude/rules/check-job-review-layers.md` / `.claude/skills/prd-template/SKILL.md` / `doc/handoff/design-decisions.md` / `backlog/I-205-getter-setter-dispatch-framework.md` / `backlog/I-D-main-framework-rule-integration-cohesive-batch.md`。**NEW** [`rule_review_list.md`](rule_review_list.md) (12,949 bytes、260 行) = project root の汎用 Claude Code rule review checklist (8 group / 24 観点)、本 session の改善経験を抽象化 + 第三者目線 over-fit 検出 review 実施済。**次タスク**: rule_review_list.md を他 rule (`.claude/rules/*.md` の spec-stage-adversarial-checklist.md 以外) に適用、徹底レビューと改善 (new session で実施) |
| **[CLOSE] PRD I-D-pre 完了 = Audit mechanism bootstrap + bootstrapping circularity 構造的解消 + framework v1.8 self-applied integration 達成** | 2026-05-11 | **5 cells matrix-driven** (= I-D parent Cell 6+8/10/17/19/28 から migration) **の cohesive batch を 6 phases で完了**。**Phase 5 = T2-pre-1 + T2-pre-2 rule wording strengthening** = `.claude/rules/check-job-review-layers.md` Layer 1 sub-step (4) factual accuracy semantic check + `.claude/rules/spec-stage-adversarial-checklist.md` Rule 9 sub-rule (d) + Rule 13 sub-rule (13-6) cell numbering convention single-source-of-truth + Versioning v1.8 entry coordinated self-applied integration set。**2 度の `/check_job` adversarial review で 14 structural fixes / 0 patches** = framework rule-audit symmetry principle (Rule 13 (13-6-c)) 2 度独立 iteration で発動 empirical 自己実証 (= 1st /check_job で Path E utility 100-byte tolerance violation 発見 → strict byte-exact comparison 採用 / 2nd /check_job deep deep で I-D-pre 自身の Cell Numbering Convention contradictory wording 発見 → Conceptual identifier vs Written form 分離 fix)。Bootstrap utilities 完成 lock-in: `verify_line_refs.py` (Method A、PRD doc heading-based line-ref drift detection) + `verify_prd_self_audits.py` (Path E、4 axes + strict byte-exact、recursive self-audit structure、own + sibling utilities 全 4 件 byte claim auto-verify) + `audit-handoff-doc-line-refs.py` (handoff doc 4 drift categories、CI step PR merge gate active)。empirical state final: 17 i_d_pre tests PASS / 5 ignored (INV-5 retroactive verify placeholder) + INV-4 baseline 3-tuple (I-050 FAIL preserve / I-205 PASS / I-D-main PASS、I-D-pre は close で audit out-of-scope) + Path E 0 drifts on I-D-main + handoff audit 0 drifts on doc/handoff/ + cargo clippy 0 warnings + cargo fmt 0 diffs + Hono bench Preservation 107/72 (production code 0 LOC change)。**詳細 (削除禁止 lesson source)** = [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-D-pre: Audit mechanism bootstrap (Path B split adoption + bootstrapping circularity 構造的解消、closed 2026-05-11)` section (= 5 cells resolution table + 6 phases timeline + framework rule-audit symmetry empirical validation evidence + 14 structural fixes / 0 patches / recursive self-audit structure 完成 evidence + 4 active TODO entries spawned post-close)。次 action = **PRD I-D-main spec stage 再開** (= 24 cells、Iteration v19 で initial iteration convergence target、Hybrid 4-条件 final rule satisfy 到達なら Implementation stage 着手) |
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
| **I-178 + I-183 + Rule corpus optimization batch** | 2026-04-25 | matrix-driven PRD framework 整備 (Spec-stage adversarial checklist + 4-layer review + 5-category defect classification) |
| **I-161 + I-171 batch** (`&&=`/`\|\|=` desugar + Bang truthy emission) | 2026-04-22〜04-25 | I-177 umbrella 起票 (Tier 0 L1) |

---

## 次の PRD 着手前の参照ポイント

- **PRD I-D-main (Framework rule integration cohesive batch、I-D-pre close 完了 2026-05-11 = 着手 ready)**: I-205/I-225/I-162 PRD chain prerequisite、24 candidates (= I-D parent 30 - I-D-pre 5 logical cells migration、documented gaps {6, 8, 10, 17, 19, 28})。**現 state**: I-D-pre 完了で bootstrap utilities + framework v1.8 full leverage 可能、first third-party adversarial review = Iteration v19 で convergence target で再開。**Primary references**: [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md) (Iteration v18 = Path B split entry preserved) + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-D-pre: ...` + `## I-224: ...` sections
- **TODO 由来 deferred entries (I-D-pre 起源、L4 latent、案 γ Phase 0 完了後再評価)**:
  - `[I-D-future-vocab-fork]`: broader vocabulary fork detection (cell # / candidate ID / matrix # 間 semantic-level mixed canonical naming)
  - `[I-D-future-audit-extensions-hardening]`: 6 candidate classes cohesive batch (audit script extensions structural hardening)
  - `[I-D-future-self-applied-symmetry-audit]`: 新 framework rule author 時の既存 PRD body self-applied compliance mandatory verify (framework v1.9 candidate)
  - `[I-205-retroactive-cell-numbering-section]`: 案 γ Phase 2 T15 batch 化、I-205 PRD doc に `## Cell Numbering Convention` + `## Spec→Impl Mapping` section 追加 (= audit scope 内自動 promote)
- **I-225 / I-162 (案 γ Phase 1、I-D-main 完了後着手)**: TODO 内 entry + 案 γ chain
- **I-205 T14-T16 (案 γ Phase 2、I-225/I-162 完了後着手)**: [`backlog/I-205-getter-setter-dispatch-framework.md`](backlog/I-205-getter-setter-dispatch-framework.md) の T11 削除 + 新 PRD I-A/I-B migration 注記
- **Closed PRDs (I-D-pre / I-224 / I-399 / I-180)**: PRD doc は `backlog/` から削除済、git log audit trail (`[CLOSE] ...` commit) + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) framework lesson archive section から access
- **I-400 / I-401 / I-402 (新 PRD 起票候補)**: I-399 T4 /check_job Review insight 由来 follow-up improvements、test infra cluster sister として I-172 / I-397 と batch 化候補
- **PRD 3 (I-177 本体)**: matrix-driven、案 A (mutation-ref) vs 案 B (writeback) を spec stage で empirical 確定
- **Phase A Step 5/6/7**: 「開発ロードマップ」section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) (削除禁止 — 過去判断 + closed PRD lessons の primary reference)

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
- **Spec stage 完了 verification**: [`.claude/rules/spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) (13-rule)
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
