---
name: analyze-ga-log
description: Use when the user provides a GitHub Actions log URL for analysis. Fetches the log via curl, saves to a file, and analyzes build/test failures, coverage results, and errors
user-invocable: true
---

# GitHub Actions ログ解析

## Trigger

ユーザーが GitHub Actions のログ URL を提示し、解析を依頼したとき。

## Actions

1. **ログ取得**: `curl` でログをダウンロードし `/tmp/gh-actions-log.txt` に保存する
   ```bash
   curl -sS -o /tmp/gh-actions-log.txt '<URL>'
   ```
   - SAS トークンの有効期限は約10分。403 エラーの場合は再取得を依頼する

2. **ファイル検証**: `wc -l` で行数を確認し、XML エラーレスポンスでないことを確認する

3. **エラー抽出**: Grep で以下のパターンを検索し、全体像を把握する
   ```
   ##[error]|FAILED|panicked|error\[|warning\[|Process completed with exit code|test result:
   ```

4. **詳細解析**: エラー周辺のコンテキストを Read で確認し、根本原因を特定する
   - テスト結果（passed/failed/ignored）
   - カバレッジ結果（閾値 vs 実測値）
   - コンパイルエラー
   - clippy / fmt の警告

5. **レポート**: 以下の形式で報告する
   - **失敗原因**: 1行サマリ
   - **詳細**: エラーメッセージ、関連ファイル、数値
   - **対処方針**: 具体的な修正方向（該当する場合）

## Prohibited

- WebFetch での取得（Azure Blob Storage の SAS URL は 403 になる）
- ログ全体を一度に Read しようとする（サイズ制限に引っかかる）
- エラー行だけ見て周辺コンテキストを確認しない

## Verification

- log file が `/tmp/<workflow-name>-<run-id>.log` に保存されている
- 失敗 step / job ごとに ID + サマリ + 詳細 + 対処方針 4 項目が記録されている
- 周辺 context (失敗行の前後 ~20 行) を Read で確認した evidence あり

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [command-output-verification.md](../../rules/command-output-verification.md) | log 出力 review 手順 (full content read、tail 禁止) |
| Skill | [investigation](../investigation/SKILL.md) | 同種の情報収集 → report/ 保存 procedure (analyze-ga-log は CI log specialised version) |
| Skill | [todo-audit](../todo-audit/SKILL.md) | log 解析で発見した defect を TODO 化する場合の format |
