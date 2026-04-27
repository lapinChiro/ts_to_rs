---
name: prd-template
description: Template and procedure for creating new PRDs in backlog/. Proceeds through Discovery (clarification questions) → PRD drafting
user-invocable: true
---

# PRD Template

## Trigger

When creating a new PRD in `backlog/`.

## Actions

### 0a. Matrix-Driven 判定 (Spec-First Gate)

**本 PRD は matrix-driven か?** 以下のいずれかに該当すれば matrix-driven:
- 入力次元に AST shape / TS type / emission context を持つ
- `problem-space-analysis.md` の直積マトリクスを構成する

**matrix-driven の場合**: `.claude/rules/spec-first-prd.md` の 2-stage workflow を適用する。
- **Spec stage** (実装前): grammar-derived matrix + tsc observation + E2E fixture (red) + checklist
- **Spec stage 完了 verification**: `.claude/rules/spec-stage-adversarial-checklist.md` の **13-rule checklist を全項目 verify** (1 = Matrix completeness + abbreviation prohibition (sub-rule 1-1/1-2/1-3) / 2 = Oracle grounding + PRD doc embed mandatory (sub-rule 2-1/2-2/2-3) / 3 = NA justification + SWC parser empirical observation (sub-rule 3-1/3-2/3-3) / 4 = Grammar consistency + doc-first dependency order (sub-rule 4-1/4-2/4-3) / 5 = E2E readiness + Stage tasks separation (sub-rule 5-1/5-2/5-3/5-4) / 6 = Matrix/Design integrity + Scope 3-tier consistency (sub-rule 6-1/6-2/6-3/6-4) / 7 = Control-flow exit sub-case / 8 = Cross-cutting invariant enumeration (sub-rule 8-5 audit verify) / 9 = Dispatch-arm sub-case alignment / 10 = Cross-axis matrix completeness (9 default axis、(i) AST dispatch hierarchy 含む) / 11 = AST node enumerate completeness check (sub-rule d-1〜d-5、`_` arm 全面禁止 + phase 別 mechanism + ast-variants.md single source of truth + pre-draft ast-variant audit + audit script CI 化) / 12 = Rule 10/11 Mandatory application + structural enforcement (sub-rule e-1〜e-8) / 13 = Spec Stage Self-Review (skill workflow Step 4.5 hard-code、sub-rule 13-1〜13-5))。1 つでも未達なら Implementation stage 移行不可。
- **Implementation stage** (spec approved 後): spec 準拠の実装のみ
- **Implementation stage 完了 verification**: `/check_job` 起動で `.claude/rules/check-job-review-layers.md` の 4-layer (Mechanical / Empirical / Structural cross-axis / Adversarial trade-off) を初回 invocation で全実施。発見 defect は `.claude/rules/post-implementation-defect-classification.md` の 5 category (Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight) に trace ベースで分類。

**non-matrix-driven の場合** (infra, refactor, bug fix): 従来通り Step 0 以降に進む。Spec stage / Implementation stage の dual review framework は適用外。

### 0b. Problem Space Analysis (最優先・最重要・絶対遵守)

**本ステップは全ての PRD で必須。スキップ・省略・後回し不可。**

`.claude/rules/problem-space-analysis.md` に従い、機能の問題空間を enumerate する。
Discovery より前、設計より前、実装より前に必ず実施する。

1. **入力次元を列挙する**: 機能の出力を決定する独立次元を「省略なしで」列挙する。
   - 変換系機能の典型: AST shape / TS type / outer context / TS strict 設定。
   - 各次元の variant を **reference doc から網羅チェック** する:
     - AST shape: `doc/grammar/ast-variants.md` の全 Tier 1/2 variant を確認
     - TS type: `doc/grammar/rust-type-variants.md` の全 18 RustType variant を確認
     - Context: `doc/grammar/emission-contexts.md` の全 51 context を確認
   - reference doc に存在する variant を「思いつかなかった」で漏らすことを防ぐ。
2. **組合せマトリクスを作成する**: 全次元の直積を表形式で enumerate し、
   各セルに以下を記録する:
   - Ideal Rust 出力 (不明なら「要調査」マーク)
   - 現状の出力 (実装確認 or 経験推定)
   - 判定: ✓ (現状 OK) / ✗ (修正必要) / NA (unreachable, 理由付き) / 要調査
   - Scope 判断: 本 PRD / 別 PRD / 後回し (理由必須)
