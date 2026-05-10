# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-05-10)

**進行中**: 案 γ Phase 0 = **PRD I-D Spec Stage Iteration v17 floor break** (= 8 third-party rounds 経過、trajectory v15:11 → v17:9 = -18% 初の floor 突破、user 方針相談 mandatory pending)。

**最新の完了**: I-224 (B2 fn main mechanism、Option β cohesive batch) close (2026-05-09)。詳細 = § 直近の完了作業 + git log `[CLOSE] I-224 PRD 完了` commit + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section。

**開発順序**: 案 γ (= I-D first → I-225 → I-162 → I-205 T14-T16)。詳細 = 下記「実行順序」section。

---

## PRD I-D Spec Stage 進捗 (2026-05-10 single session 累積)

**PRD doc**: [`backlog/I-D-framework-rule-integration-cohesive-batch.md`](backlog/I-D-framework-rule-integration-cohesive-batch.md) (1494 LOC、Iteration v1〜v17 history record 完了)

### Iteration trajectory (8 third-party rounds + 9 fix iterations)

| Round | Findings | Critical | High | Meta% | Convergence (C-1〜C-4) | Note |
|------:|---------:|---------:|-----:|------:|-----------------------|------|
| v3 | 17 | 6 | 5 | — | 0/4 FAIL | initial baseline |
| v5 | 9 | 1 | 4 | 56% | 0/4 FAIL | Phase 1 reduction -47% |
| v7 | 9 | 2 | 3 | 44% | 2/4 (C-3/C-4 PASS) | plateau |
| v9 | 11 | 3 | 5 | 45% | 0/4 FAIL | Phase 2 regression |
| v11 | 14 | 3 | 5 | 64% | 0/4 FAIL | regression peak |
| v13 | 11 | 3 | 4 | 27% | 2/4 PASS | **Method A reset** (Cell 19 bootstrap) |
| v15 | 11 | 2 | 4 | 27% | 2/4 PASS | plateau (NEW class emerge) |
| **v17** | **9** | **1** | **4** | **22%** | **2/4 PASS** | **Path E floor break** (Cell 10/17 + partial 6+8/28) |

**Trajectory shape**: v3-v7 rapid reduction → v9-v11 regression peak → v13-v15 Method A reset plateau → **v17 Path E floor break**。
3 round 連続 (v13/v15/v17) で 2/4 PASS = C-3 (trajectory diminishing) + C-4 (meta ratio ≤ 50%) PASS、C-1 (Critical=0) + C-2 (High=0) FAIL。

### Bootstrap utilities created in this session

| Utility | LOC | Cell coverage | Purpose |
|---------|----:|---------------|---------|
| [`scripts/verify_line_refs.py`](scripts/verify_line_refs.py) | 264 | Cell 19 (v11-7 line-ref factual accuracy) | Method A、Iteration v12 新設、heading-based line-ref drift detection |
| [`scripts/verify_prd_self_audits.py`](scripts/verify_prd_self_audits.py) | 368 | Cell 10/6+8/17/28 (multi-axis) | Path E、Iteration v16 新設、4 axes audit (cross-reference consistency / status pending verdict / label namespace collision / external file drift) |
| **Total bootstrap utilities** | **632** | **5/30 cells (Cell 19 ✓ + Cell 17 ✓ + Cell 10/6+8/28 partial)** | post-v16 multi-axis structural absorption |

### Cell coverage status (= 30 candidates のうち bootstrap 済 vs 未)

| Cell | Candidate | Bootstrap status | Coverage 度 |
|------|-----------|-----------------|-----------|
| 19 | v11-7 line-ref factual accuracy | ✓ | `verify_line_refs.py` で完全 absorption |
| 17 | v11-5 external file drift | ✓ | `verify_prd_self_audits.py` Axis 4 |
| 10 | v5-1 cross-reference cell consistency | ⚠ partial | Axis 1 threshold "5" arbitrary heuristic = under-detection (v17 F6) |
| 6+8 | v3-6/v4-2 status pending verdict | ⚠ partial | Axis 2 TS-X over-exclusion = under-detection (v17 F7) |
| 28 | v13-5 single-source-of-truth | ⚠ partial | Axis 3 R-x focus only、cell-slot vocabulary fork 未 cover |
| 残 25 cells | various rule wording / skill / command | ✗ 未 | Implementation stage T1-T8 lock-in 待ち |

