# Codex 向け設計書

更新日: 2026-04-02

## 目的

このリポジトリは現在 `Claude Code` 前提で開発ワークフローが構築されている。  
本設計の目的は、`Codex` でも同等の開発体験を提供できるように、設定・指示・権限・再利用手順を再設計することにある。

前提として、既存の `Claude Code` 開発環境は廃止しない。  
`Claude Code` と `Codex` は併用し、どちらから作業しても同じ repo 運用に収束する状態を目指す。

ここでいう「同等の開発体験」とは、単にコード編集が可能であることではなく、次を含む。

- 作業開始時に同じ前提知識とルールが自動で読み込まれる
- TDD、品質確認、調査、レビュー、バックログ運用などの反復手順が同じ粒度で再利用できる
- Git や権限昇格に関する安全ポリシーが同じ方向に働く
- `TODO` / `plan.md` / `backlog/` を中心とした運用が維持される
- Rust 開発に必要なローカル実行・検証が同様に行える

## 非目標

以下は本設計の非目標とする。

- `CLAUDE.md` や `.claude/` を削除すること
- Claude 専用 workflow を強制的に Codex へ置換すること
- 片方の agent でしか動かない運用に寄せること

## 現状整理

### 1. 現在の Claude Code 前提構成

このリポジトリの Claude 側構成は、実装コードよりも「運用」の層に強く表れている。

- 中央ハブは [CLAUDE.md](/home/kyohei/ts_to_rs/CLAUDE.md)
- Claude 固有資産は [`.claude/`](/home/kyohei/ts_to_rs/.claude) に集約
- 主要要素:
  - `rules/`: テスト、品質、設計、PRD、コミット前同期などの運用ルール
  - `skills/`: `/tdd`、`/quality-check`、`/investigation` などの再利用手順
  - `commands/`: `/start` などの定型起動文脈
  - `settings*.json`: Git 禁止や Cargo 実行許可、MCP 利用などの権限・実行設定

特に重要なのは以下。

- 会話は日本語
- 新機能/バグ修正は `/tdd`
- 作業完了前に `/quality-check`
- `TODO` と `plan.md` を運用上の一次情報として扱う
- Git の `commit` / `push` / `merge` はユーザーのみ
- 0 errors / 0 warnings を維持
- 変換機能変更では E2E を必須にする

### 2. このプロジェクトの本質

プロジェクト本体は Rust CLI であり、Codex 非対応のコード構造はない。

- ビルド/検証は `cargo build`, `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt`, `cargo llvm-cov`
- CI は [`.github/workflows/ci.yml`](/home/kyohei/ts_to_rs/.github/workflows/ci.yml) で標準的に実行
- Node 側補助ツールは `tools/extract-types/`, `tests/e2e/`

したがって、Codex 対応で移植すべき中心は「ツール設定」ではなく「開発運用のエンコード方法」である。

## Codex 公式ベストプラクティス調査

以下は 2026-04-02 時点での OpenAI 公式情報に基づく。

### 1. Codex の基本方針

Codex の公式ドキュメントは、良い開発体験の軸を次のように定義している。

- durable なルールはプロンプトに毎回書かず `AGENTS.md` に置く
- 個人既定値は `~/.codex/config.toml`、リポジトリ固有は `.codex/config.toml`
- 反復ワークフローは `.agents/skills` に Skill 化する
- 外部の生きた情報は MCP で取り込む
- 長期タスクは Plan / thread / subagent / worktree で管理する
- 権限は tight に始め、必要なときだけ広げる

### 2. `AGENTS.md` に関する公式知見

OpenAI の `Custom instructions with AGENTS.md` と `Best practices` によれば:

- Codex は作業前に `AGENTS.md` を自動で読む
- `~/.codex/AGENTS.md` とリポジトリ/サブディレクトリの `AGENTS.md` / `AGENTS.override.md` を段階的に連結する
- カレントディレクトリに近いファイルがより強く効く
- 長く曖昧な指示より、短く実務的な指示の方が良い
- `AGENTS.md` が肥大化したら、詳細手順は別 Markdown に切り出して参照させる

このリポジトリにとっては特に重要である。現在 `CLAUDE.md` に集中しているルールは、Codex では `AGENTS.md` を起点に分解するのが公式に沿う。

### 3. `config.toml` に関する公式知見

