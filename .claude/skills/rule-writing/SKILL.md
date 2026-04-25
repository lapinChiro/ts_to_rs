---
name: rule-writing
description: Guide for creating and updating rules. Defines two rule types (procedural and constraint), required sections, and principles
user-invocable: true
---

# Rule Writing Guide

## Trigger

When creating or updating rules in `.claude/rules/<name>.md`.

## Actions

### 1. Determine rule type

Rules are classified into two types. Required sections differ by type.

| Type | Nature | Example |
|------|--------|---------|
| **Procedural rule** | Fires on a specific event and executes concrete steps | "Run quality check on work completion" |
| **Constraint rule** | Defines always-applicable conditions and prohibitions | "Define expected results before verification" |

### 2. Author required sections per type

#### Procedural Rules

| Section | Required | Content |
|---------|----------|---------|
| **Trigger** | Required | When it fires. Describe the specific event or state |
| **Actions** | Required | What to do. Concrete steps that can be executed mechanically |
| **Prohibited** | Recommended | What not to do. List actions that tend to become loopholes |
| **Verification** | Recommended | Criteria for judging compliance. Must be objectively assessable by a third party |

If a trigger cannot be written for a procedural rule, the rule is either too abstract or should be rewritten as a constraint rule.

#### Constraint Rules

| Section | Required | Content |
|---------|----------|---------|
| **When to Apply** | Required | In what situations it is always applicable |
| **Constraints** | Required | Conditions/principles to uphold. Write as concrete behavioral guidelines |
| **Prohibited** | Recommended | List typical behavior patterns that violate the constraints |

### 3. Apply `paths:` frontmatter for load control

Adding `paths:` in YAML frontmatter at the top of a rule file causes the rule to be loaded into context **only when reading/writing files matching the specified paths**. Without `paths:`, the rule is **always loaded**.

```yaml
---
paths:
  - "src/transformer/**"
  - "src/generator/**"
---
```

| Condition | `paths:` | Reason |
|-----------|----------|--------|
| Applies only to specific directories/file types | **Specify** | Reduces context consumption, avoids noise during unrelated work |
| Applies project-wide (coding conventions, Git workflow, documentation conventions, etc.) | **Omit** | Should apply regardless of which files are touched |
| Uncertain | **Omit** | Risk of rule not firing is more severe than context consumption |

### 4. Add `## Related Rules` table

全 rule に `## Related Rules` table を必須化する。table format:

```markdown
## Related Rules

| Rule | Relation |
|------|----------|
| [rule-name.md](rule-name.md) | <relation description> |
```

参照する / される双方向 (within rule layer) を記載する。**skill / command への back-reference は rule 側で持たない** (cross-layer back-reference は CLAUDE.md hub model で代替、詳細は `doc/handoff/design-decisions.md` "Framework symmetry の到達範囲" 参照)。

### 5. Apply principles

- **LLMs interpret "what to do" instructions favorably**. Without prohibited sections, they will deviate from intent via methods that don't technically violate the wording. Writing "delete" still permits "strikethrough and keep". Prohibited sections prevent this
- **Vague instructions are ignored**. "Handle appropriately", "be careful", "consider" effectively instruct nothing. Rewrite as concrete actions
- **1 rule, 1 concern**. Do not mix multiple concerns in one file. Split when concerns grow

### 6. Integration check

新規 rule 追加時、以下も update 必要:

- `CLAUDE.md` Code of Conduct: 該当 rule への 1-line reference (適切な layer で)
- 関連 rule の `Related Rules` table: 新 rule への back-reference
- 関連 skill / command (もしあれば) の `Related Rules / Skills / Commands` table: 新 rule への参照 (skill / command 側 から rule への uni-directional は許容)

## Prohibited

- Required section の省略 (Procedural の Trigger / Actions、Constraint の When to Apply / Constraints は省略不可)
- Vague instruction (`"appropriately"` / `"as needed"` / `"carefully"`) の使用
- Multiple concern を 1 file に混在させる (1 rule 1 concern 違反)
- `Related Rules` table の省略
- 古い style (`## Trigger` を `## トリガー` 等の Japanese 表記) の使用 — section header は English 必須 (`rule-maintenance.md` 参照)
- `paths:` frontmatter を「迷ったら specify」する (uncertain 時は omit が default)

## Verification

- `.claude/rules/<name>.md` が存在
- Rule type (procedural / constraint) が明確で、対応する Required sections が揃っている
- `## Related Rules` table が ≥1 entry
- CLAUDE.md Code of Conduct で参照済 (Apply 範囲が project-wide の場合)
- 関連 rule の back-reference 確立 (rule layer 内で双方向)
- section header が全て English

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [ideal-implementation-primacy.md](../../rules/ideal-implementation-primacy.md) | 最上位原則 (新規 rule は本原則に subordinate であるべき) |
| Skill | [rule-maintenance](../rule-maintenance/SKILL.md) | rule の作成・更新後の maintenance procedure (本 skill の延長) |
| Skill | [skill-writing](../skill-writing/SKILL.md) | skill 作成 procedure (sibling、structure 原則を共有) |
| Skill | [command-writing](../command-writing/SKILL.md) | command 作成 procedure (sibling) |

## Versioning

- **v1.1** (2026-04-25): I-178+I-183 batch (Phase 1+2+3) で `## Related Rules` table 必須化を Step 4 に追加。skill / command への back-reference は rule 側未実装 (CLAUDE.md hub model で代替) を Step 4 + integration check (Step 6) で明示。
- **v1.0** (initial): Two Rule Types (Procedural / Constraint) + Required sections + `paths:` frontmatter convention + Principles を確立。
