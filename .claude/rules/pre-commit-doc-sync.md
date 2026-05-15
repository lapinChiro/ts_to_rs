# Pre-Commit Document Sync

## When to Apply

When creating or proposing a commit message (including `[WIP]` commits).

## Constraints

**Before** creating the commit message, update the following documents if they exist:

1. **Task management files** (`tasks.md`, `tasks.*.md`, etc.): Check completed tasks. Verify that the status sections accurately reflect the changes
2. **Planning file** (`plan.md`): Verify that the completed items list and next work items accurately describe the post-change state
3. **Active backlog/ PRD docs with empirical anchors to modified files**: When the change includes modifications to `.claude/rules/*.md` / `.claude/skills/*/SKILL.md` / `.claude/commands/*.md` / `scripts/*.py` (= file classes that may be empirically anchored in PRD docs via byte / mtime / line / function-count claims), enumerate active backlog/ PRD docs that empirically reference the modified files and sync their anchor tables:
   1. `grep -l '<modified-file-path>' backlog/*.md` to identify referencing PRD docs
   2. For each referencing PRD: `python3 scripts/verify_prd_self_audits.py backlog/<prd>.md` and inspect Axis 4 (external file drift) results
   3. If Axis 4 drifts detected (= byte / mtime mismatch between PRD claim and current state), sync the PRD doc's empirical anchor table (typically in `## Impact Area Audit Findings` or equivalent section) — update `Size (bytes)` / `Last modified` / rationale columns to current empirical state
   4. Re-run Path E to confirm no new external file drift (Axis 4 = 0 increase from pre-modification baseline). Other axes (Axis 1 / 2 / 3) carry pre-existing baseline drifts that are out of scope for this sync
   5. Include the sync edit in the same commit as the file modification (= avoid drift persistence across commits)

Skip if the file does not exist. Do not modify if no update is needed.

## Prohibited

- Proposing a commit message without updating documents
- Deferring document updates across multiple commits ("will update later in batch")
- Including document changes unrelated to the code changes in the same commit (only update documents corresponding to the code changes)
- Modifying `.claude/rules/*.md` / `.claude/skills/*/SKILL.md` / `.claude/commands/*.md` / `scripts/*.py` and committing without coordinating Path E re-run + active PRD doc empirical anchor sync (Constraint 3)

## Recurring problem rationale

External file claims in active backlog/ PRD docs (= byte / mtime / line-count / function-count) function as **empirical anchors** to "the specific version of the external dependency that the PRD spec is grounded in" (= bootstrap utility design). When the external file is modified without coordinated PRD doc anchor sync, the anchor goes stale and the PRD spec loses its empirical grounding, which is detected later (= post-fact) by Path E re-runs in subsequent reviews. By making the sync a pre-commit step (Constraint 3), drift never persists across commits and the bootstrap utility's drift detection always reports 0 in Axis 4 baseline state.

## Related Rules

| Rule | Relation |
|------|----------|
| [incremental-commit.md](incremental-commit.md) | Phase 単位 commit の前段で本ルールを適用 |
| [prd-completion.md](prd-completion.md) | PRD close 時の document update (TODO 削除 / plan.md 更新) |
| [bulk-edit-safety.md](bulk-edit-safety.md) | Bulk script による rules / skills / commands modification 後の Constraint 3 適用 trigger |