OpenAI の `Config basics`, `Advanced Configuration`, `Configuration Reference` によれば:

- 個人設定は `~/.codex/config.toml`
- プロジェクト設定は `.codex/config.toml`
- 優先順位は `CLI flags > profile > project config > user config > system config > built-in defaults`
- CLI / IDE / Codex app は同じ設定レイヤーを共有
- durable な設定対象は model, reasoning effort, sandbox mode, approval policy, MCP, profiles など

本件では、Claude で暗黙に共有されていた前提を `.codex/config.toml` に明示し、利用面の差異を減らすべきである。

### 4. 承認と sandbox に関する公式知見

OpenAI の `Agent approvals & security` と `Best practices` によれば:

- ローカル Codex は既定で network off
- sandbox と approval policy は別レイヤー
- 推奨は tight に始めること
- `workspace-write + on-request` が一般的な開始点
- `--full-auto` は `workspace-write + on-request` の便利 alias
- `untrusted` や granular approval も使える
- `danger-full-access` / `--yolo` は高リスクで非推奨

このプロジェクトは既に Claude 側で強い Git 制約と権限制御を持っているため、Codex でも `danger-full-access` は採用しない。

### 5. Rules に関する公式知見

OpenAI の `Rules` によれば:

- Codex は `.rules` ファイルで sandbox 外実行コマンドの許可/確認/禁止を制御できる
- `prefix_rule(pattern=[...], decision="allow" | "prompt" | "forbidden")` で定義する
- もっとも restrictive な判定が勝つ
- shell wrapper の内部コマンドも分解して判定される
- Smart approvals 時は prefix_rule 候補を提案する

これは `Claude .claude/settings*.json` の allow/deny と機能的に最も近い。  
したがって、Claude の禁止/許可方針は Codex では `.codex/rules/*.rules` へ移すのが自然である。

### 6. Skills に関する公式知見

OpenAI の `Agent Skills` と `Best practices` によれば:

- Skill は `SKILL.md` を中心とした再利用ワークフロー単位
- `.agents/skills` に保存し、CLI / IDE / Codex app で共有できる
- skill description の質が implicit invocation の成否を左右する
- 1 skill = 1 job に絞る
- 反復プロンプトや毎回修正している運用は skill 化すべき

このプロジェクトの `.claude/skills/` はまさにその対象であり、Codex へ移植しやすい。

### 7. モデル選定に関する公式知見

OpenAI の Codex モデル docs では、2026-04-02 時点で `gpt-5.4` が「professional work」向け推奨、`gpt-5.4-mini` が高速・軽量タスク向けとして提示されている。  
一方、Help Center の `Using Codex with your ChatGPT plan` は、Codex CLI / IDE の既定モデルはバージョンと `config.toml` に依存し、モデル指定は `-m` または `config.toml` で行うとしている。

この差分から導かれる設計原則は明確である。

- 既定モデルに依存しない
- リポジトリで期待するモデルを明示する
- 主作業用と軽量作業用を profile で分ける

## 設計原則

Codex 対応は以下の原則で行う。

### 原則 1. 共有ルールと surface 固有設定を分離する

同じ開発体験を作るには、まず agent 非依存の運用ルールを共通資産化し、その上で Claude / Codex それぞれの surface にマッピングする必要がある。

- 共通ルール: 言語、品質基準、Git 制約、テスト規約、タスク運用
- Codex 固有: `AGENTS.md`, `.codex/config.toml`, `.codex/rules/`, `.agents/skills/`
- Claude 固有: `CLAUDE.md`, `.claude/settings*.json`, `.claude/skills/`

### 原則 2. Codex では `AGENTS.md` を第一級ハブにする

Claude では `CLAUDE.md` が中心だが、Codex では `AGENTS.md` が自動読込される。  
Codex で同等体験を実現するには、`AGENTS.md` を入口に据える必要がある。

### 原則 3. 反復ワークフローは skill に移す

`/tdd` や `/quality-check` のような Claude 的 slash command 運用は、Codex では主に Skill で表現する。  
単なる長文指示ではなく、再利用単位として保持する。

### 原則 4. 権限は狭く、許可は rules で明示する

Claude の settings は広い allow list を持つが、Codex では `workspace-write + on-request` を基本にしつつ、よく使う安全なコマンドのみ rules で持ち上げる。