### Audit state (post-v17)

- `audit-prd-rule10-compliance.py` PRD I-D doc: PASS exit 0 ✓
- `verify_prd_self_audits.py` PRD I-D doc: 0 CURRENT spec drifts across all 4 axes ✓
- INV-4 baseline 3-tuple preserved (I-050 FAIL pre-existing / I-205 PASS / I-D PASS) ✓

### 次着手 = user 方針相談 mandatory (3 path options pending、user clarification 中断)

v17 NOT-CONVERGED + 2/4 PASS (3 round 連続) のため user 指示 2026-05-10 v15 directive 再適用 = 独断 path 選択禁止、user 確認待ち:

#### **Path E+** (recommended) — Method A coverage extension 継続 + utility self-correctness 強化

- **Work**:
  - F6 fix: Path E Axis 1 threshold "5" arbitrary heuristic を spec-traceable allow-list に置換
  - F7 fix: Path E Axis 2 TS-X over-exclusion を post-v15 wording presence 要求に refine
  - Axis 5 (NEW): Layer 1-4 cross-cutting wording semantic verify (F1 class)
  - Axis 6 (NEW): triangulate spec wording staleness (F4 class)
  - Axis 7 (NEW): trajectory placeholder freshness (F5 class)
  - 9 v17 findings manual sweep + utilities re-run + v19 third-party review
- **期待 trajectory**: v17:9 → v19:5-7 → v21:0-3 = 2-4 rounds で convergence、1-2 時間
- **Pros**: proven bootstrap pattern、structural fix、I-D scope 内維持
- **Cons**: utility correctness ceiling = v19+ で plateau possibility 否定不能、+500-700 LOC accumulated

#### **Path B** — PRD I-D split into I-D-pre + I-D-main

- **Work**:
  - I-D-pre = 5 bootstrap cells (Cell 19 + 10 + 6+8 + 17 + 28) + audit utility extension = small-scope spec stage
  - I-D-main = 残 25 candidates (post-bootstrap framework full leverage 状態)
- **期待**: I-D-pre は ~3-5 cells、minimal cross-reference surface で convergence guaranteed
- **Pros**: 構造的に最 cohesive、bootstrapping problem 完全解消
- **Cons**: PRD 起票 1 件追加、cohesive batch boundary (user 確定 2026-05-10 = "30 candidates 単一 PRD") 再確認 mandatory、開発期間延伸

#### **Path F** — Convergence criterion 工学的 re-design (asymptotic floor acknowledgment)

- **Work**: Hybrid 4-条件 を asymptotic floor 込みで re-design (例: "Critical ≤ 1 + High ≤ 4 + 連続 3 round non-regression + meta-ratio < 25%")
- **判定**: 現 v17 状態で satisfy = 即時 Spec stage close 可能
- **Pros**: 即時 Implementation stage 着手可能、framework rules lock-in 後 v15 plateau 実態解消
- **Cons**: convergence criterion 緩和 = ideal-implementation-primacy 観点で user 判断必須 (= 妥協扱い?「asymptotic 数学的事実」受容?)

### 3rd-order pattern observation (= bootstrap utility correctness ceiling)

各 bootstrap utility が **次 round の dominant defect class を自ら生成** = utility-correctness ceiling = 各 utility は次 utility で audit する必要 = 無限 chain 構造。Path E+ で structural fix 続行 vs Path B で完全境界分離 vs Path F で数学的事実受容、いずれも user 判断必要。

### Resume instructions (新 session で /start)

1. **本 plan.md の本 section 「PRD I-D Spec Stage 進捗」確認** = trajectory + bootstrap utilities + 3 path options 把握
2. **`backlog/I-D-framework-rule-integration-cohesive-batch.md` Iteration v17 entry 確認** (= 最新 third-party review 9 findings detail + path options recommendation)
3. **user に方針確認**: Path E+ (recommended) / Path B (PRD split) / Path F (criterion re-design) / Other
4. user 確認後、採用 path で v18+ 実施 OR Spec stage close OR PRD split 起票
5. PRD doc + utilities (`verify_line_refs.py` + `verify_prd_self_audits.py`) + audit script (`audit-prd-rule10-compliance.py`) は本 session 内 modify、git status で確認可能 (un-commit)
6. 工数見積: Path E+ = 1-2 時間 / Path B = 数日 (PRD 起票 + 別 spec stage) / Path F = 30 分 (criterion 改訂)

