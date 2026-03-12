# backlog の補充

## トリガー

`backlog/` が空の状態でユーザーから作業依頼を受けたとき。

## アクション

1. `TODO` を確認する
2. PRD 化が可能な項目（保留理由が解消済み、または保留理由がない項目）を特定する
3. PRD テンプレート（`.claude/rules/prd-template.md`）に従い、Discovery → PRD 起草の順で進める
4. 作成した PRD を `backlog/` に配置し、`TODO` から該当項目を削除する
5. `plan.md` の消化順序に新アイテムを挿入する

## 禁止事項

- 保留理由が未解消の項目を PRD 化すること
- Discovery（明確化質問）をスキップして PRD を書くこと
- PRD 作成後に `TODO` から該当項目を削除し忘れること
- PRD 作成後に `plan.md` への挿入を忘れること
