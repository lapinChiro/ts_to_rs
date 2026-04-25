---
name: backlog-management
description: Management rules when modifying backlog/, TODO, or plan.md. Maintains the three-layer structure flow and consistency
user-invocable: true
---

# Backlog Management

## Trigger

- When modifying `backlog/`, `TODO`, or `plan.md`
- **When PRD work is complete** (after /quality-check, before starting next PRD)

## Three-Layer Structure

| Location | Role | Scope |
|----------|------|-------|
| `plan.md` | Execution order for PRD-ified tasks | Manages execution order of PRDs in `backlog/`. Shows what to do next |
| `backlog/` | Designed, ready-to-start PRDs | 1 feature = 1 file. Follows PRD template |
| `TODO` | Pre-PRD issues and ideas | Inbox for prioritization and PRD eligibility assessment. Shows what to PRD-ify next |

### Pipeline

```
TODO (issue discovery) → backlog/ (PRD creation) → plan.md (execution order) → start → complete → delete
```

## Actions

### Flow

1. Write new ideas to `TODO`
2. **When adding new items, review the entire Tier** — Evaluate the new item's position using the 3 axes from `.claude/rules/todo-prioritization.md` and check relative priority against existing items. If the new item has higher priority than existing Tier 1 items, insert it into Tier 1 and push existing items down
3. Refine `TODO` items during grooming
4. Write PRDs for items with sufficient design clarity and place in `backlog/<name>.md`
5. Delete items from `TODO` once moved to `backlog/`
6. Leave vague/insufficient-info items in `TODO`

### `plan.md` Updates

- When items are **added** to `backlog/` — Insert the new item at the appropriate position in the ordered list
- When `backlog/` items are **completed** — **Delete** the entry from `plan.md`
- `plan.md` contains only PRD-ified task execution order. Do not transcribe TODO content

### Mandatory Steps on PRD Completion (follow this order strictly)

After completing one PRD, execute the following in order **before** starting the next PRD:

1. **TODO update (add + staleness check + ripple effect check)**:
   - Record issues/limitations discovered during work in `TODO` (follow `.claude/rules/todo-entry-standards.md`)
   - **Delete** TODO items directly resolved by the completed PRD (completion records are traceable via git history)
   - Check TODO items **indirectly affected** by the completed PRD's changes and update if needed (e.g., hold reasons expired because a dependency PRD completed, prerequisites changed, descriptions became stale)
   - Verify each item in the **hold section** still has valid hold reasons (move to PRD-eligible if prerequisite tasks completed)
   - Verify phase transition criteria descriptions match reality; update if needed
   - Update benchmark numbers at the top if they changed
2. **Backlog cleanup** — Review the completed PRD file content and **delete** it
3. **plan.md cleanup** — **Delete** the completed PRD entry and promote the next task in queue to "next task"
4. **Start next PRD** — Only after steps 1-3 are complete

Even when completing PRDs consecutively, **execute this procedure for each PRD**.

### Completed Item Handling

- **Delete** completed PRD files from `backlog/`
- **Delete** corresponding entries from `plan.md`
- Completion records are traceable via git history. No need to retain files or entries

### Backlog File Naming

- Kebab-case: `import-export.md`, `directory-support.md`
- Feature content should be guessable from the name

## Prohibited

- Keeping completed information in `plan.md` (strikethrough, completion marks, "completed" labels — no form is acceptable)
- Writing pre-PRD information (TODO content) in `plan.md`
- Keeping completed PRD files in `backlog/`
- Keeping items in `TODO` after moving to `backlog/`

## Verification

- No completed items exist in `plan.md`
- All items in `plan.md` correspond to PRDs in `backlog/`
- All PRDs in `backlog/` are listed in `plan.md`
- Items in `backlog/` do not have duplicates in `TODO`
- No completed items remain in `TODO` (TODO IDs targeted by completed PRDs are deleted)
- Phase transition criteria and benchmark numbers in `TODO` match reality

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [todo-prioritization.md](../../rules/todo-prioritization.md) | TODO 優先度判定 (本 skill が plan.md insertion 順を決める時の base) |
| Rule | [todo-entry-standards.md](../../rules/todo-entry-standards.md) | TODO 項目記載 format (本 skill の "TODO update" step で適用) |
| Rule | [prd-completion.md](../../rules/prd-completion.md) | PRD 完了基準 (本 skill が "PRD 完了処理" の前提として check) |
| Rule | [pre-commit-doc-sync.md](../../rules/pre-commit-doc-sync.md) | commit message 作成前の document update sequence |
| Skill | [backlog-replenishment](../backlog-replenishment/SKILL.md) | backlog 空時の補充 (本 skill の cleanup 後に発動) |
| Skill | [todo-audit](../todo-audit/SKILL.md) | TODO 状態監査 (本 skill が PRD close 後に invoke) |
| Skill | [prd-template](../prd-template/SKILL.md) | 次 PRD の起票 (本 skill が "Next PRD" step で参照) |
| Command | [/end](../../commands/end.md) | PRD 完了処理 + commit message 提案の trigger (本 skill が実体) |
