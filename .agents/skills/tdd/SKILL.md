---
name: tdd
description: Use when implementing a new feature or fixing a bug in this repository. Design tests first, confirm RED, then implement GREEN, refactor, and add E2E coverage when TS-to-Rust conversion behavior changes.
---

# TDD

## Use This Skill When

- 新機能を実装する
- バグを修正する

## Procedure

1. 先に検証項目を列挙する
2. 正常系、異常系、境界値を分ける
3. 対象関数の branch と type partition を洗い出す
4. まずテストを書く
5. 対象テストだけを実行して RED を確認する
6. 最小実装で GREEN にする
7. 必要なら REFACTOR を行う
8. 変換挙動の変更なら `tests/e2e/scripts/` と `tests/e2e_test.rs` を更新する

## Expectations

- テスト名は条件と期待結果が分かるようにする
- `src/**` の内部ロジックは unit test、公開挙動は integration/snapshot/E2E に振り分ける
- `doc/agent/quality-gates.md` と `CLAUDE.md` の testing rules に反しない

## Do Not

- 実装を先に書かない
- RED を確認せずに進めない
- 変換機能変更で E2E を省略しない
