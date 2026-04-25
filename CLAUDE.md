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
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 90  # coverage (threshold 90%, excluding main.rs)
cargo llvm-cov --html                  # generate HTML report (target/llvm-cov/html/)
./scripts/check-file-lines.sh        # .rs file line count check (threshold: 1000 lines)
./scripts/hono-bench.sh              # Hono conversion rate benchmark (directory mode)
./scripts/hono-bench.sh --both       # both directory + single-file modes
```

### Hono Benchmark

Measures Hono framework conversion success rate. Run after conversion feature changes to quantify impact.

- **Run**: `./scripts/hono-bench.sh` (verifies `cargo build --release` and auto-clones Hono repo)
- **Analysis**: `scripts/analyze-bench.py` auto-invoked, aggregating error JSON by category
- **History**: `bench-history.jsonl` (JSONL, one line per run). View with: `python3 -c "import sys,json; [print(f\"{json.loads(l)['timestamp'][:10]} clean={json.loads(l)['clean_pct']}% errors={json.loads(l)['error_instances']}\") for l in open('bench-history.jsonl')]"`
- **Error JSON**: `/tmp/hono-bench-errors.json`
- **Error inspection**: `scripts/inspect-errors.py` — エラーの詳細分析ツール（下記参照）

**Note**: "Clean" = zero conversion errors (`--report-unsupported` with 0 errors), separate from whether generated Rust compiles.

### Error Inspection

`/tmp/hono-bench-errors.json` の詳細分析には `scripts/inspect-errors.py` を使用する。アドホックな Python ワンライナーでの解析は禁止。

```bash
python3 scripts/inspect-errors.py                        # カテゴリ別集計
python3 scripts/inspect-errors.py --kind TYPEOF          # kind で部分一致フィルタ
python3 scripts/inspect-errors.py --category TYPEOF_TYPE # カテゴリで完全一致フィルタ
python3 scripts/inspect-errors.py --file client          # ファイル名で部分一致フィルタ
python3 scripts/inspect-errors.py --source               # エラー箇所の TS ソースを表示
python3 scripts/inspect-errors.py --discriminant --source # Discriminant エラーの AST ノード種を推定
python3 scripts/inspect-errors.py --raw                  # フィルタ後の JSON 出力
```

フィルタは組み合わせ可能（例: `--category INDEXED_ACCESS --source`）。不足機能があれば同スクリプトに追加する。

## Architecture

See [README.md](README.md#ディレクトリ構成) for directory structure.

```
TS source → Parser (SWC AST)
  → ModuleGraph (import/export analysis)
  → TypeCollector + TypeConverter (build TypeRegistry)
  → TypeResolver (pre-compute expression types, expected types, narrowing)
  → Transformer (AST + type info → IR)
  → Generator (IR → Rust source code)
  → OutputWriter (file output, mod.rs generation)
```

## Shared Agent Docs

The Claude and Codex environments are intended to coexist.

- Shared guidance for both agents lives under `doc/agent/`
- Codex entrypoint is `AGENTS.md`
- Claude-specific rules remain under `.claude/`
- Codex-specific settings live under `.codex/` and `.agents/skills/`

## Core Principles

- **Ideal implementation**: Pursue the logically most ideal implementation regardless of cost. No compromises, no ad-hoc solutions. "Too much effort" and "good enough for now" are not valid justifications
- **KISS**: Minimal complexity for current requirements. When conflicting with "ideal implementation", prioritize the ideal
- **YAGNI**: Implement only what is needed now. No unrequested features or extensions
- **DRY + Orthogonality**: DRY eliminates duplication of *knowledge*, not *code appearance*. Keep duplication if sharing increases coupling

## Code Conventions

- `unwrap()` / `expect()` only in test code — see `.claude/rules/testing.md`
- `unsafe` prohibited (requires documented reason + user approval)
- `clone()` acceptable initially; leave TODO comment for unnecessary clones
- Public types/functions must have doc comments (`///`)

## Quality Standards

Maintain **0 errors, 0 warnings** for all changes. Run /quality-check upon work completion.

Coverage threshold ratchet: when measured coverage exceeds threshold by 2+ points, raise threshold by 1 point.

## Code of Conduct

