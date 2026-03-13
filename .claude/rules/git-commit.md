# Git コミット操作の制限

## 適用条件

Git の `commit`、`push`、`merge` 等の変更確定操作すべて。

## 制約

- `git commit` はユーザーのみが行う。Claude はコミットメッセージの提案のみ行う
- `git push`、`git merge` 等のリモート操作も同様にユーザーのみが行う

## 禁止事項

- `git commit` を実行すること
- `git push` を実行すること
- `git merge` を実行すること
- ユーザーの明示的な指示なく変更を確定する操作を行うこと