### 原則 5. drift を防ぐため、共通内容は分割して参照する

`CLAUDE.md` と `AGENTS.md` に同じ長文を重複するとすぐ破綻する。  
したがって、共通ルールは `docs/agent/` などに分割し、両者から参照する構造を採る。

## 目標アーキテクチャ

### 1. ファイル構成

以下を新しい標準構成とする。

```text
AGENTS.md
CLAUDE.md
.codex/
  config.toml
  rules/
    default.rules
.agents/
  skills/
    tdd/
      SKILL.md
    quality-check/
      SKILL.md
    investigation/
      SKILL.md
    refactoring-check/
      SKILL.md
    backlog-management/
      SKILL.md
    todo-audit/
      SKILL.md
    ...
doc/
  agent/
    project-overview.md
    workflow.md
    quality-gates.md
    task-management.md
    code-review.md
    rust-tooling.md
```

### 2. 役割分担

#### `AGENTS.md`

Codex の自動読込入口。内容は短く保つ。

含めるべき内容:

- 会話言語は日本語
- この repo の目的
- 必ず読むべき補助 doc の一覧
- 主要コマンド
- `TODO` / `plan.md` / `backlog/` の位置と役割
- TDD と quality check を skill で実行する運用
- Git 制約の要約

`AGENTS.md` に全ルールを詰め込まない。詳細は `doc/agent/*.md` と skill に逃がす。

#### `CLAUDE.md`

Claude 互換のため存続。  
ただし long-form の単一ハブではなくし、`AGENTS.md` と同じ共通 docs を参照する薄いハブへ寄せる。

#### `doc/agent/*.md`

共有知識の本体。  
Claude と Codex の双方が参照する一次資料にする。

推奨分割:

- `project-overview.md`
  - パイプライン概要
  - ディレクトリ構造
  - 主要コマンド
- `workflow.md`
  - 着手、実装、調査、完了時の流れ
- `quality-gates.md`
  - quality-check の実行順と判定基準
- `task-management.md`
  - `TODO`, `plan.md`, `backlog/` の更新規約
- `code-review.md`
  - レビュー観点
- `rust-tooling.md`
  - cargo, clippy, rustfmt, llvm-cov, rust-analyzer

#### `.codex/config.toml`

Codex の repo-scoped durable config。  
個人設定ではなく、この repo で期待する既定値を定義する。

#### `.codex/rules/default.rules`

Codex の execpolicy。  
Claude settings の deny / allow のうち、repo 共有すべき安全ポリシーを移植する。

#### `.agents/skills/`

Claude skills の Codex 版。  
Codex の implicit / explicit skill invocation を前提に設計する。

## 詳細設計

### A. `AGENTS.md` 設計

#### A-1. 目的

Codex 起動直後に必要な情報だけを高速に与える。

#### A-2. 記載方針

`AGENTS.md` には次を明記する。

1. 応答言語
2. リポジトリの目的
3. 参照必須ファイル
4. 完了条件
5. Git 制約
6. Skill の利用規約

#### A-3. 記載例のイメージ

```md
# AGENTS.md

## Language
- Respond to the user in Japanese.

## First reads
- Read `plan.md`, `TODO`, and `doc/agent/workflow.md` before substantial work.

## Required checks before completion
- Run the quality-check skill or execute the commands documented in `doc/agent/quality-gates.md`.

## Git restrictions
- Do not run `git add`, `git commit`, `git push`, `git reset`, `git checkout`, `git switch`, `git merge`, or `git stash`.
- The user performs commit/push/merge operations.

## Reusable workflows
- Use the `tdd` skill for new features or bug fixes.
- Use the `quality-check` skill before reporting completion.
```

#### A-4. Claude との差分吸収

現在の `CLAUDE.md` は包括的すぎるため、そのまま `AGENTS.md` に写すと Codex 公式ベストプラクティスに反する。  
よって、`AGENTS.md` は「入口」に限定し、詳細は外部ファイル参照へ分離する。

### B. `.codex/config.toml` 設計

#### B-1. 基本方針

Codex 公式の推奨に従い、repo 固有設定を `.codex/config.toml` に置く。  
既定モデルや承認モードを明示し、surface 差異を減らす。

#### B-2. 推奨初期値

