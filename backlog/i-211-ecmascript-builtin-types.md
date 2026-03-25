# I-211: ECMAScript 標準組み込み型の追加 — 全体計画

## 背景・動機

`builtin_types.json` は `lib.dom.d.ts`（Web API）のみから抽出されており、ECMAScript 標準型（`String`, `Array`, `Date`, `Error`, `RegExp`, `Map`, `Set` 等）が含まれていない。これにより:

1. **メソッドチェーン型追跡が機能しない**: `MethodSignature` と `TypeResolver::resolve_method_return_type` は実装済みだが、TypeRegistry に `String`/`Array` が未登録のため、`"hello".trim().split(" ")` のようなビルトインメソッドの戻り値型が解決できない
2. **instanceof で未定義 struct を参照**: `x instanceof Date` が enum バリアント `Date(Date)` を生成するが、`Date` struct が未定義でコンパイルエラーになる
3. **I-112c の残存エラーの一因**: TypeRegistry に型情報がないため、オブジェクトリテラルの型推論で expected type が解決できないケースがある

さらに、現在のローダーには構造的な制約がある:

- **メソッドオーバーロード非対応**: `method.signatures.first()` で最初のシグネチャのみ取得し、残りを破棄している（`src/external_types.rs:202`）
- **Union 型の簡略化**: 複数メンバーの union（`string | number`）を第 1 要素で代表している（`src/external_types.rs:320`）。既存の `SyntheticTypeRegistry::register_union` 基盤で合成 enum に変換可能だが未統合

## 全体ゴール

1. `"hello".trim().split(" ")` で `split` のレシーバ型が `String` と解決され、戻り値型が `Vec<String>` と解決される
2. `[1,2,3].map(...).filter(...)` で `filter` のレシーバ型が `Vec<f64>` と解決される
3. `x instanceof Date` で `Date` struct が TypeRegistry に存在し、コンパイルエラーにならない
4. 複数シグネチャを持つメソッド呼び出しで、引数の数・型に基づいて適切なシグネチャが選択される
5. 型ファイルが用途別に分割され、抽出元（TypeScript バージョン、対象 lib ファイル）が文書化されている

## PRD 構成

3 つの独立した PRD に分割し、連続して実行する。各 PRD は単体で完結する完了条件を持つ。

| PRD | 内容 | 前提 |
|-----|------|------|
| **[I-211-a](i-211-a-overload-support.md)** | データモデル変更 + オーバーロード解決 + Union の合成 enum 変換 | なし |
| **[I-211-b](i-211-b-ecmascript-extraction.md)** | 抽出スクリプト拡張 + JSON ファイル分割 + ローダー更新 | I-211-a 完了 |
| **[I-211-c](i-211-c-verification.md)** | E2E テスト + ベンチマーク効果測定 | I-211-b 完了 |

## 対象外（全体共通）

- ES2016〜ES2024 の追加メソッド（TODO I-261 に記録済み）
- `Intl` 名前空間（TODO I-262 に記録済み）
- 型エイリアス（`kind: "alias"`）の TypeRegistry 登録（TODO I-263 に記録済み）
- `Array.prototype.filter` のクロージャ参照型ミスマッチ（TODO I-217）
- `Number.prototype.toFixed` の Rust 変換（TODO I-237）
