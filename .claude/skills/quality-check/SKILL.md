---
name: quality-check
description: 作業完了時（コミット前）の品質チェック手順。cargo fmt, clippy, test を全て実行し 0 エラー・0 警告を確認する
user-invocable: true
---

# 作業完了時の品質チェック

## トリガー

ユーザーから依頼された作業が完了したとき（コミット前）。

## アクション

以下を全て実行し **0 エラー・0 警告** であることを確認する:

```bash
cargo fmt --all --check > /tmp/fmt-result.txt 2>&1
cargo clippy --all-targets --all-features -- -D warnings > /tmp/clippy-result.txt 2>&1
cargo test > /tmp/test-result.txt 2>&1
```

コマンド出力の確認方法は `.claude/rules/command-output-verification.md` に従う。

エラーがあった場合:

1. 今回の変更に起因しないエラーも含め、全て修正する
2. 修正できないエラーは原因と影響を明記し、ユーザーに報告する

## 禁止事項

- テストを削除・弱体化してエラーを消すこと
- clippy 警告を `#[allow(...)]` で黙らせること（根本原因を修正する）
- 品質チェックを実行せずに「完了」と報告すること
- 出力を流し読みして「問題なさそう」と判断すること。各コマンドの終了メッセージまで確認する
- 「今回の変更に起因しない」「スコープ外」を理由に、発見した警告・エラーの修正を先送りすること。発見した時点で修正可能なら修正し、誤検知であれば調査の上その旨を報告する

## 検証

- 3 コマンド全てが終了コード 0 で完了している
- 出力ファイルの全文を Read ツールで確認した履歴がある
