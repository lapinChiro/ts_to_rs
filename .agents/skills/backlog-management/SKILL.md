---
name: backlog-management
description: Use when modifying backlog/, TODO, or plan.md, or when a PRD-sized task is complete. Keep the three-layer task system consistent and clean up completed PRD state.
---

# Backlog Management

## Three Layers

- `TODO`: pre-PRD issues
- `backlog/`: designed PRDs
- `plan.md`: execution order

## Procedure

1. 新しい issue はまず `TODO` に書く
2. 設計可能になったら `backlog/` に PRD 化する
3. `plan.md` には backlog 項目だけを載せる
4. PRD 完了時は:
   - `TODO` を更新する
   - 完了した PRD を backlog から片付ける
   - `plan.md` の完了項目を掃除する

## Verification

- `plan.md` と `backlog/` に齟齬がない
- `TODO` と `backlog/` に重複がない
- 古い hold reason や benchmark 値が残っていない
