@plan.md を確認し、作業を継続してください。
理由に関わらず、実装および設計に妥協は絶対に許容しません。
最も理想的でクリーンな実装にすることだけを考えてください。
開発工数や変更規模は判断基準になりません。最も理想的でクリーンな実装だけが正解です。
これは、プロダクションコードと自動テストのどちらにも適用される根本的なルールであり、精神です。

設計にコストをかけ、徹底的な調査を行い、緻密な設計を行うことは全体の工数を下げつつ完成形の出来をよりよいものにします。
そのため、設計と調査は特に全力で臨んでください。
バッチ化して一緒に行うべきイシューがないか、また、調査中のイシューに先行して対応するべきイシューがないかも徹底的に調査、検討してください。

**Variant note**: 本 command は session 開始時の **lifecycle entry**。ad-hoc な vague 開発 guide が必要な場合は [/step-by-step](step-by-step.md) を使用。明確な lifecycle stage (PRD 起票 / 実装 / review) がある場合は対応 skill (/prd-template / /tdd / /check_job) を直接 invoke する方が効率的。

## Action chain

1. `@plan.md` を読み、現在の状態 + 進行中作業 + prerequisite chain を把握
2. 進行中 PRD があれば該当 `backlog/<id>.md` を読み、stage (Spec / Implementation) と次 task を特定
3. PRD なし or 完了済の場合、prerequisite chain 先頭の PRD を起票 (/prd-template skill 適用)
4. 各 stage に応じて専用 skill を invoke:
   - **Spec stage**: /prd-template (Step 0a/0b matrix) → matrix 確定後 /check_job (10-rule checklist)
   - **Implementation stage**: /tdd (TDD 5-stage) → /quality-check → /check_job (4-layer review)
   - **Close**: /backlog-management または /end (commit message 提案)

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [todo-prioritization.md](../rules/todo-prioritization.md) | 次 PRD 選定時の priority 判定 |
| Rule | [spec-first-prd.md](../rules/spec-first-prd.md) | matrix-driven PRD lifecycle 判定 |
| Rule | [ideal-implementation-primacy.md](../rules/ideal-implementation-primacy.md) | 最上位原則 (prompt 内で明示参照) |
| Skill | [prd-template](../skills/prd-template/SKILL.md) | PRD 起票 (chain Step 3) |
| Skill | [tdd](../skills/tdd/SKILL.md) | Implementation stage の TDD |
| Skill | [quality-check](../skills/quality-check/SKILL.md) | work 完了前の verification |
| Skill | [backlog-management](../skills/backlog-management/SKILL.md) | PRD close 時 |
| Command | [/check_job](check_job.md) | matrix-driven PRD review |
| Command | [/end](end.md) | PRD close + commit message 提案 |
