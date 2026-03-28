---
name: prd-template
description: Template and procedure for creating new PRDs in backlog/. Proceeds through Discovery (clarification questions) → PRD drafting
user-invocable: true
---

# PRD Template

## Trigger

When creating a new PRD in `backlog/`.

## Actions

### 0. Batch Check

Once the target item is determined, check `TODO` for items that should be batched together:

- Items on the **same code path** (addressable by modifying the same functions/modules)
- Items with **explicit overlap/relation** (cross-referenced with 🔗, etc.)
- Items with the **same abstract pattern** (e.g., multiple `TsTypeOperator` variant support)

If applicable items exist, include them in the PRD scope. However, do not force-combine items on independent code paths.

### 1. Discovery

Before writing the PRD, do the following:

1. Ask the user at least 2 clarification questions:
   - Why build this now? (motivation/priority confirmation)
   - What defines success? (completion criteria alignment)
   - Are there constraints? (technical constraints, compatibility with existing features, etc.)
2. Draft the PRD only after receiving answers

### 2. PRD Drafting

Follow this template:

```markdown
# <Title>

## Background

Why this feature is needed. Current problems or issues caused by its absence.

## Goal

What should be achievable when this PRD is complete. Write in specific, verifiable terms.
Avoid vague expressions ("fast", "easy", "intuitive") — use specific numbers, thresholds, and observable behaviors.

## Scope

### In Scope

Bullet list of what this PRD implements.

### Out of Scope

Explicitly list what is excluded. Prevents scope creep.

## Design

### Technical Approach

Implementation strategy. Relationship to existing architecture, modules to modify, new modules to add.

### Design Integrity Review

Per `.claude/rules/design-integrity.md` checklist:

- **Higher-level consistency**: Consistency with one layer above (callers, dependencies, sibling modules)
- **DRY / Orthogonality / Coupling**: Issues found and resolution approach
- **Broken windows**: Existing code problems found, and decision to fix in-scope or record in TODO

If no issues, explicitly state "Verified, no issues."

### Impact Area

List of affected files/modules.

## Task List

Analyze implementation in detail. Describe each task in the following format. Assumes TDD: RED → GREEN → REFACTOR order.

### T1: <Task name>

- **Work**: What specifically to change/add. Specify target files, functions, and types
- **Completion criteria**: Conditions for this task to be considered complete. Include test additions/passing
- **Depends on**: None / T2, T3 (task IDs that must complete first)
- **Prerequisites**: State that must be satisfied before starting this task (if any)

### T2: <Task name>

- **Work**: ...
- **Completion criteria**: ...
- **Depends on**: T1
- **Prerequisites**: ...

## Test Plan

Overview of tests to add/modify. Include normal cases, error cases, and boundary values.

## Completion Criteria

Conditions for this PRD's work to be considered "complete". Include quality checks (clippy, fmt, test).

**Impact estimates (error count reduction) must be verified by tracing actual code paths for at least 3 representative error instances.** Label-based estimation (counting by error category name) is prohibited. Each traced instance must confirm that the proposed fix resolves the specific failure point in the execution path.
```

## Design Decision Principles

- **The only criterion is the ideal implementation**: "Is this the theoretically most ideal implementation?" is the sole design criterion. Development effort, cost, and impact scope are not valid design justifications. "Out of scope because effort is large" or "simplified version because impact scope is wide" are prohibited
- **Evaluate current implementation too**: Beyond new design, verify whether existing implementations diverge from ideal. If so, fix in-scope or record in TODO
- **Consistency**: Choose solutions consistent as a type system and architecture. Avoid ad-hoc hacks that handle only specific cases
- **Scope judgment**: Include what is logically part of the same problem. Exclude independently separate problems. Cost is not a criterion for scope decisions
- **Design integrity**: Always perform `.claude/rules/design-integrity.md` checks before finalizing design

## Prohibited

- Skipping Discovery (clarification questions) and writing a PRD
- Writing vague completion criteria ("works properly", "can be used without issues", etc.)
- Including future-proofing design in the PRD (YAGNI)
- Cramming multiple independent features into a single PRD
- Narrowing scope or choosing a non-ideal design because "effort is large" or "impact scope is wide"
- Using ad-hoc solutions (specific-case if branches, etc.) to avoid ideal design
- Declaring something out of scope because "Rust has no directly corresponding syntax" or "cannot be expressed in Rust" — this is a design challenge, not proof of conversion impossibility. If no method is found, interview the user
- Omitting the design integrity review (even if no issues, state "verified")
- Writing vague task work descriptions, completion criteria, or dependencies (specifically name target files, functions, and types)
- Estimating error count reduction based solely on error category labels without tracing actual code paths for representative instances (at least 3). The estimate must be grounded in confirmed execution path analysis, not hypothetical pattern matching
- Starting implementation without classifying ALL error instances in the target category by root cause. When fixing N errors in a category, first classify every instance into sub-categories by root cause (e.g., "9 from merge bug, 9 from missing return type, 9 from fallback pattern"), then address root causes in priority order. Lesson: I-267 was initially scoped as "return statement ~10 instances" based on label estimation, but individual source-level tracing revealed the dominant root cause was a TypeRegistry merge bug (9 instances), not return statement propagation
