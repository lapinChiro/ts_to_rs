---
name: todo-audit
description: Use at the end of a development session to audit TODO coverage and freshness. Detect missing TODO items, stale descriptions, and references invalidated by the latest work.
---

# TODO Audit

## Procedure

1. 変更箇所と影響範囲で `TODO`, `FIXME`, `HACK`, `WORKAROUND`, `todo!()` を検索する
2. 新しい暫定実装や fallback が `TODO` に記録されているか確認する
3. 既存 `TODO` の前提、件数、 hold reason、参照 ID が古くなっていないか確認する
4. 必要な更新を `TODO` に反映する

## Do Not

- 検索せずに「追加不要」と判断しない
- コード内 TODO を `TODO` ファイルへ転記せず放置しない
- 存在しない issue ID 参照を残さない
