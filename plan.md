# ts_to_rs 開発計画

## 次のタスク

### 実行順序（Hono 変換率 57% → 目標 65%+）

Hono 再評価（`report/hono-rescan-2025-03.md`）と依存関係に基づく実行順序。

1. `backlog/object-keyword-type.md` — `object` keyword 対応（Hono 2件、工数極小）
2. `backlog/protected-members.md` — `protected` → `pub(crate)`（工数小、#4 の前提）
3. `backlog/non-nullable-union-annotation.md` — 非 nullable union 型注記（Hono 7件、#8 の前提）
4. `backlog/extends-implements.md` — `extends` + `implements` 併用
5. `backlog/computed-enum-member.md` — enum computed member
6. `backlog/nested-function-decl.md` — ネスト関数宣言（Hono 1件）
7. `backlog/array-destructuring-ext.md` — 配列分割代入の拡張
8. `backlog/intersection-union-complex.md` — intersection + union 複合型（#3 の後）
9. `backlog/stmt-responsibility.md` — convert_stmt 責務整理
10. `backlog/default-param-extra-items.md` — extra_items 破棄修正

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
