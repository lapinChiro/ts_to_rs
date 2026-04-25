@report/ の情報を最新化、最適化してください。
誤った情報がないか、古くなっている情報がないか、ミスリードがないか、不足している情報がないかを徹底的に調査し、正してください。
さらに、調査報告自体が陳腐化している場合には、引き継ぐべき内容がないか徹底的にチェックしたうえで、ファイルを削除してください。

**Variant note**: 本 command は **report/ 専用 maintenance**。investigation 自体を新規実施する場合は [/investigation](../skills/investigation/SKILL.md) skill を、TODO + plan.md の整合性 maintenance は [/refresh_todo_and_plan](refresh_todo_and_plan.md) を使用。

## Action

1. `report/` ディレクトリの全ファイルを列挙、各 file の base commit / 観測時点を確認
2. 各 report について以下を判定:
   - **Active**: 現コードベースと整合、参照中の TODO / backlog / plan.md がある → 内容更新のみ
   - **Stale**: 参照されていない、または対象 PRD が close 済 → 引き継ぐべき finding を `doc/handoff/design-decisions.md` に集約 → 削除
   - **Inaccurate**: 現実装と乖離 → 観測再実施で正確な情報に更新、または明示的に "outdated" annotation
3. 削除する file は user 確認を取る (重要 finding が失われる risk のため)

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Skill | [investigation](../skills/investigation/SKILL.md) | report/ への保存 procedure (本 command の input source) |
| Skill | [todo-audit](../skills/todo-audit/SKILL.md) | report と TODO の整合性確認 |
| Skill | [refactoring-check](../skills/refactoring-check/SKILL.md) | refactor 候補抽出 (report base) |
