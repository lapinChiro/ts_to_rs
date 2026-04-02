# Codex 実地確認手順

更新日: 2026-04-02

## このファイルの目的

次の Codex セッションで、今回追加した Codex 用環境が実際に期待通り機能するかを検証するための実行手順をまとめる。  
このファイルを読んだ Codex は、ここに書かれた順序で確認を進めること。

## 今回追加済みの対象

Codex 用に以下を追加済み。

- `AGENTS.md`
- `.codex/config.toml`
- `.codex/rules/default.rules`
- `.agents/skills/`
  - `tdd`
  - `quality-check`
  - `investigation`
  - `refactoring-check`
  - `backlog-management`
  - `todo-audit`
- `doc/agent/`
  - `project-overview.md`
  - `workflow.md`
  - `quality-gates.md`
  - `task-management.md`
  - `code-review.md`
  - `rust-tooling.md`
- `CLAUDE.md` に shared docs 案内を追加
- `design.for_codex.md` に併用前提を追記

## 重要前提

- 既存の `Claude Code` 環境は残す設計である
- 今回の検証は「Codex 追加環境が機能するか」の確認であり、「Claude 側を壊していないか」の確認も含む
- Git の commit/push/reset/checkout/switch/stash/merge/rebase は実行しない
- 既存の未コミット変更を巻き戻さない

## セッション開始後に最初にやること

以下の順で読む。

1. `AGENTS.md`
2. `next.codex-verification.md`
3. `plan.md`
4. `TODO`
5. `doc/agent/workflow.md`
6. `doc/agent/quality-gates.md`
7. `doc/agent/task-management.md`

必要に応じて以下も読む。

- `doc/agent/project-overview.md`
- `.codex/config.toml`
- `.codex/rules/default.rules`
- `.agents/skills/*/SKILL.md`
- `CLAUDE.md`
- `design.for_codex.md`

## 検証のゴール

次の 6 点を確認する。

1. Codex が repo 入口として `AGENTS.md` を読めること
2. `.codex/config.toml` の profile / sandbox / approval 方針が有効であること
3. `.codex/rules/default.rules` の安全境界が期待通りであること
4. `.agents/skills/` の主要 skill が認識・利用可能であること
5. Codex 用追加により Claude 用資産との役割分担が破綻していないこと
6. 必要なら不足点を修正し、最終的に実用可能な状態にすること

## 実行方針

- 調査だけで止まらず、問題が明確ならその場で修正する
- ただし destructive な操作はしない
- 失敗時は「何が未対応か」を具体的に切り分ける
- 途中の確認結果は簡潔にユーザーへ共有する

## フェーズ 1: 基本認識確認

### 1-1. ファイル存在確認

次を確認する。

- `AGENTS.md`
- `.codex/config.toml`
- `.codex/rules/default.rules`
- `.agents/skills/*`
- `doc/agent/*`

期待結果:

- すべて存在する
- パス構成が設計書と一致する

### 1-2. Codex CLI の存在確認

以下を確認する。

- `which codex`
- `codex --help`

期待結果:

- `codex` が利用可能
- help が表示される

### 1-3. リポジトリ構成と dirty 状態確認

以下を確認する。

- `git status --short`
- 必要なら `git diff --stat`

目的:

- 既存未コミット変更と今回の追加ファイルを把握する
- 以後の修正で余計な差分を巻き込まない

## フェーズ 2: Codex 設定読込確認

### 2-1. `.codex/config.toml` の妥当性確認

確認項目:

- TOML として妥当か
- 既定 profile が `default` か
- `model = "gpt-5.4"` が有効か
- `approval_policy = "on-request"` が repo 既定になっているか
- `sandbox_mode = "workspace-write"` が repo 既定になっているか
- `project_doc_fallback_filenames = ["CLAUDE.md"]` が認識されるか

実施方法:

- `codex --help` の範囲で読める情報を確認
- 可能なら `codex` の debug / config 表示系コマンドを探す
- 表示系コマンドがなければ、実際に repo 内で Codex を起動したときの挙動から推定する

