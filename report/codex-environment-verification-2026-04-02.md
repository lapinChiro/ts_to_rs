# Codex Environment Verification 2026-04-02

## Summary

- Verified the Codex-specific repository assets expected by `next.codex-verification.md` exist and are internally consistent.
- Confirmed the local `codex` CLI is installed at `/home/kyohei/.nvm/versions/node/v24.14.0/bin/codex`.
- Confirmed `codex --help`, `codex debug --help`, `codex -p deep-review --help`, and `codex -p lightweight --help` all exit successfully, which indicates `.codex/config.toml` is parseable and the declared profiles are accepted.
- Confirmed the repository state is dirty only because of the newly added Codex assets plus an additive update to `CLAUDE.md`; `.claude/` remains present and untouched.
- Did not obtain direct runtime proof that `.codex/rules/default.rules` and `project_doc_fallback_filenames = ["CLAUDE.md"]` are applied during an authenticated interactive Codex session. This remains an operational uncertainty.

## Detailed Findings

### Base context

- Base commit at investigation time: `da1a702 [WIP] Batch 4b Phase 1-3: convert_ts_type を TsTypeInfo 経由 2 ステップ変換に書き換え`
- Uncommitted changes at investigation time:
  - modified: `CLAUDE.md`
  - untracked: `.agents/`, `.codex/`, `.serena/memories/`, `AGENTS.md`, `design.for_codex.md`, `doc/agent/`, `next.codex-verification.md`

### 1. Repository entrypoint and shared docs

- `AGENTS.md` exists at repo root and expresses the expected Codex entry instructions:
  - Japanese responses
  - required first reads (`plan.md`, `TODO`, `doc/agent/*.md`)
  - Git boundary
  - preferred skills
- Shared agent docs listed in `next.codex-verification.md` all exist under `doc/agent/`:
  - `code-review.md`
  - `project-overview.md`
  - `quality-gates.md`
  - `rust-tooling.md`
  - `task-management.md`
  - `workflow.md`

### 2. Codex CLI and config acceptance

- `which codex` resolved successfully.
- `codex --help` exited with code 0.
- `codex debug --help` exited with code 0.
- `codex -p deep-review --help` exited with code 0.
- `codex -p lightweight --help` exited with code 0.
- Observed CLI options are consistent with the intended policy in `.codex/config.toml`:
  - `-p, --profile`
  - `-a, --ask-for-approval` including `on-request`
  - `-s, --sandbox` including `workspace-write`
  - `--full-auto`
- Observed warning on help commands:
  - `WARNING: proceeding, even though we could not update PATH: Read-only file system (os error 30)`
  - This did not prevent command success, so it is currently a non-blocking environment warning rather than a config failure.

### 3. `.codex/config.toml` static review

- The file declares the intended repository defaults:
  - `profile = "default"`
  - `model = "gpt-5.4"`
  - `approval_policy = "on-request"`
  - `sandbox_mode = "workspace-write"`
  - `project_doc_fallback_filenames = ["CLAUDE.md"]`
- Declared profiles are present and named as expected:
  - `default`
  - `deep-review`
  - `lightweight`
- Because profile-based help invocations succeeded, there is positive evidence that the profile table structure is accepted by the installed CLI.

### 4. `.codex/rules/default.rules` static review

- The rules file exists and uses the expected `prefix_rule(...)` form.
- The intended policy split is present:
  - forbidden: destructive Git operations
  - allow: `cargo build/check/test/clippy/fmt/llvm-cov`, `rg/find/sed/ls/cat/wc`, read-only Git commands
  - prompt: `cargo fix`, install commands, network or external-system commands
- The rule content is directionally consistent with both `AGENTS.md` and `next.codex-verification.md`.
- Limitation:
  - this investigation did not execute an authenticated Codex session that could demonstrate the rules firing at runtime.

### 5. Skill recognition readiness

- Verified skill files exist:
  - `.agents/skills/tdd/SKILL.md`
  - `.agents/skills/quality-check/SKILL.md`
  - `.agents/skills/investigation/SKILL.md`
  - `.agents/skills/refactoring-check/SKILL.md`
  - `.agents/skills/backlog-management/SKILL.md`
  - `.agents/skills/todo-audit/SKILL.md`
- Descriptions are explicit and map cleanly to the trigger examples in `next.codex-verification.md`.
- The skills are single-purpose and short enough to be practical for implicit invocation.

### 6. Claude coexistence

- `.claude/` remains populated with Claude-specific commands, rules, and settings.
- The only tracked Claude-side diff observed is an additive `Shared Agent Docs` section in `CLAUDE.md`.
- No evidence was found that the Codex additions replace or delete Claude assets.

## Gaps and Uncertainties

- Direct proof that Codex loads `AGENTS.md` automatically in a fresh interactive session was not collected here. In this repository task environment, the instruction stream already included the repository guidance, so this behavior cannot be isolated cleanly from the harness.
- Direct proof that `project_doc_fallback_filenames = ["CLAUDE.md"]` affects runtime fallback behavior was not collected.
- Direct proof that `.codex/rules/default.rules` is enforced at runtime was not collected.
- The PATH warning emitted by the installed `codex` binary may be benign, but it should be rechecked in a normal interactive shell if startup behavior becomes noisy.

## Conclusion

- The repository-side Codex environment looks practically usable.
- The strongest confirmed points are file layout, config parseability, profile acceptance, skill presence, and Claude coexistence.
- The main remaining uncertainty is runtime enforcement/behavior inside a real interactive Codex session rather than static file correctness.

## References

- `next.codex-verification.md`
- `AGENTS.md`
- `.codex/config.toml`
- `.codex/rules/default.rules`
- `CLAUDE.md`
- `design.for_codex.md`
- `doc/agent/workflow.md`
- `doc/agent/quality-gates.md`
- `doc/agent/task-management.md`