3. **未確定セルを Discovery に回す**: 判定「要調査」のセルを Discovery で解消する。
   全セルに ✓ / ✗ / NA 判定が付くまで Discovery を終わらせない。
4. **PRD 本体に `Problem Space` セクションとして記録する**: マトリクスをそのまま
   PRD に転記。後続の設計・実装・テストは本マトリクスから導出する。

**禁止事項**:
- マトリクスを作らずに Discovery に進むこと。
- 「代表的な組合せのみ」「よくあるケースのみ」でマトリクスを省略すること。
- 「頻度が低い」「稀」を理由にセルを省略すること (頻度は問題空間の尺度ではない)。
- 組合せ爆発を理由に「サブセットのみ」と割り切ること (scope-out するなら別 PRD 化、
  しないなら全カバー)。
- **Abbreviation pattern (Rule 1 (1-2) 違反)**: `...` (omission ellipsis) /
  連番 row range (`| 30-35 |` 等の grouping) / `representative` / `representative cell のみ` /
  `代表的` / `省略` / `abbreviated` / `(各別 cell)` / `(同上)` / `varies` / `(... と同 logic)`
  等の placeholder 禁止 — Cartesian product **完全 enumerate** 必須、各 cell 独立 row
  (Lesson source: I-205 PRD draft v1 第三者 review F1 → Rule 1 sub-rule (1-2) 拡張)

### 0c. Rule 10 Application (Mandatory、PRD 2.7 Q5 確定 2026-04-27)

**全 PRD で必須**。`.claude/rules/spec-stage-adversarial-checklist.md` Rule 12 (Rule 10/11
Mandatory application + structural enforcement) に従い、PRD doc に以下 section を必須記入:

```markdown
## Rule 10 Application

\`\`\`yaml
Matrix-driven: yes | no
Rule 10 axes enumerated:
  - <axis 1>
  - <axis 2>
  - ...
Cross-axis orthogonal direction enumerated: yes | no
Structural reason for matrix absence: <reason、Permitted reasons から選択 or N/A (matrix-driven PRD)>
\`\`\`
```

#### Permitted reasons (matrix 不在の structural reason として選択可)

- `infra で AST input dimension irrelevant` (matrix-driven 概念が機能しない infra task)
- `refactor で機能 emission decision なし` (機能変化を導入しない refactor)
- `pure doc 改修` (実装を伴わない documentation 改修)

#### Prohibited reasons (Anti-pattern、明示禁止 list、`feedback_no_dev_cost_judgment.md` 違反)

- 「scope 小」/「light spec」/「pragmatic」/「~LOC」/「短時間」/「manageable」/「effort 大」/
  「実装 trivial」/「quick」/「easy」/「simple」

これら keyword を `Structural reason for matrix absence` に記入すると audit fail。

#### Verification step (M4 修正、PRD 2.7 確定)

skill workflow Step 4 (PRD Drafting) 完了直後に **`scripts/audit-prd-rule10-compliance.py
<new-prd-path>`** を実行する (本 skill 起動時の必須 verification step):

- exit code 非 0 (audit fail) の場合、Claude は PRD doc を修正してから skill を closing する
  (= 空のまま skill 終了不可、本 step の hard-code mechanism)
- exit code 0 (audit pass) で skill は closing 可能

audit script は本 PRD 2.7 T6 で新規作成、CI merge gate として `.github/workflows/ci.yml`
にも step 追加 (PRD 2.7 T7)。

### 1. Batch Check

Once the target item is determined, check `TODO` for items that should be batched together:

- Items on the **same code path** (addressable by modifying the same functions/modules)
- Items with **explicit overlap/relation** (cross-referenced with 🔗, etc.)
- Items with the **same abstract pattern** (e.g., multiple `TsTypeOperator` variant support)
- **Items that share the same problem space matrix** (Step 0b で同じ次元にマップされる defect は
  同一 PRD に統合する。個別 fix すると問題空間の網羅が崩れる)

If applicable items exist, include them in the PRD scope. Do not force-combine items on independent code paths.

### 2. Discovery

Before writing the PRD:

1. **First** resolve all "要調査" cells in the Problem Space matrix (Step 0b) — ask
   the user what the ideal Rust output should be for each unknown cell.
2. **(Matrix-driven PRD のみ) tsc observation**: ✗ および 要調査 のセルに対して
   TS fixture を作成し `scripts/observe-tsc.sh` で tsc / tsx の挙動を観測する。
   Ideal Rust 出力は「tsc observation の runtime stdout を Rust でも再現する」
   を原則とする。観測結果を PRD に記録する。
