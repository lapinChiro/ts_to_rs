# CLAUDE.md

TypeScript → Rust conversion codemod CLI tool.

## Response Language

Always respond to the user in **Japanese**. Commit messages must also be in **Japanese**. Code, comments, and documentation may be in English, but conversational responses and commit messages must be in Japanese.

## Tech Stack

- **Language**: Rust
- **TS parsing**: swc_ecma_parser + swc_ecma_ast
- **CLI**: clap
- **Testing**: cargo test + insta (snapshots)
- **Lint**: clippy
- **Formatting**: rustfmt

## Key Commands

```bash
cargo build                # debug build
cargo build --release      # release build
cargo check                # fast type check
cargo test                 # run all tests
cargo fix --allow-dirty --allow-staged  # auto-fix unused imports etc.
cargo clippy --all-targets --all-features -- -D warnings  # lint
cargo fmt --all --check    # format check
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89  # coverage (threshold 89%, excluding main.rs)
cargo llvm-cov --html                  # generate HTML report (target/llvm-cov/html/)
./scripts/check-file-lines.sh        # .rs file line count check (threshold: 1000 lines)
./scripts/hono-bench.sh              # Hono conversion rate benchmark (directory mode)
./scripts/hono-bench.sh --both       # both directory + single-file modes
```

### Hono Benchmark

Measures Hono framework conversion success rate. Run after conversion feature changes to quantify impact.

- **Run**: `./scripts/hono-bench.sh` (internally verifies `cargo build --release` and auto-clones Hono repo)
- **Analysis**: `scripts/analyze-bench.py` is auto-invoked at end of bench run, aggregating error JSON by category
- **History**: Each run appends one line to `bench-history.jsonl` (JSONL format). Comparable with past results
- **Error JSON**: Raw data output to `/tmp/hono-bench-errors.json`

```bash
# View history progression (sorted by timestamp)
cat bench-history.jsonl | python3 -c "
import sys, json
entries = sorted([json.loads(l) for l in sys.stdin if l.strip()], key=lambda e: e['timestamp'])
for r in entries:
    hono = r.get('hono_sha', 'N/A')
    print(f\"{r['timestamp'][:10]}  {r['git_sha']}  hono={hono}  clean={r['clean_files']}/{r['total_files']} ({r['clean_pct']}%)  errors={r['error_instances']}\")
"
```

**Note**: "Clean" means zero conversion errors (`--report-unsupported` with 0 errors), which is a separate metric from whether the generated Rust compiles.

## Architecture