- **Ideal implementation primacy** — 本プロジェクトの最上位目標は「理想的な TS→Rust トランスパイラの獲得」。ベンチ数値は defect 発見のシグナルであり最適化ターゲットではない。Structural fix > interim patch。詳細は `.claude/rules/ideal-implementation-primacy.md`
- **Problem space analysis (最上位 PRD ルール・絶対遵守)** — PRD 作成時は最初に機能の問題空間を網羅 enumerate する。TODO に書かれた defect のみを scope にするのは禁止 (defect は氷山の一角)。入力次元の組合せマトリクスを作成し、全セルに ideal 出力を定義、全セルに test を対応させ、matrix 完全カバーを完了条件に含める。詳細は `.claude/rules/problem-space-analysis.md`
- **Spec-first PRD workflow (SDCDF)** — matrix-driven PRD は Spec stage (grammar-derived matrix + tsc observation + E2E fixture) → Implementation stage の 2-stage workflow で開発する。`doc/grammar/` の reference doc を variant 列挙の source of truth とし、外部 oracle (tsc/tsx) で ideal output を grounding する。Spec stage 完了 verification は [`spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) の **10-rule checklist 全項目** を pass、Implementation stage 完了 verification は [`check-job-review-layers.md`](.claude/rules/check-job-review-layers.md) の **4-layer review** を `/check_job` 初回 invocation で全実施。発見 defect は [`post-implementation-defect-classification.md`](.claude/rules/post-implementation-defect-classification.md) の **5 category** に trace ベースで分類。詳細は `.claude/rules/spec-first-prd.md`
- **Uncertainty-driven investigation** — 不確定要素は一級市民として TODO に `[INV-N]` 形式で記録し、影響範囲が絞れるまで調査を尽くしてから実装に進む。`todo-prioritization.md` Step 0 参照
- **No unilateral conversion feasibility judgments** — "difficult in Rust" is never a valid reason to defer or deprioritize. Applies across all phases: TODO, plan.md, PRD. See `.claude/rules/conversion-feasibility.md`
- **Strict PRD completion criteria** — see `.claude/rules/prd-completion.md`
- **PRD design review**: PRD の設計セクション作成後、凝集度・責務分離・DRY の 3 観点で第三者目線のレビューを行う — see `.claude/rules/prd-design-review.md`
- **Incremental commits**: Commit at each phase completion for multi-phase work — see `.claude/rules/incremental-commit.md`
- **Pre-commit doc sync**: Update tasks.md / plan.md before commit messages — see `.claude/rules/pre-commit-doc-sync.md`
- **Bulk edit safety**: Script bulk replacements follow dry run → review → execute — see `.claude/rules/bulk-edit-safety.md`
- **Git operation restrictions**: Only the user performs `git commit` / `push` / `merge`. Claude only proposes commit messages
- **Questions with decision criteria**: Present options, pros/cons, and recommendations. No vague "Is this OK?" questions. Decide yourself when possible
- **Verification principle**: Define verification items and expected results before execution. No post-hoc judgments
- **Command output verification**: cargo test / clippy / fmt 等の出力は full content を Read して確認する。tail / 浅い filter は禁止 — see `.claude/rules/command-output-verification.md`
- **Conversion correctness priority**: 変換問題は Tier 1 (silent semantic change、最優先) > Tier 2 (compile error) > Tier 3 (unsupported syntax) で分類して優先処理。詳細は `.claude/rules/conversion-correctness-priority.md`
- **Type fallback safety**: 型 fallback (Any / wider union / HashMap) 導入 PRD は 3-step safety analysis で silent semantic change 不在を verify — see `.claude/rules/type-fallback-safety.md`
- **Pipeline integrity**: IR は構造化データで保持、transformer → generator の pipeline 方向を逆流させない。詳細は `.claude/rules/pipeline-integrity.md`
- **Design integrity**: 設計時は higher-level consistency / DRY / orthogonality / coupling の 4 観点で第三者視点 review — see `.claude/rules/design-integrity.md`
- **Cargo dependencies**: Cargo.toml 編集時は最新 version を verify、`=` pinning 禁止 — see `.claude/rules/dependencies.md`
- **Debugging**: Hypothesize root cause before next fix attempt. Never repeat the same fix twice
- **Deferred recording**: Record out-of-scope issues in `TODO` — see `.claude/rules/todo-entry-standards.md`
- **Document sync**: When changing code, update plan.md, README.md, CLAUDE.md, doc comments if they become inaccurate
- **Handoff documentation**: Document *why* decisions were made, not just what was decided
- **rust-analyzer**: Run `rust_analyzer_set_workspace` at work start. Reload after config changes. Do not ignore diagnostics errors

## Workflow

### Skills (procedural)

| Trigger | Skill |
|---------|-------|
| Session 開始 (plan.md 確認 + 作業継続) | /start (command) |
| New feature or bug fix | /tdd |
| Work completion (before commit) | /quality-check |
| Thorough review after work completion (matrix-driven PRD) | /check_job (4-layer framework: Mechanical / Empirical / Structural cross-axis / Adversarial trade-off、初回 invocation で全実施) |
| After feature addition | /refactoring-check |
| **PRD (backlog/ task) completion** | /backlog-management (TODO update → backlog deletion → plan.md cleanup → next PRD) または /end (command, commit message 提案まで) |
| End of development session | /todo-audit |
| backlog/ operations | /backlog-management |
| Work request with empty backlog/ | /backlog-replenishment |
| PRD creation | /prd-template |
| Work request with empty TODO | /todo-replenishment |
| Investigation tasks | /investigation |
| TODO review (periodic / after major additions) | /todo-grooming or /refresh_todo_and_plan (command, light touch) |
| Conversion correctness audit | /correctness-audit |
| Hono conversion loop | /hono-cycle (single) or `/loop 0 /hono-cycle` (continuous)。light bench-only は /bench (command) |
| Rule / skill / command creation or modification | /rule-writing (rules), /rule-maintenance (rules), /skill-writing (skills), /command-writing (commands) |
| Large-scale refactoring (10+ sigs, 5+ files) | /large-scale-refactor |
| GitHub Actions log analysis | /analyze-ga-log |

### Commands (slash invocation, light wrapper / prompt)

| Command | 目的 |
|---------|------|
| /start | session 開始時の plan.md 確認 + 作業継続 |
| /end | PRD 完了処理 + commit message 提案 (実体は /backlog-management skill + pre-commit-doc-sync rule) |
| /check_job | matrix-driven PRD の 4-layer review (Spec / Implementation stage 自動判別) |
| /check_problem | 残課題の振り返り (`/check_job` Layer 4 と機能近接、軽量 review) |
| /semantic_review | Tier 1 silent semantic change の専用 review (`type-fallback-safety.md` 適用) |
| /refresh_report | report/ ディレクトリの最新化 |
| /refresh_todo_and_plan | TODO + plan.md の最新化 (light、structural は /todo-grooming skill) |
| /bench | Hono ベンチ取得 + TODO 影響分析 (TDD まで進めない場合は本 command、進める場合は /hono-cycle skill) |
| /step-by-step | 調査 → PRD → 開発 → 確認 の guide (汎用 vague trigger、明確な lifecycle stage がある場合は専用 skill 推奨) |

## Proactive Improvement Principle

When discovering problems or inconsistencies, proactively investigate and fix before the user points them out:

- Do not dismiss warnings, errors, or inconsistencies as "temporary issues"
- Identify root causes before addressing problems
- Judge by "is it in the correct state?" not "is it working?"

## Skill Self-Improvement

Skills evolve with the environment. They are not static prompts.

### Observe

Record in `TODO` with `[skill-feedback:<skill-name>]` tag when:

- Skill instructions were ambiguous, causing hesitation
- Skill steps no longer match the codebase/environment
- User requested a direction change mid-skill (= insufficient instructions)
- You supplemented judgments not written in the skill

Include: what happened, why it's a problem, improvement proposal.

### Amend

When noticing improvement points during skill execution:

1. Explain the issue and its impact
2. Present a proposed diff
3. If approved, apply via `/rule-writing` + `/rule-maintenance`

### Passive Learning

When receiving behavioral correction from the user:

1. Generalize the instruction (pattern, not specific case)
2. Determine storage: project rules → `.claude/rules/`, project preferences → this `CLAUDE.md`
3. Present content and location for confirmation before writing
