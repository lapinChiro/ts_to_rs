---
name: todo-audit
description: Post-development audit for TODO omissions and staleness of existing items
user-invocable: true
---

# TODO Audit

## Trigger

End of a development session (one or more tasks completed). Run after /quality-check and /refactoring-check.

## Actions

The TODO audit covers two perspectives:

- **Addition audit** (Steps 1-2): Detect issues that should be recorded but aren't
- **Freshness audit** (Step 3): Detect existing items whose descriptions diverge from current state

Step 4 reflects the findings.

### Step 1: Code Marker Check

Search for the following patterns in changed files and files in the impact area:

```bash
grep -rn "todo!\|TODO\|FIXME\|HACK\|WORKAROUND\|XXX" src/ --include="*.rs"
```

For each detected marker:
- Check if a corresponding item exists in `TODO`
- If not, add it to `TODO`
- Pay special attention to `todo!()` macros as they cause runtime panics

### Step 2: Interim Implementation Check

Check whether the current changes introduced any of the following patterns:

- **Fallback values**: `unwrap_or(RustType::Any)`, `unwrap_or(RustType::Unit)`, etc. — interim implementations due to insufficient type inference
- **Information loss**: Ignoring part of the input (e.g., discarding the false branch in conditional types)
- **Hardcoded workarounds**: Code that special-cases specific scenarios
- **Incomplete pattern matches**: Swallowing unhandled patterns with `_ => Err(...)` or `_ => todo!()`
- **Compile test skips**: If new items were added to `skip_compile`

For each item, verify it's documented as impact area in existing `TODO` items.

### Step 3: Existing Item Freshness Check

Based on completed tasks, verify existing `TODO` items are consistent with current state:

- **Hold item prerequisite resolution**: For items in the hold section, check if referenced prerequisite tasks were completed in this session. Move resolved items to the appropriate Tier in "PRD-eligible"
- **Cross-reference validity**: Check for items referencing completed/deleted IDs. Rewrite references to non-existent targets as self-contained descriptions
- **Description accuracy**: Check whether existing items affected by current changes have inaccurate descriptions (counts, prerequisites, impact scope, etc.)

### Step 4: Reflect Findings

Apply changes detected in Steps 1-3 to `TODO`. Follow `.claude/rules/todo-entry-standards.md` for entry criteria.
- New issues: Append to existing items if possible. For independent new issues, assign a new ID and add
- Hold releases and description fixes: Apply changes detected in Step 3

## Prohibited

- Concluding "no issues" without executing the searches/checks in Steps 1-3
- Leaving code `TODO` comments without transcribing them to the `TODO` file
- Judging discovered interim implementations as "no problem since it's working" — interim implementations are tech debt that requires recording
- **Writing cross-references to unknown targets** — When referencing other `I-XX` items, if that ID doesn't exist in TODO (PRD-ified, completed, etc.), write self-contained context. Readers must understand the content without searching for the reference target

## Verification

- Evidence of `grep` search execution exists
- All interim implementations introduced by current changes are recorded in `TODO`
- Code `todo!()` macros have corresponding items in `TODO`
- Evidence of checking whether hold item prerequisites were resolved by completed tasks
- Existing item cross-references and descriptions are consistent with current state

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [todo-entry-standards.md](../../rules/todo-entry-standards.md) | TODO 項目記載 format (`[I-NNN]` / `[INV-N]` 等) |
| Rule | [todo-prioritization.md](../../rules/todo-prioritization.md) | priority 判定 (本 audit が priority labelling を verify) |
| Skill | [todo-grooming](../todo-grooming/SKILL.md) | 5-step periodic grooming (本 audit より broad scope) |
| Skill | [backlog-management](../backlog-management/SKILL.md) | TODO ↔ backlog ↔ plan.md の整合性管理 |
