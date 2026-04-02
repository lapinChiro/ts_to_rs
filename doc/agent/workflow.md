# Workflow

## Goal

Claude と Codex の両方で同じ実務フローを維持する。

## Start

大きい作業の開始前に以下を確認する。

1. `plan.md`
2. `TODO`
3. 必要なら対象 PRD in `backlog/`
4. 関連する `doc/agent/*.md`

## Implementation Flow

### 新機能またはバグ修正

1. `tdd` skill を使い、先に検証項目を決める
2. RED を確認する
3. GREEN 実装を行う
4. 必要なら REFACTOR を行う
5. 変換挙動の変更なら E2E を追加または拡張する

### 調査

1. `investigation` skill を使う
2. 必要なコード、doc、外部情報を読む
3. `report/` にレポートを保存する

### 実装中の姿勢

- 理想実装を優先する
- 一時しのぎを固定化しない
- 同じ修正を根拠なく繰り返さない
- 新たに見つかった out-of-scope 問題は `TODO` に記録する

## Before Reporting Completion

1. `quality-check` skill を使う
2. 必要なら `refactoring-check` skill を使う
3. `plan.md`、`README.md`、`CLAUDE.md`、関連 doc comments の整合性を確認する
4. `backlog/`, `TODO`, `plan.md` を変更した場合は `backlog-management` skill の規約に従う

## Git Boundary

- `git add`, `git commit`, `git push`, `git reset`, `git checkout`, `git switch`, `git stash`, `git merge`, `git rebase` は実行しない
- 必要ならコミットメッセージ案だけを書く
