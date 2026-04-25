十分な調査を行ったうえで、TODO と plan.md の情報を最新化、最適化してください。
誤った情報がないか、古くなっている情報がないか、ミスリードがないか、不足している情報がないかを徹底的に調査し、正してください。

**Light variant**: 本 command は単発の最新化を担当 (event-driven)。periodic な structural review が必要な場合は [/todo-grooming](../skills/todo-grooming/SKILL.md) skill を使用すること。

## Action

1. `./scripts/hono-bench.sh` を実行して bench-derived 件数を実測値に更新 ([`todo-entry-standards.md`](../rules/todo-entry-standards.md))
2. `git log` で最近の commit + PRD close 状況を確認、TODO の "Completed" 移行漏れを検出
3. plan.md の prerequisite chain と現状を cross-check (進行中作業 / 直近の完了作業 / 次の作業 が現実と一致するか)
4. 不一致があれば update、なければ "no changes" として report

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [todo-entry-standards.md](../rules/todo-entry-standards.md) | bench-derived 件数 update format |
| Rule | [todo-prioritization.md](../rules/todo-prioritization.md) | priority 並べ替え時の axis |
| Rule | [pre-commit-doc-sync.md](../rules/pre-commit-doc-sync.md) | document update sequence |
| Skill | [todo-grooming](../skills/todo-grooming/SKILL.md) | structural / periodic review (本 command の上位) |
| Skill | [todo-audit](../skills/todo-audit/SKILL.md) | post-development TODO audit |
| Skill | [backlog-management](../skills/backlog-management/SKILL.md) | TODO ↔ backlog ↔ plan.md 整合性 |
