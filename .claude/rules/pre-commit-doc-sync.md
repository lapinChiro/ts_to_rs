# Pre-Commit Document Sync

## When to Apply

When creating or proposing a commit message (including `[WIP]` commits).

## Constraints

**Before** creating the commit message, update the following documents if they exist:

1. **Task management files** (`tasks.md`, `tasks.*.md`, etc.): Check completed tasks. Verify that the status sections accurately reflect the changes
2. **Planning file** (`plan.md`): Verify that the completed items list and next work items accurately describe the post-change state

Skip if the file does not exist. Do not modify if no update is needed.

## Prohibited

- Proposing a commit message without updating documents
- Deferring document updates across multiple commits ("will update later in batch")
- Including document changes unrelated to the code changes in the same commit (only update documents corresponding to the code changes)
