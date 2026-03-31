---
name: rule-maintenance
description: Maintenance procedure for rule creation, updates, and deletion. Perform individual review and overall integration check
user-invocable: true
---

# Rule Maintenance

## Trigger

When creating, updating, or deleting rules.

## Actions

### Individual Rule Review (every time)

1. Verify the target rule follows /rule-writing structure
2. Verify 2 randomly selected existing rules similarly
3. Update any rules found to need changes
4. If rule deletion is warranted, state the reason and impact area and get user confirmation

### Full Integration Check (~30% probability)

1. List all rule files in `.claude/rules/`
2. Evaluate from these perspectives:
   - Too many rules? (guideline: 20+ files is a warning sign)
   - Can rules with overlapping triggers be merged?
   - Are rules addressing the same concern scattered across files?
   - Are there stale rules (diverged from current project state)?
3. If merge/deletion proposals exist, state reasons and impact area and confirm with user

### Review Perspectives

- What problem is the rule trying to solve?
- Does the current wording solve that problem?
- Does it follow /rule-writing structure (trigger, actions, prohibited, verification)?
- Is `paths:` frontmatter appropriate? (see /rule-writing "Decision Criteria"). A directory-specific rule without `paths:` wastes context. A global rule with unnecessary `paths:` risks not firing
- Does it contradict or duplicate other rules?
- Is it stale relative to current project state?
- Is the content written in English and well-organized? (see Language & Style below)

### Language & Style

Language requirements differ by directory:

| Directory | Language | Reason |
|-----------|----------|--------|
| `.claude/rules/` | **English** | Machine-consumed; consistency with codebase |
| `.claude/skills/` | **English** | Machine-consumed; consistency with codebase |
| `.claude/commands/` | **Japanese** | User-authored and user-maintained |

When reviewing or updating rules/skills:

1. **Language**: All prose, section headings, and examples must be in English. Japanese text in existing rules/skills should be translated to English during the next update touching that file
2. **Organization**: Content should be logically structured with clear headings, concise bullet points, and no redundant prose. Follow /rule-writing structure
3. **Commands exception**: `.claude/commands/` files are maintained by the user in Japanese — do not translate them to English

## Prohibited

- Deleting rules without user confirmation
- Completing rule updates without checking review perspectives
- Writing new rules or skills in Japanese (translate to English before saving)
- Leaving Japanese text untranslated when updating a rule/skill file
