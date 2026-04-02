# Code Review

## Review Priority

レビューでは次を優先する。

1. correctness bug
2. behavioral regression
3. missing or weak tests
4. unsafe design drift
5. maintainability issues with concrete impact

## Review Style

- findings-first で報告する
- 重要度順に並べる
- 要約は最後でよい
- 問題がない場合も、その旨と残余リスクを書く

## Checklist

- 仕様を壊していないか
- 変換挙動に対するテストが十分か
- pipeline の責務分離を壊していないか
- generator 依存が transformer に漏れていないか
- 一時回避が設計負債として固定化されていないか
- naming が実際の挙動と一致しているか
