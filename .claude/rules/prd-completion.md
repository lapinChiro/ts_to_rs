# Strict PRD Completion Criteria

## When to Apply

When reporting PRD work as "complete".

## Constraints

- **Problem Space matrix 全セルカバー** が最上位完了条件 (`problem-space-analysis.md` 参照)。
  reported defect 修正 + test pass は完了条件の一部に過ぎない。matrix の全セルが ideal
  仕様に一致し、全セルに lock-in test が存在しなければ未完成
- A PRD is not complete unless **all completion criteria** are met
- If some criteria are unmet, do one of the following:
  1. Meet the unmet criteria before reporting completion
  2. Ask the user: "Completion criterion X is unmet. Reason is Y. How should we proceed?" and defer to their judgment
- Even if meeting criteria is difficult, **never unilaterally reduce scope and report completion**

## Tier-transition compliance (broken-fix PRD wording、I-205 source 確定 2026-04-27)

**新機能 PRD と broken-fix PRD では Hono bench / regression criteria の wording が異なる**。
"0 regression" は新機能 PRD 用の単純化された表現。broken-fix PRD (= 既存実装が
**Tier 2 compile error / Tier 1 silent semantic change** で broken の状態を fix する PRD) では
以下の **Tier-transition compliance** 表現を使用する:

### 新機能 PRD (greenfield feature addition)

```markdown
- Hono bench: clean files / errors count 0 regression (pre = post)
```

### Broken-fix PRD (Tier-N → Tier-(N-1) transition)

```markdown
- Tier-transition compliance:
  - Pre-PRD state: existing Tier 2 errors (or Tier 1 silent semantic change) for <feature>
  - Post-PRD state: Tier 1 (compile-pass + tsc runtime stdout 一致) for <feature>
  - Hono bench result classification:
    - **Improvement** (allowed): existing related errors transition Tier-2 → Tier-1
      (clean files count 増加 / errors count 減少 が **expected**、regression ではない)
    - **Preservation** (allowed): existing related errors unchanged (Hono が <feature> を
      使用していない場合の正常な観測結果)
    - **New compile errors** (prohibited): 本 PRD 修正範囲外の features に対して
      新たな compile error 導入は **regression** = 完了 block
```

### 判定基準

PRD が broken-fix か新機能か判定するための classification:

- **Broken-fix PRD**: PRD Background に "既存実装が Tier 2/Tier 1 broken" の empirical
  evidence (compile error log、runtime divergence demo 等) を含む。Architectural concern が
  "framework defect 構造的解消" / "Tier 2 → Tier 1 完全変換" 等。
- **新機能 PRD**: PRD Background に "新規変換 path 追加" / "未対応 syntax 対応" 等の
  greenfield 文脈。Architectural concern が "新規 syntax / feature の Tier 1 完全変換"。

判定が曖昧な場合 (e.g., 既存 Tier 2 unsupported syntax を Tier 1 化、これは broken-fix
とも新機能とも見なせる) は、Hono bench を **Tier-transition compliance** 表現で記述
(より厳格な broken-fix 表現を default とする、ideal-implementation-primacy 観点)。

### Lesson source

I-205 PRD draft v1 (2026-04-27) で "Hono bench 0 regression" を broken-fix PRD context で
記述、第三者 review F12 → broken-fix PRD では既存 Tier 2 errors の Tier 1 化が
**improvement** であり regression と区別すべきと判明 → Tier-transition compliance
wording を本 rule に追加 (本 PRD I-205 self-applied integration)。

## Prohibited

- Reporting a PRD as complete with unmet completion criteria
- Deferring unmet criteria to "a subsequent PRD" and reporting completion without user confirmation
- Unilaterally reducing scope due to implementation difficulty or large effort
- Silently adding deferred items to subsequent PRDs (this breaks subsequent PRDs' assumptions)

## Rationale

PRD completion criteria serve as prerequisites for subsequent PRDs. Reporting completion with unmet criteria breaks subsequent PRD assumptions, causing silent cascading impacts. The later these issues are discovered, the higher the correction cost.

本ルールは [`ideal-implementation-primacy.md`](ideal-implementation-primacy.md) に subordinate (理想実装の達成 = matrix 全セルが仕様通り、patch による完了基準ずらし禁止)。

## Related Rules

| Rule | Relation |
|------|----------|
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 最上位原則 (本ルールが subordinate)。Patch vs structural fix の base |
| [problem-space-analysis.md](problem-space-analysis.md) | Matrix 全セルカバー条件の methodology (本ルールが参照) |
| [spec-first-prd.md](spec-first-prd.md) | Matrix-driven PRD lifecycle で本ルールを適用 |
| [spec-stage-adversarial-checklist.md](spec-stage-adversarial-checklist.md) | Spec stage 完了 verification (matrix 全セル ideal output 記載) |
| [check-job-review-layers.md](check-job-review-layers.md) | Implementation stage 完了 verification (4-layer review) |
