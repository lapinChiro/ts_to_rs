# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

1. `backlog/try-catch-ir-expansion.md` — try/catch の IR 展開（Stmt::TryCatch 廃止、break/continue 修正）
2. `backlog/inline-type-annotation-struct.md` — 型注記位置の intersection / TsTypeLit での struct 生成

以降の優先順位は `TODO` を参照。

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
