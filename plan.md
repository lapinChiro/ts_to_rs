# ts_to_rs 開発計画

## 次のタスク（Hono 変換ブロッカー優先）

以下の順序で消化する。依存関係と影響範囲の小さいものから着手し、後続の作業を安定させる。

1. `backlog/claude-rules-optimization.md` — .claude/rules/ のコンテキスト効率最適化。機能コードに影響なし
2. `backlog/spread-non-first-position.md` — 既存機能の拡張。既存テストとの整合性を保つ
2. `backlog/inline-type-literal-param.md` — 変換パイプラインの拡張が必要。設計コストが中程度
3. `backlog/conditional-type-tier1.md` — 新しい型構文カテゴリ。IR 拡張を含む大きめの作業
4. `backlog/conditional-type-tier2-fallback.md` — Tier 1 の実装が前提

## Hono 対応の開始条件

前提 6 構文はすべて実装済み（詳細は `report/hono-syntax-analysis.md`）:

1. ~~type assertion (`x as T`)~~
2. ~~`any` / `unknown` 型~~
3. ~~optional chaining (`x?.y`)~~
4. ~~nullish coalescing (`x ?? y`)~~
5. ~~spread 構文 (`...`)~~
6. ~~getter/setter~~

→ **Hono コアファイルの変換試行を開始可能**

## 未設計の項目

以下は `TODO` に記載。保留理由も `TODO` に明記。

- 所有権推論（判断保留事項 #4、時期尚早）
- Docker 開発環境（優先度低い）
- Watch モード（優先度低い）

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