See [README.md](README.md#ディレクトリ構成) for directory structure.

Conversion pipeline:

```
TS source → Parser (SWC AST)
  → ModuleGraph (import/export analysis)
  → TypeCollector + TypeConverter (build TypeRegistry)
  → TypeResolver (pre-compute expression types, expected types, narrowing)
  → Transformer (AST + type info → IR)
  → Generator (IR → Rust source code)
  → OutputWriter (file output, mod.rs generation)
```

## Core Principles

Always adhere to the following principles:

- **Ideal implementation**: Pursue the logically most ideal implementation regardless of development cost. Avoid compromises and ad-hoc solutions; choose solutions that are consistent as a type system and architecture. "Too much effort" or "good enough for now" are not valid design justifications
- **KISS**: Avoid over-engineering. Meet current requirements with minimal complexity. However, when this conflicts with "ideal implementation", prioritize the ideal
- **YAGNI**: Do not build unrequested features, improvements, or extensions. Implement only what is needed now
- **DRY + Orthogonality**: DRY eliminates duplication of *knowledge*, not duplication of *code appearance*. If sharing code increases inter-module coupling, keep the duplication

## Code Conventions

- `unwrap()` / `expect()` usage restrictions — see `.claude/rules/testing.md` for details
- `unsafe` blocks are prohibited (if necessary, document the reason in a comment and get user approval)
- `clone()` is acceptable in initial versions, but leave a TODO comment for unnecessary clones
- Public types and functions must have doc comments (`///`)

## Quality Standards

Maintain **0 errors, 0 warnings** for all changes. Run /quality-check upon work completion.

Coverage threshold ratchet: when measured coverage exceeds the threshold by 2+ points, raise the threshold by 1 point.

## Code of Conduct

- **No unilateral conversion feasibility judgments** — see `.claude/rules/conversion-feasibility.md`
- **Strict PRD completion criteria** — see `.claude/rules/prd-completion.md`
- **Incremental commits**: Commit at each phase completion for multi-phase work — see `.claude/rules/incremental-commit.md`
- **Pre-commit doc sync**: Update tasks.md / plan.md before creating commit messages — see `.claude/rules/pre-commit-doc-sync.md`
- **Bulk edit safety**: Script-based bulk replacements follow dry run → review → execute — see `.claude/rules/bulk-edit-safety.md`
- **Git operation restrictions**: Only the user performs `git commit` / `push` / `merge`. Claude only proposes commit messages
- **Questions with decision criteria**: Present options, pros/cons, and recommendations. Never ask vague questions like "Is this OK?" without decision criteria. Make decisions yourself when possible
- **Verification principle**: Define verification items and expected results before execution. No post-hoc judgments
- **Debugging**: If a fix doesn't succeed on the first attempt, hypothesize the root cause before the next fix. Never repeat the same fix twice
- **Deferred recording**: Record out-of-scope issues in `TODO` (see `.claude/rules/todo-entry-standards.md` for entry criteria)
- **Document sync**: When changing code, verify and update plan.md, README.md, CLAUDE.md, and doc comments if they become inaccurate
- **Handoff documentation**: When handoffs occur, something likely diverged from expectations. When communicating decisions, clearly document *why* the decision was made
- **rust-analyzer**: Run `rust_analyzer_set_workspace` at work start. Reload and check diagnostics after configuration changes. Do not ignore diagnostics errors

## Workflow

Always invoke the corresponding skill in these situations:

- Starting new feature or bug fix → /tdd
- Work completion (before commit) → /quality-check
- After feature addition → /refactoring-check
- **After PRD (backlog/ task) completion** → /backlog-management (strictly follow: TODO update → backlog deletion → plan.md cleanup → start next PRD)
- End of development session (before commit) → /todo-audit
- backlog/ operations → /backlog-management
- Received work request with empty backlog/ → /backlog-replenishment
- PRD creation → /prd-template
- Received work request with empty TODO → /todo-replenishment
- Investigation tasks → /investigation
- TODO review (periodic, or after major feature additions) → /todo-grooming
- Conversion correctness audit (periodic, or after major changes) → /correctness-audit
- Hono conversion improvement loop → /hono-cycle (single) or `/loop 0 /hono-cycle` (continuous)
- Rule creation or modification → /rule-writing, /rule-maintenance
- Large-scale refactoring (10+ signature changes, 5+ files of mechanical changes) → /large-scale-refactor

## Proactive Improvement Principle

When discovering problems or inconsistencies, proactively investigate and fix before the user points them out:

- Do not casually dismiss warnings, errors, or inconsistencies as "temporary issues"
- Identify root causes before addressing problems
- Judge by "is it in the correct state?" not "is it working?"

## Skill Self-Improvement

Skills are not static prompts but components that should evolve with environmental changes.

### Observe

If any of the following occur during skill execution, record them in `TODO` with the `[skill-feedback:<skill-name>]` tag after completion:

- Skill instructions were ambiguous, causing hesitation
- Skill steps no longer match the current codebase or environment
- User requested a direction change mid-skill (= sign of insufficient instructions)
- You supplemented judgments not written in the skill

Records must include: what happened, why it's a problem, and an improvement proposal.

### Amend

If improvement points are noticed during skill execution, propose improvements to the user after completion:

1. Explain the specific issue and how it affected execution
2. Present a proposed fix (diff) for the skill
3. If the user approves, apply via `/rule-writing` + `/rule-maintenance` procedures

### Passive Learning

When receiving correction instructions about Claude's own behavior from the user:

1. Generalize and abstract the instruction (as a pattern, not a specific case)
2. Determine the storage location:
   - Project-specific rules → append to or create in `.claude/rules/`
   - Personal preferences → append to `~/.claude/CLAUDE.md`
3. Present the content and storage location to the user and get confirmation before writing
