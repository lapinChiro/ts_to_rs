---
name: investigation
description: Use when the user asks for investigation, research, or analysis. Read the relevant code and documents thoroughly, and save the result as a report under report/.
---

# Investigation

## Procedure

1. 関連コードを十分に読む
2. `README.md`, `CLAUDE.md`, `TODO`, `plan.md`, 関連 PRD を確認する
3. 必要なら外部情報を調べる
4. 結果を `report/<topic>.md` に保存する
5. レポートには summary, detailed findings, references を含める

## Report Rules

- 可能なら base commit を記録する
- 調査時点で uncommitted change があるなら明記する
- 推測だけで埋めず、コード位置や資料で裏付ける

## Do Not

- 部分読みにもかかわらず全体確認済みと書かない
- 口頭要約だけで report を作らない
