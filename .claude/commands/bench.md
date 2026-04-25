Hono 変換のベンチマークを取り直し、変換失敗を定量的に分析してください。
そのうえで、 @TODO を徹底的に見直してください。

@TODO の情報を最新化、最適化したうえで、それぞれのイシューを詳細に分析し、正確な情報にしたうえで、優先順位付けを行ってください。

## Action

1. `./scripts/hono-bench.sh` を実行 (release build verify + Hono clone) → bench-history.jsonl + /tmp/hono-bench-errors.json 更新
2. `python3 scripts/inspect-errors.py` で category 別集計 (`--source` 等のフィルタ組合せで深堀り)
3. TODO の bench-derived defect 件数 ([`todo-entry-standards.md`](../rules/todo-entry-standards.md)) を `kind` 実測値で update
4. 優先順位は [`todo-prioritization.md`](../rules/todo-prioritization.md) の L1-L4 と root cause clustering で決定

**Hono cycle (TDD まで進める) の場合は本 command ではなく [/hono-cycle](../skills/hono-cycle/SKILL.md) skill を使う**。本 command は bench 取得 + TODO 見直しまでで停止する light version。

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [todo-prioritization.md](../rules/todo-prioritization.md) | Step 4 priority 判定 |
| Rule | [todo-entry-standards.md](../rules/todo-entry-standards.md) | bench-derived 件数 update format |
| Rule | [ideal-implementation-primacy.md](../rules/ideal-implementation-primacy.md) | bench 数値を optimization target にしない原則 |
| Skill | [hono-cycle](../skills/hono-cycle/SKILL.md) | full cycle (bench → TDD → re-conversion) は本 skill |
| Skill | [todo-grooming](../skills/todo-grooming/SKILL.md) | TODO 全体の structural review |
| Skill | [todo-audit](../skills/todo-audit/SKILL.md) | TODO 状態 audit |
