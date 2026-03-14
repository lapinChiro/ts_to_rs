# ts_to_rs 開発計画

## 次のタスク

### フェーズ 1: IR の型安全性強化（設計負債の解消）

IR の変更は transformer と generator の両方に波及するため、機能追加の前にまとめて行う。
個別に進めると同じファイルを何度も変更することになり非効率。

1. IR 型安全性の一括改善 — BinaryOp/UnaryOp の enum 化 + Cast の target を RustType に + Method.body を Option に
2. classes.rs 5 関数の統一リファクタリング
3. super() リライトの修正（引数数・フィールド数の不一致検出）

### フェーズ 2: 変換精度の向上（機能追加）

IR が安定した後、変換率を上げる機能を追加する。

4. nullable union の Option ラップ（type alias 位置）
5. TryCatch のエラー型パラメータ化

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
