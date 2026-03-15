# ts_to_rs 開発計画

## 次のタスク

### Phase 2 以降

割れ窓修正後に TODO から PRD 化する。詳細は `TODO` を参照。

- Phase 2: 変換正確性（builtin API 参照モデル、intersection 型注記、複数スプレッド）
- Phase 3: 型構文の網羅性（TsLitType, TsConditionalType, mapped type 等）
- Phase 4: 文・式の網羅性（switch 文、ネスト async、分割代入拡張、Cargo.toml 生成）

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
