# ts_to_rs 開発計画

## 現状

backlog/ は空。前回のバッチで 12 件の PRD を処理済み。

主な成果:
- main.rs リファクタリング（`build_shared_registry`, `default_output_dir` 抽出）
- try/catch → labeled block パターン化
- 配列インデックス `as usize` 自動挿入、parseInt パニック回避
- any/unknown → `serde_json::Value`
- type assertion のプリミティブキャスト保持
- Promise の union 内展開、conditional type フォールバック改善
- const + オブジェクト型 → `let mut`
- Math.max/min 可変引数チェーン化
- union 内 never/void の除去

## 次のフェーズ: 変換の基盤強化

### 根本課題

トランスフォーマーに **型環境（TypeEnv）がない**。各文は独立に変換され、ローカル変数・関数パラメータの型が後続の文や式に伝搬されない。

これが以下の問題群の共通原因:
- optional chaining / nullish coalescing の非 Option 型対応
- builtin API の参照モデル（`&mut self` 判定、クロージャパラメータ型）
- const のミュータビリティ解析
- 型ガード、数値変換、空配列推論

### 実行順序

**Phase 1: 基盤（TypeEnv）**

1. TypeEnv の導入 — `convert_stmt_list` にスコープ付き型マップを追加
2. `contains_throw` の全構文再帰化 — Result ラッピング漏れの修正
3. Rust 予約語エスケープ — `r#match` 等の自動エスケープ
4. nullable + 複数非null型の union — `Option<Enum>` パターン

**Phase 2: 正確性（TypeEnv 活用）**

5. optional chaining / nullish coalescing の非 Option 型対応
6. builtin API の参照モデル修正（`&mut self`、クロージャ参照型）
7. intersection 型注記位置での struct 生成
8. 複数オブジェクトスプレッド

**Phase 3: 型構文の網羅性**

9-14. TsLitType, TsConditionalType, mapped type, call signature, TsTypeLit, union 内未対応型

**Phase 4: 文・式の網羅性**

15-17. ネスト async 関数、分割代入拡張、Cargo.toml 生成

詳細は `TODO` を参照。

## Hono 対応

前提 6 構文は実装済み。Hono コアファイルの変換試行は開始可能だが、Phase 1-2 の基盤強化を先に実施することで変換品質が大幅に向上する。

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `Result<T, String>` に決定済み。エラー型は初版では `String` 固定
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
