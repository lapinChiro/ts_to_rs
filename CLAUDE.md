# CLAUDE.md

TypeScript → Rust 変換 Codemod CLI ツール。

## Tech Stack

- **言語**: Rust
- **TS解析**: swc_ecma_parser + swc_ecma_ast
- **CLI**: clap
- **テスト**: cargo test + insta (スナップショット)
- **Lint**: clippy
- **フォーマット**: rustfmt

## Key Commands

```bash
cargo build                # デバッグビルド
cargo build --release      # リリースビルド
cargo check                # 高速な型チェック
cargo test                 # 全テスト実行
cargo clippy --all-targets --all-features -- -D warnings  # lint
cargo fmt --all --check    # フォーマットチェック
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89  # カバレッジ計測（閾値89%、main.rs除外）
cargo llvm-cov --html                  # HTMLレポート生成（target/llvm-cov/html/）
./scripts/hono-bench.sh              # Hono 変換率ベンチマーク（ディレクトリモード）
./scripts/hono-bench.sh --both       # ディレクトリ + 単一ファイル両方
```

### Hono ベンチマーク

Hono フレームワークの変換成功率を計測する。変換機能の変更後に実行して効果を定量評価する。

- **実行**: `./scripts/hono-bench.sh`（内部で `cargo build --release` を確認し、Hono リポジトリを自動クローン）
- **解析**: `scripts/analyze-bench.py` がベンチ実行の最後に自動呼出しされ、エラー JSON をカテゴリ別に集計
- **履歴**: 実行ごとに `bench-history.jsonl` に 1 行追記される（JSONL 形式）。過去の結果と比較可能
- **エラー JSON**: `/tmp/hono-bench-errors.json` に生データが出力される

```bash
# 履歴の推移を確認（timestamp 順でソート）
cat bench-history.jsonl | python3 -c "
import sys, json
entries = sorted([json.loads(l) for l in sys.stdin if l.strip()], key=lambda e: e['timestamp'])
for r in entries:
    print(f\"{r['timestamp'][:10]}  {r['git_sha']}  clean={r['clean_files']}/{r['total_files']} ({r['clean_pct']}%)  errors={r['error_instances']}\")
"
```

**注意**: 「クリーン」は変換エラーなし（`--report-unsupported` でエラー 0）を意味し、生成 Rust のコンパイル可能性とは別指標。

## Architecture

