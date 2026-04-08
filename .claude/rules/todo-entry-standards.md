# TODO Entry Standards

## When to Apply

When adding new items to TODO or modifying existing items.

## Constraints

- **Instance counts must be benchmark-measured values**: Use values aggregated from the `kind` field in `/tmp/hono-bench-errors.json` after running `./scripts/hono-bench.sh`. Do not write from estimates or past memory. Use `scripts/inspect-errors.py` for error inspection (`--category`, `--discriminant --source`, etc.)
- **Source code references must include `file_path:line_number`**: Specify concrete locations, not just function/variable names (e.g., `src/registry.rs:482`). Readers must be able to immediately navigate to the location
- **Error messages must quote actual output**: Use the benchmark `kind` field values or compiler output verbatim
- **Include solution direction**: Beyond describing the problem, document the resolution direction (specific function names, approaches). This provides material for later priority judgments, not to skip Discovery during PRD creation
- **Document dependencies**: When other TODO items are prerequisites, mark with `🔗`. If the reference target doesn't exist in TODO (completed, etc.), write self-contained context
- **Delete completed items immediately**: Completion records are traceable via git history. Only add a one-line summary to the "Completed features (reference)" section

## Investigation Debt Entries (調査債務項目)

`todo-prioritization.md` Step 0 で記録する調査債務項目は以下のフォーマットで記述する。
通常の defect item とは区別するため `[INV-N]` プレフィックスを使用する。

### フォーマット

```markdown
### [INV-N] <調査項目の一文タイトル>

- **Known**: 現時点で判明している事実 (fact のみ、assumption を含めない)
- **Unknown**: 答えが必要な具体的な問い (1 文で書けること)
- **Why it matters**: この答えが不明だと何が決められないか / どの下流タスクが blocked か
- **Investigation method**: 解消手段 (probe, grep, trace, file read 等の具体手順)
- **Impact if wrong assumption**: 誤った assumption で先に進んだ場合に何が壊れるか
- **Resolution target**: いつまでに解消する必要があるか (= 何を始める前に解消すべきか)
```

### 運用ルール

- 調査債務は L1-L4 の tier とは別軸で扱い、`todo-prioritization.md` Step 0 で解消順を決める
- 解消後は debt 項目を削除し、得られた fact を該当 TODO の defect 項目に追記する
- 計画書 (`plan.md`, `backlog/*.md`) から参照される assumption は対応する INV 項目が存在
  しなければならない

## Prohibited

- Vague impact descriptions like "affects N files" (use measured instance counts)
- Documenting instance counts without running the benchmark
- Referring to "this function" or "this process" without file path or line number
- Keeping completed PRD items in "PRD created → backlog/..." format (delete them)
- **Assumption を fact として TODO に記載すること** (`「〜と思われる」「おそらく〜」は Investigation Debt Entry に格納する`)
- **調査債務が存在する状態で PRD 起票に進むこと** (`ideal-implementation-primacy.md` / `todo-prioritization.md` Step 0 違反)