### Quality Gate (post I-224 PRD close、2026-05-09)

| 指標 | 値 |
|------|-----|
| cargo test | lib **3546** / e2e_test **201 active + 80 ignored** / i224_invariants 7 / i224_helper 5 / i205_invariants 2 + 5 ignored / i205_helper 4 / i399_isolation_test 2 active + 3 ignored / 全 green |
| cargo clippy / fmt / file-line src | 0 warnings / 0 diffs / 全 src/.rs file < 1000 行 (注: `tests/i224_invariants_test.rs` 1502 行 + `tests/e2e_test.rs` 3022 行 = project-wide policy 違反、scripts/check-file-lines.sh は src/ scope = auto detect なし、I-176 entry 拡張済 = 案 γ Phase 0 後 test layout split refactor として fix 予定) |
| audit-prd-rule10 / audit-no-pub-fn-init / audit-no-init-call-site | PASS / exit=0 (INV-4 + INV-7 CI merge gate 維持) |
| Hono bench | clean **107** / errors **72** at SHA-pinned 027e3df (Preservation classification 維持) |

**bench 非決定性**: ±1 clean / ±2 errors の noise variance を [I-172] として記録 (test/bench infra defect、別 PRD)。

---

## /start 再開時の手順 (= PRD I-D Spec Stage Iteration v17 plateau からの resumption)

### Step 1: 状態確認
1. **本 plan.md「PRD I-D Spec Stage 進捗」section** = trajectory + bootstrap utilities + 3 path options 把握 (上記 § PRD I-D Spec Stage 進捗 参照)
2. **[`backlog/I-D-framework-rule-integration-cohesive-batch.md`](backlog/I-D-framework-rule-integration-cohesive-batch.md) Iteration v17 entry** = 最新 third-party review 9 findings detail + 3 path options recommendation 確認
3. **`scripts/verify_line_refs.py` + `scripts/verify_prd_self_audits.py`** = bootstrap utilities (Method A + Path E)、本 session で新設、git untracked
4. **TODO `[I-D]` entry** = 30 framework 改善 candidates 全列挙 (= 本 PRD scope 内 cells 1-30)
5. **closed PRDs** (I-224 / I-399 / I-180): `backlog/` から削除済、git log + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) からアクセス

### Step 2: User 方針確認 (= mandatory before v18+)

PRD I-D Spec stage は **Iteration v17 で NOT-CONVERGED + 2/4 PASS (3 round 連続)**。User 指示 2026-05-10 v15 directive 再適用 = **独断 path 選択禁止、user 方針確認後採用**。

3 path options 提示:

- **Path E+** (recommended): Method A coverage extension 継続 + utility self-correctness 強化 (F6/F7 fix + Axis 5/6/7 追加) + 9 findings manual sweep + v19 verify。期待 convergence: 2-4 rounds (1-2 時間)
- **Path B**: PRD I-D split into I-D-pre (= 5 bootstrap cells) + I-D-main (= 残 25 cells)。bootstrapping problem 完全構造的解消、cohesive batch boundary 再確認 mandatory
- **Path F**: Convergence criterion 工学的 re-design (asymptotic floor acknowledgment)。現 v17 状態で satisfy = 即時 Spec stage close 可能、user 判断 (妥協扱いか asymptotic 数学的事実受容か)

### Step 3: 採用 path で実施

#### Path E+ 採用時:
1. `scripts/verify_prd_self_audits.py` の F6/F7 fix + Axis 5/6/7 追加 (~100-150 LOC)
2. PRD doc に対し utilities re-run、検出 drifts auto-fix
3. v17 9 findings (F1〜F9) manual sweep with Method G discipline
4. Iteration v18 fix log + Iteration v19 third-party adversarial review dispatch
5. v19 結果評価: convergence なら Spec stage close、未達なら user 再相談

#### Path B 採用時:
1. cohesive batch boundary 再確認 (= 5 bootstrap cells で I-D-pre split の妥当性 user 確認)
2. PRD I-D-pre 新規起票 (= bootstrap cells のみ、small-scope spec stage)
3. 現 PRD I-D を I-D-main に rename + scope reduce (= 残 25 candidates)
4. I-D-pre spec stage → Implementation stage で utilities formal lock-in
5. I-D-main spec stage を bootstrapped framework full leverage 状態で再開