ディレクトリ構成は [README.md](README.md#ディレクトリ構成) を参照。

変換パイプライン: TS source → `parser` (SWC AST) → `transformer` (IR) → `generator` (Rust source)

## Core Principles

以下の原則を常に遵守すること:

- **理想的な実装**: 開発コストを度外視し、論理的に最も理想的な実装を目指す。妥協やアドホックな対応を避け、型システム・アーキテクチャとして一貫した解法を選ぶ。「工数が大きいから」「今はこれで十分だから」は設計判断の理由にならない
- **KISS**: 過剰設計を避けよ。最小限の複雑さで現在の要件を満たすこと。ただし「理想的な実装」と矛盾する場合は理想を優先する
- **YAGNI**: 要求されていない機能・改善・拡張を作るな。今必要なものだけを実装せよ
- **DRY + 直交性**: DRYは「知識の重複」を排除する原則であり、「コードの見た目の重複」を排除する原則ではない。共通化によってモジュール間の結合が増えるなら、重複を残せ

## Code Conventions

- `unwrap()` / `expect()` の使用制限 — 詳細は `.claude/rules/testing.md` 参照
- `unsafe` ブロック禁止（必要な場合はコメントで理由を明記し、ユーザーの承認を得ること）
- `clone()` は初版では許容するが、不要な clone はコメントで TODO を残す
- pub な型・関数にはドキュメントコメント (`///`) を付ける

## Quality Standards

全ての変更に対し **0エラー・0警告** を維持すること。作業完了時は /quality-check を実行する。

カバレッジ閾値のラチェット運用: 実測値が閾値を 2pt 以上上回ったら、閾値を 1pt 引き上げる。

## 行動規範

- **変換可能性の独断禁止** — 詳細は `.claude/rules/conversion-feasibility.md` 参照
- **PRD 完了条件の厳守** — 詳細は `.claude/rules/prd-completion.md` 参照
- **段階的コミット**: 複数フェーズの作業では各フェーズ完了時にコミットする — 詳細は `.claude/rules/incremental-commit.md` 参照
- **コミット前ドキュメント同期**: コミットメッセージ作成前に tasks.md / plan.md を最新化する — 詳細は `.claude/rules/pre-commit-doc-sync.md` 参照
- **一括編集の安全手順**: スクリプトによる一括置換は dry run → 確認 → 実行 — 詳細は `.claude/rules/bulk-edit-safety.md` 参照
- **Git 操作の制限**: `git commit` / `push` / `merge` はユーザーのみが行う。Claude はコミットメッセージの提案のみ
- **判断軸のある質問**: 選択肢・メリデメ・推奨案を提示して確認する。「これでよいですか？」のような判断軸のない質問をしない。自分で判断できることは判断して進める
- **検証の原則**: 検証項目・期待結果を事前に定義してから実行する。後付け判定の禁止
- **デバッグ**: 修正が 1 回で成功しなかったら、次の修正前に根本原因の仮説を立てる。同じ修正を 2 回繰り返さない
- **後回しの記録**: スコープ外の課題は `TODO` に記録する（記載基準は `.claude/rules/todo-entry-standards.md` 参照）
- **ドキュメント同期**: コード変更時に plan.md, README.md, CLAUDE.md, doc コメントが不正確になっていないか確認・更新する
- **引継ぎ時の記載**: 後続への引継ぎが発生する場合には想定と異なった何かが発生している可能性が高い。判断を伝達する場合には、"なぜ"そのような判断をしたのか、の"なぜ"を明確に記載しなければいけない
- **rust-analyzer**: 作業開始時に `rust_analyzer_set_workspace` を実行。構成変更後は再読み込み + diagnostics 確認。diagnostics のエラーを無視しない

## ワークフロー

以下の状況では対応する Skill を必ず呼び出すこと:

- 新機能・バグ修正の着手 → /tdd
- 作業完了時（コミット前） → /quality-check
- 機能追加完了後 → /refactoring-check
- **PRD（backlog/ のタスク）完了後** → /backlog-management（TODO 更新 → backlog 削除 → plan.md 整理 → 次の PRD 着手の順序を厳守）
- 開発セッションの最後（コミット前） → /todo-audit
- backlog/ の操作 → /backlog-management
- backlog/ が空で作業依頼を受けた → /backlog-replenishment
- PRD の作成 → /prd-template
- TODO が空で作業依頼を受けた → /todo-replenishment
- 調査タスク → /investigation
- TODO の棚卸し（定期的、または大きな機能追加の完了後） → /todo-grooming
- 変換正当性の監査（定期的、または大規模変更後） → /correctness-audit
- Hono 変換改善の開発ループ → /hono-cycle（単発）または `/loop 0 /hono-cycle`（連続）
- ルールの作成・変更 → /rule-writing, /rule-maintenance
- 大規模リファクタリング（10箇所以上のシグネチャ変更、5ファイル以上にまたがる機械的変更） → /large-scale-refactor

## 自発的改善の原則

問題や不整合を発見したら、ユーザーに指摘される前に自発的に調査・修正すること:

- 警告・エラー・不整合を「一時的な問題」として安易に無視しない
- 問題の根本原因を特定してから対処する
- 「動いているから良い」ではなく「正しい状態か」を基準にする

## スキルの自己改善

スキルは静的なプロンプトではなく、環境の変化に応じて進化すべきコンポーネントである。

### 振り返り（Observe）

スキル実行中に以下のいずれかに気づいたら、実行完了後に `TODO` へ `[skill-feedback:<スキル名>]` タグ付きで記録する:

- スキルの指示が曖昧で、判断に迷った箇所があった
- スキルのステップが現在のコードベースや環境に合わなくなっていた
- ユーザーがスキルの途中で方向修正を求めた（＝指示が不十分だったサイン）
- スキルに書かれていない判断を自分で補った

記録は「何が起きたか」「なぜ問題か」「改善案」の3点を含める。

### 能動的改善（Amend）

スキル実行中に改善点に気づいた場合、実行完了後にユーザーに改善提案を行う:

1. 問題の具体的な内容と、実行中にどう影響したかを説明する
2. スキルの修正案（差分）を提示する
3. ユーザーが承認したら `/rule-writing` + `/rule-maintenance` の手順で反映する

### 受動的学習

ユーザーからClaude自身の挙動に関する修正指示を受けたとき:

1. 指示を一般化・抽象化する（特定のケースではなくパターンとして）
2. 保存先を判断する:
   - プロジェクト固有のルール → `.claude/rules/` に追記または新規作成
   - 個人的な好み → `~/.claude/CLAUDE.md` に追記
3. 書き込む内容と保存先をユーザーに提示し、確認を得てから書き込む
