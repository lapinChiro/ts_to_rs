---
name: hono-cycle
description: Single cycle of Hono conversion → error analysis → TODO grooming → PRD creation → TDD implementation → re-conversion
user-invocable: true
---

# Hono Conversion Improvement Cycle

## Trigger

When the user runs `/hono-cycle`, or when auto-executed via `/loop`.

## Prerequisites

- Hono source exists at `/tmp/hono-src/` (if not, fetch with `git clone --depth 1 https://github.com/honojs/hono.git /tmp/hono-src`)
- `cargo build --release` succeeds

## Measurement Mode Principle

**Hono conversion measurement must always use directory mode.** Single-file mode lacks cross-file type resolution within the project and produces results that diverge from reality.

- **Directory mode** (required): `./target/release/ts_to_rs /tmp/hono-src/src/` — Converts with all file type definitions shared via `build_shared_registry()`. This represents "actual conversion capability"
- **Single-file mode** (reference only): `./target/release/ts_to_rs file.ts` — No type sharing. Use only for individual file debugging

Report directory mode results as the primary metric. If there's a difference from single-file mode, include both.

## Cycle Steps

Execute the following 6 steps in order. Confirm each step's completion before proceeding.

### Step 1: Run Hono Conversion and Collect Errors

1. Build the tool with `cargo build --release`
2. **Run the benchmark script** to collect errors:

```bash
# Directory mode (primary measurement) — always use this
./scripts/hono-bench.sh

# Both modes comparison (when diff analysis is needed)
./scripts/hono-bench.sh --both

# Single-file mode only for individual file debugging
./target/release/ts_to_rs /tmp/hono-src/src/some/file.ts 2>&1
```

3. Check instance counts by error category from script output
4. Results are auto-appended to `bench-history.jsonl` and diffs from previous entry are displayed
5. Identify the latest entry by `timestamp` field for comparison (do not rely on line order)

**File structure**:
- `scripts/hono-bench.sh` — Benchmark execution script (entry point)
- `scripts/analyze-bench.py` — Error JSON analysis (auto-invoked by hono-bench.sh)
- `bench-history.jsonl` — Result history (appended each run). Schema: `{timestamp, git_sha, total_files, clean_files, clean_pct, error_instances, categories}`
- `/tmp/hono-bench-errors.json` — Raw error data (temp file, overwritten each run)

### Step 2: Error Analysis and TODO Update

1. Compare with the previous entry in `bench-history.jsonl` and check changes (`clean_pct` and `error_instances` increase/decrease, category-level changes)
2. **Identify newly surfaced errors** (categories not present before, or categories with increased counts)
3. For new errors:
   - Check source code and identify specific TS patterns
   - **Focus on whether errors are occurring for patterns that should already be handled** (signal for insufficient tests)
   - If not in `TODO`, assign a new ID and add
4. Update `report/hono-conversion-rate-analysis.md` with latest results

### Step 3: TODO Priority Grooming

1. Review all TODO Tiers. Evaluate using the 3 axes from `.claude/rules/todo-prioritization.md`
2. Update priorities based on Hono impact file counts
3. **Patterns that should be handled but are failing** always get highest priority

### Step 4: PRD Creation

1. Select the top PRD-eligible item from TODO
2. Consider whether related items can be batched (same root cause, same module fix)
3. Create a PRD per `/prd-template` format
   - Skip Discovery clarification questions (reason: inefficient to run Discovery every loop iteration. Steps 1-3 provide sufficient information for autonomous judgment)
   - Always include "confirm error resolution in Hono re-conversion" as a completion criterion
4. Place PRD in `backlog/` and update `plan.md`

### Step 5: TDD Implementation

1. Implement following the `/tdd` workflow:
   - Test design → RED → GREEN → REFACTOR → E2E
2. After implementation, run `/quality-check` (cargo fmt, clippy, test all at 0 errors, 0 warnings)
3. After passing quality check, verify the target errors are resolved in the relevant Hono files

### Step 6: Cycle Completion

1. Follow `/backlog-management` procedure for post-processing:
   - Update TODO (add issues discovered during work)
   - Delete completed PRD from backlog/
   - Update plan.md
2. Report cycle results to the user:
   - Fixed syntax patterns
   - Change in clean file count (directory mode basis)
   - Highest priority issue for the next cycle

## Interruption Conditions

Interrupt the cycle and report to the user if any of the following apply:

- **TODO is empty**: No new issues found (conversion rate is sufficiently high)
- **Design decision needed**: Multiple approaches exist and user's direction decision is required
- **Major design change needed**: Issue requires existing architecture changes (e.g., IR extension)
- **3 consecutive cycles with no conversion rate change**: All remaining issues are high difficulty; strategy reassessment needed

## Prohibited

- **Judging error distribution from single-file mode results only** — always measure in directory mode
- Entering implementation without error analysis
- Skipping TODO priority grooming
- Reporting cycle completion without passing quality check
- Auto-executing git commit / push (only the user does this)
- Implementing multiple PRDs simultaneously in one cycle (complete one PRD at a time)