```toml
model = "gpt-5.4"
model_reasoning_effort = "medium"
model_verbosity = "low"
approval_policy = "on-request"
sandbox_mode = "workspace-write"
allow_login_shell = false
project_doc_max_bytes = 65536
project_doc_fallback_filenames = ["CLAUDE.md"]

[sandbox_workspace_write]
network_access = false
```

#### B-3. この設定の意図

- `model = "gpt-5.4"`
  - 既定モデル差異を排除するため pin する
  - 2026-04-02 時点の Codex docs では `gpt-5.4` が flagship 推奨
- `model_reasoning_effort = "medium"`
  - 日常開発の標準
  - 重い調査・レビュー用は profile に逃がす
- `model_verbosity = "low"`
  - 冗長出力を抑え、Codex の出力スタイルを Claude 的に寄せる
- `approval_policy = "on-request"`
  - 既定で tight
  - 必要時のみ昇格
- `sandbox_mode = "workspace-write"`
  - 通常開発を妨げない最小限
- `allow_login_shell = false`
  - shell 初期化による予期しない差分を減らす
- `project_doc_fallback_filenames = ["CLAUDE.md"]`
  - 移行期間中、`AGENTS.md` が未整備な領域でも `CLAUDE.md` を補助 instruction source として扱えるようにする

#### B-4. profile 設計

CLI 利用時のために profile を追加する。

```toml
[profiles.deep-review]
model = "gpt-5.4"
model_reasoning_effort = "high"
approval_policy = "on-request"

[profiles.lightweight]
model = "gpt-5.4-mini"
model_reasoning_effort = "low"
approval_policy = "on-request"
```

用途:

- `deep-review`
  - 調査、設計、レビュー、大規模リファクタ
- `lightweight`
  - 小修正、探索、補助作業、サブエージェント向け

#### B-5. MCP 設計

Codex 公式は「最初から全部つながない」ことを推奨している。  
この repo では最小構成として以下のみを推奨する。

- `rust-analyzer` 相当の Rust コード理解 MCP
- 必要が明確な場合だけ追加 MCP

MCP 導入の原則:

- repo 外の生きた情報が必要
- 頻繁に変わる情報が必要
- コピー&ペーストを減らしたい
- チーム全体で再利用したい

### C. `.codex/rules/default.rules` 設計

#### C-1. 目的

Claude settings の deny/allow を Codex の execpolicy に移植し、同じ安全性と同じ作業感を作る。

#### C-2. 禁止するコマンド

Claude で禁止されている Git 系 destructive 操作を Codex でも forbidden にする。

対象:

- `git add`
- `git commit`
- `git push`
- `git reset`
- `git checkout`
- `git switch`
- `git stash`
- `git merge`
- `git rebase`

理由:

- この repo では Git の最終操作はユーザー責任
- Codex にも同じ境界を維持させる

#### C-3. 自動許可する安全コマンド

以下は sandbox 外昇格が必要なケースでも、将来の Smart approval 候補と整合する prefix_rule を用意する。

ただし「自動許可」は必要最小限にとどめる。

候補:

- `cargo check`
- `cargo test`
- `cargo clippy`
- `cargo fmt`
- `cargo build`
- `cargo llvm-cov`
- `rg`
- `find`
- `sed`
- `ls`
- `git status`
- `git diff`
- `git log`
- `git show`

設計意図:

- 読み取り/検証中心の反復動作は friction を減らす
- 破壊的 Git とネットワーク系は別扱いにする

#### C-4. prompt に留めるコマンド

次は `prompt` にする。

- `npm install`
- `cargo install`
- `docker *`
- `curl *`
- `gh *`
- `aws *`
- repo 外書き込みを伴うコマンド
- network access を要求するコマンド

#### C-5. ルール設計の原則

- broad allow は作らない
- destructive 系は `forbidden`
- ネットワーク/外部サービスは `prompt`
- 読み取り/検証中心のみ `allow`

### D. Skill 設計

#### D-1. 移植方針

`.claude/skills/` を 1:1 で雑に移すのではなく、Codex 公式の「1 skill = 1 job」に合わせて再設計する。

#### D-2. 優先移植 skill

優先度 A:

- `tdd`
- `quality-check`
- `investigation`
- `refactoring-check`
- `correctness-audit`

優先度 B:

- `backlog-management`
- `todo-audit`
- `todo-grooming`
- `todo-replenishment`
- `backlog-replenishment`
- `prd-template`

