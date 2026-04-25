---
name: command-writing
description: Guide for creating and updating slash commands in .claude/commands/. Defines required sections, skill / rule reference structure
user-invocable: true
---

# Command Writing

## Trigger

When creating a new slash command in `.claude/commands/<name>.md`, or significantly modifying an existing command's structure.

## Actions

### 1. Determine command purpose

Slash command は user が `/<name>` で起動する prompt entry point。本質的に **skill の wrapper / trigger / shortcut** または **standalone prompt** の 2 種:

| Type | 例 | When to use |
|------|----|-----------|
| **Skill wrapper** | /end (= /backlog-management の wrapper) | 既存 skill を user-friendly な短 invocation で呼ぶ |
| **Composite** | /start (= chain of skills) | 複数 skill を chain させる lifecycle entry |
| **Standalone prompt** | /check_problem (light review) | skill 化するほど structural ではない汎用 prompt |

### 2. Create file

```
.claude/commands/<name>.md
```

ディレクトリ構造なし、1 file = 1 command。kebab-case 命名 (`refresh_todo_and_plan.md` のような snake_case は legacy、新規は kebab-case 推奨だが、user invocation は `_` も許容)。

### 3. Required structure

```markdown
<command の core prompt (Japanese で OK、日本語応答が project standard)>

**Variant note** (該当する場合): 本 command は <X> の light/heavy variant、対極の structural form は <Y skill / command>

## Action

<具体 action chain (numbered list、各 step で skill / rule / command を明示参照)>

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [<rule>.md](../rules/<rule>.md) | <relation> |
| Skill | [<skill>](../skills/<skill>/SKILL.md) | <relation> |
| Command | [/<cmd>](<cmd>.md) | <relation> |
```

### 4. Action chain の書き方

- **Skill wrapper の場合**: 1-3 step の short chain で済む。各 step に対応 skill を明示
- **Composite の場合**: 各 step が異なる skill を invoke、stage 判定 logic を記述 (e.g., `/start` の "進行中 PRD があれば該当 backlog/<id>.md を読む")
- **Standalone prompt の場合**: skill 化されていない logic を直接記述。ただし **似た structural form (skill / command)** を Variant note で明示し、選択基準を提示

### 5. Discoverability

新規 command 追加時、`CLAUDE.md` の Workflow table の "Commands" section に row を追加。command の目的 + 上位 skill との関係を 1 line で記述。

### 6. Convention

- **Japanese conversational + English structure**: prompt 本体は Japanese (project standard)、`## Action` / `## Related Rules / Skills / Commands` 等の section header は English
- **No Prohibited section**: command は prompt entry point なので Prohibited は skill / rule 側に move
- **No Verification section**: command 自体に verify 手順は不要 (skill が verify を持つ)

## Prohibited

- 1 line bare prompt のみで commit する (action chain と Related table を必須化)
- Skill が既に同等機能を提供している場合、wrapper として明示せず重複定義する
- `CLAUDE.md` Workflow table に登録しない (Discoverability gap)
- Related table の省略
- Variant note の欠落 (similar skill / command が存在する場合は必須、user 選択基準を提示)
- 言語混在の structure section (`## アクション` ではなく `## Action` に統一)

## Verification

- `.claude/commands/<name>.md` が存在
- Action chain section が存在 (numbered list)
- `## Related Rules / Skills / Commands` table が存在し ≥1 entry
- `CLAUDE.md` Workflow table の "Commands" section に登録済
- Variant note (該当する場合) が選択基準を明示
- 関連 skill / rule / command で bidirectional reference 確立

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Skill | [skill-writing](../skill-writing/SKILL.md) | skill 作成 procedure (本 skill の sibling、structure 原則を共有) |
| Skill | [rule-writing](../rule-writing/SKILL.md) | rule 作成 procedure (top-level instruction primacy 原則を共有) |
| Skill | [rule-maintenance](../rule-maintenance/SKILL.md) | corpus maintenance (command 追加時の bidirectional reference 更新で適用) |

## Versioning

- **v1.0** (2026-04-25): I-178+I-183 batch (Phase 2) で command 自体の作成 procedure を新設。Required structure (action chain + Related table) + Variant note convention + Discoverability (CLAUDE.md Workflow table 登録) + Convention (Japanese + English structure / no Prohibited / no Verification for commands) を確立。