3. Ask the user at least 2 additional clarification questions:
   - Why build this now? (motivation/priority confirmation)
   - What defines success? (completion criteria alignment)
   - Are there constraints? (technical constraints, compatibility with existing features, etc.)
4. Draft the PRD only after all Problem Space cells have determined ideal outputs
   and the user has answered motivation/success/constraint questions.

### 3. Impact Area Code Review

**Before writing the Task List**, review the production code and test code in the impact area. This catches broken windows and design issues before they propagate into the new implementation.

#### 3-pre. Empirical file path verify (RC-3 source、I-205 確定 2026-04-27)

**Mandatory step**: Impact Area で listing する全 file path を `find` / `Read` で
**empirical verify** する (存在 + 想定行番号 + 想定 function 名)。Uncertain expression は
audit fail の対象:

- 禁止 expression: `(or 該当)` / `(or 該当 file)` / `TBD` / `or` ambiguity (e.g., `update.rs (or 別 file)`) /
  `？` / `要確認` (PRD draft 時点で要確認は許容されない、verify 後 commit)
- 例: `src/transformer/expressions/update.rs (or 該当 file)` → 不正確 (該当 file 不在)、
  empirical verify で `src/transformer/expressions/assignments.rs:331 convert_update_expr` と
  確定してから listing。
- Audit verify mechanism: `audit-prd-rule10-compliance.py` で `## Impact Area` section の
  uncertain expression を regex match → audit fail (RC-3 enforcement)

#### 3-pre-2. Pre-draft ast-variant audit (RC-8 source、Rule 11 (d-5) 適用)

**Mandatory step**: 本 PRD scope の修正対象 file (Impact Area で listing) に対し:

```bash
python3 scripts/audit-ast-variant-coverage.py --files <impact-area-files>
```

を run、結果を PRD doc 内 `## Impact Area Audit Findings` section に embed。各 violation
について本 PRD scope で fix or I-203 (codebase-wide AST exhaustiveness) へ defer の判断を
spec-traceable に記録。`## Impact Area Audit Findings` section 不在 + matrix-driven PRD →
audit fail。

#### 3a. Production Code Quality Review

Read all files in the impact area and evaluate:

1. **DRY (knowledge duplication)**: Is the same conversion rule, type mapping, or business logic duplicated across multiple locations? If so, would the PRD's changes make the duplication worse, or is this an opportunity to consolidate?
2. **Orthogonality**: Does each function/module have a single, well-defined responsibility? Are there functions that mix concerns (e.g., type collection + type conversion, or AST analysis + IR generation)?
3. **Cohesion**: Are related functions grouped together in the same module? Are unrelated functions co-located due to historical accident?
4. **Coupling**: Are there unnecessary dependencies between modules? Would the PRD's changes increase coupling?
5. **Doc comments**: Are public functions documented? Are doc comments accurate (not stale from past refactors)?

Produce an issue table:

```
| Issue | Location | Category | Severity | Action |
|-------|----------|----------|----------|--------|
| P1    | foo.rs:42 | DRY | Medium | Fix in PRD |
| P2    | bar.rs:100 | Stale doc | Low | Fix in PRD |
```

Issues found must be either fixed in the PRD's task list or recorded in TODO with justification for deferral.

#### 3b. Test Coverage Review

Review existing tests in the impact area using the test techniques from `.claude/rules/testing.md`:

1. **Enumerate decision points** (C1 branch coverage): List every `if`, `match` arm, `if let`, and early `return` in the affected functions. Map each to existing tests
2. **Identify equivalence partitions**: List input partitions (AST variants, type variants, error/success paths). Check coverage
3. **Check boundary values**: Empty collections, single vs multi elements, 0/1/N counts
4. **Build decision table**: When 2+ independent conditions exist, enumerate combinations and check coverage
5. **Detect incorrect expectations**: Tests that pass but assert wrong behavior (bug-affirming tests)
6. **Test quality**: Do assertions have descriptive messages? Are test names accurate (`test_<target>_<condition>_<expected>`)?  Are there fragile assertions (substring matching where exact matching is possible)?

Produce a gap table:

```
| Gap | Missing Pattern | Technique | Severity |
|-----|----------------|-----------|----------|
| G1  | Option None-fill | C1 (D22) | High     |
```

Include **all** identified gaps (both production code issues and test gaps) in the PRD's task list, regardless of severity. No gap is too small to test — incomplete coverage is a broken window.

### 4. PRD Drafting

Follow this template:

