# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-05-11、Path B split adoption + I-D-pre Implementation Phase 1+2+3 完了 + /check_job A1-A9 ideal-clean fix + Iteration v3 Option α design decision lock-in + **Phase 3 audit script extensions 完了 + /check_job 4-layer review pass (= C1 branch coverage 達成)** = Phase 4 着手 ready)

**進行中**: 案 γ Phase 0 = **PRD I-D-pre Implementation Phase 4 着手 ready (= audit-handoff-doc-line-refs.py 新設 + CI integration)** (= Phase 1 test infra setup + Phase 2 bootstrap utility formal lock-in + /check_job A1-A9 fix + Iteration v3 baseline + Option α decision + **Phase 3 audit script extensions 完了 + 4-layer review pass**: 3 NEW verify functions + helper `has_cell_numbering_convention_section()` + formatter `_format_path_e_drifts()` を `scripts/audit-prd-rule10-compliance.py` に embed、Path E utility を library import で source of truth 共有、4 test stubs `#[ignore]` 解除 + 6 fixtures 新設 + /check_job 4-layer review で I1 (Helper False branch test 不在) 即時 fix (= I-205-like fixture + False branch test 追加) で **5/5 tests PASS** + C1 branch coverage 達成、残 4 findings (I2/R1/R2/R3) は L4 latent TODO `[I-D-future-audit-extensions-hardening]` cohesive batch lock-in、INV-4 4-tuple baseline preserved、Path E drifts 0 maintained)。

**Phase 1+2 完了 state**:
- ✓ **Phase 1**: 6 test stub files + tests/fixtures/i_d_pre/ infrastructure setup
- ✓ **Phase 2.1 (T1-pre-5)**: `scripts/verify_line_refs.py` Method A formal lock-in (metadata header + 2 tests passing + 2 fixtures)
- ✓ **Phase 2.2 (T1-pre-6)**: `scripts/verify_prd_self_audits.py` Path E formal lock-in + F6/F7/Axis 3 fix + 4 additive utility fixes (= find_section_range bug fix + find_repo_root + IMPACT_AREA_BYTES_RE extension + expand_cell_list multi-pattern + SECTION_COVERAGE_POLICY 5 sections coverage + STALE_STATUS_PATTERNS legacy dead code 削除) (5 tests passing + 8 fixtures)
- ✓ **/check_job A1-A9 fix**: 9 findings (Spec gap 3 + Implementation gap 2 + Low 4) 全 fix、Iteration v2 entry retroactive embed、ideal-clean 達成

**Phase 3 着手前 baseline analysis + Option α design decision (2026-05-11 確定、Iteration v3 entry retroactive embed)**:

3 real PRDs (I-D-pre / I-D-main / I-205) の cell appearance 分布 + Spec→Impl Mapping section presence + `## Cell Numbering Convention` section presence を empirical 測定 (= `/tmp/phase3_baseline_analysis.py` + structural grep):

| PRD | matrix cells | range | documented gaps | Spec→Impl Mapping | Cell Numbering Convention | Path E drifts | audit-prd-rule10 exit |
|-----|--------------|-------|----------------|--------------------|--------------------------|--------------:|----------------------|
| I-D-pre | 5 | 1-5 | 0 (contiguous) | ✓ 1-to-1 alignment | ✓ 存在 | 0 ✓ | 0 (PASS) |
| I-D-main | 30 | 1-30 | 0 (contiguous) | ✓ 1-to-1 alignment | ✓ 存在 | 0 ✓ | 0 (PASS) |
| I-205 | 60 | 1-81 | **{29, 34, 35, 41, 45, 48-49, 51-59, 65-69}** = 21 cells skipped | **不在** | **不在** | **2 drifts** (Axis 1 既知 false-positive) | 0 (PASS、Rule 10/11/12 + 4 (4-3) は別 logic で satisfy) |

**Option α design decision (user 確定 2026-05-11)** = audit script 側 mirror (T1-pre-1 + T1-pre-2 + T1-pre-4 で実装) は `## Cell Numbering Convention` section 有無で auto-detect、I-205 は audit out-of-scope:

- **Auto-detect helper** = `has_cell_numbering_convention_section(content)` (= `^##\s+Cell Numbering Convention\b` regex)、3 NEW verify functions の冒頭で early-return → I-205 false-positive 解消 + INV-4 4-tuple baseline preserve
- **Cohesion 純化**: I-205 PRD doc は touch しない (= 1 PRD = 1 architectural concern)、I-D-pre Iteration v3 entry + TODO `[I-205-retroactive-cell-numbering-section]` で I-205 retroactive intent spec-traceable lock-in
- **I-205 retroactive update intent (= 案 γ Phase 2 T15 batch 化)**: I-205 PRD doc に `## Cell Numbering Convention` + `## Spec→Impl Mapping` section 追加 = framework rule retroactive compliance。section 追加で audit scope 内に自動 promote (= helper True 判定 = future-proof design)
- **Rejected options**: Option β (Path E utility 側修正、framework rule structural enforcement 弱化 = ideal-implementation-primacy 違反)、Option γ (audit script 独立 design、DRY 違反)、Option α-batched (cohesion violation + scope creep)

