# PRD I-D-pre — Audit mechanism bootstrap (Path B small-scope spec stage)

**Status**: Spec stage Iteration v1 (本 draft 初版、2026-05-10、PRD I-D split Path B 採用由来)
**起票日**: 2026-05-10 (案 γ Phase 0 = PRD I-D Path B split user 確定 2026-05-10)
**Origin**: PRD I-D Iteration v17 plateau の bootstrapping problem empirical evidence (= self-applied audit utility correctness ceiling = 各 utility が次 round の dominant defect class を自ら生成する無限 chain 構造) を構造的解消するため、PRD I-D を **I-D-pre (bootstrap audit mechanism construction、本 PRD)** + **I-D-main (post-bootstrap framework rule integration、別 PRD)** の 2 PRD に split (user 確定 2026-05-10、Path B option)
**架構的 concern**: Self-applied PRD audit mechanism (= `scripts/verify_*.py` + `scripts/audit-handoff-doc-line-refs.py` + 関連 audit functions) を **Implementation lock-in** = formal audit utilities として完成、I-D-main spec stage iteration が **completed bootstrap utilities 上で initial iteration convergence** 可能な base state を確立する

---

## Background

### 直接動機 (PRD I-D Iteration v17 bootstrapping problem empirical evidence)

PRD I-D Spec Stage Iteration v1〜v17 (= 8 third-party rounds + 9 fix iterations、本 session 累積 2026-05-10) で、**self-applied audit utility correctness ceiling** = 3rd-order pattern が empirical 発覚:

1. **Method A bootstrap (Iteration v12)**: `scripts/verify_line_refs.py` (264 LOC) を Cell 19 (v11-7) line-ref drift 完全 absorption 目的で早期実装 → Iteration v13-v15 で **別 defect class emerge** (= Scope partition + status staleness + cross-cutting wording semantic mismatch)
2. **Path E bootstrap (Iteration v16)**: `scripts/verify_prd_self_audits.py` (368 LOC、4 axes) を Cell 10/6+8/17/28 multi-axis partial absorption 目的で実装 → Iteration v17 で **Path E utility 自身の under-detection class emerge** (F6 = Axis 1 threshold "5" arbitrary heuristic で Scope partition cells ≤ 5 missing silently pass / F7 = Axis 2 TS-X over-exclusion で post-v15 wording stale 検出失敗)
3. **Pattern**: 各 bootstrap utility が **次 round の dominant defect class を自ら生成** = utility-correctness ceiling = 各 utility は次 utility で audit する必要 = **無限 chain 構造**
4. **数学的事実**: ~30% defect introduction rate per fix × 6-10 fixes/round = expected 1.2-3 new defects/round = **absolute 0 unreachable in finite rounds without bootstrap absorption**

### Path B split rationale (user 確定 2026-05-10)

Iteration v17 NOT-CONVERGED + 2/4 PASS (3 round 連続 v13/v15/v17) state を踏まえ、3 path options 提示 → user 採用 = **Path B (PRD I-D split into I-D-pre + I-D-main)**:

- **Path E+ (continue)**: utility self-correctness 強化で trajectory v17:9 → v19:5-7 → v21:0-3 期待 → **rejected** = utility correctness ceiling = bootstrap chain 継続 = 妥協禁止 directive 違反
- **Path B (PRD split)**: bootstrapping circularity 構造的解消 (= utility を Implementation lock-in 後、完成 utility で残 25 cells を audit) → **accepted** = 1 PRD = 1 architectural concern 原則準拠 + structural fix
- **Path F (criterion re-design)**: convergence criterion 緩和 = asymptotic floor 受容 → **rejected** = explicit compromise = 妥協禁止 directive 違反

**Cohesion principle 適合 evidence**:
- Cell 19, 10, 6+8, 17, 28 (5 cells) = `scripts/verify_*.py` audit mechanism construction (single architectural concern)
- Cell 1-5, 7, 9, 11-16, 18, 20-27, 29-30 (24 cells) = `.claude/rules/` + skill + command spec changes (異なる architectural concern)
- 既に I-E (= v13-2 Promise builtin runtime + v13-3 lib API vs CLI binary inconsistency) は同様の architectural concern split 由来で別 PRD 化済 (案 γ Phase 0.5、2026-05-10)
- memory `feedback_prd_cohesion_granularity.md`: "規模 ≠ 判断軸、architectural concern が判断軸"

### I-D-pre 完了の structural impact

I-D-pre 完了 = I-D-main spec stage が **bootstrap utility 完成済 base** で再開可能 = bootstrapping circularity 排除 = initial iteration convergence target (= user 指示 v15 directive 再適用 mandatory state からの脱却)。後続 PRD (= I-225 / I-162 / I-205 T14-T16 / 全 future PRDs) も同 utilities を leverage、framework leverage 達成。

---

## Problem Space

### Matrix-driven 判定 (Step 0a)

**判定**: matrix-driven (special form、Rule 1 (1-4) orthogonality merge legitimacy 適用、I-D parent PRD と同 framework structure)。

**Rationale**:
- PRD I-D parent matrix 30 cells の subset (5 logical units / 6 row numbers) を I-D-pre scope として継承
- 5 cells は各々 distinct な resolution tuple (= target_file / modification_type / verification_mechanism / test_contract) を持つ "cell" として enumerable
- 本 PRD I-D-pre 自身の self-applied integration (= 完成した bootstrap utilities が本 PRD doc 自身を audit) のため matrix structure 必須

### 入力次元 (Dimensions)

#### Primary Axis A: Candidate ID (5 variants、I-D parent から migration)

各 candidate は PRD I-D parent matrix から I-D-pre architectural concern (= audit mechanism construction) で migrate された **discrete** な audit utility construction signal。各 variant は本 PRD で **1 cell に対応 + 1 resolution tuple を持つ**。

