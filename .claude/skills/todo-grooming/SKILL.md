---
name: todo-grooming
description: Periodic TODO inventory. Re-evaluate holds, out-of-scope items, content accuracy, and priorities
user-invocable: true
---

# TODO Grooming

## Trigger

- User requests TODO cleanup
- After major feature additions (prerequisites may have changed)
- Before selecting next work when backlog is empty

## Actions

Execute the following 5 steps in order. Report changes and reasoning to the user for each step.

### Step 1: Hold Item Review

For all items in the `## Hold` section, verify whether hold reasons are still valid.

**"Waiting for prerequisite task completion" items:**
- Check codebase and tests to self-determine if the prerequisite task is complete
- If complete → Move to `PRD-eligible` section at the appropriate Tier
- No user confirmation needed (determinable from code)

**"Waiting for design direction decision" items:**
- If user confirmation is needed, ask one at a time
- If determinable without confirmation (e.g., a similar direction was already decided for another feature), self-determine

### Step 2: Out-of-Scope Re-evaluation

Re-evaluate all items in the `## Out of Scope` section from these perspectives:

- **Changed prerequisites**: Is the original reason for scoping out still valid?
- **Emerged demand**: Have recent developments added related features, creating demand?
- **Changed effort**: Have other feature implementations made a previously expensive feature cheaper?
- **Conversion feasibility reconsideration**: For items scoped out because "Rust has no direct syntax equivalent", reconsider whether conversion is truly impossible. Evaluate alternative representations using proc macros, traits, enums, etc. If no conversion method is found, interview the user (do not independently judge "impossible")

Move items to `PRD-eligible` if re-evaluation warrants it.

### Step 3: PRD-eligible Item Review

For all items in the `## PRD-eligible` section, verify:

- **Instance count verification**: Run `./scripts/hono-bench.sh` and aggregate by `kind` field in error JSON (`/tmp/hono-bench-errors.json`). Update if counts diverge from TODO entries
- **Add undocumented error categories**: If benchmark detects error categories not in TODO, assign new IDs and add. If same root cause as existing items, append to existing
- **Description accuracy**: Verify all items against these criteria:
  - Source code references (function names, file paths) match current codebase. Update if moved/renamed
  - When referring to specific source locations, use `file_path:line_number` format (e.g., `src/registry.rs:482`)
  - Error messages quote actual output (use `kind` field values verbatim, not estimates)
  - Numeric values like file line counts are measured values
- **Duplicate consolidation**: If multiple items share the same root cause, consider batching
- **Dependency updates**: Check if `🔗 Depends on:` marks have been resolved. Update dependency descriptions for items that depended on completed PRDs
- **Remove completed items**: Check for already-implemented items remaining. Also check for orphaned PRD files in `backlog/`

### Step 4: Skill Feedback Processing

Check TODO items tagged with `[skill-feedback:<skill-name>]`:

- If **2+ feedback items** accumulated for the same skill, analyze patterns and propose skill modifications to the user
- If **1 feedback item** but the divergence between skill instructions and current environment is obvious, propose a fix
- If fix is approved and applied, delete corresponding feedback items from TODO
- If fix is deemed unnecessary, append the reasoning to the feedback item for re-evaluation in the next grooming

### Step 5: Priority Re-evaluation

Re-evaluate all items using `.claude/rules/todo-prioritization.md`:

1. **Root cause clustering**: Group issues sharing the same root cause
2. **Priority level assignment**: Classify each cluster as L1 (reliability) through L4 (localized)
3. **Tier placement**: Map priority levels to Tiers

#### Tier ↔ Level Mapping

| Tier | Corresponding Priority Levels |
|------|-------------------------------|
| **Tier 0** | L1 (Reliability Foundation) + L2 (Design Foundation) |
| **Tier 1** | L3 (Expanding Technical Debt) — blockers and high-leverage items |
| **Tier 2** | Remaining L3 + L4 items with small fix cost |
| **Tier 3** | L4 (Localized Problems) |

Explicitly state reasoning for judgments (e.g., "Moved I-XX to Tier 3 because: L4 — isolated to a single syntax pattern, no downstream impact").

### Output

Report grooming results to the user in this format:

1. **Moved items**: From where to where, and why
2. **Updated items**: What was changed, and why
3. **Deleted items**: What was removed, and why
4. **Tier changes**: Which items moved to which Tier, and why

## Prohibited

- Skipping Steps 1-4 and only executing Step 5 (prioritizing with stale information is pointless)
- Judging hold reasons as "still valid" without verification
- Judging description accuracy as "fine" without verification
- Omitting priority judgment reasoning

## Verification

- All hold section items have verified hold reason validity
- All out-of-scope section items have been re-evaluated
- All PRD-eligible section item descriptions match current codebase
- All priority changes have explicit reasoning
- `[skill-feedback:*]` tagged items have been analyzed for patterns and a disposition determined