#### Path F 採用時:
1. `.claude/rules/check-job-review-layers.md` Cell 30 spec を asymptotic floor 込みで revise
2. 新 convergence criterion で Iteration v17 状態 verify (= "Critical ≤ 1 + High ≤ 4 + 3 round non-regression + meta < 25%" 等で satisfy 確認)
3. PRD doc Spec Stage Tasks status を全 COMPLETE 同期 + Iteration v18 entry で convergence 確定 record
4. Implementation stage 着手準備 (T1-T8 task execution)

### Step 4: 後続 prerequisite chain (案 γ Phase 1〜)
PRD I-D Spec stage close + Implementation stage 完了後、案 γ Phase 1 (= I-225 → I-162 → I-205 T14-T16) 着手。詳細 = 下記「実行順序」section + 「次の作業 table」。

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
[次] **PRD I-D = Framework rule integration cohesive batch** (= 30 candidates / 14 rounds adversarial review 累積 (当初 32 件 - v13-2/v13-3 PRD I-E migrate 2026-05-10)、4 度連続 v12-2 pattern 構造的解消)。**起票時 primary reference** = TODO `[I-D]` entry + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section
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
| **進行中 (案 γ Phase 0)** | L4 (framework quality) | **PRD I-D Framework rule integration cohesive batch (Spec Stage Iteration v17 plateau)** | 30 framework 改善 candidates の cohesive integration、Spec Stage で 8 third-party rounds 経過、v17 で trajectory floor break (11→9)、2/4 PASS 3 round 連続。**現状**: Path E bootstrap (= verify_line_refs.py 264 LOC + verify_prd_self_audits.py 368 LOC) で Cell 19/17 完全 absorb + Cell 10/6+8/28 partial absorb。**次 action**: user 方針確認 (Path E+ recommended / Path B PRD split / Path F criterion re-design) → 採用 path で v18+ 実施 OR Spec stage close。詳細 = 上記「PRD I-D Spec Stage 進捗」section + [`backlog/I-D-framework-rule-integration-cohesive-batch.md`](backlog/I-D-framework-rule-integration-cohesive-batch.md) Iteration v17 entry |
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

---

## 直近の完了作業 (audit trail summary)

実装詳細は git log、設計判断 archive は [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD / Phase | 日付 | 後続への影響 |
|-------------|------|-------------|
| **PRD I-D Spec Stage Iteration v1-v17 progress (= 8 third-party rounds + 9 fix iterations + 2 bootstrap utilities 632 LOC)** | 2026-05-10 (single session 累積) | PRD doc 1494 LOC、Iteration v17 で trajectory floor break (11→9、-18%、Critical 半減 2→1、meta 22% history 最低)。Method A bootstrap (Cell 19 verify_line_refs.py) で line-ref drift class 完全 absorption + Path E bootstrap (Cell 10/6+8/17/28 verify_prd_self_audits.py 4 axes) で multi-axis partial absorption。Spec stage close 未到達 (2/4 PASS 3 round 連続)、user 方針相談 mandatory pending = Path E+ / Path B / Path F option 提示。詳細 = 上記「PRD I-D Spec Stage 進捗」section + [`backlog/I-D-framework-rule-integration-cohesive-batch.md`](backlog/I-D-framework-rule-integration-cohesive-batch.md) Iteration v17 entry |
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

- **PRD I-D (Framework rule integration cohesive batch、進行中 = 案 γ Phase 0、Spec Stage Iteration v17 plateau)**: I-205/I-225/I-162 PRD chain prerequisite、30 candidates。**現 state**: Spec stage で 8 third-party rounds 経過、v17 trajectory floor break (11→9)、2/4 PASS 3 round 連続、user 方針相談 pending。**Primary references**: 本 plan.md「PRD I-D Spec Stage 進捗」section (= trajectory + bootstrap utilities + 3 path options) + [`backlog/I-D-framework-rule-integration-cohesive-batch.md`](backlog/I-D-framework-rule-integration-cohesive-batch.md) Iteration v17 entry + [`scripts/verify_line_refs.py`](scripts/verify_line_refs.py) (Method A、Cell 19) + [`scripts/verify_prd_self_audits.py`](scripts/verify_prd_self_audits.py) (Path E、Cell 10/6+8/17/28)。**Cohesive batch boundary** (= 単一 PRD or sub-domain split) は user 確定 2026-05-10 = "30 candidates 単一 PRD"、Path B 採用時のみ再確認
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
