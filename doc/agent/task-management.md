# Task Management

## Three Layers

- `TODO`: PRD 化前の issue inventory
- `backlog/`: 設計済み PRD
- `plan.md`: 実行順序と現在の主作業

流れ:

```text
TODO -> backlog/ -> plan.md -> implementation -> completion cleanup
```

## Rules

- まず新しい発見事項は `TODO` に記録する
- 設計可能になったら `backlog/` へ PRD 化する
- `plan.md` には PRD 化された項目だけを載せる
- 完了した PRD は `backlog/` と `plan.md` に残し続けない
- out-of-scope の発見事項はその場で `TODO` に記録する

## When You Change These Files

以下を見て整合性を崩さない。

- `TODO` の item が `backlog/` と二重管理になっていないか
- `plan.md` の項目が `backlog/` に存在するか
- 既に解消済みの issue が `TODO` に残っていないか
- benchmark 数値や hold reason が古くなっていないか

## Commit-adjacent Rule

このリポジトリでは Git の最終操作はユーザーが行う。  
ただし、コミット前に `plan.md` や関連 task docs を同期させるというルール自体は維持する。
