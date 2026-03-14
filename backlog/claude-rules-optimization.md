# .claude/rules/ のコンテキスト効率最適化

## 背景・動機

現在 `.claude/rules/` に 21 ファイル（計 607 行）が存在し、全て `paths` フロントマターを持たないため、CLAUDE.md（69 行）と合わせて **毎セッション約 676 行が常時コンテキストにロードされている**。

Rules は CLAUDE.md を分割しただけであり、コンテキスト消費の観点ではメリットがない。Claude Code には `paths` フロントマター（条件付きロード）や Skills（オンデマンドロード）といった仕組みがあり、これらを活用することで常時ロード量を大幅に削減できる。

## ゴール

- 常時ロードされるコンテキスト量を **676 行 → 200 行以下** に削減する
- ルールの遵守率は維持する（Skills 化したルールは CLAUDE.md のポインタで確実にトリガーする）
- `cargo test`, `cargo clippy`, `cargo fmt` が変更前と同じ結果になる（機能コードへの影響なし）

## スコープ

### 対象

- `.claude/rules/` 内の全 21 ファイルの再配置
- `CLAUDE.md` の更新（統合ルール + Skills ポインタ）
- `.claude/skills/` の新規作成（ワークフロー系ルールの移行先）
- `paths` フロントマターの付与（スコープ付きルール）

### 対象外

- ルールの内容自体の変更（構造の移動のみ）
- 機能コード（`src/`）への変更
- テストコードへの変更

## 設計

### 分類

全 21 ファイルを以下の 3 カテゴリに分類する:

#### カテゴリ A: CLAUDE.md に統合（常時必要・短い）

全ての作業に適用される基本原則。要点を圧縮して CLAUDE.md に統合する。

| ファイル | 行数 | 圧縮後の目安 |
|---------|------|------------|
| `decision-questions.md` | 16 | 3 行 |
| `git-commit.md` | 17 | 2 行 |
| `verification.md` | 21 | 3 行 |
| `debugging.md` | 24 | 3 行 |
| `deferred-items.md` | 16 | 2 行 |
| `documentation.md` | 26 | 3 行 |
| `rust-analyzer.md` | 19 | 3 行 |
| **計** | **139** | **~20 行** |

#### カテゴリ B: `paths` フロントマター付き Rules（条件付きロード）

特定のファイルパターンにスコープが限定できるルール。マッチするファイルにアクセスしたときだけロードされる。

| ファイル | 行数 | paths |
|---------|------|-------|
| `testing.md` | 34 | `["tests/**", "src/**/tests.rs"]` |
| `dependencies.md` | 17 | `["Cargo.toml"]` |
| `conversion-feasibility.md` | 21 | `["src/transformer/**"]` |
| **計** | **72** | — |

#### カテゴリ C: Skills に移行（オンデマンドロード）

特定のワークフローでのみ必要なルール。CLAUDE.md にポインタを置き、必要時に自動/手動で呼び出す。

| ファイル | 行数 | Skill 名 |
|---------|------|---------|
| `prd-template.md` | 77 | `prd-template` |
| `backlog.md` | 52 | `backlog-management` |
| `backlog-replenishment.md` | 43 | `backlog-replenishment` |
| `rule-writing.md` | 41 | `rule-writing` |
| `rule-maintenance.md` | 37 | `rule-maintenance` |
| `quality-check.md` | 36 | `quality-check` |
| `refactoring.md` | 33 | `refactoring-check` |
| `tdd.md` | 31 | `tdd` |
| `investigation.md` | 26 | `investigation` |
| `todo-replenishment.md` | 20 | `todo-replenishment` |
| **計** | **396** | — |

### CLAUDE.md のポインタセクション

CLAUDE.md に以下のセクションを追加し、Skills の確実なトリガーを担保する:

```markdown
## ワークフロー

以下の状況では対応する Skill を必ず呼び出すこと:

- 新機能・バグ修正の着手 → /tdd
- 作業完了時（コミット前） → /quality-check
- 機能追加完了後 → /refactoring-check
- backlog/ の操作 → /backlog-management
- backlog/ が空で作業依頼を受けた → /backlog-replenishment
- PRD の作成 → /prd-template
- TODO が空で作業依頼を受けた → /todo-replenishment
- 調査タスク → /investigation
- ルールの作成・変更 → /rule-writing, /rule-maintenance
```

### 影響範囲

| 変更対象 | 操作 |
|---------|------|
| `CLAUDE.md` | カテゴリ A の要点統合 + ポインタセクション追加 |
| `.claude/rules/` | カテゴリ A の 7 ファイル削除、カテゴリ B の 3 ファイルに `paths` 付与、カテゴリ C の 10 ファイル削除 |
| `.claude/skills/` | カテゴリ C の 10 Skill を新規作成 |

### コンテキスト消費の試算

| 項目 | 現状 | 最適化後 |
|------|------|---------|
| CLAUDE.md | 69 行 | ~105 行（+20 統合 +16 ポインタ） |
| Rules（常時ロード） | 607 行 | 0 行 |
| Rules（条件付きロード） | 0 行 | 72 行（該当ファイルアクセス時のみ） |
| Skills 説明文 | 0 行 | ~20 行（常時、説明のみ） |
| Skills 本体 | 0 行 | 396 行（呼出時のみ） |
| **常時ロード合計** | **676 行** | **~125 行** |

## 作業ステップ

- [ ] ステップ 1: カテゴリ C の 10 Skills を `.claude/skills/<name>/SKILL.md` として作成する。内容は既存ルールファイルをそのまま移行（フロントマター追加のみ）
- [ ] ステップ 2: カテゴリ B の 3 ファイルに `paths` フロントマターを付与する
- [ ] ステップ 3: CLAUDE.md にカテゴリ A の要点を圧縮統合し、ポインタセクションを追加する
- [ ] ステップ 4: 移行済みの `.claude/rules/` ファイル（カテゴリ A: 7 ファイル、カテゴリ C: 10 ファイル）を削除する
- [ ] ステップ 5: 検証 — 新しいセッションを想定し、全 Skill が正しい構造（フロントマター + 本体）になっていることを確認する

## テスト計画

- **構造検証**: 全 `.claude/skills/*/SKILL.md` が有効なフロントマター（name, description）を持つ
- **paths 検証**: カテゴリ B の 3 ファイルが正しい `paths` フロントマターを持つ
- **CLAUDE.md 検証**: 200 行以下であること
- **ポインタ網羅性**: CLAUDE.md のポインタが全 10 Skills を参照している
- **内容保全**: 移行前後でルールの内容が欠損していないこと（diff で確認）
- **機能コード無影響**: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all --check` が全て通ること

## 完了条件

- `.claude/rules/` に残るファイルが 3 つ（カテゴリ B）のみ
- `.claude/skills/` に 10 つの Skill が存在する
- CLAUDE.md が 200 行以下で、全 Skill へのポインタを含む
- `cargo test`, `cargo clippy`, `cargo fmt --check` が全て 0 エラー・0 警告
- 常時ロードされるコンテキスト量が 200 行以下
