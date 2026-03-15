# ts_to_rs 開発計画

## 次のタスク

### 実行順序

**開発環境の改善（先に実施）:**

1. `backlog/main-rs-refactoring.md` — main.rs リファクタリング
2. `backlog/coverage-threshold-optimization.md` — カバレッジ閾値最適化（#1 の完了が前提）

**Critical — 生成コードがコンパイル不可:**

3. `backlog/optional-chaining-nullish-fix.md` — optional chaining / nullish coalescing の非 Option 型対応
4. `backlog/try-catch-control-flow.md` — try/catch の制御フロー（break/continue、throw 型不一致）
5. `backlog/number-integer-context.md` — number の整数コンテキスト（配列インデックス `as usize`、parseInt パニック回避）
6. `backlog/any-unknown-representation.md` — any/unknown の実用的表現（`Box<dyn Any>` → `serde_json::Value`）

**High — 意味的に誤りまたは情報損失:**

7. `backlog/type-assertion-preserve.md` — type assertion の型情報保持
8. `backlog/promise-conditional-fallback.md` — Promise の union 内展開 + conditional type フォールバック改善
9. `backlog/const-mutability.md` — const のミュータビリティ差異（オブジェクト型 → `let mut`）
10. `backlog/test-quality-improvement.md` — テスト品質改善（コンパイル不可スナップショット修正、欠落テスト追加、アサーション強化）

**Medium — エッジケース・小規模修正:**

11. `backlog/expression-semantics-fixes.md` — 式変換の修正（Math 可変引数、複数スプレッド、三項演算子等）
12. `backlog/type-edge-cases.md` — 型エッジケース（never/void の union 処理、intersection 型注記位置）

## Hono 対応の開始条件

前提 6 構文はすべて実装済み:

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
