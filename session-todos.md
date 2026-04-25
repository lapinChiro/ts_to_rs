# Session TODOs (Interim Patch Removal Criteria)

このファイルは、進行中 session で適用された **interim patch (暫定対応)** の **削除基準** を集約します。

## 役割

[`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) の interim patch 4 条件 (PRD 同時起票 / `// INTERIM:` コメント / silent semantic change 不在 / 本 file への削除基準記載) のうち、**条件 4** を満たすための storage です。

## Format

各 entry は以下の形式で記述:

```markdown
## INTERIM-NNN: <description>

- **Patch location**: `<file_path>:<line_number>` (e.g., `src/transformer/expressions/binary.rs:142`)
- **Structural fix task**: `[I-XXX]` または `backlog/<id>.md` への link
- **When to remove**: <condition、e.g., "I-XXX completed and pre/post empirical verified", "structural fix が確立し silent semantic change が再現しない">
- **Patch type**: <structural fix を patch で代替している場合の理由 / scope>
- **Silent semantic change verification**: <patch 適用前後で runtime 挙動 diff が empty であることの確認 evidence>
```

## Lifecycle

1. Interim patch 適用時、本 file に entry を **同時** 追加
2. Patch 箇所のコメントに `// INTERIM: INTERIM-NNN` を記載 (本 file の entry ID と一致)
3. Structural fix PRD の完了時、対応 entry を削除
4. 本 file が **空 (entry 0 件)** の状態が clean state、interim patch 残存ゼロを意味する

## 現状 (2026-04-25)

(空) — 現在 interim patch entry は無し。

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [ideal-implementation-primacy.md](.claude/rules/ideal-implementation-primacy.md) | interim patch 4 条件の base、本 file の存在前提 |
| Rule | [check-job-review-layers.md](.claude/rules/check-job-review-layers.md) | Layer 4 (Adversarial trade-off) で interim patch 評価時に本 file を参照 |
| Rule | [conversion-correctness-priority.md](.claude/rules/conversion-correctness-priority.md) | silent semantic change 不在の verification base |