優先度 C:

- `rule-writing`
- `rule-maintenance`
- `large-scale-refactor`
- `hono-cycle`

#### D-3. Codex 向け skill 記述ルール

各 `SKILL.md` は次を必須にする。

- `name`
- `description`
- 明確な trigger 条件
- しないこと
- 入力
- 実行手順
- 完了条件

例:

```md
---
name: quality-check
description: Run the Rust validation gate for completed work in this repository. Use when implementation is done and before reporting completion.
---

1. Run cargo fix --allow-dirty --allow-staged
2. Run cargo fmt --all --check
3. Run cargo clippy --all-targets --all-features -- -D warnings
4. Run cargo test
5. Run ./scripts/check-file-lines.sh
6. If the change affects coverage-sensitive areas, run cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89
7. Read outputs and report failures precisely
```

#### D-4. Claude command との対応表

| Claude | Codex |
|---|---|
| `/tdd` | `tdd` skill |
| `/quality-check` | `quality-check` skill |
| `/investigation` | `investigation` skill |
| `/refactoring-check` | `refactoring-check` skill |
| `/start` | `AGENTS.md` + `workflow.md` の起動手順 |
| `/semantic_review` | `code-review.md` 参照 + `/review` |
| `/end` | `quality-check` + `todo-audit` skill |

#### D-5. implicit invocation を成功させる設計

Codex 公式に従い、description は曖昧にしない。

悪い例:

- "General development helper"

良い例:

- "Use when implementing a new feature or bug fix in this repository. Perform test design first, confirm RED, then implement GREEN, refactor, and add E2E coverage if the change affects TS-to-Rust conversion behavior."

### E. 共有文書設計

#### E-1. `doc/agent/workflow.md`

内容:

- 着手時に `plan.md` と `TODO` を読む
- 新機能/バグ修正は TDD
- 実装中は関係文書を同期
- 完了前に quality check
- コミット提案前に `plan.md` / 関連 doc を更新

#### E-2. `doc/agent/quality-gates.md`

内容:

- 実行コマンド
- 実行順
- 大きい出力の扱い
- 0 errors / 0 warnings 基準
- E2E 必須条件

#### E-3. `doc/agent/task-management.md`

内容:

- `TODO` は PRD 化前イシュー一覧
- `plan.md` は現在の主作業
- `backlog/` は PRD 詳細
- out-of-scope は TODO へ記録
- commit 前同期の対象

#### E-4. `doc/agent/code-review.md`

内容:

- レビューは finding-first
- 優先順位は correctness, regression, test gaps
- summary は二次

これは Codex の `/review` と相性が良い。

### F. モデル・surface 差分吸収設計

#### F-1. モデルの pin

Codex docs と ChatGPT help の間で既定モデル記述に差異があるため、repo 設計としては model pin を採る。

採用:

- default: `gpt-5.4`
- lightweight: `gpt-5.4-mini`

#### F-2. surface 非依存の操作感

CLI / IDE / App をまたいで体験をそろえるため、以下は必ず repo 側に寄せる。

- `AGENTS.md`
- `.codex/config.toml`
- `.agents/skills`
- `.codex/rules`

逆に個人差に委ねるもの:

- `~/.codex/AGENTS.md`
- `~/.codex/config.toml` の個人好み
- 通知設定

### G. Claude との完全互換性を守るための設計

#### G-1. 共通内容の一次情報化

次を一次情報として扱う。

- `doc/agent/*.md`

二次ハブ:

- `AGENTS.md`
- `CLAUDE.md`

この構成により、ルール変更時は共通文書を直し、両 surface から参照するだけで済む。

#### G-2. 移行期間の互換策

移行期間中は以下を許容する。

- `project_doc_fallback_filenames = ["CLAUDE.md"]`
- `AGENTS.md` から `CLAUDE.md` の重要部を参照

ただし最終形では `CLAUDE.md` 依存を弱める。

## 実装計画

### Phase 1. ハブ整備

1. `AGENTS.md` を追加
2. `doc/agent/` を新設
3. `CLAUDE.md` を共通 docs 参照型に整理

完了条件:

- Codex が root で `AGENTS.md` を読む
- Claude と Codex の両方が同じ共通 docs を参照する

### Phase 2. 設定整備