5 candidates 全列挙 (= renumbered 1-5、Cell 28 v13-5 single-source-of-truth principle = I-D-pre matrix # canonical):

| # | Candidate ID | I-D parent migration source | Severity classification |
|---|------|-------------|-------------------------|
| 1 | v3-6+v4-2 | I-D Cell 6+8 (consolidated) | High (sub-rule pending verdict ↔ findings count consistency check + Critical=0 claim ↔ stale verdict consistency = 共有 audit function) |
| 2 | v5-1 | I-D Cell 10 | High (cross-reference cell consistency = matrix と各 cross-ref context の cell # appearance consistency check) |
| 3 | v11-5 | I-D Cell 17 | High (audit-handoff-doc-line-refs.py NEW + CI integration = handoff doc `<file>:<line>` cross-reference structural automated detection) |
| 4 | v11-7 | I-D Cell 19 | High (Layer 1 factual accuracy semantic check sub-step + Method A audit auto-verify mechanism = `scripts/verify_line_refs.py` formal lock-in) |
| 5 | v13-5 | I-D Cell 28 | High (Cell numbering convention single-source-of-truth enforcement + audit auto-detect = `verify_cell_numbering_drift_detection`) |

#### Auxiliary Axis (derived per Rule 1 (1-4) orthogonality merge legitimacy)

各 candidate は Axis A (Candidate ID) から **1-to-1 で derive される** 以下 5 attributes を持つ:

- **Aux 1 (Target file)**: 改修 target file (= candidate の resolution が touch する file path)
- **Aux 2 (Target rule section)**: rule file 内の specific section / sub-rule (= 改修 wording の location、cells 4 + 5 のみ)
- **Aux 3 (Modification type)**: rule wording 強化 / new audit function / new audit script / existing utility formal lock-in
- **Aux 4 (Verification mechanism)**: audit script auto-verify / manual checklist self-applied
- **Aux 5 (Test contract)**: 各 candidate の lock-in test (= regression 防止 mechanism、test fn name + assertion)

これら auxiliary attributes は Axis A から **functionally 決定**、Rule 1 (1-4) orthogonality merge legitimacy 適用、Cartesian product expansion 不要 = **5 rows linear matrix** で完全 enumerate 達成。

#### Orthogonality verification statement (Rule 1 (1-4-a) compliant)

**Source cell #**: 全 5 cells は **mutually distinct** (各 candidate の resolution tuple が unique)。Reference source cell # は self (= 各 cell が他 cell と independent)。Auxiliary axes は Axis A から derive、Rule 1 (1-4-b) Spec-stage structural consistency verify は **5 row × ~5 col linear matrix** で structurally consistent (= 各 cell の auxiliary tuple は mutually distinct)。よって本 PRD では auxiliary axes を **derived columns として merge declaration**、Cartesian product expansion 不要を Rule 1 (1-4) compliant に確立。

#### Spec-stage structural consistency verify (Rule 1 (1-4-b) compliant)

各 candidate の resolution tuple は本 PRD `## Oracle Observations` section 内 5 個別 sub-section (`### Cell N: <candidate-id>` 命名 convention、`## Cell Numbering Convention` section で single-source-of-truth として explicit declare) で structural consistency を spec-traceable に verify。matrix table cell # 列 ↔ Oracle Observations sub-section heading の `Cell N` 番号 ↔ Spec→Impl Dispatch Arm Mapping table cell # 列 の **三者 1-to-1 mapping** は audit script `verify_dispatch_arm_mapping_table` (= I-D parent T1-6 で新設対象、本 PRD I-D-pre scope 外 = I-D-main で実装) では verify されないため、**本 PRD 内では `verify_cell_numbering_drift_detection` (= 本 PRD T1-4 = Cell 5 v13-5 candidate で新設) のみで structural integrity 担保**。三者 1-to-1 mapping の formal audit は I-D-main で完成。

#### Spec-stage referenced cell symmetry probe (Rule 1 (1-4-c) compliant)

5 cells は mutually independent (= referenced source cell が self) のため symmetry probe N/A。代わりに 5 cells ↔ Implementation Stage Tasks T1-T2 / T6-T8 の 1-to-1 mapping を本 PRD `## Spec→Impl Dispatch Arm Mapping` section で hard-code、symmetry probe の代替として 5 candidates → 各 task → 各 test contract の chain consistency を spec-traceable に確立。

### 組合せマトリクス (5 cells)

| # | Candidate | I-D source | Target file | Target rule section | Modification type | Verification mechanism | Test contract | Ideal output | 現状 | 判定 | Scope |
|---|-----------|------------|-------------|---------------------|-------------------|-----------------------|---------------|--------------|------|------|-------|
| 1 | v3-6+v4-2 | I-D Cell 6+8 | `scripts/audit-prd-rule10-compliance.py` | New function `verify_pending_verdict_findings_consistency` (consolidated = v3-6 + v4-2 共有 function) | new audit function | audit script auto-verify | `test_audit_pending_verdict_count_consistency` + `test_audit_critical0_claim_stale_verdict_inconsistency` | sub-rule 表に "(TS-X 後 verify)" 等 pending verdict 残存時、findings count >=1 + Critical=0 claim ↔ stale verdict label inconsistency を flag | partial (Path E utility Axis 2 で TS-X over-exclusion class、F7 fix で formal lock-in) | ✗ | 本 PRD |
| 2 | v5-1 | I-D Cell 10 | `scripts/audit-prd-rule10-compliance.py` | New function `verify_cross_reference_cell_consistency` | new audit function | audit script auto-verify | `test_audit_cross_reference_cell_appearance_consistency` | matrix と各 cross-reference context (= In Scope / Out of Scope / Tier 2 reclassify / INV-N verification lists / Test Plan / TN completion criteria) の cell # appearance consistency を auto verify、`missing in N+ sections` pattern を syntactic detect | partial (Path E utility Axis 1 で threshold "5" arbitrary heuristic class、F6 fix で formal lock-in = spec-traceable allow-list 置換) | ✗ | 本 PRD |
| 3 | v11-5 | I-D Cell 17 | `scripts/audit-handoff-doc-line-refs.py` (NEW) + `.github/workflows/ci.yml` (extend) | New audit script + CI integration | new audit script | audit script auto-verify | `test_audit_handoff_doc_line_refs_drift_detection` | Handoff doc (`doc/handoff/*.md`) の `<file>:<line>` cross-reference が actual file に存在 + line content syntactic verify、line drift を structural detect、CI で merge gate active | unimplemented (Path E utility Axis 4 = external file drift で partial coverage、formal NEW script で完全 lock-in) | ✗ | 本 PRD |
| 4 | v11-7 | I-D Cell 19 | `.claude/rules/check-job-review-layers.md` + `scripts/verify_line_refs.py` (formal lock-in) | Layer 1 (Mechanical) sub-step 追加 (factual accuracy semantic check) + Method A audit utility formal lock-in | rule wording 強化 + existing utility formal lock-in | manual checklist + audit script auto-verify | `test_layer1_factual_accuracy_semantic_check_documented` + `test_audit_line_ref_drift_detection_via_method_a` | (a) Layer 1 sub-step "factual accuracy semantic check: 修正 doc / comment 内の固有名詞 (PRD ID / Iteration v# / task ID / file path / line ref) が claim する意味と一致することを semantic check" 追加 + (b) `scripts/verify_line_refs.py` (264 LOC、Iteration v12 bootstrap 由来) を formal Method A audit utility として lock-in、PRD doc heading-based line-ref drift detection の regression-tested auto-verify mechanism | partial (Method A utility 既存だが formal lock-in 未、Layer 1 sub-step rule wording 未 embed) | ✗ | 本 PRD |
| 5 | v13-5 | I-D Cell 28 | `.claude/rules/spec-stage-adversarial-checklist.md` + `scripts/audit-prd-rule10-compliance.py` | Rule 9 / Rule 13 sub-rule 追加 (Cell numbering convention single-source-of-truth) + new audit function `verify_cell_numbering_drift_detection` | rule wording 強化 + new audit function | audit script auto-verify | `test_rule9_cell_numbering_convention_documented` + `test_audit_cell_numbering_drift_detection` | (a) framework rule で "single-source-of-truth = matrix #" mandatory 化 + (b) framework rule で "convention drift detection" を audit script 経由 auto-detect (= 新 verify function 実装) + (c) PRD spec stage で cell numbering convention を `## Cell Numbering Convention` section 内に explicit declare mandatory | partial (Path E utility Axis 3 で R-x focus only、cell-slot vocabulary fork 未 cover、formal lock-in で完全 absorption) | ✗ | 本 PRD |

判定凡例: ✓ (現状 OK) / ✗ (修正必要) / NA (unreachable, 理由付き) / 要調査 (Discovery で解消)。

**Cartesian product completeness verify**: 5 cells = 5 candidates の完全 enumerate (Path B split で I-D parent から I-D-pre architectural concern boundary に migrate)。Auxiliary axes は Axis A から derive (Rule 1 (1-4) orthogonality merge legitimacy 適用)、Cartesian product 不要。本 PRD 自身を `audit-prd-rule10-compliance.py` の新 function `verify_cartesian_product_completeness` (= I-D parent Cell 1 R-1 candidate、I-D-main scope) で auto verify は **本 PRD scope 外** (= I-D-main で完成)、本 PRD では既存 audit functions (= 本 PRD T1 で新設の 3 functions = T1-1/T1-2/T1-4 + T1-3a/3b NEW script + T1-5 Method A formal lock-in + T1-6 Path E formal lock-in) で self-applied integration verify。

### Spec-Stage Adversarial Review Checklist

Spec stage 完了 verification は `.claude/rules/spec-stage-adversarial-checklist.md` の **13-rule checklist** を本 PRD `## Spec Review Iteration Log` section に転記して全項目 verification する (DRY のため checklist 内容は本 PRD doc に再記載しない、rule file が single source of truth)。13-rule に 1 つでも未達があれば Implementation stage 移行不可。

---

## Oracle Observations

通常 matrix-driven PRD で必須の `## Oracle Observations` section は、TS→Rust conversion PRD で tsc / tsx output を grounding source とする。本 framework PRD では grounding source が **異なる** ため、本 section は **adapted form (= Current Rule/Script State Snapshot)** として embed (= 各 candidate の current rule wording / script behavior を pre-state として record、resolution 後の post-state と diff 取れるよう現状 lock-in)。本 adapted form は Rule 2 (2-2) section embed mandatory を framework PRD context で satisfy する自然な extension (= TS source の代わりに framework rule source を grounding、tsc output の代わりに current rule wording / audit function inventory を Pre-state record)。

### Cell 1: v3-6+v4-2 (consolidated pending verdict + Critical=0 stale verdict consistency)

- **Current state**: `verify_rule13_spec_review_iteration_log` (line 595 in audit-prd-rule10-compliance.py) は existence verify のみ、pending verdict consistency check + Critical=0 claim ↔ stale verdict consistency check 不在
- **Pre-state probe**: `grep -A 30 "verify_rule13_spec_review_iteration_log" scripts/audit-prd-rule10-compliance.py` で内部 logic 確認 → 両 check 未実装 (確認 2026-05-10)
- **Path E utility partial coverage**: `scripts/verify_prd_self_audits.py` Axis 2 (line 218-234 `verify_status_pending_verdict`) で TS-X heading 内 stale Status を blanket exclude する class、Iteration v17 F7 で under-detection structural defect 発覚 (= v15-F2 line 754 stale "IN PROGRESS (v1〜v10、v11 期待)" wording を Path E が auto-detect 失敗)
- **Ideal post-state**: 既存 `verify_rule13_spec_review_iteration_log` を strengthening、または新 function `verify_pending_verdict_findings_consistency` 追加 (= sub-rule 表内 `(TS-X 後 verify)` / `partial` / `要 TS-N で完成` 等 pending verdict pattern detect、findings count = 0 claim と inconsistency を flag) + `Critical=0 claim ↔ stale verdict consistency` sub-check 同 function 内 集約実装 (= v3-6 + v4-2 cohesive batch)。**F7 fix integrated** (Iteration v17 F7 由来): TS-X over-exclusion を post-v15 wording presence 要求に refine (= TS-X heading 内でも v15+ wording なら flag = blanket exclude 解消)
- **Rationale**: PRD I-224 iteration v2 で false-positive 0 findings claim、iteration v3 でも初版で同 pattern 残存。iteration v4 Medium 2 で stale verdict label を残したまま Critical=0 claim、third-party adversarial で発覚。Iteration v17 F7 で Path E utility 自身の under-detection class 発覚 = formal audit function lock-in 必須

### Cell 2: v5-1 (cross-reference cell consistency)

- **Current state**: `scripts/audit-prd-rule10-compliance.py` 内に該当 function 不在
- **Pre-state probe**: `grep "cross_reference_cell" scripts/audit-prd-rule10-compliance.py` → 0 hits (確認 2026-05-10)
- **Path E utility partial coverage**: `scripts/verify_prd_self_audits.py` Axis 1 (line 181-185 `verify_cross_reference_cell_consistency`) で Scope partition cells ≤ 5 missing で silently pass する class、Iteration v17 F6 で under-detection structural defect 発覚 (= v13-F1 cell 21 missing class 検出失敗)
- **Ideal post-state**: 新 function `verify_cross_reference_cell_consistency(prd_path, content) -> list[str]` 追加 (= matrix table と各 cross-reference context (= In Scope / Out of Scope / Tier 2 reclassify / INV-N verification lists / dispatch tree comments / Test Plan / TN completion criteria) の cell # appearance consistency を auto verify、`missing in N+ sections` pattern を syntactic detect)。**F6 fix integrated** (Iteration v17 F6 由来): Axis 1 tolerance threshold "5" arbitrary heuristic を **spec-traceable allow-list** に置換 (= Scope partition exception を formal declare、其他 missing cells は flag)
- **Rationale**: PRD I-224 iteration v5 で 80-cell × 6 cross-reference contexts dense matrix の cell # appearance gap (cells 27/40 等が複数 sections で missing) を発見、`/check_job` review で identify。Iteration v17 F6 で Path E utility 自身の under-detection class 発覚 = formal audit function lock-in 必須

### Cell 3: v11-5 (audit-handoff-doc-line-refs.py 新設 + CI integration)

- **Current state**: `scripts/audit-handoff-doc-line-refs.py` 不在、`.github/workflows/ci.yml` に該当 step 不在
- **Pre-state probe**: `ls scripts/audit-handoff-doc-line-refs.py` → No such file (確認 2026-05-10)、`grep "audit-handoff-doc-line-refs" .github/workflows/ci.yml` → 0 hits
- **Path E utility partial coverage**: `scripts/verify_prd_self_audits.py` Axis 4 (external file drift) で partial coverage、ただし NEW dedicated script で完全 lock-in 必須 = handoff doc 専用 line-ref drift detection
- **Ideal post-state**: `scripts/audit-handoff-doc-line-refs.py` 新設 (~150 行、handoff doc grep `\.rs:\d+` pattern 抽出 + each `<src_file>:<line>` reference の actual file 存在 check + line content syntactic verify)、`.github/workflows/ci.yml` に CI step として integrate (PR merge gate)
- **Rationale**: PRD I-224 T6a で line-ref drift 2 件 (`__ts_main:130→133` typo + `__ts_do_while_loop:346→356` git history drift) は cargo test で捕捉不能、structural automated detection 不在 = future doc edit + src/ refactor の組合せで silent drift 再発 risk

### Cell 4: v11-7 (Layer 1 factual accuracy semantic check sub-step + Method A audit utility formal lock-in)

- **Current state (rule wording side)**: `.claude/rules/check-job-review-layers.md` Layer 1 (Mechanical) sub-step に factual accuracy semantic check 不在
- **Current state (Method A utility side)**: `scripts/verify_line_refs.py` (264 LOC) は本 session Iteration v12 (2026-05-10) bootstrap 早期実装で existence、ただし formal Implementation lock-in 未 (= regression-tested auto-verify mechanism として未確立、PRD doc heading-based line-ref drift detection の formal audit utility status 未)
- **Pre-state probe**: `grep -n "factual accuracy semantic\|意味と一致" .claude/rules/check-job-review-layers.md` → 0 hits + `ls -la scripts/verify_line_refs.py` → exists 264 行 (確認 2026-05-10)
- **Ideal post-state**: 2 mechanism coordinated implementation:
  - (a) Layer 1 (Mechanical) sub-step 追加 (= "factual accuracy semantic check: 修正 doc / comment 内の固有名詞 (PRD ID / Iteration v# / task ID / file path / line ref) が claim する意味と一致することを semantic check (= 単純 grep + 存在 check ではなく、reference が claim する意味論的 context との一致を verify)")
  - (b) `scripts/verify_line_refs.py` formal lock-in (= regression-tested utility status 確立、`tests/i_d_pre_method_a_test.rs` で auto-verify mechanism、heading-based line-ref drift detection の coverage scope explicit declare)
- **Rationale**: PRD I-224 T6a 1st-round で `Iteration v11 で expressions/mod.rs 同居 cohesion 化` factual conflate (= I-224 v11 と I-205 由来 expressions/mod.rs の文脈 mix) を catch せず通過、2nd-round adversarial で発見、framework rule level での Layer 1 semantic check sub-step 必須。Iteration v12 で Method A bootstrap utility (`scripts/verify_line_refs.py`) 早期実装、本 PRD I-D-pre で formal lock-in 完成

### Cell 5: v13-5 (Cell numbering convention single-source-of-truth enforcement + audit auto-detect)

- **Current state**: `.claude/rules/spec-stage-adversarial-checklist.md` Rule 9 / Rule 13 に cell numbering convention single-source-of-truth enforcement rule 不在、`audit-prd-rule10-compliance.py` 内に convention drift auto-detect function 不在
- **Pre-state probe**: `grep -n "cell numbering convention\|single-source-of-truth.*matrix #\|convention drift" .claude/rules/spec-stage-adversarial-checklist.md scripts/audit-prd-rule10-compliance.py` → 0 hits (確認 2026-05-10)
- **Path E utility partial coverage**: `scripts/verify_prd_self_audits.py` Axis 3 (R-x focus only) で partial coverage、**cell-slot identifier-level fork** 未 cover (= "cell-slot N" / "cell-slot #N" 数値 identifier 用法 = canonical 違反 detection 未対応)
- **Scope refinement (A3 fix /check_job L3-2)**: 本 PRD I-D-pre Cell 5 scope は **identifier-level fork detection** に限定 (= numeric identifier 用法のみ flag、descriptive uses ("cell-slot occurrence" / "cell-slot vocabulary fork" 等 concept descriptors) は legitimate として allow)。**Broader vocabulary fork detection** (= "cell # / candidate ID / matrix #" 間の mixed canonical naming, semantic-level 検出) は別 framework concern で本 PRD scope 外、TODO `[I-D-future-vocab-fork]` 候補として deferred (= 後続 PRD で取り扱う)
- **Ideal post-state**: 3 mechanism coordinated implementation:
  - (a) framework rule で "single-source-of-truth = matrix #" mandatory 化
  - (b) framework rule で "convention drift detection (= identifier-level fork)" を audit script 経由 auto-detect (= 新 verify function `verify_cell_numbering_drift_detection` + Path E Axis 3 narrow detection extension `CELL_SLOT_AS_IDENTIFIER_RE`)
  - (c) PRD spec stage で cell numbering convention を `## Cell Numbering Convention` section 内に explicit declare mandatory
- **Rationale**: PRD I-224 で 2 surface convention drift (= INV-3 entries は matrix # numbering、e2e fixture filenames は sequential filename numbering で同名異物 confusion 発生) を patch 化、structural fix 不在 = framework rule level での single-source-of-truth enforcement 必須。Path E Axis 3 narrow detection 形 (= identifier-level fork only) で本 PRD scope 完了、broader vocabulary fork は別 PRD で structural extend

---

## SWC Parser Empirical Lock-ins

**N/A**: 本 framework PRD は AST shape 構造的 mutual exclusion (= NA cells) を持たない。matrix の 5 cells は全 in-scope、NA cell 0 のため SWC parser empirical lock-in は構造的に不要。Rule 3 (3-1) compliant (NA reasoning が spec-traceable: framework PRD は AST input dimension irrelevant per Rule 12 (e-3) Permitted reasons)。

---

## Impact Area Audit Findings

### Pre-draft ast-variant audit (Rule 11 (d-5) compliance)

```bash
python3 scripts/audit-ast-variant-coverage.py --files <impact-area-files>
```

**Result**: N/A — Impact Area files は `.claude/rules/*.md` (markdown) + `scripts/audit-prd-rule10-compliance.py` (Python) + `scripts/audit-handoff-doc-line-refs.py` (Python NEW) + `scripts/verify_line_refs.py` (Python existing) + `scripts/verify_prd_self_audits.py` (Python existing) で **Rust source file 不在**。`audit-ast-variant-coverage.py` は Rust source の AST variant exhaustiveness audit を target、本 PRD の impact area には適用範囲外 (= AST input dimension irrelevant per Rule 12 (e-3))。

**Audit script extension target**: 本 PRD T1 で `audit-prd-rule10-compliance.py` 自体に新 verify functions を追加 + `audit-handoff-doc-line-refs.py` 新設 + 既存 `verify_line_refs.py` / `verify_prd_self_audits.py` formal lock-in、本 audit scripts 自身の structural correctness audit (= Python AST level の exhaustiveness、`_` arm 全廃、命名 convention) は Layer 1 mechanical review で manual verify (Test Plan section 参照)。

### Adapted Impact Area Review

framework PRD として、上記 audit script では replace できない以下 manual review を Spec stage で実施:

| Violation | Location | Phase | Decision | Rationale |
|-----------|----------|-------|----------|-----------|
| Rule wording の duplicated knowledge (DRY 違反候補) | `.claude/rules/check-job-review-layers.md` Layer 1 + `.claude/rules/spec-stage-adversarial-checklist.md` Rule 9/13 | rule file (markdown) | 本 PRD scope で fix | T2-pre (rule wording 強化、cells 4 + 5 のみ) で各 candidate の wording 改修時に DRY 違反を解消、cross-reference を `## Related Rules` table で集約 |
| audit script の duplicated logic patterns (DRY 違反候補) | `scripts/audit-prd-rule10-compliance.py` (~26 functions) + `scripts/verify_line_refs.py` + `scripts/verify_prd_self_audits.py` | Python source | 本 PRD scope で fix | T1-pre (audit script extension + utility formal lock-in) で新 verify functions 追加時、共通 helper (= section parsing / cell # extraction / pattern matching) を抽出、existing functions も refactor 対象 |

### Empirical file path verify (Rule 11 (d-5) sub-rule、I-205 RC-3 source)

本 PRD Impact Area で listing する全 file paths は empirical verify 済 (= 2026-05-10 `ls -la` + `wc -l` 確認):

| File | Status | Size (bytes) | LOC | Last modified | Empirical verify |
|------|--------|--------------|-----|---------------|------------------|
| `.claude/rules/check-job-review-layers.md` | exists | 16159 | 338 | 2026-04-25 22:18 | ✓ verified |
| `.claude/rules/spec-stage-adversarial-checklist.md` | exists | 42965 | 518 | 2026-04-28 21:15 | ✓ verified |
| `scripts/audit-prd-rule10-compliance.py` | exists | 44451 | 1033 | 2026-05-11 | ✓ verified (29 functions enumerated post Phase 3 + deep deep review fix = +3 verify functions + 1 helper + 1 formatter、T1-pre-1 + T1-pre-2 + T1-pre-4 audit script extensions + sys.path.insert + `# noqa: E402` 排除 = proper top-level import) |
| `scripts/verify_line_refs.py` | exists | 11517 | 297 | 2026-05-11 (Phase 2 T1-pre-5 formal lock-in 完了) | ✓ verified (Method A utility、formal lock-in DONE = metadata header + auto-verify test contract) |
| `scripts/verify_prd_self_audits.py` | exists | 31728 | 644 | 2026-05-11 (Phase 2 T1-pre-6 formal lock-in + F6/F7/Axis 3 + 4 additive utility fixes 完了) | ✓ verified (Path E utility 4 axes、formal lock-in DONE = byte claim 追加で Path E utility 自身が own self-audit する recursive structure 完成) |
| `scripts/audit-handoff-doc-line-refs.py` | exists | 9773 | 260 | 2026-05-11 | ✓ verified (Cell 3 / v11-5、Phase 4 T1-pre-3a 新設 + /check_problem Issue #3 fix で INVALID_RANGE category 追加、handoff doc `<path>:<line>` cross-ref drift detection、4 categories: INVALID_RANGE / MISSING_FILE / OUT_OF_BOUNDS / AMBIGUOUS) |
| `.github/workflows/ci.yml` | exists | — | — | — | ✓ verified (CI integration target for new audit-handoff-doc-line-refs.py) |

**Uncertain expression check** (RC-3 source、I-205 確定 2026-04-27): 上記 table に `(or 該当)` / `TBD` / `？` / `要確認` 等 uncertain expression 不在 (= empirical verify 完了)。`audit-prd-rule10-compliance.py` `verify_impact_area_uncertain_expressions` で auto verify。

---

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - Primary Axis A (Candidate ID, 5 variants): v3-6+v4-2 / v5-1 / v11-5 / v11-7 / v13-5
  - Auxiliary Axis (derived per Rule 1 (1-4) orthogonality merge legitimacy):
    - Aux 1 (Target file): .claude/rules/check-job-review-layers.md / .claude/rules/spec-stage-adversarial-checklist.md / scripts/audit-prd-rule10-compliance.py / scripts/verify_line_refs.py / scripts/verify_prd_self_audits.py / scripts/audit-handoff-doc-line-refs.py (NEW) / .github/workflows/ci.yml
    - Aux 2 (Target rule section): Rule 9 / Rule 13 sub-rules / Layer 1 sub-step
    - Aux 3 (Modification type): rule wording 強化 / new audit function / new audit script / existing utility formal lock-in
    - Aux 4 (Verification mechanism): audit script auto-verify / manual checklist self-applied
    - Aux 5 (Test contract): per-candidate lock-in test (test fn name + assertion + reference)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: N/A
```

### Cross-axis orthogonal direction detail (yaml 外 prose、Rule 10 3 step methodology)

- **解決軸** (= "self-applied PRD audit mechanism bootstrap construction") の **対立軸** (Rule 10 Step (I) 逆問題視点) = "self-applied audit utility correctness ceiling = bootstrap chain 無限化" → 5 candidates の resolution 全てが utility correctness ceiling 構造的解消を target (= I-D-main spec stage で initial iteration convergence 可能な base state 確立)
- **実装 dispatch trace** (Rule 10 Step (II)) = audit script の各 verify function が PRD doc の specific structural pattern を dispatch、本 PRD で 5 cells を全 enumerate、各 cell の dispatch 先 verify function を auxiliary axis Aux 4 で record
- **影響伝搬 chain** (Rule 10 Step (III)) = "rule wording 改修 (cells 4 + 5) → audit script 拡張必要 (cells 1 + 2 + 5)" / "audit script 拡張 → existing PRD docs compliance 確保" / "Method A + Path E utility formal lock-in → I-D-main spec stage initial iteration convergence enable"、本 chain は本 PRD T1-pre / T2-pre / T6 / T7 / T8 dependency order で structural enforce
- **Structural reason for matrix absence**: N/A (= matrix-driven PRD、上記 5 cells で完全 enumerate のため matrix absence は該当しない)

`Structural reason for matrix absence` field の Prohibited keywords 不在は audit script `verify_rule10_application` で auto verify。

---

## Goal

本 PRD 完了時、以下が達成される:

1. **5 bootstrap audit mechanism cells の structural lock-in**: 全 5 cells の resolution が rule file / audit script / new audit script / formal utility lock-in に embed、各 cell に対応する **lock-in test** (= regression 防止 mechanism、test fn name + assertion + reference) が `tests/i_d_pre_*` 系列で fill in 済
2. **Bootstrap utility formal lock-in (= bootstrapping circularity 構造的解消)**: `scripts/verify_line_refs.py` (Method A、Cell 4 v11-7) + `scripts/verify_prd_self_audits.py` (Path E、Cell 1 + 2 + 5 multi-axis、F6/F7 fix integrated) + `scripts/audit-handoff-doc-line-refs.py` (NEW、Cell 3 v11-5) が **formal audit utilities** として regression-tested lock-in、I-D-main spec stage で initial iteration convergence 可能な base state 確立
3. **Self-applied integration**: 本 PRD I-D-pre 自身が新 audit functions + formal utilities で structural compliance verify (= 本 PRD doc 自身が 完成 utilities で audit pass)
4. **I-D-main prerequisite achievement**: I-D-pre 完了 = I-D-main 着手 prerequisite satisfy (= bootstrap utility 完成済 base 確立、I-D-main 24 cells が initial iteration convergence target で再開可能)

### Verifiable success criteria

- 5 cells の matrix table が `audit-prd-rule10-compliance.py` で全 verify function PASS (= 本 PRD 新設 functions 含む)
- 各 cell に対応する `tests/i_d_pre_<candidate>_test.rs` または `tests/i_d_pre_<candidate>_helper_test.rs` が `cargo test` で全 PASS
- 本 PRD doc 自身が `python3 scripts/audit-prd-rule10-compliance.py backlog/I-D-pre-audit-mechanism-bootstrap.md` で exit code 0 (audit pass)
- `scripts/verify_line_refs.py` + `scripts/verify_prd_self_audits.py` + `scripts/audit-handoff-doc-line-refs.py` が **formal regression-tested utilities** として lock-in (= `tests/i_d_pre_method_a_test.rs` + `tests/i_d_pre_path_e_test.rs` + `tests/i_d_pre_handoff_audit_test.rs` で auto-verify)
- `.github/workflows/ci.yml` に `scripts/audit-handoff-doc-line-refs.py` を CI step として integrate、PR merge gate

---

## Scope

### In Scope

本 PRD で **structural lock-in 完成** する 5 bootstrap audit mechanism cells (matrix # 1-5):

- **Cell 1 (v3-6+v4-2、I-D Cell 6+8)**: `verify_pending_verdict_findings_consistency` consolidated audit function 新設 + Path E Axis 2 F7 fix
- **Cell 2 (v5-1、I-D Cell 10)**: `verify_cross_reference_cell_consistency` audit function 新設 + Path E Axis 1 F6 fix (allow-list 置換)
- **Cell 3 (v11-5、I-D Cell 17)**: `scripts/audit-handoff-doc-line-refs.py` 新設 + CI integration + Path E Axis 4 完全 absorption
- **Cell 4 (v11-7、I-D Cell 19)**: Layer 1 (Mechanical) sub-step rule wording 追加 + `scripts/verify_line_refs.py` formal Method A utility lock-in
- **Cell 5 (v13-5、I-D Cell 28)**: Rule 9 / Rule 13 sub-rule rule wording 追加 + `verify_cell_numbering_drift_detection` audit function 新設 + Path E Axis 3 cell-slot vocabulary fork coverage extension

**Self-applied integration verification**: 本 PRD I-D-pre doc 自身が新 audit functions + formal utilities で structural compliance verify (= matrix table self-audit + Spec Review Iteration Log self-applied 13-rule verify + close 前 third-party `/check_job` invocation で convergence criterion satisfy 到達 confirm)。

### Out of Scope

別 PRD で扱う / 永続的に framework 外:

- **PRD I-D-main (framework rule integration cohesive batch、24 cells)**: PRD I-D parent から I-D-pre architectural concern (= 5 audit mechanism cells) を分離した残り 24 cells (= Cell 1, 2, 3, 4, 5, 7, 9, 11, 12, 13, 14, 15, 16, 18, 20, 21, 22, 23, 24, 25, 26, 27, 29, 30 of original I-D matrix) は **post-bootstrap framework full leverage 状態** で I-D-main で initial iteration convergence target。本 PRD I-D-pre 完了 = I-D-main spec stage 着手 prerequisite
- **PRD I-E (lib/CLI API + Web API runtime integration cohesive batch、TODO `[I-E]` entry)**: v13-2 + v13-3 候補は orthogonal architectural concern として PRD I-E に migrate split (2026-05-10 user 確定)、本 PRD I-D-pre と並行可能
- **PRD I-203 (codebase-wide AST exhaustiveness compliance)**: src/ codebase 全体の `_` arm + Tier 1/2 mismatch 一斉解消は別 PRD (= 本 PRD I-D-main で establish する Rule 11 (d-1) `_` arm 全廃を後続適用)
- **PRD I-225 / I-162 / I-205 T14-T16 chain (案 γ Phase 1/2)**: I-D-pre + I-D-main 完了後着手

### Tier 2 honest error reclassify

**N/A**: 本 framework PRD は TS→Rust conversion mechanism を改修しないため、Tier 2 honest error reclassify candidate 不在。framework rule の wording 強化 / audit function 追加 / utility formal lock-in は ideal-implementation-primacy 観点で **structural improvement** として全 In Scope。

---

## Invariants

本 PRD で確立する **5 invariants**。各 invariant は Rule 8 4-item structure (a)(b)(c)(d) で記述、`tests/i_d_pre_invariants_test.rs` に test stub を spec stage で author + Implementation T1〜T8 で fill in。

### INV-1: 5 bootstrap audit mechanism cells structural lock-in

- **(a) Property statement**: 本 PRD で establish する 5 bootstrap audit mechanism cells の resolution が、対応する rule file / audit script / new audit script / formal utility lock-in に **structural embed** され、各 cell に対応する lock-in test が `tests/i_d_pre_*` 系列で `cargo test` PASS する
- **(b) Justification**: 違反すると 5 cells のいずれかが embed 漏れ / test contract 不在 = I-D-main spec stage 着手 prerequisite 未達 = bootstrapping circularity 残存 risk
- **(c) Verification method**: 各 cell の `tests/i_d_pre_<candidate>_test.rs` または `tests/i_d_pre_<candidate>_helper_test.rs` が `cargo test` で PASS、+ 本 PRD doc 自身が `audit-prd-rule10-compliance.py` で exit code 0、+ matrix table の cell # と test fn name の 1-to-1 mapping を本 PRD `## Spec→Impl Dispatch Arm Mapping` section で hard-code して manual verify (= `verify_dispatch_arm_mapping_table` audit function は I-D-main scope のため I-D-pre では manual)。test fn `test_invariant_1_5_cells_lockin_test_collection` (`tests/i_d_pre_invariants_test.rs`) を 集約 entry として、5 candidate-specific tests (= `tests/i_d_pre_audit_extensions_test.rs::test_*` + `tests/i_d_pre_rule_wording_test.rs::test_*` + `tests/i_d_pre_method_a_test.rs::test_*` + `tests/i_d_pre_path_e_test.rs::test_*` + `tests/i_d_pre_handoff_audit_test.rs::test_*` 系列) を delegated execution で aggregate verify
- **(d) Failure detectability**: compile error (test 不在で `cargo test` 失敗) / audit script fail (= structural compliance 違反、CI merge gate で detect)

### INV-2: Bootstrap utility formal lock-in (= bootstrapping circularity 構造的解消)

- **(a) Property statement**: `scripts/verify_line_refs.py` (Method A、264 LOC) + `scripts/verify_prd_self_audits.py` (Path E、368 LOC + F6/F7 fix + Axis 3 cell-slot vocabulary extension) + `scripts/audit-handoff-doc-line-refs.py` (NEW、~150 LOC) が **formal audit utilities** として regression-tested lock-in、各 utility が own test contract で auto-verify mechanism 確立
- **(b) Justification**: 違反すると bootstrap utilities が ad-hoc script 状態残存 = 利用可能だが regression-tested 状態でない = future utility modification で silent under-detection class 再発 risk
- **(c) Verification method**: 各 utility に対応する test (`tests/i_d_pre_method_a_test.rs` + `tests/i_d_pre_path_e_test.rs` + `tests/i_d_pre_handoff_audit_test.rs`) が `cargo test` PASS、+ 各 utility が CI で本 PRD doc + active backlog/ PRD docs に対し run + drift detected で fail。test fn `test_invariant_2_bootstrap_utilities_formal_lockin` (`tests/i_d_pre_invariants_test.rs`) で各 utility の test contract delegated invocation
- **(d) Failure detectability**: utility test contract fail (= regression-tested status 不成立) / utility が PRD doc に対し under-detection class 残存 (= F6/F7 fix 未完成) / CI run 不在 (= INV-3 整合違反)

### INV-3: Audit script CI integration + merge gate (本 PRD scope = audit-handoff-doc-line-refs.py のみ)

- **(a) Property statement**: 本 PRD で新設する `scripts/audit-handoff-doc-line-refs.py` が `.github/workflows/ci.yml` に CI step として integrate 済、PR merge gate として **exit code 非 0 で merge block** される (= I-D-main で establish する全 audit script CI integration の subset、本 PRD では handoff doc audit script 1 件のみ scope 内)
- **(b) Justification**: 違反すると handoff doc line-ref drift が manual reminder 依存になり、structural enforcement 不在 = future PRDs で同 silent drift 再発 risk
- **(c) Verification method**: `.github/workflows/ci.yml` grep で `audit-handoff-doc-line-refs.py` invocation step 存在 verify + GitHub Actions で本 PRD merge 前に actual run + exit code 0 観測。test fn `test_invariant_3_handoff_audit_ci_integration_present` (`tests/i_d_pre_invariants_test.rs`) で CI workflow file 内 invocation step 存在 grep-based assert
- **(d) Failure detectability**: CI run fail (= GitHub Actions log で audit script exit code 非 0) / merge attempt rejected (= merge gate active proof)

### INV-4: Existing PRD docs compliance preservation (delta-based regression lock-in、3-tuple baseline)

- **(a) Property statement**: 本 PRD で establish する新 audit verify mechanisms (= cells 1, 2, 5 audit functions + utility formal lock-in) を 既存 PRD docs (= active backlog/I-050-any-coercion-umbrella.md + backlog/I-205-getter-setter-dispatch-framework.md + backlog/I-D-main-framework-rule-integration-cohesive-batch.md) に対し run、**delta-based regression 0** (= pre-I-D-pre baseline state を preserve、新 audit functions が既存 PRD docs を新たに invalid 化しない)
- **(b) Justification**: 違反すると本 PRD が既存 PRDs を invalid 化、structural lock-in artifacts (= I-205 framework lessons embed + I-D-main spec stage state) が破壊
- **(c) Verification method**: 本 PRD T6 task で既存 PRD docs に対する audit run + delta-based regression 0 確認、CI で active backlog/ 全 PRD doc に対する audit を merge gate 化。test fn `test_invariant_4_existing_prds_baseline_preservation` (`tests/i_d_pre_invariants_test.rs`) で **baseline-aware assertion**: I-050 = pre-existing FAIL state preserve (= audit script exit code 1 + violation message が "missing `## Rule 10 Application` heading" 一致) + I-205 = PASS preserve (= exit code 0) + I-D-main = PASS (= exit code 0) + I-D-pre = PASS (= exit code 0) を 4-tuple assertion logic で verify (= I-050 baseline failure を test loop から exclude with "pre-existing baseline annotation" 方式)
- **(d) Failure detectability**: audit script fail (= 既存 PRD doc が新 verify mechanism で reject、regression detect)

### INV-5: I-D-main prerequisite achievement

- **(a) Property statement**: I-D-pre 完了 = I-D-main spec stage 着手 prerequisite satisfy (= bootstrap utility 完成済 base 確立、I-D-main 24 cells が initial iteration convergence target で再開可能)
- **(b) Justification**: 違反すると I-D-main spec stage が依然として bootstrapping circularity 残存 state で再開、Path B split の structural fix 効果 unrealized
- **(c) Verification method**: I-D-pre close commit 後、I-D-main spec stage の **first third-party adversarial review** で findings count が **Hybrid 4-条件 final rule (C-1 Critical=0 + C-2 High=0 + C-3 trajectory diminishing OR Critical 0 + C-4 meta-finding ratio <= 50%) 全条件 satisfy** 到達 (= I-D-main initial iteration convergence empirical proof)。test fn `test_invariant_5_i_d_main_initial_iteration_convergence` (`tests/i_d_pre_invariants_test.rs`) は I-D-pre close 後 I-D-main spec stage 完了時に retroactive assert (= I-D-pre 完了時点では `#[ignore]` placeholder、I-D-main 完了時に enable + assert)
- **(d) Failure detectability**: I-D-main first round で findings >= prior-state baseline (= bootstrap effect unrealized) / I-D-main spec stage が plateau pattern 再現 (= bootstrapping circularity 構造的未解消)

---

## Cell Numbering Convention (Cell 5 v13-5 candidate self-applied、本 PRD I-D-pre matrix #)

**Single-source-of-truth = matrix #** (Cell 5 v13-5 candidate principle、本 PRD I-D-pre 自身に適用):

- 本 PRD I-D-pre matrix # = canonical identifier (= 1〜5)
- I-D parent migration source = 各 cell の "I-D source" 列で 6+8 / 10 / 17 / 19 / 28 を historical reference として preserve (= traceability 維持)
- 本 PRD doc 内全 cross-reference (= Scope / Invariants / Spec→Impl Dispatch Arm Mapping / Test Plan / Completion Criteria) で I-D-pre matrix # (= 1〜5) を canonical identifier として使用
- Test fn naming convention: `test_<purpose>_cell<N>_<candidate>` 形式 (例: `test_audit_pending_verdict_count_consistency` for Cell 1 v3-6+v4-2)
- Convention drift detection は本 PRD T1-4 (cell 5 / v13-5 audit auto-detect = `verify_cell_numbering_drift_detection` 新設) で auto verify

**Vocabulary fork prohibition** (= Path E Axis 3 narrow `CELL_SLOT_AS_IDENTIFIER_RE` detection で本 PRD scope absorption):
- "cell #" / "Cell N" / "candidate ID" / "I-D source" / "matrix #" は **single canonical naming** として本 PRD doc で使用 (= human-side convention)
- **Identifier-level fork** (= "cell-slot N" / "cell-slot #N" 数値 identifier 用法) は本 PRD scope で audit script 経由 detect (= `CELL_SLOT_AS_IDENTIFIER_RE`)
- **Descriptive uses** (= "cell-slot occurrence" / "cell-slot vocabulary fork" 等 concept descriptors) は legitimate (= human convention 表現として allow)
- "cell-slot occurrence" wording は本 PRD I-D-pre では使用しない (= I-D parent doc の cross-cutting cells multi-layer occurrence concept は I-D-main scope、I-D-pre 5 cells は cross-cutting なし)
- **Broader vocabulary fork detection** (= "cell # / candidate ID / matrix #" 間の semantic-level mixed canonical naming detection、= 例: 同 PRD 内で "cell 1" と "matrix # 1" を interchangeably 使用) は別 framework concern として deferred、TODO `[I-D-future-vocab-fork]` 候補

---

## Design

### Technical Approach

5 candidates の resolution を以下 2 layer で structural integrate (= I-D parent 4 layer 構造の subset、Layer 3 procedure step + Layer 4 skill / command は I-D-pre scope 外):

#### Layer 1: Audit script extensions + utility formal lock-in (T1-pre = 6 sub-tasks total)

**Mapping (cell → T1-pre sub-task)**:
- **New verify functions in audit-prd-rule10-compliance.py = 3 functions**: T1-pre-1 (Cell 1, v3-6+v4-2 = `verify_pending_verdict_findings_consistency` consolidated 新設、F7 fix integrated) / T1-pre-2 (Cell 2, v5-1 = `verify_cross_reference_cell_consistency` 新設、F6 fix = allow-list 置換 integrated) / T1-pre-4 (Cell 5, v13-5 = `verify_cell_numbering_drift_detection` 新設)
- **New audit script (separate file) + CI integration = 2 sub-tasks**: T1-pre-3a (Cell 3, v11-5 part 1 = `scripts/audit-handoff-doc-line-refs.py` 新設) + T1-pre-3b (Cell 3, v11-5 part 2 = `.github/workflows/ci.yml` CI step integration)
- **Existing utility formal lock-in = 2 sub-tasks**: T1-pre-5 (Cell 4, v11-7 part 2 = `scripts/verify_line_refs.py` Method A formal lock-in、regression-tested utility status 確立) + T1-pre-6 (Cells 1+2+5, v3-6+v4-2 / v5-1 / v13-5 part 2 = `scripts/verify_prd_self_audits.py` Path E formal lock-in、F6/F7 fix integrated + Axis 3 cell-slot vocabulary extension integrated)
- **Total T1-pre sub-tasks = 6 (= 3 new verify functions + 1 new audit script + 1 CI integration step + 1 Method A formal lock-in + 1 Path E formal lock-in、ただし 1 + 1 = 2 utility formal lock-in)、arithmetic: 3 NEW functions + 1 NEW script + 1 CI + 2 formal lock-in = 7 sub-tasks but T1-pre-3 split into 3a/3b + T1-pre-6 covers 3 cells in 1 task = 6 distinct sub-tasks**

**Approach**:
- `scripts/audit-prd-rule10-compliance.py` (906 行 / 26 functions) に **3 new verify functions** = 3 audit script 内 改修
- `scripts/audit-handoff-doc-line-refs.py` (NEW) を 1 件新設 (Cell 3 / v11-5)
- `scripts/verify_line_refs.py` (264 LOC、Iteration v12 bootstrap 由来) を formal Method A utility lock-in (= regression-tested utility status 確立、test contract 整備)
- `scripts/verify_prd_self_audits.py` (368 LOC、Iteration v16 bootstrap 由来) を formal Path E utility lock-in + F6/F7 fix integrated (= Axis 1 threshold "5" → spec-traceable allow-list / Axis 2 TS-X over-exclusion → post-v15 wording presence requirement) + Axis 3 cell-slot vocabulary fork coverage extension
- 各 verify function は **per-cell** structural pattern detection、**single-responsibility** principle (DRY) で書く

**File structure changes**:
- `scripts/audit-prd-rule10-compliance.py`: 906 行 → ~1050 行見込み (= 3 new functions + helper utilities 抽出)
- `scripts/audit-handoff-doc-line-refs.py`: 新設 ~150 行 (= handoff doc grep + line ref existence check)
- `scripts/verify_line_refs.py`: 264 行 → ~280 行見込み (= formal lock-in metadata + test integration interface)
- `scripts/verify_prd_self_audits.py`: 368 行 → ~480 行見込み (= F6/F7 fix + Axis 3 cell-slot vocabulary extension + helper refactor)

#### Layer 2: Rule wording strengthening (cells 4, 5 = 2 candidates)

**Approach**:
- `.claude/rules/check-job-review-layers.md` (338 行 / 4 layers) に Layer 1 sub-step 拡張 (Cell 4 / v11-7 = factual accuracy semantic check)
- `.claude/rules/spec-stage-adversarial-checklist.md` (518 行 / 13 rules) に Rule 9 / Rule 13 sub-rule 拡張 (Cell 5 / v13-5 = cell numbering convention single-source-of-truth)
- Each rule wording 拡張は **既存 rule の sub-rule extension** form (= 新 rule 全廃、既存 rule の sub-rule 番号 増加 = 後方互換維持)
- Cell 4 (v11-7 Layer 1 wording) と T1-pre-5 (Method A formal lock-in) は coordinated implementation = 同 cell 内 dual-layer change (= Cell 4 が Layer 1 + Layer 2 dual-layer slot)
- Cell 5 (v13-5 Rule 9/13 wording) と T1-pre-4 (verify_cell_numbering_drift_detection) と T1-pre-6 (Path E Axis 3 extension) は coordinated = triple-layer cohesive

**File structure changes**:
- `.claude/rules/check-job-review-layers.md`: 338 行 → ~380 行見込み (= Layer 1 sub-step 追加、Versioning section に v1.8 entry 追加)
- `.claude/rules/spec-stage-adversarial-checklist.md`: 518 行 → ~580 行見込み (= Rule 9 / Rule 13 sub-rule 追加)

### Spec→Impl Dispatch Arm Mapping (Rule 9 (a) compliance、本 PRD I-D-pre self-applied)

各 in-scope matrix cell ↔ Implementation Stage Tasks T1-pre / T2-pre の **1-to-1 correspondence table**:

| Cell # | Candidate | I-D source | Implementation Task | Test contract path | Audit verify (本 PRD で establish) |
|--------|-----------|------------|---------------------|--------------------|--------|
| 1 | v3-6+v4-2 | I-D Cell 6+8 | T1-pre-1 (audit: verify_pending_verdict_findings_consistency 新設、F7 fix integrated) + T1-pre-6 part (Path E Axis 2 formal lock-in) | `tests/i_d_pre_audit_extensions_test.rs::test_audit_pending_verdict_count_consistency` + `tests/i_d_pre_path_e_test.rs::test_path_e_axis2_post_v15_wording_detection` | self-applied: 本 PRD で pending verdict 不在 (Spec stage 完了状態) PASS |
| 2 | v5-1 | I-D Cell 10 | T1-pre-2 (audit: verify_cross_reference_cell_consistency 新設、F6 fix = allow-list 置換 integrated) + T1-pre-6 part (Path E Axis 1 formal lock-in) | `tests/i_d_pre_audit_extensions_test.rs::test_audit_cross_reference_cell_appearance_consistency` + `tests/i_d_pre_path_e_test.rs::test_path_e_axis1_allow_list_replacement` | self-applied: 本 PRD の matrix と cross-reference contexts (Scope / Invariants / Spec→Impl Mapping / Test Plan) で 5 cells appearance consistency PASS |
| 3 | v11-5 | I-D Cell 17 | T1-pre-3a (`scripts/audit-handoff-doc-line-refs.py` 新設) + T1-pre-3b (`.github/workflows/ci.yml` CI step integration) | `tests/i_d_pre_handoff_audit_test.rs::test_audit_handoff_doc_line_refs_drift_detection` | CI run でグリーン |
| 4 | v11-7 | I-D Cell 19 | T2-pre-1 (Layer 1 sub-step: factual accuracy semantic check rule wording 追加) + T1-pre-5 (`scripts/verify_line_refs.py` Method A formal lock-in) | `tests/i_d_pre_rule_wording_test.rs::test_layer1_factual_accuracy_semantic_check_documented` + `tests/i_d_pre_method_a_test.rs::test_method_a_line_ref_drift_detection` | manual checklist (Layer 1 wording) + self-applied (Method A utility が本 PRD doc を auto-verify) |
| 5 | v13-5 | I-D Cell 28 | T2-pre-2 (Rule 9 / Rule 13 sub-rule: Cell numbering convention single-source-of-truth 追加) + T1-pre-4 (audit: verify_cell_numbering_drift_detection 新設) + T1-pre-6 part (Path E Axis 3 cell-slot vocabulary extension) | `tests/i_d_pre_rule_wording_test.rs::test_rule9_cell_numbering_convention_documented` + `tests/i_d_pre_audit_extensions_test.rs::test_audit_cell_numbering_drift_detection` + `tests/i_d_pre_path_e_test.rs::test_path_e_axis3_cell_slot_vocabulary_coverage` | self-applied: 本 PRD `## Cell Numbering Convention` section 内 explicit declare PASS |

**Mapping completeness verify**: 5 cells × 1-to-1 task mapping = 全 cells が T1-pre-1〜T1-pre-6 + T2-pre-1〜T2-pre-2 series tasks に exact dispatch (= no double-claim、no fall-through)。本 PRD I-D-pre では `verify_dispatch_arm_mapping_table` audit function (= I-D parent Cell 9 / v4-3 candidate、I-D-main scope) は未実装、本 mapping table は **manual review で integrity 担保** (= 5 cells small-scope のため manual tractable、I-D-main で formal audit auto-verify 完成)。

### Design Integrity Review

Per `.claude/rules/design-integrity.md` checklist:

- **Higher-level consistency**:
  - 本 PRD の改修対象 (`scripts/audit-*.py` + `scripts/verify_*.py` + `.claude/rules/*.md` + `.github/workflows/ci.yml`) は **PRD framework infrastructure** layer に属し、上位 layer (= main project conversion pipeline) と orthogonal
  - audit script 拡張 / utility formal lock-in / rule wording 強化 は各々が **single architectural concern** (= bootstrap audit mechanism construction = bootstrapping circularity 構造的解消) に subordinate、higher-level consistency 維持
  - I-D-main との boundary は明確 (= 本 PRD I-D-pre 完了 prerequisite で I-D-main 着手、両 PRD は orthogonal architectural concern boundary)

- **DRY (knowledge duplication)**:
  - cells 1 / 2 / 5 の audit functions は `audit-prd-rule10-compliance.py` 内 共通 helper utilities (= section parsing / cell # extraction / pattern matching) を抽出して DRY 解消
  - Method A / Path E utilities の formal lock-in は既存 264 + 368 LOC を base、新規 implementation 不要 (= utility 自身は既に存在、formal lock-in mechanism = test contract + regression-testing 整備のみ)
  - Cell 4 (v11-7) Layer 1 wording と Method A utility の coordinated implementation は cohesive (= 同 audit concern の rule wording 側 + utility 側、両者が同 cell に属する structural cohesion)

- **Orthogonality**:
  - 5 cells は mutually distinct architectural sub-concerns (= pending verdict / cross-reference / handoff doc line-ref / line-ref factual accuracy / cell numbering convention) で orthogonal
  - 各 cell の resolution は他 cells の resolution に依存しない (= 独立 implementable、parallel work 可能)

- **Coupling**:
  - Method A utility (Cell 4) と Path E utility (Cells 1+2+5) の formal lock-in 間は orthogonal (= 異なる utility、異なる concern coverage)
  - audit-prd-rule10-compliance.py 拡張 (Cells 1+2+5) は同 file 内 sub-functions、cohesive grouping
  - audit-handoff-doc-line-refs.py NEW (Cell 3) は完全独立 file、coupling 不在

### Impact Area

本 PRD で touch する file:
- `.claude/rules/check-job-review-layers.md` (Cell 4 Layer 1 sub-step)
- `.claude/rules/spec-stage-adversarial-checklist.md` (Cell 5 Rule 9/13 sub-rule)
- `scripts/audit-prd-rule10-compliance.py` (Cells 1+2+5 audit functions)
- `scripts/verify_line_refs.py` (Cell 4 Method A formal lock-in)
- `scripts/verify_prd_self_audits.py` (Cells 1+2+5 Path E formal lock-in + F6/F7 fix + Axis 3 extension)
- `scripts/audit-handoff-doc-line-refs.py` (Cell 3 NEW)
- `.github/workflows/ci.yml` (Cell 3 CI integration)
- `tests/i_d_pre_*` 系列 NEW (= invariants / audit extensions / rule wording / method_a / path_e / handoff_audit test files)

### Semantic Safety Analysis

**N/A**: 本 framework PRD は production code (= TS→Rust conversion mechanism) を改修しない。framework rule の wording 強化 / audit function 追加 / utility formal lock-in は ideal-implementation-primacy 観点で **structural improvement**、semantic safety analysis は適用範囲外 (= conversion semantic 不変)。

---

## Spec Stage Tasks (Stage 1 artifacts 完成 task)

### TS-pre-0: Cartesian product matrix completeness

- **Work**: 本 PRD `## Problem Space ### 組合せマトリクス (5 cells)` section の Cartesian product 5 cells 完全 enumerate 確認 (Path B split で I-D parent から I-D-pre architectural concern boundary に migrate)
- **Completion criteria**: matrix table 5 rows + 全 cells に Ideal output 記載 + 全 cells に判定 (= ✗ 全 5 cells)
- **Status**: COMPLETE (本 spec stage v1 で fill 済)

### TS-pre-1: Current Rule/Script State Snapshot completion

- **Work**: 本 PRD `## Oracle Observations` section に 5 cells 全 sub-section embed (= Cell 1〜Cell 5、Current state + Pre-state probe + Path E utility partial coverage + Ideal post-state + Rationale)
- **Completion criteria**: 5 cells sub-section 全 embed + 各 cell の Pre-state probe コマンド出力結果 record (= grep 0 hits 等)
- **Status**: COMPLETE (本 spec stage v1 で fill 済)

### TS-pre-2: Test contract stub authoring (`tests/i_d_pre_*` 系列)

- **Work**: 本 PRD で言及される全 test fn に対し、`tests/i_d_pre_audit_extensions_test.rs` + `tests/i_d_pre_rule_wording_test.rs` + `tests/i_d_pre_method_a_test.rs` + `tests/i_d_pre_path_e_test.rs` + `tests/i_d_pre_handoff_audit_test.rs` + `tests/i_d_pre_invariants_test.rs` で stub `#[test] #[ignore]` author (= Implementation stage で `#[ignore]` 解除 + assertion fill in)
- **Completion criteria**: 全 test fn が stub state で `cargo test` recognize (= compile pass、`#[ignore]` 状態) + test fn name が Spec→Impl Dispatch Arm Mapping table と 1-to-1 sync
- **Status**: PENDING (Implementation stage 着手前に completion mandatory)

### TS-pre-3: Self-applied audit script verify run

- **Work**: 本 PRD doc 自身に対し `python3 scripts/audit-prd-rule10-compliance.py backlog/I-D-pre-audit-mechanism-bootstrap.md` run + 既存 26 functions PASS + 新 3 functions (= 本 PRD T1-pre-1/2/4 で実装) の test fixture run
- **Completion criteria**: 既存 26 functions exit code 0 + 新 functions の synthetic PRD fixture-based unit test PASS
- **Status**: PENDING (Implementation T1-pre 完了後 verify)

### TS-pre-4: Impact Area audit findings record

- **Work**: 本 PRD `## Impact Area Audit Findings` section に Pre-draft ast-variant audit (N/A reasoning) + Adapted Impact Area Review + Empirical file path verify (= 全 listed file に対する `ls -la` + `wc -l` 結果) record
- **Completion criteria**: 全 listed file の existence + size + LOC + last modified が table format で record + N/A reasoning が Rule 12 (e-3) Permitted reason に traceable
- **Status**: COMPLETE (本 spec stage v1 で fill 済)

### TS-pre-5: 13-rule self-applied verify (Spec Stage Self-Review)

- **Work**: 本 PRD `## Spec Review Iteration Log` section v1 entry で `.claude/rules/spec-stage-adversarial-checklist.md` 13-rule の全項目 verification、findings 検出 → 該当 rule の sub-rule に traceable
- **Completion criteria**: 13-rule 全 PASS (= Critical 0 + High 0 finding) + 検出 findings 全 fix 後 v1 → v2 iteration で再 verify (recursive)
- **Status**: IN PROGRESS (本 v1 draft で実施、findings → fix → v2 で convergence target)

---

## Implementation Stage Tasks

### T1-pre: Audit script extensions + utility formal lock-in (= 6 sub-tasks)

#### T1-pre-1: verify_pending_verdict_findings_consistency 新設 + F7 fix integrated (Cell 1 / v3-6+v4-2)

- **Work**: `scripts/audit-prd-rule10-compliance.py` に新 function 追加 (= sub-rule 表内 `(TS-X 後 verify)` 等 pending verdict pattern detect + Critical=0 claim ↔ stale verdict consistency sub-check 集約 = v3-6 + v4-2 cohesive batch + F7 fix = post-v15 wording presence 要求で TS-X over-exclusion 解消)
- **Completion criteria**: function 追加 + `tests/i_d_pre_audit_extensions_test.rs::test_audit_pending_verdict_count_consistency` PASS (= synthetic PRD doc fixture で pending verdict 残存 + Critical=0 claim → flag detect)
- **Depends on**: None (Layer 1 の最初の sub-task)
- **Prerequisites**: Spec stage TS-pre-1 〜 TS-pre-5 全 complete

#### T1-pre-2: verify_cross_reference_cell_consistency 新設 + F6 fix integrated (Cell 2 / v5-1)

- **Work**: `scripts/audit-prd-rule10-compliance.py` に新 function 追加 (= matrix と各 cross-reference context の cell # appearance consistency auto verify + F6 fix = Axis 1 threshold "5" arbitrary heuristic を spec-traceable allow-list に置換)
- **Completion criteria**: function 追加 + `tests/i_d_pre_audit_extensions_test.rs::test_audit_cross_reference_cell_appearance_consistency` PASS (= synthetic PRD doc fixture で cross-ref missing pattern detect + allow-list exception 動作 verify)

#### T1-pre-3a: scripts/audit-handoff-doc-line-refs.py 新設 (Cell 3 / v11-5 part 1)

- **Work**: `scripts/audit-handoff-doc-line-refs.py` (~150 行) を新設、handoff doc grep `\.rs:\d+` pattern 抽出 + each `<src_file>:<line>` reference の actual file 存在 check + line content syntactic verify
- **Completion criteria**: script exists + standalone test (`python3 scripts/audit-handoff-doc-line-refs.py doc/handoff/`) で existing handoff doc に対し PASS or detected drift を report

#### T1-pre-3b: .github/workflows/ci.yml integration (Cell 3 / v11-5 part 2)

- **Work**: `.github/workflows/ci.yml` に `python3 scripts/audit-handoff-doc-line-refs.py doc/handoff/` step を追加、PR merge gate active 化
- **Completion criteria**: GitHub Actions log で audit step run 観測 + drift introduce で merge block 確認 (= INV-3 evidence)
- **Depends on**: T1-pre-3a

#### T1-pre-4: verify_cell_numbering_drift_detection 新設 (Cell 5 / v13-5 audit part)

- **Work**: `scripts/audit-prd-rule10-compliance.py` に新 function 追加 (= matrix # canonical identifier ↔ Spec→Impl Mapping table cell # ↔ 各 cross-reference context cell # の 三者 1-to-1 mapping verify + cell-slot vocabulary fork drift detection)
- **Completion criteria**: function 追加 + `tests/i_d_pre_audit_extensions_test.rs::test_audit_cell_numbering_drift_detection` PASS (= synthetic PRD doc fixture で convention drift detect + vocabulary fork detect)

#### T1-pre-5: scripts/verify_line_refs.py Method A formal lock-in (Cell 4 / v11-7 utility part)

- **Work**: `scripts/verify_line_refs.py` (264 LOC、Iteration v12 bootstrap 由来) を formal Method A utility lock-in (= regression-tested utility status 確立、test contract 整備)。具体: (a) utility metadata header (= purpose / coverage scope / regression-tested status) embed + (b) `tests/i_d_pre_method_a_test.rs` で utility own behavior の auto-verify mechanism (= synthetic PRD doc fixture-based positive + negative tests)
- **Completion criteria**: `tests/i_d_pre_method_a_test.rs::test_method_a_line_ref_drift_detection` PASS (= synthetic PRD doc fixture で heading-based line-ref drift detect、negative case で no-drift PASS) + utility metadata header embed verify

#### T1-pre-6: scripts/verify_prd_self_audits.py Path E formal lock-in + F6/F7 fix + Axis 3 extension + 4 additive utility fixes (Cells 1+2+5 / v3-6+v4-2 / v5-1 / v13-5 utility part)

- **Work**: `scripts/verify_prd_self_audits.py` (368 LOC、Iteration v16 bootstrap 由来) を formal Path E utility lock-in。具体:
  - (a) **F6 fix** (Axis 1 threshold "5" → spec-traceable allow-list、Cell 2 / v5-1 audit function と coordinated)
  - (b) **F7 fix** (Axis 2 TS-X over-exclusion → post-v15 wording presence 要求、Cell 1 / v3-6+v4-2 audit function と coordinated)
  - (c) **Axis 3 narrow detection** (cell-slot identifier-level fork detection = `CELL_SLOT_AS_IDENTIFIER_RE`、Cell 5 / v13-5 audit function と coordinated、A3 fix で scope refined)
  - (d) **utility metadata header embed**
  - (e) `tests/i_d_pre_path_e_test.rs` で utility own behavior の auto-verify mechanism
  - **Additive utility fixes (Iteration v2 retroactive embed = A1/A2 fix /check_job L3-1/L3-3 由来)**:
    - (f) **`find_section_range` bug fix** = section end calculation EOF-fallback (= former `headings[-1].line + 1` で last sub-heading 後 content excluded bug、A5 fix で `total_lines` parameter 追加 = sentinel 排除)
    - (g) **`find_repo_root` addition** = robust repo root detection via Cargo.toml ancestor walk (= former `prd_path.parent.parent` fragile for test fixtures)
    - (h) **`IMPACT_AREA_BYTES_RE` extension** = .toml/.json extension support + 12-digit byte counts (~ 1 TB) (A7 fix /check_job L1-4 由来)
    - (i) **`expand_cell_list` multi-pattern** = case-insensitive (Pattern 1) + standalone "Cell N" (Pattern 2) + bracket-list `{N, ...}` (Pattern 3) + markdown table column "| N |" (Pattern 4)
    - (j) **`SECTION_COVERAGE_POLICY` 5 sections coverage** = Cell 2 v5-1 oracle observation の 7 contexts grouped (A2 fix /check_job L3-3 由来): Scope full + Spec→Impl Mapping full + Invariants partition_ok + Test Plan partition_ok + Implementation Stage Tasks partition_ok
    - (k) **`STALE_STATUS_PATTERNS` dead code 削除** = empty list `[]` (= I-D-parent-specific legacy pattern grep 0 hits empirical confirm、A4 fix /check_job L3-5 由来)
- **Completion criteria**: `tests/i_d_pre_path_e_test.rs::test_path_e_axis1_allow_list_replacement + test_path_e_axis2_post_v15_wording_detection + test_path_e_axis3_cell_slot_vocabulary_coverage + test_path_e_axis4_external_file_drift_detection + test_path_e_utility_metadata_header_embed` 全 PASS (= 5 tests) + I-D-pre / I-D-main 両 PRD 0 drifts (post 5 sections coverage extension) empirical confirm

**T1-pre 共通 work**: 各 sub-task で
- 該当 verify function を `audit-prd-rule10-compliance.py` に追加 (or 新 audit script に追加 or 既存 utility に formal lock-in 整備)
- 対応 test contract `tests/i_d_pre_*` を `#[test]` (Spec stage の `#[ignore]` から解除) + assertion implement
- 共通 helper utilities (= section parsing / cell # extraction / pattern matching) を抽出 (DRY refactor)
- audit script run で本 PRD doc 自身を audit、新 verify function PASS 確認

**T1-pre 共通 completion criteria**: 全 6 sub-tasks 完了 + 全 audit改修 PASS for 本 PRD doc + active backlog/ PRDs に対し **INV-4 baseline-aware delta-based regression 0** (= I-050 = pre-existing FAIL state preserve、I-205 + I-D-main + I-D-pre = exit code 0、4-tuple INV-4 spec satisfy)

### T2-pre: Rule wording strengthening (= 2 sub-tasks)

#### T2-pre-1: check-job-review-layers.md Layer 1 sub-step 追加 (factual accuracy、Cell 4 / v11-7 rule part)

- **Work**: `.claude/rules/check-job-review-layers.md` Layer 1 (Mechanical) sub-step 追加 (= "factual accuracy semantic check: 修正 doc / comment 内の固有名詞 (PRD ID / Iteration v# / task ID / file path / line ref) が claim する意味と一致することを semantic check (= 単純 grep + 存在 check ではなく、reference が claim する意味論的 context との一致を verify)")。Versioning section に v1.8 entry 追加。`scripts/verify_line_refs.py` (Method A) を rule wording 内 reference として明示 (= structural enforcement mechanism として hard-code)
- **Completion criteria**: rule file embed + `tests/i_d_pre_rule_wording_test.rs::test_layer1_factual_accuracy_semantic_check_documented` PASS (= grep-based assertion で specific text pattern 存在 verify)

#### T2-pre-2: spec-stage-adversarial-checklist.md Rule 9 / Rule 13 sub-rule 追加 (Cell numbering、Cell 5 / v13-5 rule part)

- **Work**: `.claude/rules/spec-stage-adversarial-checklist.md` Rule 9 / Rule 13 sub-rule 追加 (= (a) framework rule で "single-source-of-truth = matrix #" mandatory 化 + (b) framework rule で "convention drift detection" を audit script 経由 auto-detect + (c) PRD spec stage で cell numbering convention を `## Cell Numbering Convention` section 内に explicit declare mandatory)。Versioning section に v1.8 entry 追加。`verify_cell_numbering_drift_detection` (T1-pre-4 で実装) を rule wording 内 reference として明示
- **Completion criteria**: rule file embed + `tests/i_d_pre_rule_wording_test.rs::test_rule9_cell_numbering_convention_documented` PASS

**T2-pre 共通 work**: 各 sub-task で
- 該当 rule sub-rule を rule file に embed (既存 rule の sub-rule extension form、後方互換維持)
- Versioning section に v1.8 entry 追加 (= 本 PRD I-D-pre の self-applied integration as cumulative version)
- 対応 test contract を grep-based assertion で実装 (= rule file 内 specific text pattern 存在 verify)

**T2-pre 共通 completion criteria**: 全 2 sub-tasks 完了 + `tests/i_d_pre_rule_wording_test.rs` 全 PASS + rule file の Versioning section v1.8 entry 存在

### T6-pre: Existing PRD docs compliance maintenance (= 1 task、INV-4 baseline-aware delta-based regression 0 spec)

- **Work**: 本 PRD T1-pre / T2-pre で establish した新 audit verify mechanisms に対し、active backlog/ 全 PRD docs (= `backlog/I-050-any-coercion-umbrella.md` + `backlog/I-205-getter-setter-dispatch-framework.md` + `backlog/I-D-main-framework-rule-integration-cohesive-batch.md` + 本 PRD doc) を audit run、**INV-4 baseline-aware delta-based regression 0** 確認 (= I-050 = pre-existing FAIL state preserve、I-205 + I-D-main + I-D-pre = exit code 0 = 4-tuple baseline assertion、INV-4 evidence)
- **Completion criteria**: `for prd in backlog/*.md; do python3 scripts/audit-prd-rule10-compliance.py "$prd"; done` の **delta-based regression 0** 達成 (= I-050 baseline FAIL state 維持 + I-205 + I-D-main + I-D-pre exit code 0、4-tuple INV-4 spec satisfy)、CI で active backlog/ 全 PRD doc に対する audit を merge gate 化 (= `.github/workflows/ci.yml` integration verify、INV-3 evidence)

### T7-pre: Self-applied integration final verify (= 1 task、5 cells cumulative)

- **Work**: 本 PRD doc 自身に対する self-applied + third-party `/check_job` invocation chain (= I-D parent Cell 27 v13-4 / Cell 30 v13-7 candidates は I-D-main scope だが、本 PRD では simplified self-applied + single third-party round で convergence target、small-scope のため)、convergence criterion で findings count 0 到達まで recursive iteration
- **Completion criteria**: third-party `/check_job` invocation で **Critical = 0 + High = 0** 到達 (= simplified convergence、5 cells small-scope で initial iteration target)、最終 iteration history を `## Spec Review Iteration Log` v(N+1) に record (= Implementation stage 完了 self-applied review)

### T8-pre: Documentation + plan.md update + PRD close (= 1 task)

- **Work**: `doc/handoff/design-decisions.md` に本 PRD I-D-pre の bootstrap audit mechanism construction lessons section embed (= 5 cells の resolution lessons + Path B split rationale empirical proof + bootstrapping circularity 構造的解消 verify)、plan.md 更新 (= 案 γ Phase 0 = I-D-main 着手 ready 表示 = bootstrap utility 完成 base 確立)、PRD close commit
- **Completion criteria**: design-decisions.md 新 section 存在 + plan.md update 確認 + `[CLOSE] I-D-pre PRD 完了` commit 作成

---

## Spec Review Iteration Log

**Historical line refs preservation policy** (Iteration v1 で formal 確定 = I-D parent Method A bootstrap policy 継承): 本 section 内各 Iteration entry の **fix log + finding description で言及される line refs は entry 作成時点での file state を反映** する historical record。post-entry の file growth で line numbers が drift する場合、historical line refs は **preserve as-written** で historical accuracy 維持。**Current spec sections** (= `## Background` / `## Problem Space` / `## Oracle Observations` / `## Cell Numbering Convention` / `## Goal` / `## Scope` / `## Invariants` / `## Design` / `## Spec Stage Tasks` / `## Implementation Stage Tasks` / `## Test Plan` / `## Completion Criteria` / `## 🔗 Cross-references` 等) の line refs は **`scripts/verify_line_refs.py` で auto-detect + empirical sync 必須** (= I-D parent Iteration v12 で Method A bootstrap、Cell 4 / v11-7 audit auto-verify mechanism の早期実装、本 PRD I-D-pre で formal lock-in)。

### Iteration v1 (2026-05-10、本 draft 初版、Path B split user 確定由来)

- **Source state**: PRD I-D parent Spec Stage Iteration v17 plateau (= 9 findings、Critical 1 / High 4、2/4 PASS 3 round 連続) → user 方針確認 = Path B (PRD split into I-D-pre + I-D-main) 採用 + I-D-pre scope = 5 bootstrap cells (Cell 19/10/6+8/17/28) as-is 確定 2026-05-10
- **本 draft work**: I-D parent matrix 30 cells から I-D-pre architectural concern (= audit mechanism construction) 5 logical cells (= 6 row numbers: 6+8/10/17/19/28) を migration、cell # canonical identifier を **renumbered 1-5** (Cell 5 v13-5 single-source-of-truth principle 適用)、I-D source column で historical traceability 維持
- **13-rule self-applied verify (本 v1 draft)**: 本 v1 draft 完了直後実施、結果を v2 entry で record (= recursive self-applied verify pattern、I-D parent Rule 13 13-3 適用)
- **Findings count (= initial v1 draft completion 後 self-applied verify、TS-pre-5 task で実施)**: TBD (= v1 draft 完了直後 self-applied verify run で findings record、本 entry 後で update)
- **Convergence criterion application (simplified for I-D-pre small-scope)**: I-D parent Hybrid 4-条件 final rule の subset = **Critical = 0 + High = 0 + 1 third-party round** で simplified convergence (= 5 cells small-scope のため initial iteration convergence target、I-D parent の C-3 trajectory diminishing + C-4 meta-finding ratio は scope 上 不要)
- **Spec stage 完了判定**: PENDING (本 v1 draft 完了直後 self-applied verify で findings 検出 → fix → v2 で convergence target、third-party adversarial review で final verify)

### Iteration v2 (2026-05-11、Implementation Phase 1+2 完了 + /check_job 4-layer review 結果 retroactive embed = A1 fix /check_job L3-1)

- **Source state**: Implementation Phase 1 (test infra setup) + Phase 2 (Method A + Path E formal lock-in) 完了 → /check_job 4-layer review で 9 findings 発見 (= L1-1〜L1-6 + L3-1〜L3-6 + L4-T1〜L4-T5 grouped)
- **/check_job review findings (Spec gap = 3 / Implementation gap = 2 / Review insight = 1)**:
  - **L3-1 (Spec gap、High)**: T1-pre-6 で additive fix 4 件混入 = `find_section_range` bug fix + `find_repo_root` addition + `IMPACT_AREA_BYTES_RE` extension + `expand_cell_list` multi-pattern。本 v2 entry で **retroactive embed** = T1-pre-6 spec extension として 4 件 sub-task 化 (= A1 fix)
  - **L3-3 (Spec gap、High)**: SECTION_COVERAGE_POLICY 2/7 sections coverage = Cell 2 v5-1 oracle observation の partial implementation。**A2 fix** で 5 sections (= 7 contexts grouped) coverage に拡張 (= Scope full + Spec→Impl Mapping full + Invariants partition_ok + Test Plan partition_ok + Implementation Stage Tasks partition_ok)
  - **L3-2 (Implementation gap、Medium)**: Cell 5 oracle observation "完全 absorption" claim vs CELL_SLOT_AS_IDENTIFIER_RE narrow scope の divergence。**A3 fix** で oracle wording を narrow detection と sync (= identifier-level fork detection scope に refine、broader vocabulary fork は別 PRD `[I-D-future-vocab-fork]` 候補に deferred)
  - **L3-5 (Implementation gap、Medium)**: STALE_STATUS_PATTERNS legacy I-D-parent-specific dead code preserved。**A4 fix** で empty list `[]` に置換 (= grep 0 hits empirical confirm)
  - **L3-6 (Spec gap、Medium)**: Test Plan section に fixture pattern + helper convention spec 不在。**A6 fix** で per-axis isolated fixture design pattern + test helper convention を Test Plan section に embed
  - **L1-1 (Review insight)**: stub tests with `unimplemented!()` panic on `--include-ignored` = project precedent (I-205) と一致、acceptable
  - **L1-2 (Low)**: `find_section_range` sentinel `10**9` magic number。**A5 fix** で `is_historical_iteration_log_line` に total_lines parameter 追加 = API 整合
  - **L1-4 (Low)**: IMPACT_AREA_BYTES_RE 9-digit max byte limit。**A7 fix** で 12-digit (~ 1 TB) に拡張
  - **L1-5 (Low)**: Method A test "non-zero count check" vs Path E test "exact count check" inconsistency。**A8 fix** で Method A test も exact count assertion (= "drifts: 2") に refine
- **T1-pre-6 spec extension (additive fix 4 件 retroactive embed = A1 fix L3-1)**:
  - (a) `find_section_range` bug fix = section end calculation EOF-fallback (= former `headings[-1].line + 1` で last sub-heading 後 content excluded bug)
  - (b) `find_repo_root` addition = robust repo root detection via Cargo.toml ancestor walk (= former `prd_path.parent.parent` fragile for test fixtures)
  - (c) `IMPACT_AREA_BYTES_RE` extension = .toml/.json extension support + 12-digit byte counts
  - (d) `expand_cell_list` multi-pattern = case-insensitive (Pattern 1) + standalone "Cell N" (Pattern 2) + bracket-list `{N, ...}` (Pattern 3) + markdown table column "| N |" (Pattern 4、A2 fix follow-up)
- **Findings count**: Critical 0 / High 2 (L3-1 L3-3 = 全 fix 完了 by Phase 3) / Medium 3 (L3-2 L3-5 L3-6 = 全 fix 完了) / Low 4 (L1-2 L1-4 L1-5 + L1-3/L1-6 deferred to Phase 3-6) / Review insight 1 (L1-1)
- **Resolution status (post v2 fix work)**:
  - A1 (Iteration v2 entry retroactive embed): ✓ DONE (本 entry)
  - A2 (SECTION_COVERAGE_POLICY 5 sections + Pattern 4 extension): ✓ DONE
  - A3 (Cell 5 oracle wording refine): ✓ DONE
  - A4 (STALE_STATUS_PATTERNS dead code 削除): ✓ DONE
  - A5 (is_historical_iteration_log_line API 整合): ✓ DONE
  - A6 (Test Plan fixture pattern + helper convention spec embed): ✓ DONE
  - A7 (IMPACT_AREA_BYTES_RE 12-digit): ✓ DONE
  - A8 (Method A test exact count): ✓ DONE
  - A9 (cargo clippy / fmt verify): IN PROGRESS (Phase 6 T6-pre quality gate で final verify)
- **Convergence criterion application**: post v2 fix work で **Critical = 0 + High = 0 達成見込み** (= simplified convergence target)、third-party `/check_job` re-invocation で final verify required
- **Spec stage 完了判定**: ⚠ PENDING (= post v2 fix work + third-party review で final verify、convergence 到達なら Spec stage close + Phase 3 着手)

### Iteration v3 (2026-05-11、Phase 3 着手前 empirical baseline analysis + Option α design decision lock-in)

- **Source state**: Iteration v2 ideal-clean state 達成後、Phase 3 (= T1-pre-1 + T1-pre-2 + T1-pre-4 audit script extensions) 着手前 empirical baseline analysis を実施 (= `/tmp/phase3_baseline_analysis.py`)。3 real PRDs (I-D-pre / I-D-main / I-205) の cell appearance 分布 + Spec→Impl Mapping section presence + Cell Numbering Convention section presence を測定
- **Empirical findings**: I-D-pre + I-D-main は matrix ↔ Spec→Impl Mapping 1-to-1 alignment ✓ + `## Cell Numbering Convention` section 存在 ✓。**I-205 は対照的 pattern**: documented gaps 21 cells ({29, 34, 35, 41, 45, 48-49, 51-59, 65-69}) + Spec→Impl Mapping section 不在 + `## Cell Numbering Convention` section 不在。empirical evidence で **Path E utility が I-205 で 2 drifts (= Axis 1 cross-reference)** 発見、これは既知 baseline (= INV-4 4-tuple 4 entry目 I-205 PASS = audit-prd-rule10-compliance.py exit 0、Path E utility の 2 drifts は別 baseline)
- **🔴 Critical design constraint**: Phase 3 で `verify_cross_reference_cell_consistency` を audit-prd-rule10-compliance.py 側に mirror 実装すると、I-205 で同 false-positive (= Axis 1 missing cells [1-81]) が audit-prd-rule10-compliance.py 経由でも発生 → **INV-4 4-tuple baseline (I-205 PASS) 破壊 risk**
- **Design option evaluation (ideal-implementation-primacy adversarial review)**:
  - **Option α (recommended、user 確定 2026-05-11)**: audit script 側 mirror は `## Cell Numbering Convention` section 有無で auto-detect、I-205 は 新 verify functions の audit out-of-scope。helper `has_cell_numbering_convention_section()` を新設、3 NEW verify functions (= T1-pre-1 / T1-pre-2 / T1-pre-4) の冒頭で early-return。Pros: I-D-pre cohesion 維持 (1 PRD = 1 architectural concern)、Path E utility 不変、scope creep なし、INV-4 4-tuple baseline preserve。Cons: I-205 一時的 audit auto-detect 外 → I-205 retroactive update (= `## Cell Numbering Convention` + `## Spec→Impl Mapping` section 追加) は **案 γ Phase 2 (T15 = I-205 close 前 /check_job 4-layer review) で実施予定**、section 追加で audit scope 内に自動 promote (= future-proof design)
  - **Option β (Path E utility 側修正 + audit script mirror で同 logic) — REJECTED**: utility 修正 = "I-205 PRD pattern を許容する logic 化" = framework rule structural enforcement 弱化 = ideal-implementation-primacy 違反
  - **Option γ (audit script 独立 design) — REJECTED**: DRY 違反 (Path E utility との semantic 乖離)
  - **Option α-batched (Phase 3 + I-205 retroactive update batch 化) — NOT ADOPTED**: ideal-clean directive 完全充足だが I-D-pre cohesion (1 PRD = 1 architectural concern) 違反 + I-205 PRD doc 数百行 scope creep
- **I-205 retroactive update intent (= 案 γ Phase 2 T15 で実施予定)**: I-205 PRD doc に `## Cell Numbering Convention` + `## Spec→Impl Mapping` section 追加 = framework rule retroactive compliance、TODO `[I-205-retroactive-cell-numbering-section]` (新規起票) に Cross-reference + when-to-promote 条件 documented
- **Phase 3 完了 (2026-05-11)**: Option α 採用確定 + I-205 retroactive intent TODO lock-in 後、Phase 3 implementation 完了。具体: (a) `scripts/audit-prd-rule10-compliance.py` に **3 NEW verify functions** (T1-pre-1 `verify_pending_verdict_findings_consistency` + T1-pre-2 `verify_cross_reference_cell_consistency` + T1-pre-4 `verify_cell_numbering_drift_detection`) embed、(b) helper `has_cell_numbering_convention_section()` + formatter `_format_path_e_drifts()` 追加、(c) Path E utility (`scripts/verify_prd_self_audits.py`) を library import で source of truth 共有 (= DRY 達成、改善 auto sync)、(d) 4 test stubs `#[ignore]` 解除 + assertion fill in + 6 fixtures 新設 (A6 fixture design pattern 適用、per-axis isolation = realistic mini-PRD 構造)、(e) Impact Area Audit Findings table の `scripts/audit-prd-rule10-compliance.py` byte claim 37310 → 44234 + LOC 906 → 1030 sync (= Path E utility 0 drifts 維持)
- **Empirical state (post Phase 3)**: `cargo test --test i_d_pre_audit_extensions_test` = 4/4 PASS + INV-4 4-tuple baseline preserved (I-050 FAIL preserve / I-205 PASS / I-D-pre PASS / I-D-main PASS) + Path E utility 0 drifts on both real PRDs + cargo clippy 0 warnings + cargo fmt 0 diffs + file-line OK
- **Phase 3 直後 `/check_job` 4-layer review (2026-05-11)**: Layer 1/2/3/4 全実施、findings 分類:
  - **Implementation gap = 2**: (I1) `has_cell_numbering_convention_section() == False` early-return branch test 不在 = testing.md C1 branch coverage 違反 (High、即時 fix)、(I2) Path E utility API stability test 不在 (Medium、別 PRD candidate)
  - **Spec gap = 0** (= Phase 3 spec の意図に準拠、design choice は spec の許容範囲内)
  - **Grammar gap = 0 / Oracle gap = 0** (= audit script PRD、TS→Rust conversion ではない)
  - **Review insight = 3**: (R1) scripts/ scope file-size policy 未確立 (= audit script 1030 行)、(R2) Path E utility ↔ audit script wrapper output 1-to-1 mapping byte-exact invariant test 不在、(R3) Path E utility API stability contract spec 不在
- **Action Items resolution (initial /check_job 4-layer review)**:
  - **#1 (Implementation gap I1、High、即時 fix)**: I-205-like fixture `audit_out_of_scope_skip.md` 新設 (= `## Cell Numbering Convention` section 不在 + 3 violation patterns 含む、3 NEW functions 全 skip = audit PASS verify) + test `test_audit_out_of_scope_skip_on_missing_cell_numbering_convention` 追加 = **5/5 tests PASS** (= C1 branch coverage 達成: True branch 4 tests + False branch 1 test)
  - **#2-5 (Implementation gap I2 + Review insights R1/R2/R3、L4 latent)**: TODO `[I-D-future-audit-extensions-hardening]` cohesive batch entry に lock-in (= 3 candidate classes C1=Path E API stability / C2=byte-exact invariant / C3=scripts/ scope file-size policy、案 γ Phase 0 完了後再評価)
- **/check_job deep deep review (2026-05-11) で initial review が missed の妥協発見**:
  - **NEW Implementation gap I3 (High、即時 fix)**: `sys.path.insert(0, str(Path(__file__).resolve().parent))` + `# noqa: E402` suppression = mechanical 妥協 (= Python 自動 sys.path[0] 設定で `python3 script.py` 実行時 script directory が自動追加 = redundant)。**Fix**: sys.path.insert 排除 + `from verify_prd_self_audits import ...` を PEP 8 compliant top-level position に移動 + `# noqa: E402` 削除 = proper top-level import 達成
  - **NEW Implementation gap I4 (Medium、即時 fix)**: `audit_out_of_scope_skip` fixture の "3 violation patterns 含む" claim が audit-prd-rule10-compliance.py 経由 audit のみで indirect verify = silent miss risk (= fixture authoring mistake で patterns 欠落でも Option α skip test PASS、circular evidence)。**Fix**: NEW test `test_out_of_scope_fixture_violation_patterns_present_via_path_e` 追加 (= Path E utility 経由で 3 drifts (Axis 1 + Axis 2 + Axis 3) を direct verify、Option α auto-detect skip correctness の dual verify lock-in、= empirical proof from 2 independent observations) = **6/6 tests PASS**
  - **NEW Implementation gap I5 (Medium、別 PRD candidate)**: Closed PRD test 不在 (= `is_active_prd == False` で新 functions invoke されない動作の明示 verify)。既存 audit logic test 責務 = Phase 3 scope outside、TODO `[I-D-future-audit-extensions-hardening]` C4 candidate として lock-in (= /check_problem A1 fix 2026-05-11 で実施、4 candidate classes C1-C4 consistent state 達成)
- **`/check_problem` review (2026-05-11、deep deep review 直後)**: session 内未対応 issue の light な振り返り、A1 inconsistency 発見 + 即時 fix:
  - **A1 (TODO consistency 違反)**: Iteration v3 entry が "I5 → TODO C4 candidate として後続追加検討" と claim したが、TODO `[I-D-future-audit-extensions-hardening]` には C1-C3 のみ列挙 + C4 未 embed の inconsistency。**Fix**: TODO entry を 3 candidate classes → 4 candidate classes (C1-C4) に拡張、C4 candidate (= Closed PRD fixture + symmetric branch coverage test、`is_active_prd` False branch coverage、L4 latent) を embed = Iteration v3 entry claim と完全 consistent state 達成
  - **C1 (dspy reports handling)**: `report/dspy-*.md` 5 untracked files (= dspy research、Phase 3 architectural concern と完全 orthogonal、~140 KB) の Phase 3 commit handling = user 判断 → user 自身で別対応 (= Phase 3 commit scope outside 維持、cohesion 純化)
- **Empirical state (post deep deep fix)**: `cargo test --test i_d_pre_audit_extensions_test` = 6/6 PASS (True branch 4 + False branch 1 + dual verify 1) + INV-4 4-tuple baseline preserved + Path E utility 0 drifts (= byte claim 44234 → 44451 sync) + cargo clippy 0 warnings + cargo fmt 0 diffs + audit script LOC 1030 → 1033 (= sys.path.insert 削除 -2 行 + dual verify helper +28 行 = net +3 行、`# noqa` suppression 0 件)
- **Convergence criterion application**: 本 v3 entry = Phase 3 着手前 design decision lock-in + Phase 3 完了 + /check_job 4-layer review (= I1 fix) + /check_job deep deep review (= I3 + I4 即時 fix、I5 → TODO C4) + /check_problem (= A1 TODO consistency fix、C1 dspy reports user 別対応) retroactive embed (= 9 findings classified、I1+I3+I4+A1 fix 済 / I2+I5 → TODO C2+C4 / R1-3 → TODO C1+C3+R-spec / dspy = scope outside)、Critical = 0 + High = 0 維持
- **Spec stage 完了判定**: ✓ Phase 3 完了 (= 3 NEW audit functions + helper + formatter + proper top-level library import (= sys.path.insert + `# noqa: E402` 排除) + DRY 達成 + 6 tests (= True branch 4 + False branch 1 + dual verify 1) + 7 fixtures (= positive 3 + negative 3 + out-of-scope 1) + INV-4 baseline preserve + /check_job 4-layer + deep deep review pass + /check_problem A1 fix で TODO C1-C4 4 candidate classes consistent state + Action Items L4 latent TODO lock-in)、Phase 4 (= `scripts/audit-handoff-doc-line-refs.py` 新設 + CI integration) 着手 ready。Final third-party `/check_job` re-invocation は Phase 6 T7-pre で実施

### Iteration v4 (2026-05-11、Implementation Phase 4 完了 = T1-pre-3a + T1-pre-3b + handoff doc structural fix)

- **Source state**: Phase 3 完了 + Iteration v3 ideal-clean state 達成後、Phase 4 (= T1-pre-3a `scripts/audit-handoff-doc-line-refs.py` 新設 + T1-pre-3b `.github/workflows/ci.yml` CI step integration) 着手
- **Phase 4 implementation 完了 (2026-05-11)**:
  - **(a) T1-pre-3a**: `scripts/audit-handoff-doc-line-refs.py` (246 行) 新設 = handoff doc grep `<path>.<ext>:<line>(-<end>)?` regex 抽出 (8 拡張子: .rs/.md/.py/.sh/.yml/.yaml/.toml/.json) + each ref に対し (1) as-is interpretation (repo root resolve) + (2) glob fallback (common roots: src/ scripts/ tests/ .claude/ doc/ .github/) + (3) OOB filter (line ≤ file_lines) + (4) drift classification (MISSING_FILE / OUT_OF_BOUNDS / AMBIGUOUS = >1 in-bounds candidate after OOB filter)。directory invocation で `*.md` recursive walk 対応
  - **(b) handoff doc structural drift 発見 + fix (= Phase 4 内 structural fix、scope creep ではない)**: empirical probe で `doc/handoff/design-decisions.md` に **5 ambiguous refs** (= section-context-implicit prefix style = brittle convention) を検出。disambiguation methodology = (1) glob candidates 全列挙 (2) OOB filter (3) section context + line content syntactic match で正しい候補確定 (4) 同 section 内 partial-path convention に合わせて explicit prefix 追加。Fix list: `interfaces.rs:466` → `pipeline/type_converter/interfaces.rs:466` / `interfaces.rs:141` → `pipeline/type_converter/interfaces.rs:141` / `type_aliases.rs:370` → `pipeline/type_converter/type_aliases.rs:370` / `error_handling.rs:436` (× 2 occurrences) → `transformer/statements/error_handling.rs:436`。Post-fix state: doc/handoff/ 30 refs / 0 drifts / exit 0 PASS
  - **(c) T1-pre-3b**: `.github/workflows/ci.yml` に `python3 scripts/audit-handoff-doc-line-refs.py doc/handoff/` step を `Audit PRD Rule 10/11/12` step 直後に追加 = PR merge gate active 化 (= INV-3 evidence)
  - **(d) test fill-in**: `tests/i_d_pre_handoff_audit_test.rs` 2 stubs `#[ignore]` 解除 + assertion fill in。`run_handoff_audit` subprocess helper を Phase 3 `run_audit` / `path_e_total_drifts` と同 pattern で実装、`parse_total_drifts` helper で stdout parse。`test_audit_handoff_doc_line_refs_drift_detection` = positive fixture で 3 drift categories (OUT_OF_BOUNDS / MISSING_FILE / AMBIGUOUS) を stderr substring match で個別 verify + negative fixture で 0 drifts + exit 0 verify。`test_audit_handoff_doc_line_refs_standalone_baseline` = `doc/handoff/` directory に対する 0 drifts frozen baseline lock-in (= future PR で drift 混入されたら regression detect)
  - **(e) fixtures 新設**: `tests/fixtures/i_d_pre/positive/handoff_drift.md` (= 3 drift categories 全 trigger) + `tests/fixtures/i_d_pre/negative/handoff_clean.md` (= Cargo.toml:1 / README.md:1 / self-reference / single-candidate glob で全 verified)。README.md update で fixture pair documented
- **Self-applied 4-layer review (Phase 4、Layer 1-4 全実施)**:
  - **Layer 1 (Mechanical) ✓**: production code 内 unwrap/expect/panic 不在 (test helper のみ panic 使用、testing.md 規定通り)、test name 形式準拠、各 drift category に独立 assertion (C1 branch coverage)、cargo clippy 0 warnings、cargo fmt 0 diffs、file-line OK
  - **Layer 2 (Empirical) ✓**: positive fixture で 3 drifts trigger 確認、negative fixture で 0 drifts 確認、doc/handoff/ standalone で 0 drifts (post structural fix) 確認、INV-4 4-tuple baseline preserved (I-050 FAIL preserve / I-205 PASS / I-D-pre PASS / I-D-main PASS)、Path E utility 0 drifts on both real PRDs
  - **Layer 3 (Structural cross-axis) ✓**: orthogonal axes enumerate = (a) drift category (OUT_OF_BOUNDS / MISSING_FILE / AMBIGUOUS、全 covered) / (b) path form (full repo-relative / partial / bare basename、全 covered) / (c) line spec (single line / range `<start>-<end>`、両対応) / (d) invocation form (file / directory、両対応)。Spec gap = 0
  - **Layer 4 (Adversarial trade-off) ✓**: pre/post matrix = pre-PRD で audit unavailable (handoff doc drift = silent latent state、5 ambiguous refs が brittle convention 経由で潜在) → post-PRD で audit script + 5 structural fix + CI merge gate active = ideal-clean state 達成 + future regression block。Trade-off statement = "section-context-implicit prefix convention を explicit-path convention へ 5 箇所 migration、コスト = 3 行 doc edit、利得 = empirical correctness lock-in (= silent reference rot 防止)"。Patch vs Structural: 本 PRD は structural fix (= audit mechanism 自体の lock-in)、interim patch なし
- **Initial Findings count (self-applied 4-layer review immediately after Phase 4 implementation)**: Critical 0 / High 0 / Medium 0 / Low 0 / Review insight 0 (= self-claimed ideal-clean)
- **`/check_job` 4-layer review (commit-pending state、third-party-style adversarial review、2026-05-11)**: 上記 self-claim を adversarial に再検証、**2 件の Implementation gap (Medium) + 2 件の Review insight (High / Low) を retroactively 発見** = v12-2 pattern 5 度目 empirical 再発 evidence:
  - **L1-1 (Implementation gap、Medium)**: `classify_ref` の `if not in_bounds: True` branch (= OOB via glob fallback、bare basename + all candidates OOB) が positive fixture 3 drifts (= OUT_OF_BOUNDS-via-as-is + MISSING_FILE + AMBIGUOUS) でも exercise されず = **testing.md C1 branch coverage 違反**。**Fix**: positive fixture に Drift 4 (`audit-handoff-doc-line-refs.py:99999` = scripts/ 内 1 candidate + 246 < 99999 OOB) 追加 + assertion で "glob candidate(s) below line" substring verify
  - **L1-2 (Implementation gap、Medium)**: line-spec axis (single vs range) の equivalence partition coverage 不在 = 全 fixture が single-line ref のみ、range form `<start>-<end>` (regex capture 経路 + `upper = ref.end` dispatch) が test で explicit exercise されない (= `doc/handoff/` baseline test が implicit に range refs を含む `mod.rs:27-47` 等を exercise するが explicit assertion 不在 = 仮に regex range capture 破綻時 silent miss)。**Fix**: positive fixture に Drift 5 (`src/lib.rs:99990-99999` = OOB range form) + negative fixture に `src/lib.rs:1-10` (in-bounds range form) 追加 + assertion で line_spec "99990-99999" 表記 verify + negative line refs count 4 → 5 assertion
  - **R1 (Review insight、High)**: Iteration v4 entry の initial "Layer 3 ✓ findings 0" 自己 claim = **直交軸 enumerate を thorough に行わずの false-positive 0** = `/check_job` で 2 axes coverage gap (= dispatch combinations / line-spec axis) を発見 = **v12-2 pattern 5 度目 empirical 再発** (= `[I-D-main]` 改善 v13-4 / v13-7 candidate "self-applied + third-party 二重 review mandatory + convergence criterion" の更 1 件 empirical 補強)。Resolution: 本 v4 entry に retroactive embed (= meta-correct な transparency)、I-D-main scope で structural fix
  - **R2 (Review insight、Low)**: PRD Cell 3 spec の "line content syntactic verify" wording が ambiguous (= bounds-only vs blank-line-check vs context-match) = framework rule level 判断、現状 bounds-only 実装は実用 sufficient。Resolution: TODO `[I-D-future-audit-extensions-hardening]` C5 candidate 新規 lock-in (= L4 latent、framework leverage 後再評価)
- **Action Items resolution (`/check_job` 4-layer review fix work)**:
  - **#1 (L1-1、即時 fix)**: ✓ DONE = positive fixture Drift 4 (`audit-handoff-doc-line-refs.py:99999`) 追加 + assertion 更新 = "glob candidate(s) below line" substring verify
  - **#2 (L1-2、即時 fix)**: ✓ DONE = positive fixture Drift 5 (`src/lib.rs:99990-99999`) + negative `src/lib.rs:1-10` 追加 + assertion 更新 = range form line_spec "99990-99999" + negative refs count 5 verify
  - **#3 (R1、即時 fix retroactive embed)**: ✓ DONE = 本 entry に `/check_job` 4-layer review findings + Action Items section retroactive embed + TODO `[I-D-main]` v13-4/v13-7 candidate 5 度目 empirical evidence contextual link 追加 (= v12-2 pattern recurrence chain 1→2→3→4→5)
  - **#4 (R2、L4 latent)**: ✓ DONE = TODO `[I-D-future-audit-extensions-hardening]` C5 candidate (= "line content syntactic verify" wording strengthening、bounds-only → bounds + non-blank line content check 等) 新規 lock-in
- **/check_job post-fix Findings count**: Critical 0 / High 0 / Medium 0 (= L1-1 + L1-2 全 fix) / Low 0 / Review insight 2 (= R1 contextual link + R2 TODO lock-in、両 retroactive transparency embed)
- **/check_problem (Phase 4 後続、2026-05-11)**: `/check_job` fix work 後の light review、**4 additional issues 発見** = v12-2 pattern 6 度目 empirical 再発 evidence (= `/check_job` 自身も thorough ではなかった、recursive 性質):
  - **Issue #1 (Documentation drift、Medium、即時 fix DONE)**: Impact Area table stale LOC = `verify_line_refs.py` 264→297 (Phase 2 metadata + lock-in additions) + `verify_prd_self_audits.py` 368→644 (Phase 2 + 4 additive utility fixes) = Phase 2 完了時点で stale、Phase 3/4 review でも catch されず。**Fix**: LOC sync + **byte counts 追加** (= 11517 / 31728) で Path E utility (Axis 4) auto-detect 対象化 = utility 自身を audit する **recursive self-audit structure 完成** (= ideal-clean strengthening)
  - **Issue #2 (Scope decision、Low、user 判断待ち)**: `doc/handoff/design-decisions.md:1300` "4 度連続 v12-2 pattern recurrence" section update 候補 = I-224 archive cohesion (= 1 doc = 1 PRD lesson archive 原則) vs broader recurrence count tracking trade-off。**Resolution**: user 判断待ち (= 本 PRD scope outside、deferred-with-context)
  - **Issue #3 (Silent failure mode、Medium、即時 fix DONE)**: `<path>:<start>-<end>` で `start > end` (backwards range typo、例: `mod.rs:100-50` 意図は `50-100`) を script が silent pass = `upper = ref.end` で end-based OOB check のみ、start > end の semantic 整合性 verify 不在 = silent failure mode。**Fix**: `classify_ref` に backwards-range detection 追加 = 新 drift category **INVALID_RANGE** (= 4 categories 化、MISSING_FILE / OUT_OF_BOUNDS / AMBIGUOUS + INVALID_RANGE)、positive fixture Drift 6 (`src/lib.rs:100-50`) 追加、assertion で `[INVALID_RANGE]` + "backwards range" substring verify。post-fix script LOC 246→260、bytes 9131→9773 = Impact Area row 自動再 sync (Path E utility が detect → fix)、recursive self-audit structure 動作 evidence
  - **Issue #4 (Future-proofing、Low、TODO C6 lock-in DONE)**: `GLOB_ROOTS` hardcoded 6 dirs = 将来 `tools/` / `examples/` 等の新 dir 追加時 false MISSING_FILE risk。**Resolution**: TODO `[I-D-future-audit-extensions-hardening]` C6 candidate 新規 lock-in (= Cargo.toml metadata leverage / `.claude/audit-config.toml` declarative config 等 4 options)、L4 latent
  - **Issue #5 (Test partition gap、Low、skip 提案)**: directory + multi-file drift fixture 不在。loop logic は trivial sum、risk-to-cost で skip 判断、user 判断項目として presented
- **/check_problem post-fix Findings count**: Critical 0 / High 0 / Medium 0 (= Issue #1 + #3 fix DONE) / Low 1 deferred (= Issue #4 TODO lock-in) / Open 2 (= Issue #2 + #5 user 判断待ち、本 PRD scope outside)
- **Empirical state (post `/check_job` + `/check_problem` fix, 2026-05-11)**: `cargo test --test i_d_pre_handoff_audit_test` = 2/2 PASS (= 拡張 6 drift patterns: 4 categories × dispatch combinations + range form negative coverage 含む) + `cargo test --tests` 全 PASS + INV-4 4-tuple baseline preserved + Path E 0 drifts on both real PRDs (= byte claims audit-handoff-doc-line-refs.py 9773 + verify_line_refs.py 11517 + verify_prd_self_audits.py 31728 全 sync 済) + audit-handoff-doc-line-refs.py 0 drifts on doc/handoff/ + cargo clippy 0 warnings + cargo fmt 0 diffs + file-line OK + CI step integrated。**Recursive self-audit structure 完成** = Path E utility が Impact Area 内 3 audit utilities 全ての byte claim を auto-verify (= structural drift prevention、Phase 2/3/4 で過去発生した stale claim pattern を future 再発 block)
- **Convergence criterion application**: Phase 4 ideal-clean 達成 post `/check_job` + `/check_problem` fix (= Critical 0 + High 0 + Medium 0 + 4-layer review with retroactive Action Items embed pass + recursive self-audit structure 完成 + 2 open issues are user-decision-pending not defect-deferred)
- **Spec stage 完了判定**: ✓ Phase 4 完了 post `/check_job` + `/check_problem` fix = `scripts/audit-handoff-doc-line-refs.py` (260 行、9773 bytes、4 drift categories) + handoff doc structural fix (5 ambiguous refs → explicit partial-path) + CI integration + 2 tests PASS (= 6 drift assertions、4 categories all covered) + 2 fixtures (6 positive drift patterns + 5 clean refs) + 4-layer review + /check_problem review + 全 fix + retroactive Action Items embed + recursive self-audit structure 完成。次着手 = Phase 5 (= T2-pre-1 + T2-pre-2 rule wording strengthening) ready。Final third-party `/check_job` re-invocation は Phase 6 T7-pre で実施

(以下 iteration 増えるごとに追記、convergence 到達まで recursive)

---

## Test Plan

### Fixture design pattern (A6 fix /check_job L3-6、Path B split 2026-05-11)

**Per-axis isolated fixture design pattern** (= 各 fixture が ONE axis のみ trigger する制約):
- 各 audit verify function に対し **2 fixtures** (positive + negative) を `tests/fixtures/i_d_pre/{positive,negative}/<test_module>_<scenario>.md` に配置
- Positive fixture = 故意に target axis の violation pattern を含む、他 axes は clean
- Negative fixture = 全 axes clean (= 違反 pattern 不在)
- 全 fixtures は **realistic mini-PRD 構造** (= Problem Space matrix + Rule 10 Application + Scope + Invariants + Spec→Impl Mapping + Implementation Stage Tasks + Test Plan の minimum sections) を含む = SECTION_COVERAGE_POLICY 5 sections 全 enumerate により "section not found" false positive 排除

**Test helper convention** (`tests/i_d_pre_path_e_test.rs` を template):
- `fn run_path_e(fixture: &str) -> (i32, String)` = audit script subprocess invocation + (exit_code, stdout) tuple return
- `fn axis_drift_count(stdout: &str, axis_num: u32) -> usize` = stdout から特定 axis の drift count 抽出
- 各 test fn = (positive + negative) 両 fixture に対し axis-specific exact count assertion
- Method A test (`tests/i_d_pre_method_a_test.rs`) も同 pattern (= exact count assertion、A8 fix /check_job L1-5 reconciled)

**Fixture naming**: `<test_module>_axisN_<scenario>.md` (例: `path_e_axis1_partition_violation.md` / `path_e_axis2_clean.md`) で audit module + axis + scenario が file name から self-documenting

### Test category 1: Audit extensions tests (`tests/i_d_pre_audit_extensions_test.rs`)

- **Synthetic PRD fixture-based tests**: 各 audit verify function に対し、synthetic PRD doc fixture (= 故意に違反 pattern を含む / 含まない 2 variants) を構築、audit function 出力を assert
- **Test cases per cell** (~3 functions): cells 1, 2, 5 = 3 audit functions、各 function に対し ≥1 positive test (= 違反 pattern を fixture で含む) + ≥1 negative test (= 違反 pattern なしで PASS)
- **Self-applied integration test**: 本 PRD doc 自身を fixture として使用、`audit_prd(self_path)` で全新 verify functions PASS confirm

### Test category 2: Rule wording tests (`tests/i_d_pre_rule_wording_test.rs`)

- **Grep-based assertion tests**: 各 rule wording strengthening について、rule file 内 specific text pattern 存在を assert
- **Test cases per candidate** (~2 wording candidates): cells 4 (Layer 1 factual accuracy) + 5 (Rule 9/13 cell numbering convention) = 2 rule wording cells、各 cell に対し ≥1 grep-assertion test
- **Versioning verify**: 各 rule file の Versioning section に v1.8 entry 存在 verify

### Test category 3: Method A utility tests (`tests/i_d_pre_method_a_test.rs`)

- **Synthetic PRD fixture-based tests**: `scripts/verify_line_refs.py` (Method A) own behavior auto-verify
- **Test cases**: positive (= synthetic PRD で heading-based line-ref drift を含む fixture で detect) + negative (= drift 不在 fixture で no-detection PASS) + utility metadata verify (= header embed)

### Test category 4: Path E utility tests (`tests/i_d_pre_path_e_test.rs`)

- **Synthetic PRD fixture-based tests**: `scripts/verify_prd_self_audits.py` (Path E) own behavior auto-verify、4 axes (Axis 1/2/3/4) 各々 + F6/F7 fix + Axis 3 cell-slot vocabulary extension の動作 verify
- **Test cases**: 4 axes × (positive + negative) = 8 base tests + F6 fix verify (allow-list 動作) + F7 fix verify (post-v15 wording detection) + Axis 3 vocabulary fork detection = ~11 tests

### Test category 5: Handoff audit tests (`tests/i_d_pre_handoff_audit_test.rs`)

- **Synthetic handoff doc fixture-based tests**: `scripts/audit-handoff-doc-line-refs.py` (NEW) own behavior auto-verify
- **Test cases**: positive (= synthetic handoff doc で `<file>:<line>` drift を含む fixture で detect) + negative (= drift 不在 fixture で no-detection PASS) + standalone CLI invocation test

### Test category 6: Self-applied integration tests (`tests/i_d_pre_invariants_test.rs`)

- **INV-1〜INV-5 verify**: 各 invariant の test contracts を `#[test]` で fill in
- **Cross-axis check**: 5 cells × Implementation Tasks T1-pre / T2-pre 1-to-1 mapping verify (manual review、I-D-main で formal audit)
- **INV-5 (I-D-main prerequisite achievement)**: I-D-pre 完了時点では `#[ignore]` placeholder、I-D-main 完了時に retroactive enable + assert

### Test runtime

- 全 test contracts は `cargo test --test i_d_pre_*` で execute
- CI (`.github/workflows/ci.yml`) に integrate、PR merge gate

---

## Completion Criteria

本 PRD I-D-pre 完了の必要十分条件 (`prd-completion.md` 厳格適用):

1. **Matrix completeness (最上位完了条件)**: 5 cells の全 candidate に対し、対応する resolution が rule file / audit script / new audit script / formal utility lock-in に embed 済 + 各 cell に対応する lock-in test が `cargo test` PASS
2. **Bootstrap utility formal lock-in (= bootstrapping circularity 構造的解消)**: `scripts/verify_line_refs.py` + `scripts/verify_prd_self_audits.py` (F6/F7 fix + Axis 3 extension integrated) + `scripts/audit-handoff-doc-line-refs.py` (NEW) が **formal regression-tested utilities** として lock-in、各 utility own test contract で auto-verify (= INV-2 evidence)
3. **Self-applied integration**: 本 PRD doc 自身が `python3 scripts/audit-prd-rule10-compliance.py backlog/I-D-pre-audit-mechanism-bootstrap.md` で **exit code 0** + 本 PRD spec stage iteration log で third-party `/check_job` invocation 経由 **Critical = 0 + High = 0 simplified convergence** 到達 (= INV-1 evidence)
4. **CI integration**: 新 audit script (= `scripts/audit-handoff-doc-line-refs.py`) が `.github/workflows/ci.yml` に CI step として integrate、PR merge gate active (= INV-3 evidence)
5. **Existing PRD docs compliance preservation (INV-4 baseline-aware delta-based regression 0)**: active backlog/ 全 PRD docs (= I-050 baseline FAIL preserve / I-205 PASS / I-D-main PASS / I-D-pre PASS) に対する新 audit verify mechanisms run が **4-tuple baseline assertion satisfy** (= INV-4 evidence)
6. **Quality gate**: `cargo test --test i_d_pre_*` 全 PASS + `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings + `cargo fmt --all --check` 0 diffs + `./scripts/check-file-lines.sh` 0 violations
7. **I-D-main prerequisite achievement (INV-5)**: I-D-pre close 後、I-D-main spec stage 着手 prerequisite satisfy 確認 (= I-D-main first third-party adversarial review で convergence criterion satisfy 到達 retroactive verify)
8. **Documentation sync**: `doc/handoff/design-decisions.md` に I-D-pre close 後 access path として新 section embed (= 5 cells bootstrap audit mechanism construction lessons + Path B split rationale empirical proof + bootstrapping circularity 構造的解消 verify) + plan.md update (= 案 γ Phase 0 = I-D-main 着手 ready)

### Tier-transition compliance (broken-fix PRD wording 適用、`prd-completion.md`)

本 PRD は **broken-fix PRD** に相当 (= existing framework state での bootstrapping circularity 構造的 gap = "self-applied audit utility correctness ceiling" pattern を fix):

- Pre-PRD state: PRD I-D Spec Stage Iteration v17 で bootstrapping circularity empirical recurrence (= Method A v12 → 別 class emerge / Path E v16 → 別 class emerge)、convergence criterion satisfy 不能
- Post-PRD state: 5 cells bootstrap audit mechanism formal lock-in による **bootstrapping circularity 構造的解消** (= I-D-main spec stage が completed bootstrap utilities 上で initial iteration convergence 可能 base state 確立) (= structural improvement、Tier 不適用 = framework PRD)
- Hono bench result classification: **Preservation** (allowed): production code 0 LOC change のため Hono bench に影響不在 (= clean files / errors count 不変、本 PRD は framework infra の cohesive batch、TS→Rust conversion mechanism は touch せず)

### Impact estimates

本 PRD は code path レベル impact ではなく **framework rule level impact**。5 cells の structural lock-in が:
- **I-D-main spec stage iteration cost 構造的削減**: post-bootstrap framework full leverage で initial iteration convergence 可能化 (= empirical proof は I-D-main 着手後 first third-party adversarial review で観測)
- **後続 PRDs spec stage iteration cost 構造的削減 (compounding benefit)**: I-D-main 完了後 全 future PRDs (= I-225 / I-162 / I-205 T14-T16 等) で bootstrap utilities full leverage、Iteration v1 convergence target

---

## 🔗 Cross-references

- **PRD I-D-main**: 本 PRD I-D-pre 完了後着手 = I-D parent から I-D-pre architectural concern (= 5 audit mechanism cells) を分離した残り 24 cells、post-bootstrap framework full leverage 状態で initial iteration convergence target。本 PRD I-D-pre が prerequisite
- **PRD I-D parent (split source)**: PRD I-D parent doc は本 split で I-D-main に rename + scope reduce (= 24 cells)、本 I-D-pre PRD は I-D parent matrix 30 cells から 5 cells migration 由来。Spec Review Iteration Log v1-v17 history は I-D-main doc に preserve (= Path B split rationale empirical proof source)
- **PRD I-224**: bootstrapping problem の origin lesson source (= 4 度連続 v12-2 pattern empirical recurrence)、close 後 access、詳細 lesson source = `doc/handoff/design-decisions.md` `## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain` section
- **PRD I-205 / I-225 / I-162**: I-D-pre + I-D-main 完了後着手 = 案 γ Phase 1/Phase 2 (framework rule full leverage 達成)
- **PRD I-E**: PRD I-D parent scope 分離由来 (v13-2 / v13-3 candidates migrate)、orthogonal architectural concern (= lib/CLI API + Web API runtime integration)、本 PRD I-D-pre と並行可能
- **TODO `[I-D-pre]` entry**: 5 cells 全列挙 + iteration history audit trail
- **TODO `[I-D-main]` entry**: 24 cells 全列挙 + iteration history (I-D parent v1-v17 preserve)
- **改修対象 file**: `.claude/rules/check-job-review-layers.md` / `.claude/rules/spec-stage-adversarial-checklist.md` / `scripts/audit-prd-rule10-compliance.py` / `scripts/verify_line_refs.py` / `scripts/verify_prd_self_audits.py` / `scripts/audit-handoff-doc-line-refs.py` (NEW) / `.github/workflows/ci.yml`
