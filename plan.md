# ts_to_rs 開発計画

## 次のタスク

### Phase 1: 基盤強化

TypeEnv（型環境）の導入により、変換の正確性を根本から改善する。

1. `backlog/type-env-introduction.md` — TypeEnv のデータ構造とシグネチャ導入
2. `backlog/type-env-expr-resolution.md` — 式の型解決関数
3. `backlog/type-env-opt-chain.md` — optional chaining / nullish coalescing の型判定
4. `backlog/contains-throw-recursion.md` — contains_throw の全構文再帰化
5. `backlog/rust-reserved-word-escape.md` — Rust 予約語エスケープ
6. `backlog/nullable-multi-type-union.md` — nullable + 複数非null型の union

### Phase 2 以降

Phase 1 完了後に TODO から PRD 化する。詳細は `TODO` を参照。

- Phase 2: 変換正確性（builtin API 参照モデル、intersection 型注記、複数スプレッド）
- Phase 3: 型構文の網羅性（TsLitType, TsConditionalType, mapped type 等）
- Phase 4: 文・式の網羅性（ネスト async、分割代入拡張、Cargo.toml 生成）

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