```markdown
# <Title>

## Background

Why this feature is needed. Current problems or issues caused by its absence.

## Problem Space (必須・最上位セクション)

`.claude/rules/problem-space-analysis.md` に従い、機能の問題空間を完全に enumerate する。
本セクションが不完全な PRD は起票・実装を認めない。

### 入力次元 (Dimensions)

機能の出力を決定する独立次元を列挙する。省略なし。

- **次元 A (例: LHS AST shape)**: Lit / Ident / Member(Computed) / Member(Ident) / Call /
  OptChain / TsAs / TsNonNull / TsTypeAssertion / Arrow / Fn / Cond / Await / Unary /
  Bin / New / Paren / Seq / Array / Object / Tpl / ...
- **次元 B (例: LHS TS type)**: Option<T> / T(primitive) / Any / Unknown / TypeVar /
  Vec<T> / Vec<Option<T>> / HashMap / Tuple / Struct Named / Enum Named / Fn / ...
- **次元 C (例: outer context)**: return / var decl+annotation / var decl no-annotation /
  assign target / call arg / destructuring default / class field init / ternary branch /
  match arm body / spread / template literal expr / await operand / ...

### 組合せマトリクス

全次元の直積を表形式で記述する。

| # | A | B | C | Ideal 出力 | 現状 | 判定 | Scope |
|---|---|---|---|-----------|------|------|-------|
| 1 | Lit | String | return | `x.to_string()` | `x.to_string()` | ✓ | — |
| 2 | Member(Computed) | Vec<T> | return | `.get(i).cloned().unwrap_or_else(\|\| d)` | panic | ✗ | 本 PRD |
| 3 | TsAs | Option<T> | return | `inner.unwrap_or_else(\|\| d)` | compile error | ✗ | 別 PRD (I-NNN) |
| ... | ... | ... | ... | ... | ... | ... | ... |

判定凡例: ✓ (現状 OK) / ✗ (修正必要) / NA (unreachable, 理由付き) / 要調査 (Discovery で解消)

### Spec-Stage Adversarial Review Checklist

Spec stage 完了 verification は `.claude/rules/spec-stage-adversarial-checklist.md` の **13-rule checklist** を本 PRD の `## Spec Review Iteration Log` section に転記して全項目 verification する (DRY のため checklist 内容は本 skill に再記載しない、rule file が single source of truth)。13-rule に 1 つでも未達があれば Implementation stage 移行不可。

## Oracle Observations (matrix-driven PRD で必須、Rule 2 (2-2) hard-code)

各 ✗ / 要調査 cell について以下 4 項目を embed (`scripts/observe-tsc.sh` 出力転記):

### Cell <#>: <description>

- **TS fixture path**: `tests/e2e/scripts/<prd-id>/cell-NN-*.ts`
- **tsc / tsx output**:
  ```
  stdout: <captured>
  stderr: <captured>
  exit_code: <0 or non-0>
  ```
- **Cell number reference**: matrix table の cell # と 1-to-1 link
- **Ideal output rationale**: tsc 出力から Rust ideal output を derive する論理
  (preserve / reject / equivalent / Tier 2 honest error)

(全 ✗ / 要調査 cell について繰返、`audit-prd-rule10-compliance.py` で section 不在 audit fail)

## SWC Parser Empirical Lock-ins (NA cell present で必須、Rule 3 (3-2) hard-code)

各 NA cell について SWC parser empirical lock-in test の reference を記載:

### NA cell <#>: <description>

- **Spec-traceable reason**: TS spec syntax error / grammar constraint / Rust type system 構造的制約
- **SWC parser empirical evidence**:
  - **Test path**: `tests/swc_parser_*_test.rs::test_<name>`
  - **Behavior**: SWC parser が `Err` を返す or 期待 AST shape を構築しない
  - **If accept**: NA cell ではなく Tier 2 honest error reclassify (Rule 3 (3-3))

(全 NA cell について繰返、SWC parser accept 確認時は本 PRD scope 内で Tier 2 reclassify)

## Impact Area Audit Findings (matrix-driven PRD で必須、Rule 11 (d-5) hard-code)

```bash
python3 scripts/audit-ast-variant-coverage.py --files <impact-area-files>
```

実行結果を embed、各 violation に対する判断 (本 PRD scope で fix or I-203 へ defer) を
spec-traceable に記録:

