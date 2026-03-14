# .claude/rules/ のコンテキスト効率に関する調査

**基準コミット**: `0107548`

## 現状の問題

`.claude/rules/` に 21 ファイル・計 607 行のルールがある。これらは **全て `paths` フロントマターを持たない** ため、CLAUDE.md（69行）と合わせて **毎会話の開始時に全量（約 676 行）がコンテキストにロードされる**。

つまり、CLAUDE.md を分割しただけであり、コンテキスト消費の観点では分割前と同等。むしろファイルヘッダーやシステムメッセージのオーバーヘッド分だけ増えている。

## Claude Code の主要機能比較

| 機能 | ロードタイミング | 毎リクエスト? | コンテキスト効率 | 用途 |
|------|-----------------|--------------|----------------|------|
| **CLAUDE.md** | セッション開始 | YES | 中 | 常時必要なルール、ビルドコマンド |
| **Rules（paths なし）** | セッション開始 | YES | 中（CLAUDE.md と同等） | CLAUDE.md と同じ |
| **Rules（paths あり）** | ファイルアクセス時 | NO | 高 | スコープ付きガイドライン |
| **Skills** | 説明のみ起動時/本体は呼出時 | 説明のみ | 最高 | ワークフロー、リファレンス |
| **Sub-agents** | スポーン時 | NO | 分離 | 複雑タスク、並列作業 |
| **Hooks** | トリガー時 | NO | ゼロ | 自動化（LLM 外で実行） |

### 重要な発見: `paths` フロントマター

Rules ファイルに `paths` フロントマターを付けると、**マッチするファイルにアクセスしたときだけロードされる**:

```markdown
---
paths:
  - "src/transformer/**/*.rs"
---

# Transformer 固有のルール
...
```

これが CLAUDE.md 分割の唯一のメリットを引き出す方法。

### Skills の仕組み

- `.claude/skills/<name>/SKILL.md` に配置
- セッション開始時は **説明文のみ** ロード（コンテキストの約 2%）
- 本体は `/skill-name` で呼び出すか、Claude が関連性を判断して自動ロード
- ワークフローやリファレンス資料に最適

## 現在のルールの分類と最適化案

### A. 常時必要（CLAUDE.md に統合）

以下は全ての作業に適用される基本原則なので、CLAUDE.md に残す:

| ファイル | 行数 | 理由 |
|---------|------|------|
| `verification.md` | 21 | 全ての検証作業に適用 |
| `debugging.md` | 24 | 全てのデバッグに適用 |
| `decision-questions.md` | 16 | 全ての質問に適用 |
| `git-commit.md` | 17 | 全ての Git 操作に適用 |

### B. スコープ付き Rules（`paths` フロントマター）

ファイルパターンで絞れるルール:

| ファイル | 行数 | paths 候補 |
|---------|------|-----------|
| `testing.md` | 34 | `["tests/**", "src/**/tests.rs"]` |
| `tdd.md` | 31 | `["tests/**", "src/**/tests.rs"]` |
| `dependencies.md` | 17 | `["Cargo.toml"]` |
| `rust-analyzer.md` | 19 | 全ファイル対象だが、実質的にはコード編集時のみ |
| `conversion-feasibility.md` | 21 | `["src/transformer/**"]` |

### C. Skills に移行

トリガーが明確で、必要時にのみロードすべきもの:

| ファイル | 行数 | Skill 化の理由 |
|---------|------|---------------|
| `prd-template.md` | 77 | PRD 作成時のみ必要。最大のファイル |
| `backlog.md` | 52 | backlog 操作時のみ必要 |
| `backlog-replenishment.md` | 43 | backlog 空のとき限定 |
| `todo-replenishment.md` | 20 | TODO 空のとき限定 |
| `investigation.md` | 26 | 調査依頼時のみ |
| `rule-writing.md` | 41 | ルール作成時のみ |
| `rule-maintenance.md` | 37 | ルール変更時のみ |
| `refactoring.md` | 33 | 機能追加完了時のみ |

### D. CLAUDE.md に短縮統合可能

内容が短く、独立ファイルにする必要性が薄いもの:

| ファイル | 行数 | 統合先 |
|---------|------|--------|
| `deferred-items.md` | 16 | CLAUDE.md の Core Principles に 2 行で追記 |
| `documentation.md` | 26 | CLAUDE.md の Quality Standards に要約 |

## 最適化後の試算

| カテゴリ | 現状（常時ロード行数） | 最適化後 |
|---------|---------------------|---------|
| CLAUDE.md | 69 | ~120（A+D 統合） |
| Rules（paths なし） | 607 | 0 |
| Rules（paths あり） | 0 | ~120（B: 必要時のみ） |
| Skills | 0 | ~330（C: 呼出時のみ） |
| **合計（常時ロード）** | **676** | **~120** |

常時コンテキスト消費を **約 82% 削減** できる見込み。

## 推奨アクション

1. **CLAUDE.md を充実させる**: 常時必要なルール（A + D）を統合し、200 行以下を目標にする
2. **`paths` フロントマターを活用**: テスト・依存関係など、スコープが明確なルール（B）に付与
3. **Skills に移行**: PRD テンプレート、backlog 管理、調査手順など、特定ワークフロー用のルール（C）
4. **不要な分割をやめる**: 16 行のルールを独立ファイルにする必要はない

## 注意事項

- Skills は `.claude/skills/<name>/SKILL.md` の形式で配置する
- Skills のフロントマターで `description` を書くと、Claude が自動判断でロードできる
- `disable-model-invocation: true` を付けると手動呼出専用になり、説明文のコンテキストも消費しない
- Sub-agents は CLAUDE.md を継承するが、会話履歴や Skills は継承しない
