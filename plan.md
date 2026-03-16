# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

1. `backlog/e2e-bugfix-codegen-semantics.md` — E2E テストで発見されたコード生成・変換バグの修正（I-52〜I-57）
2. `backlog/utility-type-support.md` — ユーティリティ型の TypeRegistry 連携（Partial/Required/Pick/Omit/NonNullable）
3. `backlog/switch-improvement.md` — switch 文の改善（fall-through 検出 + 文字列 match + パターン IR 改善）

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