| Violation | Location | Phase | Decision | Rationale |
|-----------|----------|-------|----------|-----------|
| Rule 11 d-1 `_ => ` arm | foo.rs:42 | Transformer | 本 PRD scope で fix | dispatch arm 拡張に伴い既存 `_ => ` arm を explicit enumerate 化 |
| Rule 11 d-3 Tier mismatch | bar.rs section | TypeResolver | I-203 defer | 本 PRD architectural concern と orthogonal |

## Rule 10 Application

**全 PRD で必須記入** (PRD 2.7 Q5 確定 2026-04-27、`.claude/rules/spec-stage-adversarial-checklist.md`
Rule 12 (Rule 10/11 Mandatory application + structural enforcement))。

```yaml
Matrix-driven: yes | no
Rule 10 axes enumerated:
  - <axis 1>
  - <axis 2>
  - ...
Cross-axis orthogonal direction enumerated: yes | no
Structural reason for matrix absence: <reason、Permitted reasons から選択 or N/A (matrix-driven PRD)>
```

`Structural reason for matrix absence` に Prohibited keywords (「scope 小」/「light spec」/
「pragmatic」/「~LOC」/「短時間」/「manageable」/「effort 大」/「実装 trivial」/「quick」/
「easy」/「simple」) を含む場合は audit fail。詳細は skill Step 0c 参照。

## Goal

What should be achievable when this PRD is complete. Write in specific, verifiable terms.
Avoid vague expressions ("fast", "easy", "intuitive") — use specific numbers, thresholds, and observable behaviors.

## Scope (3-tier 形式 hard-code、Rule 6 (6-2) 適用)

### In Scope

本 PRD で **Tier 1 完全変換** する features (Cartesian matrix で `本 PRD` Scope 列の cell に対応)。

### Out of Scope

別 PRD or 永続 unsupported な features (Cartesian matrix で `別 PRD (I-NNN)` 列に対応)。

### Tier 2 honest error reclassify

本 PRD で **Tier 2 honest error 化** する features (= 別 PRD で Tier 1 化候補、orthogonal
architectural concern; Cartesian matrix で `Tier 2 honest error reclassify (本 PRD)` 列に対応)。
これは silent drop / silent failure を排除し、user に compile-time error として明示する
reclassify、ideal-implementation-primacy 観点で structural improvement。

## Invariants (matrix-driven PRD で必須独立 section、Rule 8 (8-5) audit verify)

機能仕様の中で「matrix cell に展開できない / 全 cell で同時に成立する必要がある」transversal
property を列挙。各 invariant について以下 4 項目必須:

### INV-N: <statement>

- **(a) Property statement**: 1 文で書けるレベルの不変条件
- **(b) Justification**: なぜこの invariant が必要か (違反時の defect class)
- **(c) Verification method**: 実装後の verify 手順 (probe / test / static analysis)
- **(d) Failure detectability**: invariant 違反が compile error / runtime error /
  silent semantic change のどれで顕在化するか

## Design

### Technical Approach

Implementation strategy. Relationship to existing architecture, modules to modify, new modules to add.

### Design Integrity Review

Per `.claude/rules/design-integrity.md` checklist:

- **Higher-level consistency**: Consistency with one layer above (callers, dependencies, sibling modules)
- **DRY / Orthogonality / Coupling**: Issues found and resolution approach
- **Broken windows**: Existing code problems found, and decision to fix in-scope or record in TODO

If no issues, explicitly state "Verified, no issues."

### Impact Area

List of affected files/modules.

### Semantic Safety Analysis

**Required when the PRD introduces type fallbacks, type approximation, or changes type resolution behavior.** Follow the procedure in `.claude/rules/type-fallback-safety.md`:

1. **List all type fallback patterns** introduced by this PRD (e.g., `T[K]` → union of all field types, unresolvable type → `Any`)
2. **For each pattern, classify usage sites**:
   - Function return types: Does the fallback type cause compile errors or silent behavior changes?
   - Field types: Could `serde_json::Value` satisfy type constraints where a concrete type was expected?
   - Variable types: Could assignments or comparisons silently succeed with wrong types?
3. **Verdict per pattern**: Safe (compile error or identical behavior) / UNSAFE (silent semantic change)
4. **If any UNSAFE pattern exists**: Redesign to eliminate it before proceeding

If the PRD does not change type resolution, state "Not applicable — no type fallback changes."

## Spec Stage Tasks (matrix-driven PRD で必須、Rule 5 (5-2) 適用)

Stage 1 artifacts 完成 task を列挙 (matrix construction / oracle observation /
fixture creation / SWC parser empirical lock-in / impact area audit findings record)。
**code 改修 (`src/` 修正) を含めること禁止** (= Stage 1/2 boundary 違反)。

