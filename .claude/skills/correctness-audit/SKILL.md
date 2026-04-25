---
name: correctness-audit
description: Thorough audit of conversion logic and test correctness, PRD-ifying issues found. Used as a periodic quality gate
user-invocable: true
---

# Conversion Correctness Audit

## Trigger

- User requests a correctness check or audit
- After a major feature addition cycle (以下の条件のいずれかを満たした時点で発動推奨):
  - **N=5 PRDs 完了**: 直前 audit から 5 件以上の PRD が close 済 (defect の累積を整理する閾値)
  - **Phase boundary**: Phase A / Phase B 等の development phase 切り替わり時 (phase 内の累積 defect を一括整理)
  - **Tier promotion 発生**: 直前 audit 後に L2 → L1 promotion (e.g., I-177 のような silent semantic change 顕在化) が起きた場合 (root cause の波及範囲を再評価)
  - **User-requested**: 上記閾値未達でも user が明示要求した場合

## Actions

Execute the following 3 investigations **in parallel**, saving results as a report in `report/`.
発見された defect は [`conversion-correctness-priority.md`](../../rules/conversion-correctness-priority.md) の Tier 1 (silent semantic change) / Tier 2 (compile error) / Tier 3 (unsupported syntax) で分類し、Tier 1 を最優先で PRD 化する。

### 1. Type Conversion Accuracy Audit

Target files: `src/transformer/types/`, `src/ir.rs`, `src/generator/types.rs`

Verify the following for all type mappings (`pipeline-integrity.md` の IR 構造化原則 + `type-fallback-safety.md` の安全性分析を適用):

- **Type equivalence**: Does the TS type correctly map to the Rust type? No information lost or added?
- **Compilability**: Can the generated Rust code compile with rustc?
- **Edge cases**: Do type combinations (special types in unions, nested generics, etc.) break anything?
- **Type fallback safety**: Any → 具体型 / 具体型 → Any の fallback が silent semantic change を導入していないか ([`type-fallback-safety.md`](../../rules/type-fallback-safety.md) の 3-step analysis)

Specific check items:
- Validity of each `TsKeywordTypeKind` → `RustType` mapping
- All union/intersection patterns (nullable, non-nullable, compound)
- Generics and type parameter propagation
- Behavior differences between type annotation positions vs type alias positions

### 2. Statement/Expression Semantics Audit

Target files: `src/transformer/statements/`, `src/transformer/expressions/`, `src/transformer/functions/`, `src/transformer/classes.rs`, `src/generator/statements.rs`, `src/generator/expressions.rs`

Verify the following for all conversion patterns:

- **Control flow preservation**: Do break/continue/return/throw operate in the correct scope?
- **Expression type safety**: Are generated expressions compilable? (type mismatches, method availability)
- **Runtime behavior equivalence**: Differences in panic vs NaN, mutability, ownership, etc.
- **Edge cases**: Nested structures, compound patterns, implicit type conversions

### 3. Test Quality Audit

Target files: `src/**/tests.rs`, `tests/integration_test.rs`, `tests/compile_test.rs`, `tests/snapshots/`

Verify the following for all tests ([`testing.md`](../../rules/testing.md) の test design techniques を適用):

- **Expected value accuracy**: Can the expected Rust code actually compile? Are the semantics correct?
- **Assertion strength**: Are there tests using only `matches!()` or `is_ok()` without verifying content?
- **Missing tests**: Do normal cases, error cases, and boundary value tests exist for each conversion pattern?
- **Snapshot validity**: Is the code in snapshots skipped in compile_test correct?
- **Tests verify what they should**: Do test names match actual verification content?

### 4. Report Creation

Save findings to `report/conversion-correctness-audit.md`. The report must:

- Include the base commit
- Classify problems by severity (Critical / High / Medium)
- Assign IDs to each problem with specific code locations (file:line_number)
- If a previous audit exists, record changes from last time (resolved, new, unchanged)

### 5. PRD Creation

For all discovered Critical / High problems:

- Check if a related PRD already exists in backlog
- If not, create a PRD per `/prd-template` and place in `backlog/`
- Insert into `plan.md` execution order
- Record Medium problems in `TODO` (PRD creation is optional)

## Prohibited

- Reading only some files and reporting "confirmed the whole thing"
- Reporting problems based only on speculation or generalities (support with specific code locations)
- Missing problems reported in the previous audit (reference the previous report and produce diffs)
- Reporting "no issues" and stopping (explicitly judge OK/NG for every conversion path)
- Judging test expected values as "correct because tests pass" (independently verify the expected values themselves)

## Verification

- `report/conversion-correctness-audit.md` is created/updated
- Verification results are documented for all type mappings and statement/expression conversion patterns
- All discovered Critical / High problems exist as PRDs in `backlog/`
- Diffs from previous audit results are recorded (not required for first audit)

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [conversion-correctness-priority.md](../../rules/conversion-correctness-priority.md) | Tier 1/2/3 分類 (本 audit が defect 分類で適用) |
| Rule | [type-fallback-safety.md](../../rules/type-fallback-safety.md) | 型 fallback 安全性 (Type Conversion Accuracy Audit で適用) |
| Rule | [pipeline-integrity.md](../../rules/pipeline-integrity.md) | IR 構造化 + transformer/generator 分離 (audit 観点) |
| Rule | [testing.md](../../rules/testing.md) | test design techniques (Test Quality Audit で適用) |
| Skill | [investigation](../investigation/SKILL.md) | report/ への保存 procedure (本 audit と同型) |
| Skill | [prd-template](../prd-template/SKILL.md) | 発見 defect の PRD 化 (本 audit の Step 5) |
| Skill | [refactoring-check](../refactoring-check/SKILL.md) | feature 後 review (本 audit より periodic、軽量) |
| Command | [/check_job](../../commands/check_job.md) | matrix-driven PRD review (本 audit の subset / specialized form) |
| Command | [/semantic_review](../../commands/semantic_review.md) | Tier 1 silent semantic change 専用 review |