1. `.codex/config.toml` を追加
2. `.codex/rules/default.rules` を追加
3. 必要な fallback filename と sandbox/approval を設定

完了条件:

- Codex の default 挙動が repo 固有に安定する
- Git 禁止ルールが有効

### Phase 3. Skill 移植

1. 優先度 A skill を `.agents/skills` に移植
2. 既存 `.claude/skills` との差分を埋める
3. description を implicit invocation 向けに最適化

完了条件:

- `tdd` と `quality-check` が Codex で使える
- 実装/完了フローが Claude とほぼ同等になる

### Phase 4. 運用検証

以下のシナリオで検証する。

1. ルートで `Codex` を起動し、active instruction sources を確認
2. バグ修正タスクで `tdd` skill が使える
3. 実装完了時に `quality-check` が走る
4. `git commit` が禁止される
5. `cargo test`, `cargo clippy`, `cargo fmt` が friction 少なく実行できる

## 受け入れ基準

以下を満たしたら「Claude Code と Codex の開発体験が実用上揃った」と判定する。

- Codex が起動直後に repo の前提知識を自動取得できる
- 主要な反復手順が skill として再利用できる
- 完了報告前の品質ゲートが同じになる
- Git の危険操作境界が同じになる
- `TODO` / `plan.md` / `backlog/` を使う運用が維持される
- Rust 開発に必要なチェックが Codex でも同じように流せる
- surface が CLI / IDE / App に変わっても挙動差が小さい

## 設計上の判断

### 判断 1. `AGENTS.md` を新設し、Codex の入口にする

理由:

- Codex 公式の自動読込対象
- 毎回プロンプトでルールを繰り返す必要がなくなる
- Claude と違い、Codex ではここを中心に組むのが自然

### 判断 2. `CLAUDE.md` は消さず、共通 docs 参照型に薄くする

理由:

- 既存運用を壊さない
- Claude と Codex の分岐を最小化できる
- ただし一次情報の重複は避ける

### 判断 3. `.claude/skills` は `.agents/skills` へ段階移植する

理由:

- Codex 公式も skills を反復ワークフローの本命としている
- slash command の完全再現より、再利用単位の保存が重要

### 判断 4. 権限は tight default + rules allowlist にする

理由:

- Codex 公式と整合
- Claude 側の安全運用とも整合
- フルアクセス前提の設計はこの repo に不要

### 判断 5. model は pin する

理由:

- 既定モデルの docs 記述差異を吸収できる
- 再現性が高まる

## 次に実装すべきもの

優先順位は以下。

1. `AGENTS.md`
2. `doc/agent/` の共通文書群
3. `.codex/config.toml`
4. `.codex/rules/default.rules`
5. `.agents/skills/tdd`
6. `.agents/skills/quality-check`

この順に進めると、最小コストで Codex の開発体験を Claude に近づけられる。

## 参考ソース

調査に使った主要ソース:

- OpenAI Developers, `Best practices – Codex`
  - https://developers.openai.com/codex/learn/best-practices
- OpenAI Developers, `Custom instructions with AGENTS.md – Codex`
  - https://developers.openai.com/codex/guides/agents-md
- OpenAI Developers, `Config basics – Codex`
  - https://developers.openai.com/codex/config-basic
- OpenAI Developers, `Advanced Configuration – Codex`
  - https://developers.openai.com/codex/config-advanced
- OpenAI Developers, `Configuration Reference – Codex`
  - https://developers.openai.com/codex/config-reference
- OpenAI Developers, `Rules – Codex`
  - https://developers.openai.com/codex/rules
- OpenAI Developers, `Agent Skills – Codex`
  - https://developers.openai.com/codex/skills
- OpenAI Developers, `Agent approvals & security – Codex`
  - https://developers.openai.com/codex/agent-approvals-security
- OpenAI Developers, `Codex Models`
  - https://developers.openai.com/codex/models
- OpenAI Help Center, `Using Codex with your ChatGPT plan`
  - https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan

補足:

- モデル既定値や利用可能モデルは時間変動があるため、実装時も再確認する
- 本設計における `.codex/rules/` 配置は OpenAI docs の rules/ team-config 記述に基づく
- `project_doc_fallback_filenames = ["CLAUDE.md"]` は本 repo の移行互換のための設計判断であり、Codex 公式必須要件ではない