**次着手** = **PRD I-D-pre Implementation Phase 3 = 3 NEW verify functions + helper + 6 fixtures + 4 test stubs `#[ignore]` 解除**。詳細 = 下記「/start 再開時の手順」Step 3 + I-D-pre PRD doc `## Implementation Stage Tasks > T1-pre-1〜T1-pre-4` section + Iteration v3 entry。

**最新の完了**: PRD I-D-pre Phase 1+2 + /check_job A1-A9 fix + Phase 3 着手前 baseline analysis (2026-05-11)。前回 PRD close = I-224 (B2 fn main mechanism、Option β cohesive batch) close (2026-05-09)。詳細 = § 直近の完了作業。

**開発順序**: 案 γ (= **I-D-pre Phase 3-6 → I-D-main → I-225 → I-162 → I-205 T14-T16**、Path B split で I-D を 2 PRD serial sequence に展開)。詳細 = 下記「実行順序」section。

---

## PRD I-D Path B split adoption (2026-05-11) + I-D parent Spec Stage 進捗 (2026-05-10 single session 累積)

**Path B split adoption (2026-05-11 user 確定)**:
- **PRD I-D-pre** (NEW、`backlog/I-D-pre-audit-mechanism-bootstrap.md`、~660 LOC): 5 audit mechanism cells (= I-D parent Cell 6+8/10/17/19/28、6 row numbers) を migration、bootstrap utility formal lock-in scope、案 γ Phase 0 prerequisite
- **PRD I-D-main** (renamed from I-D parent、`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`、~1516 LOC): 24 framework rule integration cells (= I-D parent matrix # 1, 2, 3, 4, 5, 7, 9, 11, 12, 13, 14, 15, 16, 18, 20, 21, 22, 23, 24, 25, 26, 27, 29, 30、original cell numbers preserved with documented gaps {6, 8, 10, 17, 19, 28} per Cell 28 v13-5 single-source-of-truth principle)、post-bootstrap framework full leverage scope、I-D-pre 完了後 initial iteration convergence target で再開
- **両 PRD audit pass**: `python3 scripts/audit-prd-rule10-compliance.py backlog/I-D-{pre,main}-*.md` exit 0 ✓ (= INV-4 4-tuple baseline: I-050 FAIL preserve / I-205 PASS / I-D-pre PASS / I-D-main PASS)

**Path B split rationale (= ideal-implementation-primacy + 妥協禁止 directive 適用結果)**:
- **3 path options 評価**: Path E+ (continue) = utility correctness ceiling = 無限 chain 継続 = 妥協 → rejected / Path F (criterion re-design) = asymptotic floor 受容 = explicit compromise → rejected / **Path B (PRD split)** = bootstrapping circularity 構造的解消 + 1 PRD = 1 architectural concern 原則準拠 → **accepted**
- **Cohesion principle 適合 evidence**: 5 audit mechanism cells と 24 rule integration cells が異なる architectural concern (= memory `feedback_prd_cohesion_granularity.md` 整合)、既に I-E split (2026-05-10) も同 framework 由来

**PRD I-D parent (split source) 進捗 (= preserve as historical evidence)**:

**PRD doc**: [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md) (旧 I-D parent rename、1516 LOC、Iteration v1〜v17 history + v18 Path B split entry record 完了)

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

### 次着手 = PRD I-D-pre Implementation stage (Path B split adoption 後、user 確定 2026-05-11)

**Path B 採用済 2026-05-11**: 下記 3 path options のうち Path B (PRD split) を user 採用、I-D-pre + I-D-main 2 PRD serial sequence で構造的解消。

**次の作業** = PRD I-D-pre Implementation stage 着手 = 6 sub-tasks (T1-pre-1〜T1-pre-6 = audit script extensions + utility formal lock-in) + 2 sub-tasks (T2-pre-1/T2-pre-2 = rule wording strengthening) + T6/T7/T8 = 計 ~10 tasks。完了 = bootstrap utility formal lock-in + I-D-main spec stage initial iteration convergence target で再開可能化。

**Historical record (= Path B 採用前 user 確認時の 3 path options、accepted = Path B)**:

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

## /start 再開時の手順 (= PRD I-D-pre Phase 1+2+3 + /check_job 4-layer + deep deep review + /check_problem 全 pass 完了後 = Phase 4 着手 ready)

### Step 1: 現在の state empirical verify (= sanity check、Phase 3 + all reviews 完了 state preservation)

**baseline verification commands** (= /start 直後に必ず run):

```bash
# 1. 全 i_d_pre tests (Method A 2 + Path E 5 + Audit ext 6 = 13 PASS / Phase 4-6 stubs 9 ignored)
cargo test --tests --no-fail-fast 2>&1 | grep -E "i_d_pre|test result"
# Expected (各 test file):
#   - i_d_pre_audit_extensions_test: 6 passed (True branch 4 + False branch 1 + dual verify 1)
#   - i_d_pre_handoff_audit_test:    0 passed; 0 failed; 2 ignored (Phase 4 で fill in)
#   - i_d_pre_invariants_test:       0 passed; 0 failed; 5 ignored (Phase 6 で fill in)
#   - i_d_pre_method_a_test:         2 passed (Phase 2.1 完了 evidence、保持)
#   - i_d_pre_path_e_test:           5 passed (Phase 2.2 完了 evidence、保持)
#   - i_d_pre_rule_wording_test:     0 passed; 0 failed; 2 ignored (Phase 5 で fill in)

# 2. INV-4 4-tuple baseline preservation (audit-prd-rule10-compliance.py)
for prd in backlog/I-050-any-coercion-umbrella.md backlog/I-205-getter-setter-dispatch-framework.md backlog/I-D-pre-audit-mechanism-bootstrap.md backlog/I-D-main-framework-rule-integration-cohesive-batch.md; do
    python3 scripts/audit-prd-rule10-compliance.py "$prd" 2>&1 | head -1
done
# Expected:
#   - I-050: FAIL: 1 compliance violation(s):     ← pre-existing baseline preserve
#   - I-205: PASS                                  ← Option α auto-detect で audit out-of-scope (= 新 verify functions skip、既存 framework rules で PASS)
#   - I-D-pre: PASS                                ← Phase 3 完了 evidence (= 自己 audit PASS、INV-1 evidence)
#   - I-D-main: PASS                               ← Path B split rename + Impact Area byte claim sync 維持

# 3. Path E 0 drifts (= ideal-clean state 維持 post Phase 3 + deep deep review fix)
for prd in backlog/I-D-pre-audit-mechanism-bootstrap.md backlog/I-D-main-framework-rule-integration-cohesive-batch.md; do
    python3 scripts/verify_prd_self_audits.py "$prd" 2>&1 | grep "^Total drifts"
done
# Expected: I-D-pre 0 drifts / I-D-main 0 drifts (= byte claim 44451 sync 済)

# 4. cargo clippy / fmt (clean state preserved)
cargo clippy --all-targets --all-features -- -D warnings  # exit 0、0 warnings
cargo fmt --all --check                                    # exit 0、0 diffs

# 5. No mechanical suppression (= deep deep review I3 fix preserved)
grep -n "noqa" scripts/audit-prd-rule10-compliance.py
# Expected: (no output) = `# noqa` suppression 0 件
```

**1 つでも expected 不一致 = state mismatch、Phase 4 着手前に root cause 調査**。

### Step 2: 主要 reference docs 読込 (= Phase 4 着手前 context 把握)

1. **本 plan.md「直近の完了作業」 table** = Phase 3 + 4-layer + deep deep + check_problem 全 pass entry (2026-05-11) で完了 scope + ideal-clean 達成状態 把握
2. **[`backlog/I-D-pre-audit-mechanism-bootstrap.md`](backlog/I-D-pre-audit-mechanism-bootstrap.md)** = 5 cells PRD doc:
   - `## Spec Review Iteration Log > Iteration v3` (= Option α design decision + Phase 3 完了 + /check_job 4-layer + deep deep review + Action Item fix retroactive embed log)
   - `## Implementation Stage Tasks > T1-pre-3a / T1-pre-3b` = **Phase 4 spec** (= 次着手 = `scripts/audit-handoff-doc-line-refs.py` 新設 + CI integration)
   - `## Test Plan > Test category 5 (handoff_audit_test)` (= Phase 4 fixture + test pattern reference)
   - `## Implementation Stage Tasks > T2-pre-1 / T2-pre-2` = **Phase 5 spec** (= Phase 4 後 = rule wording strengthening)
3. **[`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md)** = 24 cells PRD doc (Iteration v18 = Path B split entry preserved、I-D-pre 完了後 spec stage 再開 = WAITING state)
4. **TODO entries**:
   - `[I-D-pre]` = 5 cells full enumeration + iteration history audit trail
   - `[I-D-main]` = 24 cells + iteration v1-v17 + v18 history
   - `[I-D-future-vocab-fork]` = broader vocabulary fork detection deferred (L4 latent)
   - **`[I-D-future-audit-extensions-hardening]`** = Phase 3 /check_job 4-layer + deep deep review 由来 4 candidate classes (C1=Path E API stability / C2=byte-exact invariant / C3=scripts/ file-size policy / C4=Closed PRD test) cohesive batch (L4 latent)
   - **`[I-205-retroactive-cell-numbering-section]`** = Phase 3 Option α decision 由来、案 γ Phase 2 T15 batch 化、I-205 PRD doc に `## Cell Numbering Convention` + `## Spec→Impl Mapping` section 追加で audit scope 内自動 promote (= future-proof design)
5. **closed PRDs** (I-224 / I-399 / I-180): `backlog/` から削除済、git log + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) からアクセス

### Step 3: PRD I-D-pre Implementation Phase 4 着手 (= 次 action = handoff audit script + CI integration)

**Implementation tasks roadmap** = ~10 tasks across **6 phases** (各 phase 完了で incremental commit、TDD discipline 適用):

#### ✓ Phase 1: Test infrastructure setup (COMPLETE 2026-05-11)
- 6 test stub files (`tests/i_d_pre_*.rs`) + `tests/fixtures/i_d_pre/{positive,negative}/` infrastructure setup

#### ✓ Phase 2: Bootstrap utility formal lock-in (COMPLETE 2026-05-11、post /check_job A1-A9 fix)
- **Phase 2.1 (T1-pre-5)**: `scripts/verify_line_refs.py` Method A formal lock-in (metadata header + 2 tests passing + 2 fixtures)
- **Phase 2.2 (T1-pre-6)**: `scripts/verify_prd_self_audits.py` Path E formal lock-in + F6/F7 fix + Axis 3 extension + 4 additive utility fixes (5 tests passing + 8 fixtures)
- **/check_job A1-A9 fix**: 9 findings retroactive Iteration v2 embed = ideal-clean 達成

#### ✓ Phase 3: Audit script extensions (COMPLETE 2026-05-11、T1-pre-1 + T1-pre-2 + T1-pre-4、3 NEW functions + helper + formatter in `scripts/audit-prd-rule10-compliance.py`)
- **T1-pre-1**: `verify_pending_verdict_findings_consistency` consolidated audit function (= v3-6 + v4-2 cohesive batch + F7 fix integrated、Cell 1)
- **T1-pre-2**: `verify_cross_reference_cell_consistency` audit function (= F6 fix integrated、Cell 2)
- **T1-pre-4**: `verify_cell_numbering_drift_detection` audit function (Cell 5 / v13-5 audit part)
- **Helper**: `has_cell_numbering_convention_section()` (= Option α auto-detect、I-205 audit out-of-scope 自動分類)
- **Formatter**: `_format_path_e_drifts()` (= Path E utility drift output を audit script convention 形式に変換)
- **DRY 達成**: Path E utility を **proper top-level library import** で source of truth 共有 (= deep deep review I3 fix で sys.path.insert + `# noqa: E402` suppression 排除、PEP 8 compliant top-level import)
- **Tests**: 6 PASS (True branch 4 + False branch 1 helper False auto-detect + dual verify 1 fixture violation patterns presence via Path E) + 7 fixtures (positive 3 + negative 3 + out-of-scope 1)
- **Reviews pass**: /check_job 4-layer (= Action Item I1 即時 fix) + /check_job deep deep (= I3 mechanical compromise 排除 + I4 dual verify lock-in) + /check_problem (= TODO C4 candidate embed)

#### ★ Phase 4: NEW audit script + CI integration (NEXT、T1-pre-3a + T1-pre-3b、Cell 3 / v11-5)
- **Step 4.1 (T1-pre-3a)**: `scripts/audit-handoff-doc-line-refs.py` (NEW、~150 行) 新設 (= handoff doc grep `\.rs:\d+` + actual file existence + line content syntactic verify)
  - **Test stub**: `tests/i_d_pre_handoff_audit_test.rs` 2 stubs (現在 `#[ignore]`、Phase 4 で `#[ignore]` 解除 + assertion fill in)
  - **Fixture 新設**: handoff doc synthetic fixtures (positive = drift 含む / negative = drift 不在)
- **Step 4.2 (T1-pre-3b)**: `.github/workflows/ci.yml` CI step integration (= PR merge gate active 化)
- **Phase 4 完了基準**: `tests/i_d_pre_handoff_audit_test.rs` 2 stubs `#[ignore]` 解除 + assertion fill in PASS + standalone `python3 scripts/audit-handoff-doc-line-refs.py doc/handoff/` で existing handoff doc に対し PASS or detected drift report + INV-4 4-tuple baseline preserved + cargo clippy/fmt clean
- **Phase 4 implementation 注意**: Phase 3 で確立した test helper pattern (= `run_audit` + `path_e_total_drifts` subprocess invocation helpers in `tests/i_d_pre_audit_extensions_test.rs`) と同 pattern を `tests/i_d_pre_handoff_audit_test.rs` でも適用、必要なら 3 scripts 経由共通 helper への refactor 検討 (= L4 latent、code dup ではなく knowledge dup 観点で評価)

#### Phase 5: Rule wording strengthening (T2-pre-1 + T2-pre-2、Phase 2-3 完了後)
- **Step 5.1 (T2-pre-1)**: `.claude/rules/check-job-review-layers.md` Layer 1 (Mechanical) sub-step 追加 (= factual accuracy semantic check、Cell 4)、Versioning section v1.8 entry
- **Step 5.2 (T2-pre-2)**: `.claude/rules/spec-stage-adversarial-checklist.md` Rule 9 / Rule 13 sub-rule 追加 (= Cell numbering convention single-source-of-truth、Cell 5)、Versioning section v1.8 entry
- **完了 verify**: `tests/i_d_pre_rule_wording_test.rs` 全 PASS (grep-based assertion で specific text pattern 存在 verify)

#### Phase 6: Compliance + final verify + close (T6-pre + T7-pre + T8-pre、Phase 3-5 完了後)
- **Step 6.1 (T6-pre)**: 既存 PRD docs に対する 4-tuple INV-4 baseline-aware delta-based regression 0 verify
- **Step 6.2 (T7-pre)**: 本 PRD doc 自身に対する self-applied + third-party `/check_job` invocation chain (= simplified Critical=0/High=0 convergence、5 cells small-scope)、`## Spec Review Iteration Log` v3+ record
- **Step 6.3 (T8-pre)**: `doc/handoff/design-decisions.md` に I-D-pre lessons section embed + plan.md update (= 案 γ Phase 0 = I-D-main 着手 ready) + `[CLOSE] I-D-pre PRD 完了` commit message proposal
- **完了 verify**: `tests/i_d_pre_invariants_test.rs` 全 PASS (INV-1 〜 INV-5、INV-5 は I-D-main retroactive verify のため `#[ignore]` placeholder)

**PRD I-D-pre 全完了基準**: `## Completion Criteria` 全 satisfy (= 5 cells matrix completeness + bootstrap utility formal lock-in + self-applied integration + CI integration + INV-4 4-tuple baseline + INV-5 retroactive verify)。

### Step 4: PRD I-D-main spec stage 再開 (I-D-pre 完了後)

I-D-pre 完了 = bootstrap utility 完成済 base 確立 = I-D-main spec stage initial iteration convergence target で再開:
1. `backlog/I-D-main-framework-rule-integration-cohesive-batch.md` 最新 spec state 確認 (= Iteration v18 entry + Path B split scope 24 cells)
2. Implementation stage tasks (T1-T8) 着手前に first third-party adversarial review (= Iteration v19) 実施、Hybrid 4-条件 final rule で convergence target
3. convergence 達成 = Implementation stage 着手、未達 = recursive iteration

### Step 5: 後続 prerequisite chain (案 γ Phase 1〜)
PRD I-D-pre + I-D-main 両 close + Implementation stage 完了後、案 γ Phase 1 (= I-225 → I-162 → I-205 T14-T16) 着手。詳細 = 下記「実行順序」section + 「次の作業 table」。

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
[進行中、Phase 3 着手 ready] **PRD I-D-pre = Audit mechanism bootstrap (Path B split user 確定 2026-05-11、5 cells)** (= Iteration v17 plateau の bootstrapping problem 構造的解消、`scripts/verify_line_refs.py` (Method A) + `scripts/verify_prd_self_audits.py` (Path E、F6/F7 fix integrated + Axis 3 extension + 4 additive utility fixes) + `scripts/audit-handoff-doc-line-refs.py` (Phase 4 NEW) を formal regression-tested utilities として lock-in)。**現 state (2026-05-11)**: ✓ Phase 1 (test infra) + ✓ Phase 2 (Method A + Path E formal lock-in、7 tests PASS、ideal-clean state 達成 post /check_job A1-A9 fix)、Phase 3 (= audit script extensions = 3 NEW functions in audit-prd-rule10-compliance.py) 着手 ready。**Primary reference** = TODO `[I-D-pre]` entry + [`backlog/I-D-pre-audit-mechanism-bootstrap.md`](backlog/I-D-pre-audit-mechanism-bootstrap.md) (~770 LOC post v2)
   ↓
[次] **PRD I-D-main = Framework rule integration cohesive batch (Path B split 後、24 cells)** (= post-bootstrap framework full leverage で initial iteration convergence target で再開、I-D-pre 完了 prerequisite)。**起票時 primary reference** = TODO `[I-D-main]` entry + [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md) (1516 LOC、Iteration v1-v17 + v18 Path B split entry preserved) + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section
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
| **次着手 (案 γ Phase 0、Phase 1+2+3 完了、Phase 4 着手 ready)** | L3 (audit mechanism construction) | **PRD I-D-pre Audit mechanism bootstrap (Phase 4 着手 ready 2026-05-11)** | 5 audit mechanism cells (= I-D parent Cell 6+8/10/17/19/28 から migration) の cohesive batch。**完了済**: Phase 1 (test infra 6 files + fixtures dir) + Phase 2.1 (Method A formal lock-in、`scripts/verify_line_refs.py` metadata + 2 tests passing) + Phase 2.2 (Path E formal lock-in、`scripts/verify_prd_self_audits.py` F6/F7/Axis 3 fix + 4 additive utility fixes、5 tests passing) + Iteration v2 (= /check_job A1-A9 fix retroactive embed、ideal-clean 達成) + Iteration v3 (Option α design decision lock-in) + **Phase 3 (= 3 NEW verify functions + helper `has_cell_numbering_convention_section()` + formatter `_format_path_e_drifts()` + Path E utility library import + 4 tests PASS + 6 fixtures)**。**次 action (Phase 4)**: T1-pre-3a + T1-pre-3b = `scripts/audit-handoff-doc-line-refs.py` (NEW、~150 行) 新設 + `.github/workflows/ci.yml` CI step integration (= handoff doc line-ref drift detection、Cell 3 / v11-5)、`tests/i_d_pre_handoff_audit_test.rs` 2 stubs `#[ignore]` 解除。詳細 = [`backlog/I-D-pre-audit-mechanism-bootstrap.md`](backlog/I-D-pre-audit-mechanism-bootstrap.md) Iteration v3 entry + `## Implementation Stage Tasks > T1-pre-3a/T1-pre-3b` |
| **次々候補 (案 γ Phase 0、I-D-pre 完了後再開)** | L3 (framework rule level structural compliance) | **PRD I-D-main Framework rule integration cohesive batch (Path B split 後、24 cells、I-D-pre 完了 prerequisite 待ち)** | 24 framework rule integration cells (= I-D parent matrix # 1, 2, 3, 4, 5, 7, 9, 11, 12, 13, 14, 15, 16, 18, 20, 21, 22, 23, 24, 25, 26, 27, 29, 30、original numbers preserved with documented gaps {6, 8, 10, 17, 19, 28}) の cohesive integration、I-D-pre 完成 bootstrap utilities full leverage で post-bootstrap framework state 確立後、initial iteration convergence target で再開。**現状**: WAITING for I-D-pre completion。**次 action**: I-D-pre close 後 first third-party adversarial review = Iteration v19 で convergence target、Hybrid 4-条件 final rule satisfy 到達なら Implementation stage 着手。詳細 = [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md) Iteration v18 entry + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: ...` section |
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
| **PRD I-D-pre Implementation Phase 3 完了 + /check_job 4-layer + deep deep review pass (= audit script extensions、Option α auto-detect + C1 branch coverage + dual verify + proper top-level import)** | 2026-05-11 | `scripts/audit-prd-rule10-compliance.py` に **3 NEW verify functions** (T1-pre-1 `verify_pending_verdict_findings_consistency` + T1-pre-2 `verify_cross_reference_cell_consistency` + T1-pre-4 `verify_cell_numbering_drift_detection`) + **helper `has_cell_numbering_convention_section()`** (= Option α auto-detect、I-205 audit out-of-scope 自動分類) + **formatter `_format_path_e_drifts()`** embed。Path E utility を **proper top-level library import で source of truth 共有** (= sys.path.insert + `# noqa: E402` 排除 = Python 自動 sys.path[0] 利用、PEP 8 compliant、DRY 達成)。**4 test stubs `#[ignore]` 解除 + 7 fixtures 新設** (positive 3 + negative 3 + out-of-scope 1)。**/check_job 4-layer review** で **Helper False branch test 不在** 発見 → I-205-like fixture `audit_out_of_scope_skip.md` 新設 + Option α skip verify test 追加 = 5 tests PASS。**/check_job deep deep review** で **(i) sys.path.insert + `# noqa: E402` mechanical 妥協 (Implementation gap I3、High)** + **(ii) fixture violation patterns presence indirect verify (= silent miss risk、Implementation gap I4、Medium)** 発見 → 即時 fix: (i) proper top-level import 移動 + suppression 排除、(ii) `test_out_of_scope_fixture_violation_patterns_present_via_path_e` 追加 (= Path E utility 経由 3 drifts direct verify = Option α auto-detect skip correctness の dual verify lock-in) = **6 tests PASS**。残 findings (I2/I5 + R1/R2/R3) は L4 latent、TODO `[I-D-future-audit-extensions-hardening]` cohesive batch entry に structural lock-in。empirical state: 6/6 PASS + INV-4 4-tuple baseline preserved + Path E utility 0 drifts (= byte claim 37310 → 44451 sync) + cargo clippy 0 warnings + cargo fmt 0 diffs + file-line OK + `# noqa` suppression 0 件。audit script LOC 906 → 1033 (+127 行、29 functions enumerated、no mechanical compromise)。次 action = **Phase 4** = `scripts/audit-handoff-doc-line-refs.py` (NEW、~150 行) 新設 + `.github/workflows/ci.yml` CI step integration |
| **PRD I-D-pre Iteration v3 = Phase 3 着手前 baseline analysis + Option α design decision lock-in** | 2026-05-11 | Phase 3 着手前 empirical baseline analysis 結果を Iteration v3 entry に retroactive embed + Option α (= `## Cell Numbering Convention` section 有無で auto-detect、I-205 audit out-of-scope) user 確定。TODO `[I-205-retroactive-cell-numbering-section]` 新規起票 (= 案 γ Phase 2 T15 batch 化、I-205 PRD doc に section 追加で audit scope 内 promote)。Cohesion 純化 = I-205 PRD doc touch せず、I-D-pre Iteration v3 entry + TODO で spec-traceable lock-in。Phase 3 implementation ready (= 3 NEW verify functions + helper `has_cell_numbering_convention_section()` + 6 fixtures + 4 test stubs `#[ignore]` 解除) |
| **PRD I-D-pre Phase 3 着手前 empirical baseline analysis 完了 + I-205 false-positive 発見** | 2026-05-11 | 3 real PRDs (I-D-pre / I-D-main / I-205) で `/tmp/phase3_baseline_analysis.py` script run → I-D-pre/main は matrix ↔ Spec→Impl Mapping 1-to-1 alignment ✓、**I-205 は Spec→Impl Mapping section 不在 + documented gaps 21 cells**、empirical evidence で **Path E utility が I-205 で既に 2 drifts (= false-positive)** 発見。Phase 3 で `verify_cross_reference_cell_consistency` を audit script 側 mirror すると同 false-positive 再発 = INV-4 4-tuple baseline 破壊 risk。次 session で **Option α / β / γ design decision** を user 確認後 Step 3 実装着手 → **Iteration v3 entry で Option α 確定 (上記 row 参照)** |
| **PRD I-D-pre Implementation Phase 1+2 完了 + /check_job A1-A9 ideal-clean fix 完了 (Iteration v2 retroactive embed)** | 2026-05-11 (single session 完了) | **Phase 1 完了**: 6 test stub files (`tests/i_d_pre_*.rs`) + `tests/fixtures/i_d_pre/{positive,negative}/` infra setup。**Phase 2.1 完了 (T1-pre-5)**: `scripts/verify_line_refs.py` Method A formal lock-in (metadata header + 2 tests PASS + 2 fixtures)。**Phase 2.2 完了 (T1-pre-6)**: `scripts/verify_prd_self_audits.py` Path E formal lock-in + F6 fix (Axis 1 spec-traceable allow-list) + F7 fix (Axis 2 post-v15 wording detection + TS-pre-N regex) + Axis 3 narrow extension (`CELL_SLOT_AS_IDENTIFIER_RE`) + **4 additive utility fixes** (= find_section_range bug fix + find_repo_root + IMPACT_AREA_BYTES_RE 12-digit + expand_cell_list 4 patterns + SECTION_COVERAGE_POLICY 5 sections + STALE_STATUS_PATTERNS 削除) + 5 tests PASS + 8 fixtures。**/check_job 4-layer review A1-A9 ideal-clean fix**: 9 findings (Spec gap 3 + Implementation gap 2 + Low 4) 全 fix、Iteration v2 entry retroactive embed、TODO `[I-D-future-vocab-fork]` 別 PRD 候補 entry 起票 (= broader vocabulary fork detection deferred)。empirical state: Method A 2/2 + Path E 5/5 = **7 PASS** + 13 stubs ignored、INV-4 4-tuple baseline preserved (I-050 FAIL preserve / I-205 PASS / I-D-pre PASS / I-D-main PASS)、Path E 0 drifts on both real PRDs、cargo clippy 0 warnings + cargo fmt 0 diffs。次 action = **Phase 3** (= T1-pre-1 + T1-pre-2 + T1-pre-4 audit script extensions、3 NEW verify functions in `scripts/audit-prd-rule10-compliance.py`) |
| **PRD I-D Path B split adoption + I-D-pre PRD draft v1 完成 + I-D parent → I-D-main rename + scope reduce** | 2026-05-11 (single session 完了、上記 entry の prerequisite step) | Path B split user 確定 (= ideal-implementation-primacy + 妥協禁止 directive 適用結果、Iteration v17 plateau の bootstrap utility correctness ceiling = 無限 chain 構造 を構造的解消)。新規 PRD I-D-pre (= 5 audit mechanism cells、`backlog/I-D-pre-audit-mechanism-bootstrap.md` ~660 LOC initial) 起票 + I-D parent rename to I-D-main (= 24 framework rule integration cells、original numbers preserved with documented gaps {6, 8, 10, 17, 19, 28}、`backlog/I-D-main-framework-rule-integration-cohesive-batch.md` 1516 LOC、Iteration v1-v17 + v18 Path B split entry preserved)。両 PRD `audit-prd-rule10-compliance.py` exit 0 ✓ + INV-4 4-tuple baseline verified。詳細 = 上記「PRD I-D Path B split adoption + I-D parent Spec Stage 進捗」section + 「次の作業 table」section |
| **PRD I-D Spec Stage Iteration v1-v17 progress (= 8 third-party rounds + 9 fix iterations + 2 bootstrap utilities 632 LOC、Path B split source = parent stage history)** | 2026-05-10 (single session 累積) | I-D parent PRD doc 1494 LOC、Iteration v17 で trajectory floor break (11→9、-18%、Critical 半減 2→1、meta 22% history 最低)。Method A bootstrap (Cell 19 verify_line_refs.py) で line-ref drift class 完全 absorption + Path E bootstrap (Cell 10/6+8/17/28 verify_prd_self_audits.py 4 axes) で multi-axis partial absorption。Spec stage close 未到達 (2/4 PASS 3 round 連続)、bootstrapping circularity 構造的解消の必要性 empirical 確認 → 翌日 (2026-05-11) Path B split user 確定で構造的解消。詳細 = `backlog/I-D-main-framework-rule-integration-cohesive-batch.md` Iteration v1-v17 entries (preserved from I-D parent) |
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

- **PRD I-D-pre (Audit mechanism bootstrap、Phase 1+2 完了 + /check_job A1-A9 fix 完了、Phase 3 着手 ready)**: I-D-main 着手 prerequisite、5 audit mechanism cells (= I-D parent Cell 6+8/10/17/19/28 from Path B split 2026-05-11)。**現 state (2026-05-11)**: Phase 1+2 完了 + Iteration v2 ideal-clean state 達成 (= /check_job A1-A9 fix retroactive embed)、Method A 2 PASS + Path E 5 PASS = 7 tests PASS、INV-4 4-tuple baseline preserved、Path E 0 drifts on both real PRDs。**Primary references**: [`backlog/I-D-pre-audit-mechanism-bootstrap.md`](backlog/I-D-pre-audit-mechanism-bootstrap.md) (~770 LOC post v2) `## Spec Review Iteration Log > Iteration v2` (= /check_job fix history) + `## Implementation Stage Tasks > T1-pre-1/T1-pre-2/T1-pre-4` (= **Phase 3 spec**) + `## Test Plan > Fixture design pattern + Test helper convention` (= A6 fix で追加された fixture pattern + helper spec、Phase 3 fixture 新設時 reference) + [`scripts/verify_line_refs.py`](scripts/verify_line_refs.py) (Method A、formal lock-in 完了) + [`scripts/verify_prd_self_audits.py`](scripts/verify_prd_self_audits.py) (Path E、formal lock-in + F6/F7/Axis 3 + 4 additive utility fixes 完了)。**Phase 3 NEW target**: `scripts/audit-prd-rule10-compliance.py` に 3 NEW verify functions (= verify_pending_verdict_findings_consistency + verify_cross_reference_cell_consistency + verify_cell_numbering_drift_detection) + `tests/i_d_pre_audit_extensions_test.rs` 4 stubs `#[ignore]` 解除 + 6 fixtures 新設
- **PRD I-D-main (Framework rule integration cohesive batch、I-D-pre 完了後再開、Iteration v18 = Path B split adoption record)**: I-205/I-225/I-162 PRD chain prerequisite、24 candidates (= I-D parent 30 - I-D-pre 5 logical cells migration、original numbers preserved with documented gaps {6, 8, 10, 17, 19, 28})。**現 state**: WAITING for I-D-pre completion、I-D-pre 完了後 first third-party adversarial review = Iteration v19 で convergence target で再開。**Primary references**: [`backlog/I-D-main-framework-rule-integration-cohesive-batch.md`](backlog/I-D-main-framework-rule-integration-cohesive-batch.md) (1516 LOC、Iteration v1-v17 + v18 Path B split entry preserved) + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) `## I-224: ...` section
- **TODO `[I-D-future-vocab-fork]` (新規 deferred entry、/check_job A3 fix 由来)**: PRD I-D-pre Cell 5 narrow CELL_SLOT_AS_IDENTIFIER_RE detection scope の補完、broader vocabulary fork detection (= "cell # / candidate ID / matrix #" 間 semantic-level mixed canonical naming) を別 framework concern として deferred。4 candidate classes (C1 cell # vs matrix # / C2 candidate ID vs cell # / C3 section heading vs body wording / C4 external reference canonical fork)、L4 latent priority、案 γ Phase 0 完了後再評価
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
