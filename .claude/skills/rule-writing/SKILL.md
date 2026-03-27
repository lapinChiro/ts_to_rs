---
name: rule-writing
description: Guide for creating and updating rules. Defines two rule types (procedural and constraint), required sections, and principles
user-invocable: true
---

# Rule Writing Guide

## Trigger

When creating or updating rules.

## Two Rule Types

Rules are classified into two types. Required sections differ by type.

| Type | Nature | Example |
|------|--------|---------|
| **Procedural rule** | Fires on a specific event and executes concrete steps | "Run quality check on work completion" |
| **Constraint rule** | Defines always-applicable conditions and prohibitions | "Define expected results before verification" |

## Structure

### Procedural Rules

| Section | Required | Content |
|---------|----------|---------|
| **Trigger** | Required | When it fires. Describe the specific event or state |
| **Actions** | Required | What to do. Concrete steps that can be executed mechanically |
| **Prohibited** | Recommended | What not to do. List actions that tend to become loopholes |
| **Verification** | Recommended | Criteria for judging compliance. Must be objectively assessable by a third party |

If a trigger cannot be written for a procedural rule, the rule is either too abstract or should be rewritten as a constraint rule.

### Constraint Rules

| Section | Required | Content |
|---------|----------|---------|
| **When to Apply** | Required | In what situations it is always applicable |
| **Constraints** | Required | Conditions/principles to uphold. Write as concrete behavioral guidelines |
| **Prohibited** | Recommended | List typical behavior patterns that violate the constraints |

## `paths:` Frontmatter for Load Control

Adding `paths:` in YAML frontmatter at the top of a rule file causes the rule to be loaded into context **only when reading/writing files matching the specified paths**. Without `paths:`, the rule is **always loaded**.

```yaml
---
paths:
  - "src/transformer/**"
  - "src/generator/**"
---
```

### Decision Criteria

| Condition | `paths:` | Reason |
|-----------|----------|--------|
| Applies only to specific directories/file types | **Specify** | Reduces context consumption, avoids noise during unrelated work |
| Applies project-wide (coding conventions, Git workflow, documentation conventions, etc.) | **Omit** | Should apply regardless of which files are touched |
| Uncertain | **Omit** | Risk of rule not firing is more severe than context consumption |

## Principles

- **LLMs interpret "what to do" instructions favorably**. Without prohibited sections, they will deviate from intent via methods that don't technically violate the wording. Writing "delete" still permits "strikethrough and keep". Prohibited sections prevent this
- **Vague instructions are ignored**. "Handle appropriately", "be careful", "consider" effectively instruct nothing. Rewrite as concrete actions
- **1 rule, 1 concern**. Do not mix multiple concerns in one file. Split when concerns grow