期待結果:

- config エラーなく起動できる
- profile 指定が通る

失敗時:

- 不明な config key が原因なら key 名を修正する
- docs と実 CLI 挙動に差がある場合は、CLI が受理する構文へ合わせる

### 2-2. profile 確認

以下を確認する。

- `default`
- `deep-review`
- `lightweight`

確認ポイント:

- profile 名が解決されるか
- profile 指定時にエラーにならないか

期待結果:

- `-p deep-review`
- `-p lightweight`

などの指定で CLI エラーにならない

失敗時:

- profile 定義位置または key 名を修正する

## フェーズ 3: `AGENTS.md` 読込確認

### 3-1. 自動読込の想定どおりか確認

目的:

- Codex が root の `AGENTS.md` を作業前 instruction source として見ているか確認する

方法:

- repo root で Codex を起動した場合に、応答スタイルや初期方針が `AGENTS.md` と整合するかを見る
- 可能なら active instruction sources を出せる方法を確認する
- もし CLI に明示機能がなければ、次の観測点で代替確認する

観測点:

- 日本語で応答するか
- `plan.md` / `TODO` / `doc/agent/workflow.md` を先に読む行動を取るか
- Git 危険操作を避けるか
- skill 活用前提の指示に従うか

期待結果:

- `AGENTS.md` と整合する行動が観測できる

失敗時:

- `AGENTS.md` の記述を短く強くする
- 読み順や禁止事項が曖昧なら修正する

### 3-2. `CLAUDE.md` fallback の影響確認

目的:

- Codex 側で `CLAUDE.md` を fallback source としたことで、不要に Claude 固有挙動へ引っ張られていないか確認する

観点:

- `AGENTS.md` の方が入口として優先されているか
- `CLAUDE.md` の内容は補助的に参照されるだけか

期待結果:

- Codex の第一入口は `AGENTS.md`
- `CLAUDE.md` は補助参照に留まる

失敗時:

- `project_doc_fallback_filenames` を外すか、`AGENTS.md` の強度を上げる

## フェーズ 4: rules 確認

### 4-1. forbidden Git 操作の確認

確認対象:

- `git add`
- `git commit`
- `git push`
- `git reset`
- `git checkout`
- `git switch`
- `git stash`
- `git merge`
- `git rebase`

目的:

- `.codex/rules/default.rules` の forbidden が期待通り効くか確認する

安全な確認方法:

- 実際に destructive コマンドを流さず、Codex がそれらを提案した場合の扱いを確認する
- 可能ならルール構文の妥当性チェックや説明表示で検証する

期待結果:

- これらは実行対象として扱われない、または拒否される

失敗時:

- `.rules` の構文または pattern 定義を修正する

### 4-2. allow / prompt の確認

allow 対象:

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
- `cat`
- `wc`
- `git status`
- `git diff`
- `git log`
- `git show`

prompt 対象:

- `cargo fix`
- `npm install`
- `curl`
- `gh`
- `aws`
- `docker`

確認観点:

- allow 対象が不必要に確認を求めないか
- prompt 対象で確認が必要になるか

失敗時:

- `allow` と `prompt` の境界を調整する
- `cargo fix` を allow に上げるかどうかは repo 運用に照らして再判断する

## フェーズ 5: skill 認識確認

### 5-1. skill ファイル構成確認

確認対象:

- `.agents/skills/tdd/SKILL.md`
- `.agents/skills/quality-check/SKILL.md`
- `.agents/skills/investigation/SKILL.md`
- `.agents/skills/refactoring-check/SKILL.md`
- `.agents/skills/backlog-management/SKILL.md`
- `.agents/skills/todo-audit/SKILL.md`

見る点:

- frontmatter の `name` と `description`
- body が 1 skill = 1 job になっているか
- description が trigger を明確に表せているか

### 5-2. 実際の trigger 妥当性確認

