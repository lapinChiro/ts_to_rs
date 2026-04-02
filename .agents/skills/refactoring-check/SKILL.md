---
name: refactoring-check
description: Use after feature work or notable code changes to review the impact area for refactoring candidates, design drift, and broken-window issues, then record follow-up work in TODO or backlog if needed.
---

# Refactoring Check

## Procedure

1. 変更影響範囲のファイルを読み直す
2. 次を点検する
   - 責務分離
   - DRY 違反
   - 命名と実挙動のズレ
   - workaround の固定化
   - 層違反や broken window
3. 問題があれば適切に記録する
   - 既存 PRD があるなら優先度を見直す
   - PRD 化できるなら `backlog/` に追加する
   - まだ曖昧なら `TODO` に記録する

## Do Not

- feature 実装と refactor を無秩序に混ぜない
- 「汚い」など曖昧な表現だけで記録しない
