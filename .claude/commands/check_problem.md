最も理想的でクリーンな実装にすることだけ正解です。
今回の開発で見つかった課題や未対応の問題で解決していないものはありませんか？
徹底的に振り返り、確認してください。
見つかった課題は今回のスコープで対応するべきものは対応してください。
修正規模が大きい、スコープが違うなど、対応方針に判断が必要なものについては相談してください。

**Light review variant**: 本 command は session 内で発見した未対応問題の **light な振り返り** を担当。深い review が必要な場合は [/check_job](check_job.md) (matrix-driven 4-layer) を使用すること。

## Action

1. session 内 conversation history + 修正 diff を再読し、未対応 / 言及だけで終わった issue を全列挙
2. 各 issue を以下に分類:
   - (a) 本 PRD scope 内 → 即時対応
   - (b) PRD scope 外 (別 PRD 候補) → TODO 起票 ([`todo-entry-standards.md`](../rules/todo-entry-standards.md) format)
   - (c) 判断要 → user に options + pros/cons 提示 ([`ideal-implementation-primacy.md`](../rules/ideal-implementation-primacy.md) の `Decision Criteria`)
3. (b)(c) 後、修正規模 / scope 判断をまとめて user に提示

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [ideal-implementation-primacy.md](../rules/ideal-implementation-primacy.md) | 妥協禁止原則 (本 command の core) |
| Rule | [todo-entry-standards.md](../rules/todo-entry-standards.md) | (b) scope 外 issue 起票 format |
| Rule | [todo-prioritization.md](../rules/todo-prioritization.md) | scope 判断時の priority 軸 |
| Skill | [todo-audit](../skills/todo-audit/SKILL.md) | session 後の TODO 補完 (本 command の structural variant) |
| Command | [/check_job](check_job.md) | 深い review (matrix-driven 4-layer)、本 command の上位 |
| Command | [/semantic_review](semantic_review.md) | Tier 1 silent semantic change 専用 review |
