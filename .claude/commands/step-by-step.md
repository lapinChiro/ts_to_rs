調査結果に基づいて、現状を確認する → PRD をスキルに則って作成する → 開発する → 現状を確認する、という順序で修正を進めましょう。

**Generic guide**: 本 command は明確な lifecycle stage が無い ad-hoc な開発を対象とした汎用 guide。明確な stage がある場合は専用 command / skill を使用することを推奨:

- session 開始 → /start
- PRD 起票 → /prd-template skill
- 実装 → /tdd skill
- review → /check_job
- close → /end

## Action chain

1. 現状確認: @plan.md / @TODO / 関連 source code を読む (本 command は調査済 input が前提)
2. PRD 起票: /prd-template skill を invoke (matrix-driven 判定 + Discovery + Problem Space 定義)
3. 開発: /tdd skill を invoke (test → impl → refactor → E2E)
4. 現状確認 (post): /quality-check + /check_job で完了 verification
5. close: /end command で commit message 提案

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Skill | [prd-template](../skills/prd-template/SKILL.md) | Step 2 PRD 起票 |
| Skill | [tdd](../skills/tdd/SKILL.md) | Step 3 開発 |
| Skill | [quality-check](../skills/quality-check/SKILL.md) | Step 4 完了 verification |
| Skill | [investigation](../skills/investigation/SKILL.md) | Step 1 現状確認の structural form |
| Command | [/start](start.md) | session 開始 trigger (本 command の structural form) |
| Command | [/check_job](check_job.md) | Step 4 thorough review |
| Command | [/end](end.md) | Step 5 commit message |
