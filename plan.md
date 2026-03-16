# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

`backlog/silent-semantics-bugs.md` (I-60, I-17)

## キュー

1. `backlog/small-generator-fixes.md` (I-85, I-84, I-87)
2. `backlog/object-expression-fixes.md` (I-86, I-89)
3. `backlog/to-string-unification.md` (I-92, I-88, I-67)
4. `backlog/closure-function-fixes.md` (I-80, I-81, I-82)

## 保留中

- `backlog/e2e-io-test-infrastructure.md` (I-49, I-50, I-51) — I-24（外部パッケージ型解決）完了後に着手

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
