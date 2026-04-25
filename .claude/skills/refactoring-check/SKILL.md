---
name: refactoring-check
description: Post-completion procedure to evaluate and record refactoring candidates after feature additions/changes
user-invocable: true
---

# Refactoring Candidate Recording

## Trigger

When feature addition/change work is complete.

## Actions

1. Identify files in the impact area of the current changes
2. Read each file in the impact area with fresh eyes, evaluating refactoring needs from these perspectives:
   - Follow `.claude/rules/design-integrity.md` checklist: verify **higher-level design consistency**, **DRY (knowledge duplication)**, **orthogonality**, **coupling**
   - Are names diverging from actual behavior?
   - Are there places forced into workarounds because the design isn't what it should be? (Are workarounds becoming entrenched?)
   - **Do broken windows exist?** — A module exceeding its proper responsibilities, cross-layer dependencies, design compromises left unaddressed. Broken windows start small but gradually expand from that point. Addressing them when discovered is the lowest cost
3. If refactoring is needed, take the appropriate action:
   - Related PRD already exists in `backlog/` → Raise its priority in `plan.md`
   - Concrete enough to create a PRD → Create a PRD in `backlog/`
   - Still vague/insufficient information → Record in `TODO` (specifically describe what's wrong and why it needs fixing)
   - **For broken windows** → Immediately create a PRD before the impact area expands further, and raise its priority in `plan.md`
4. If action was taken, report the reasoning to the user

## Prohibited

- Performing refactoring simultaneously during feature addition work (do not mix scopes)
- Recording with vague descriptions like "code is messy" or "room for improvement"
- Skipping refactoring candidate review and reporting work as complete
- Duplicating items in `TODO` that already have PRDs in `backlog/`
- Skipping or minimizing refactoring evaluation because the current change is small (change volume is unrelated to improvement potential)

## Verification

- Evidence exists of reviewing code in the impact area upon work completion
- Recorded items include specific problems and reasoning
- No duplication with existing `backlog/` or `plan.md` entries

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [design-integrity.md](../../rules/design-integrity.md) | refactor 候補判定の base (cohesion / DRY / orthogonality / coupling) |
| Rule | [todo-entry-standards.md](../../rules/todo-entry-standards.md) | refactor 候補を TODO 起票する format |
| Skill | [large-scale-refactor](../large-scale-refactor/SKILL.md) | 抽出した refactor 候補のうち大規模なものは本 skill で実施 |
| Skill | [correctness-audit](../correctness-audit/SKILL.md) | conversion correctness 観点の review (本 skill より深い、periodic) |
| Skill | [investigation](../investigation/SKILL.md) | refactor 候補の詳細調査 |
