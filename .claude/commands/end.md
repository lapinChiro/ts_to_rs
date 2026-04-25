PRD の完了処理を行ったうえで、コミットメッセージを作成してください。

**Variant note**: 本 command は **`/backlog-management` skill の wrapper + commit message 提案 entry**。skill 単独では commit message proposal 機能なし、本 command が PRD close 時の標準 entry point。

## Action chain

1. **Backlog management**: /backlog-management skill の "Mandatory Steps on PRD Completion" を実施
   - TODO の該当 entry 削除
   - backlog/<id>.md を archive (close marker 追加)
   - plan.md「直近の完了作業」table に追加、「次の作業」table から除去
   - prerequisite chain の更新
2. **Document sync**: [`pre-commit-doc-sync.md`](../rules/pre-commit-doc-sync.md) に従い tasks.md / plan.md / TODO の整合性を verify
3. **Quality gate**: /quality-check skill で cargo fix / fmt / clippy / test の 0 errors / 0 warnings を確認
4. **Commit message 提案**: [`incremental-commit.md`](../rules/incremental-commit.md) の format ([CLOSE] で始まる Japanese message) で提案
5. **User の git commit を待つ**: Claude は commit を実行しない (CLAUDE.md "Git operation restrictions" 参照)

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [prd-completion.md](../rules/prd-completion.md) | PRD 完了基準 (action 1 で verify) |
| Rule | [pre-commit-doc-sync.md](../rules/pre-commit-doc-sync.md) | document update sequence (action 2) |
| Rule | [incremental-commit.md](../rules/incremental-commit.md) | commit message format (action 4) |
| Skill | [backlog-management](../skills/backlog-management/SKILL.md) | post-PRD step 1-3 (action 1 の実体) |
| Skill | [quality-check](../skills/quality-check/SKILL.md) | action 3 の実体 |
| Skill | [todo-audit](../skills/todo-audit/SKILL.md) | action 1 後の TODO 状態 verify |
| Command | [/start](start.md) | 次 PRD 開始 trigger (本 command の successor) |
