# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

（なし — backlog/ から次の PRD を選定する）

## 保留中

- `backlog/e2e-test-reliability.md` (I-32) — 部分完了。残り 3 件のスキップ解消は変換ロジックの構造的問題（I-26/28/35）の解決が前提
- `backlog/e2e-io-test-infrastructure.md` (I-49, I-50, I-51) — Node.js API（fs, http, process.stdin）の変換機能が未実装のため、テスト基盤を作っても動作検証不可。I-24（外部パッケージ型解決）完了後に着手

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