### TS-0: Cartesian product matrix completeness

- **Work**: Problem Space Cartesian product matrix を完全 enumerate (~N cells)、
  全 cell に判定 (✓/✗/NA/regression lock-in) を付与、abbreviation pattern 排除
- **Completion criteria**: matrix table 内 `...` / range grouping / placeholder 不在、
  全 cell 独立 row、`audit-prd-rule10-compliance.py` PASS

### TS-1: Oracle observation log embed

- **Work**: 各 ✗ / 要調査 cell について TS fixture 作成、`scripts/observe-tsc.sh` 実行、
  PRD doc `## Oracle Observations` section に embed
- **Completion criteria**: 全 ✗/要調査 cell について 4 項目 (fixture path / tsc output /
  cell # link / ideal output rationale) 記載

### TS-2: SWC parser empirical lock-in

- **Work**: 全 NA cell について `tests/swc_parser_*_test.rs` で SWC parser empirical
  lock-in test 作成、PRD doc `## SWC Parser Empirical Lock-ins` section に embed
- **Completion criteria**: 全 NA cell について SWC behavior verify、accept 確認時は
  Tier 2 honest error reclassify (Rule 3 (3-3))

### TS-3: E2E fixture creation (red 状態)

- **Work**: 各 ✗ cell に対応 `tests/e2e/scripts/<prd-id>/cell-NN-*.ts` fixture 作成、
  `scripts/record-cell-oracle.sh` で expected output 記録 (red 状態 = ts_to_rs 出力と
  expected 不一致)
- **Completion criteria**: `cargo test --test e2e_test` で全 fixture red 確認

### TS-4: Impact Area audit findings record

- **Work**: `python3 scripts/audit-ast-variant-coverage.py --files <impact-area-files>`
  実行、結果を PRD doc `## Impact Area Audit Findings` section に embed、各 violation の
  決定 (本 PRD scope or I-203 defer) 記録
- **Completion criteria**: 全 violations 列挙 + 決定記載

## Implementation Stage Tasks (Stage 2 code change task)

Stage 2 で実装する `src/` 修正 task を列挙。Assumes TDD: RED → GREEN → REFACTOR order。

### T1: <Task name>

- **Work**: What specifically to change/add. Specify target files, functions, and types
- **Completion criteria**: Conditions for this task to be considered complete. Include test additions/passing
- **Depends on**: None / T2, T3 (task IDs that must complete first)
- **Prerequisites**: State that must be satisfied before starting this task (if any)

### T2: <Task name>

- **Work**: ...
- **Completion criteria**: ...
- **Depends on**: T1
- **Prerequisites**: ...

## Spec Review Iteration Log (matrix-driven PRD で必須、Rule 13 (13-2) 適用)

Spec Stage Self-Review (skill workflow Step 4.5) の iteration history を record。

### Iteration v1 (date)

- **Findings count**: Critical N / High N / Medium N / Low N
- **Findings detail**: 各 finding の summary + RC clustering (root cause を grouping)
- **Resolution**: PRD doc fix + (該当する場合) framework self-applied integration

(以下 iteration 増えるごとに追記)

## Test Plan

Overview of tests to add/modify. Includes:
- Tests derived from the feature change itself
- Tests derived from the test coverage review (gap analysis)
- Normal cases, error cases, and boundary values

## Completion Criteria

Conditions for this PRD's work to be considered "complete". Include quality checks (clippy, fmt, test).

**Matrix completeness requirement (最上位完了条件)**: Problem Space マトリクスの全セルに対する
テストが存在し、各セルの実出力が ideal 仕様と一致すること。1 セルでも未カバー、または
「多分 OK」で済ませたセルがあれば PRD は未完成。

**Impact estimates (error count reduction) must be verified by tracing actual code paths for at least 3 representative error instances.** Label-based estimation (counting by error category name) is prohibited. Each traced instance must confirm that the proposed fix resolves the specific failure point in the execution path.

**Tier-transition compliance (broken-fix PRD のみ、`prd-completion.md` 適用)**:
新機能 PRD では "Hono bench: clean files / errors count 0 regression"。Broken-fix PRD では
"Tier-transition compliance" 表現を使う (existing Tier 2 errors transition Tier 1 = improvement、
new compile errors prohibited)。詳細 `.claude/rules/prd-completion.md` 参照。
```

### 4.5. Spec Stage Self-Review (Rule 13 hard-code、I-205 source 確定 2026-04-27)

**Mandatory step**: PRD draft 完了直後 (Step 4 完了直後) に **13-rule self-applied verify** を
systematic 適用。skill 内で以下 check items を逐一 verify:

1. **Rule 1 (Matrix completeness + abbreviation prohibition)**:
   - (1-1) 全 cell に ideal output 記載?
   - (1-2) `...` / range grouping / `representative` / `(各別 cell)` / `varies` 等 abbreviation 不在?
   - (1-3) `audit-prd-rule10-compliance.py` PASS?
2. **Rule 2 (Oracle grounding + PRD doc embed)**: `## Oracle Observations` section に全 ✗/要調査 cell の 4 項目 (TS fixture / tsc output / cell # link / ideal rationale) 記載?
3. **Rule 3 (NA justification + SWC parser empirical)**: 全 NA cell に SWC parser empirical lock-in test reference?
4. **Rule 4 (Grammar consistency + doc-first)**: doc update task が code 改修 task の prerequisite?
5. **Rule 5 (E2E readiness + Stage tasks separation)**: `## Spec Stage Tasks` + `## Implementation Stage Tasks` 2-section split?
6. **Rule 6 (Matrix/Design integrity + Scope 3-tier)**: matrix Ideal output ↔ Design token-level 一致 + Scope 3-tier (`In Scope` + `Out of Scope` + `Tier 2 honest error reclassify`) hard-code?
7. **Rule 7 (Control-flow exit sub-case)**: body shape / branch shape sub-case 完全 enumerate?
8. **Rule 8 (Cross-cutting invariant)**: `## Invariants` 独立 section + 各 invariant 4 項目 (a)(b)(c)(d) 記載?
9. **Rule 9 (Dispatch-arm sub-case alignment)**: 実装 dispatch arm と matrix cell 1-to-1 対応?
10. **Rule 10 (Cross-axis matrix completeness)**: 9 default axis enumerate?
11. **Rule 11 (AST node enumerate completeness)**: `_ => ` arm 全廃 + `## Impact Area Audit Findings` section embed?
12. **Rule 12 (Rule 10/11 Mandatory + structural)**: `## Rule 10 Application` section 記入 + audit script PASS?
13. **Rule 13 (Spec Stage Self-Review)**: 本 step 自身、findings を `## Spec Review Iteration Log` に record?

各 finding を PRD doc `## Spec Review Iteration Log` section に record (iteration v1/v2/...)。
Critical findings (Implementation stage 移行 block する findings) 全 fix 後、再度 self-review pass で
Spec stage 完了判定。**Self-review skip 不可** (Rule 13 (13-1)、本 step 不在のまま skill closing 不可)。

#### Audit verify mechanism

`audit-prd-rule10-compliance.py` で `## Spec Review Iteration Log` section 不在 or "self-review
not performed" placeholder のみ → audit fail (Rule 13 (13-4))。

## Design Decision Principles

- **The only criterion is the ideal implementation**: "Is this the theoretically most ideal implementation?" is the sole design criterion. Development effort, cost, and impact scope are not valid design justifications
- **Evaluate current implementation too**: Beyond new design, verify whether existing implementations diverge from ideal. If so, fix in-scope or record in TODO
- **Consistency**: Choose solutions consistent as a type system and architecture. Avoid ad-hoc hacks that handle only specific cases
- **Scope judgment**: Include what is logically part of the same problem. Exclude independently separate problems. Cost is not a criterion for scope decisions
- **Design integrity**: Always perform `.claude/rules/design-integrity.md` checks before finalizing design

## Prohibited

- **Skipping Problem Space Analysis (Step 0b)** — 全 PRD で最優先・必須・例外なし。
  問題空間マトリクスなしに Discovery/設計/実装に進むことは `problem-space-analysis.md`
  違反であり、PRD としての有効性がない。
- **Declaring PRD complete with incomplete matrix** — 「reported defect が fix され
  tests pass」では完了ではない。全セル ideal 仕様一致 + lock-in test が完了条件。
- Skipping Discovery (clarification questions) and writing a PRD
- **Skipping the impact area code review** — every PRD must include both a production code quality review (DRY, orthogonality, cohesion, coupling, doc comments) AND a systematic test coverage review using test techniques before writing the task list
- Writing vague completion criteria ("works properly", "can be used without issues", etc.)
- Including future-proofing design in the PRD (YAGNI)
- Cramming multiple independent features into a single PRD
- Narrowing scope or choosing a non-ideal design because "effort is large" or "impact scope is wide"
- Using ad-hoc solutions (specific-case if branches, etc.) to avoid ideal design
- Declaring something out of scope because "Rust has no directly corresponding syntax" or "cannot be expressed in Rust" — this is a design challenge, not proof of conversion impossibility. If no method is found, interview the user
- Omitting the design integrity review (even if no issues, state "verified")
- Omitting the semantic safety analysis when the PRD changes type resolution or introduces type fallbacks (see `.claude/rules/type-fallback-safety.md`)
- Writing vague task work descriptions, completion criteria, or dependencies (specifically name target files, functions, and types)
- Estimating error count reduction based solely on error category labels without tracing actual code paths for representative instances (at least 3). The estimate must be grounded in confirmed execution path analysis, not hypothetical pattern matching
- Starting implementation without classifying ALL error instances in the target category by root cause. When fixing N errors in a category, first classify every instance into sub-categories by root cause (e.g., "9 from merge bug, 9 from missing return type, 9 from fallback pattern"), then address root causes in priority order. Lesson: I-267 was initially scoped as "return statement ~10 instances" based on label estimation, but individual source-level tracing revealed the dominant root cause was a TypeRegistry merge bug (9 instances), not return statement propagation

## Verification

- `backlog/<prd-id>.md` が存在し、template の必須 section (Background / Problem Space / Goal / Scope / Design / Task List / Test Plan / Completion Criteria) 全て含む
- Step 0a (matrix-driven 判定) の結論が PRD 冒頭に明記されている
- Step 0b (Problem Space) の matrix が全 cell に判定 (✓/✗/NA/要調査) を持つ (空セル 0)
- (matrix-driven の場合) `spec-stage-adversarial-checklist.md` 12-rule 全項目を本 PRD 内で verification 済 (Rule 1-10 + Rule 11 AST node enumerate completeness check + Rule 12 Mandatory application + structural enforcement)
- (全 PRD 共通) `## Rule 10 Application` section が記入済 + `scripts/audit-prd-rule10-compliance.py <new-prd-path>` exit code 0 (audit pass) — exit code 非 0 (audit fail) の場合は PRD doc 修正後 skill closing
- Step 3 (Impact Area Code Review) で production code + test coverage の review 結果が PRD に記載されている
- Discovery (Step 2) で user に対し motivation / success / constraint の 3 種 hearing が完了
- TODO の関連 entry が 🔗 link 等で本 PRD と連結

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [problem-space-analysis.md](../../rules/problem-space-analysis.md) | Step 0b の matrix construction methodology (single source of truth) |
| Rule | [spec-first-prd.md](../../rules/spec-first-prd.md) | matrix-driven PRD lifecycle (Stage 1/2 workflow) |
| Rule | [spec-stage-adversarial-checklist.md](../../rules/spec-stage-adversarial-checklist.md) | Spec stage 完了 verification (12-rule checklist、本 skill が参照) |
| Rule | [check-job-review-layers.md](../../rules/check-job-review-layers.md) | Implementation stage 完了 verification (4-layer review、`/check_job` で適用) |
| Rule | [post-implementation-defect-classification.md](../../rules/post-implementation-defect-classification.md) | Implementation review で発見 defect の 5 category 分類 |
| Rule | [design-integrity.md](../../rules/design-integrity.md) | Design Integrity Review (Step 3 の base) |
| Rule | [type-fallback-safety.md](../../rules/type-fallback-safety.md) | Semantic Safety Analysis (型 fallback PRD で必須) |
| Rule | [testing.md](../../rules/testing.md) | Test Coverage Review (Step 3b) で適用する test technique |
| Rule | [conversion-correctness-priority.md](../../rules/conversion-correctness-priority.md) | Tier 1 silent semantic change の判定 |
| Rule | [todo-entry-standards.md](../../rules/todo-entry-standards.md) | Out-of-scope items を TODO 起票する際の format |
| Skill | [tdd](../tdd/SKILL.md) | PRD 起票後の Implementation stage で TDD 適用 |
| Skill | [backlog-management](../backlog-management/SKILL.md) | PRD 完了時の post-processing |
| Skill | [investigation](../investigation/SKILL.md) | 設計前の調査 (report/ への保存) |
| Command | [/check_job](../../commands/check_job.md) | Spec stage / Implementation stage の review trigger |
| Command | [/start](../../commands/start.md) | session 開始 (本 skill の Step 0 から再開) |
| Command | [/end](../../commands/end.md) | PRD close 時の trigger |
