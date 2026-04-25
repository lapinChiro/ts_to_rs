# Incremental Commit Principle

## When to Apply

When performing work consisting of multiple phases (Phase A, Phase B, etc.) or multiple steps (Step 1, Step 2, etc.).

## Constraints

- When a phase/step is complete and `cargo check` or `cargo test` passes, commit **before** proceeding to the next phase
- Before creating the commit message, update task management and planning files per `pre-commit-doc-sync.md`
- Prefix commit messages with `[WIP]` and specify the completed phase (e.g., `[WIP] P6: Phase A — add tctx parameter`)
- Before running `git checkout` / `git stash`, verify that changes to preserve have been committed

## Prohibited

- Running `git checkout -- <dir>` with uncommitted changes (uncommitted changes from other phases get swept in)
- Bundling multiple phases' changes into a single commit (all phases are lost on reset)
- Proceeding to the next phase without creating a `[WIP]` commit

## Related Rules

| Rule | Relation |
|------|----------|
| [pre-commit-doc-sync.md](pre-commit-doc-sync.md) | Commit message 作成前の document update 手順 |
| [bulk-edit-safety.md](bulk-edit-safety.md) | Bulk edit を phase 単位で commit する原則 |
| [command-output-verification.md](command-output-verification.md) | 各 phase 完了時の `cargo check` / `cargo test` verify |