以下の仮入力に対して、適切な skill が呼ばれそうかを評価する。

- 「このバグを直して」
  - `tdd` が有力
- 「修正が終わったので最終確認して」
  - `quality-check` が有力
- 「この挙動を調査して report にまとめて」
  - `investigation` が有力
- 「この変更範囲にリファクタ候補がないか見て」
  - `refactoring-check` が有力
- 「TODO と backlog と plan の整合を取って」
  - `backlog-management` が有力
- 「開発セッション終了前に TODO の抜け漏れを見て」
  - `todo-audit` が有力

期待結果:

- description だけ見ても用途が明確

失敗時:

- description をより具体的に書き換える

## フェーズ 6: 軽い実地シナリオ確認

次の小シナリオを 1 つ以上行う。

### シナリオ A: 調査タスク

例:

- 「Codex 環境がどう構成されているか確認して」

観点:

- `AGENTS.md` の reading order に従うか
- `investigation` か通常探索で `doc/agent/` を活用するか
- 日本語で返すか

### シナリオ B: 完了前チェック相当

例:

- 「品質確認をして」

観点:

- `quality-check` に沿ったコマンド順序になるか
- `cargo fix` など変更を伴うコマンドの扱いが適切か
- 出力検証姿勢が `quality-gates.md` と整合するか

### シナリオ C: 実装開始相当

例:

- 「このバグを直して」

観点:

- `tdd` 的フローに入るか
- `plan.md` と `TODO` を先に確認するか
- 変換挙動変更で E2E を意識するか

## フェーズ 7: Claude 併用互換性確認

確認すること:

- `.claude/` 配下は untouched か
- `CLAUDE.md` の既存内容を壊していないか
- Codex 用追加が Claude 用運用を上書きしていないか

期待結果:

- Claude は従来どおり使える
- Codex は追加の入口を持つ
- shared guidance は `doc/agent/` で共通化されている

## フェーズ 8: 問題が見つかった場合の修正方針

### config 問題

対象:

- `.codex/config.toml`

対応:

- key 名
- profile 構造
- unsupported 項目

### rules 問題

対象:

- `.codex/rules/default.rules`

対応:

- pattern の絞り込み
- forbidden / prompt / allow の再配分
- docs と実 CLI の差異吸収

### skill 問題

対象:

- `.agents/skills/*/SKILL.md`

対応:

- description を trigger ベースで具体化
- body を短く修正
- 1 skill = 1 job に戻す

### docs 問題

対象:

- `AGENTS.md`
- `doc/agent/*.md`
- `CLAUDE.md`

対応:

- 入口に書くべきものと詳細 docs の分離を見直す
- 重複を減らす
- Codex が従いやすい表現へ調整する

## 検証後に必ず更新するもの

実地確認の結果、問題を直した場合は次を更新する。

- `design.for_codex.md`
  - 実装との差分が出たら反映する
- `AGENTS.md`
  - 読まれ方に合わせて改善する
- `.codex/config.toml`
  - 実 CLI に合わせて修正する
- `.codex/rules/default.rules`
  - 実挙動に合わせて修正する
- `.agents/skills/*`
  - trigger や記述粒度を調整する
- 必要なら `CLAUDE.md`
  - shared docs 参照がずれたら追従する

## 最終報告で必ず含めること

次の Codex セッションの最終報告には、少なくとも以下を含めること。

1. 何を確認したか
2. 何が期待通り動いたか
3. 何が動かなかったか
4. その場で何を修正したか
5. まだ残っている不確実性
6. 次にやるべきこと

## 望ましい完了状態

以下を満たせば、今回の Codex 環境整備はひとまず成功とみなしてよい。

- Codex が root で `AGENTS.md` 前提の行動を取る
- `.codex/config.toml` が実際に受理される
- `.codex/rules/default.rules` が安全境界として機能する
- `.agents/skills/` の主要 skill が意味のある単位で使える
- Claude 環境はそのまま残っている
- 必要な実地修正が完了している
