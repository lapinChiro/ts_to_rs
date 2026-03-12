# ts_to_rs 開発計画

## 次のタスク

1. [generator-split](backlog/generator-split.md) — generator.rs のモジュール分割
2. [array-literal](backlog/array-literal.md) — 配列リテラル → `vec![...]` 変換
3. [object-literal](backlog/object-literal.md) — 型注記付きオブジェクトリテラル → 構造体初期化式
4. [ternary-operator](backlog/ternary-operator.md) — 三項演算子 → `if` 式
5. [unsupported-syntax-detection](backlog/unsupported-syntax-detection.md) — 未対応構文の検出・JSON レポート
6. [break-continue](backlog/break-continue.md) — break/continue（ラベル付き含む）
7. [class-inheritance](backlog/class-inheritance.md) — クラス継承（extends → trait + struct）

## 未設計の項目

以下は `TODO` に記載。保留理由も `TODO` に明記。

- async/await → tokio（判断保留事項 #3 に依存）
- エラーハンドリング（判断保留事項 #3 が未決定）
- 所有権推論（判断保留事項 #4、時期尚早）
- Docker 開発環境（優先度低い）
- Watch モード（優先度低い）

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
