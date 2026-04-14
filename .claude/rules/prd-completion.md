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

## Prohibited

- Reporting a PRD as complete with unmet completion criteria
- Deferring unmet criteria to "a subsequent PRD" and reporting completion without user confirmation
- Unilaterally reducing scope due to implementation difficulty or large effort
- Silently adding deferred items to subsequent PRDs (this breaks subsequent PRDs' assumptions)

## Rationale

PRD completion criteria serve as prerequisites for subsequent PRDs. Reporting completion with unmet criteria breaks subsequent PRD assumptions, causing silent cascading impacts. The later these issues are discovered, the higher the correction cost.
