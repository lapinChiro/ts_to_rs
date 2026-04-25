---
name: skill-writing
description: Guide for creating and updating skills. Defines required sections, frontmatter, and Related Rules / Skills / Commands integration
user-invocable: true
---

# Skill Writing

## Trigger

When creating a new skill in `.claude/skills/<name>/SKILL.md`, or significantly modifying an existing skill's structure.

## Actions

### 1. Define skill purpose (single responsibility)

各 skill は **single responsibility** を持つ。複数の concern を 1 つの skill に詰め込まない。複数 concern が必要なら skill 分割を検討。

判定基準:
- skill の目的を 1 文で記述できるか?
- 「A かつ B を行う」のような複合 description は分割 candidate
- skill 名 (kebab-case) が purpose を端的に表現できているか?

### 2. Create directory structure

```
.claude/skills/<name>/
└── SKILL.md
```

将来 reference 資料 (template, example) が必要になったら同一 directory 内に追加可能。SKILL.md は必須。

### 3. Required sections

```markdown
---
name: <kebab-case-name>
description: <one-line summary, used by Skill tool dispatch>
user-invocable: true|false
---

# <Title>

## Trigger

<起動条件を明確に。"after X" / "when Y" / "on Z" 等の具体表現>

## Actions

<step-by-step procedure。番号付き list 推奨。各 step に concrete action (具体的な command / file path / decision criterion)>

## Prohibited

<rule-writing.md と同じ原則: 禁止事項を明示的に列挙。無いと LLM は wording を回避して逸脱する>

## Verification

<skill 完了の checkable conditions。"X exists" / "Y is N or above" 等>

## Related Rules / Skills / Commands

<table 形式で参照関係を bidirectional に記載 (skill ↔ rule / skill ↔ skill / skill ↔ command の direction)>

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [<rule>.md](../../rules/<rule>.md) | <relation description> |
| Skill | [<skill>](../<skill>/SKILL.md) | <relation> |
| Command | [/<cmd>](../../commands/<cmd>.md) | <relation> |
```

### 4. Frontmatter conventions

- **name**: file path と一致 (`<name>/SKILL.md` の `<name>`)
- **description**: 1 line、Skill tool dispatch で参照される (短く明確に、jargon 回避)
- **user-invocable**: user が `/<name>` で直接呼べる場合 `true`、内部 helper のみなら `false`

### 5. Section principles (rule-writing.md からの継承)

- **LLM は「what to do」を favorable に解釈する**: Prohibited section なしだと wording を回避して deviate する
- **Vague instructions are ignored**: "appropriately" / "carefully" / "consider" は instructable でない、concrete action に書き換える
- **1 skill, 1 concern**: skill が肥大化したら分割検討

### 6. Related table の必須項目

- 参照する rule (どの rule を invoke するか)
- 参照する skill (内部で他 skill を呼ぶ場合)
- 参照する command (slash command の trigger 関係)
- back-reference: 自分を参照する rule / skill / command も含める (corpus の bidirectional graph 維持)

### 7. Integration check

新規 skill 追加時、以下も update が必要:

- `CLAUDE.md` Workflow table: user-invocable=true なら row 追加
- 関連 rule の `Related Rules` table: 新 skill への参照追加
- 関連 skill の `Related Rules / Skills / Commands` table: 双方向参照
- 関連 command (もしあれば) の table: 双方向参照

## Prohibited

- Required section の省略 (Trigger / Actions / Prohibited は必須)
- description に jargon / abbreviation を含める (Skill tool dispatch に支障)
- Related table の省略
- Bidirectional reference の片方向化 (新規 skill が rule X を参照するなら rule X 側にも back-reference)
- Multiple concerns を 1 skill に詰め込む (1 skill 1 concern)
- Vague trigger / vague action ("appropriately" / "as needed" 禁止)
- skill name に動詞でない名詞のみを使う (`testing` は許容、`verbose-output` は scope 推測困難)

## Verification

- `<name>/SKILL.md` が存在し frontmatter (name / description / user-invocable) が完備
- Required section 4 つ (Trigger / Actions / Prohibited / Verification) 全て存在
- `## Related Rules / Skills / Commands` table が存在し ≥1 entry
- CLAUDE.md Workflow table に登録済 (user-invocable=true の場合)
- 関連 rule / skill / command で bidirectional reference 確立

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Skill | [rule-writing](../rule-writing/SKILL.md) | rule 作成 procedure (本 skill の sibling、structure 原則を共有) |
| Skill | [command-writing](../command-writing/SKILL.md) | command 作成 procedure (本 skill と complementary) |
| Skill | [rule-maintenance](../rule-maintenance/SKILL.md) | rule corpus maintenance (skill 追加時の cross-rule reference 更新で適用) |

## Versioning

- **v1.0** (2026-04-25): I-178+I-183 batch (Phase 2) で skill 自体の作成 procedure を新設。Required section (Trigger / Actions / Prohibited / Verification) + Related table 必須化 + frontmatter convention + integration check (CLAUDE.md / 関連 rule / skill / command) を確立。
