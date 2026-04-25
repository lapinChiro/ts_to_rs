---
name: large-scale-refactor
description: Large-scale refactoring procedure. 5 steps: analysis → design → task breakdown → review → implementation, committing per phase
user-invocable: true
---

# Large-Scale Refactoring Procedure

## Trigger

When starting work that matches any of the following:
- Changing 10+ function signatures
- Mechanical changes spanning 5+ files
- Dependencies between changed functions require simultaneous modification for compilation

## Actions

Execute the following 5 steps in **strict order**. Do not skip steps.

### Step 1: Analysis

**Exhaustively** enumerate all locations requiring changes.

1. Identify all functions needing changes with `grep` (file path, line number, current signature)
2. Identify each function's callers
3. Build the dependency graph: "changing A requires also changing B"
4. Record results in the "Analysis" section of `tasks.md`

### Step 2: Design

Define change patterns with **concrete code examples**.

1. Define before → after signature transformation patterns
2. Define caller transformation patterns
3. List edge cases (special signatures, calls within conditionals, etc.) and determine handling for each
4. Design new types, functions, or helpers if needed
5. Verify design integrity per `.claude/rules/design-integrity.md` (higher-level consistency, DRY, orthogonality, coupling. If broken windows are found, include in tasks or record in TODO)
6. Record results in the "Design" section of `tasks.md`

### Step 3: Task Breakdown

Create a task list in the "Implementation Tasks" section of `tasks.md` meeting these conditions:

- Divide tasks into **phases**. Each phase completes in a `cargo check`-passing state and is a **committable unit**
- Each task within a phase involves changes to **at most 1 file**
- Specify task execution order (based on dependencies)
- Include **completion criteria** for each task (e.g., "cargo check shows 0 errors for this file")
- Include a **commit task** at the end of each phase (per `.claude/rules/incremental-commit.md`, `[WIP]` commit)
- Include **final verification steps** after all tasks (cargo test, clippy, etc.)
- Use checkbox `- [ ]` format

### Step 4: Review

Review `tasks.md` from these perspectives and fix any issues:

1. **Completeness**: Are all locations from the analysis covered by tasks? Re-verify with `grep`
2. **Dependency consistency**: If task A depends on task B, is B ordered before A?
3. **Compilability**: Is the design such that `cargo check` passes at each task's completion? If not, adjust granularity (e.g., combine multiple files into one task)
4. **Edge case coverage**: Are all design edge cases reflected in tasks?
5. **Test impact**: If test file signature changes are needed, are they separated as dedicated tasks?

Record review results in the "Review Results" section of `tasks.md` (no issues / corrections made).

### Step 5: Implementation

Execute tasks from `tasks.md` in order, top to bottom.

1. Read `tasks.md` before starting a task and confirm the current task
2. When using bulk replacements (Python scripts, sed, etc.), follow `.claude/rules/bulk-edit-safety.md` (dry run → review → execute)
3. Update checkbox to `- [x]` upon task completion
4. Execute completion criteria and confirm they're met. Follow `.claude/rules/command-output-verification.md` for output verification
5. When all tasks in a phase are complete, commit per `.claude/rules/incremental-commit.md`
6. After all tasks are complete, execute final verification steps

## Prohibited

- Starting Step 5 (implementation) without completing Steps 1-4
- "Discovering" change locations not in the analysis during implementation and handling them ad-hoc (if discovered, add to tasks.md before addressing)
- Delegating implementation to sub-agents (sub-agents may be used for analysis support)
- Completing tasks without updating `tasks.md`
- Mixing changes from different tasks in one task's work

## Verification

- `tasks.md` exists and contains "Analysis", "Design", "Implementation Tasks", and "Review Results" sections
- All task checkboxes are `[x]`
- Final verification step execution results are recorded

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [design-integrity.md](../../rules/design-integrity.md) | Design step での 4 観点 review (cohesion / DRY / orthogonality / coupling) |
| Rule | [pipeline-integrity.md](../../rules/pipeline-integrity.md) | refactor 対象が pipeline boundary を跨ぐ場合の整合性 |
| Rule | [incremental-commit.md](../../rules/incremental-commit.md) | phase 完了 commit 原則 (本 skill は phase 単位 commit) |
| Rule | [bulk-edit-safety.md](../../rules/bulk-edit-safety.md) | bulk edit 適用時の dry-run procedure |
| Rule | [command-output-verification.md](../../rules/command-output-verification.md) | refactor 後の cargo check / test 出力 verify |
| Skill | [refactoring-check](../refactoring-check/SKILL.md) | feature 後の refactor 候補抽出 (本 skill より periodic、軽量) |
| Skill | [quality-check](../quality-check/SKILL.md) | refactor 完了 verification |
| Skill | [tdd](../tdd/SKILL.md) | refactor 内で test 追加が必要な場合の procedure |
