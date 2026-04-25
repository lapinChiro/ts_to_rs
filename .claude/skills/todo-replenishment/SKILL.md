---
name: todo-replenishment
description: Replenishment procedure when TODO is empty and user requests work. Analyze current implementation and propose with user hearing
user-invocable: true
---

# TODO Replenishment

## Trigger

When `TODO` is empty (no unrefined ideas) and the user requests work.

## Actions

1. Analyze the current implementation (supported conversions, unsupported syntax, known limitations)
2. Considering the repo's purpose (practical TS → Rust conversion), propose next valuable features/improvements:
   - Unsupported TS syntax (conversion feature expansion)
   - Generated code quality improvements (ownership, error handling, etc.)
   - Development infrastructure (tests, CI, DX)
3. Interview the user to confirm priorities and direction
4. Write agreed items to `TODO`

## Prohibited

- Writing items to `TODO` without user interview
- Asking only "What should we do?" without presenting analysis results (must include concrete proposals)
- Excluding TS syntax from proposals because "Rust has no direct syntax equivalent" — if no conversion method is found, interview the user (do not independently judge "impossible")

## Verification

- TODO に new entry が ≥1 件追加されている (user 承認後の追加)
- 各 entry が `todo-entry-standards.md` の format (priority / kind 件数 / source location / solution direction) 準拠
- user に提示した proposal が analysis 結果 (調査済 candidate item の List) を含んでいる
- 「Rust 表現困難」を理由に exclude した item が存在しない (excluded items は user hearing 経由のみ)

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [todo-prioritization.md](../../rules/todo-prioritization.md) | priority 判定 (proposal 提示時に適用) |
| Rule | [todo-entry-standards.md](../../rules/todo-entry-standards.md) | 起票 format (user 承認後に適用) |
| Skill | [investigation](../investigation/SKILL.md) | proposal 元情報収集 |
| Skill | [todo-grooming](../todo-grooming/SKILL.md) | TODO 補充後の整合性確認 |
