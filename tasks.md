# I-270 types.rs エラー完全解消 — タスク一覧

## 概要

Hono ディレクトリコンパイルの `types.rs` エラーを全解消し、157/158 以上を達成する。

## 進捗

- [x] **T1**: フィールド名サニタイズ — 6 エラー解消
- [x] **T2**: Uint8Array 除外修正 + 推移的依存 — 2 エラー解消（KeyAlgorithm, RsaOtherPrimesInfo）
- [x] **T4**: 再帰型 Box ラップ — 1 エラー解消（Function）
- [x] **is_derivable_type 修正**: `RustType::Any`（serde_json::Value）を derivable に — 9 エラー回避
- 合計: 36 → **26 エラー** (10 件解消)

## 要修正: T1 のフィールド名サニタイズの適用範囲

### 問題

現在の T1 実装は `src/generator/mod.rs:74` の **struct 定義出力のみ** に `sanitize_field_name` を適用している。しかし同じフィールド名は以下でも出力される:

- **struct リテラル式**: `MyStruct { Content-Type: value }` — 未サニタイズ
- **フィールドアクセス式**: `obj.Content-Type` — 未サニタイズ

struct 定義では `content_type` だが、使用箇所では `Content-Type` のままとなり、`field not found` エラーが発生する。

### 理想的な修正

generator 出力時の個別対処ではなく、**IR に入る時点でフィールド名をサニタイズする**。`StructField.name` が常に有効な Rust 識別子であれば、generator は一貫して正しい名前を出力できる。

### 修正箇所

1. **`src/pipeline/type_converter.rs`** — インライン struct（`register_inline_struct`）のフィールド作成時にサニタイズ
2. **`src/pipeline/type_resolver.rs`** — 匿名 struct のフィールド作成時にサニタイズ
3. **`src/external_types.rs`** — 外部型フィールドの変換時にサニタイズ（現在は camelCase のまま TypeDef に格納）
4. **`src/pipeline/external_struct_generator.rs`** — 外部 struct 生成時（既に `camel_to_snake` あり、`sanitize_field_name` に統合）
5. **`src/generator/mod.rs:74`** — `sanitize_field_name` を削除し、`escape_ident` に戻す（IR が正しければ generator は変換不要）

### 設計原則

- IR（`StructField.name`）は **有効な Rust 識別子** を保持する責務を持つ
- generator は IR をそのまま出力する（追加の変換をしない）
- サニタイズ関数は共有ユーティリティとして 1 箇所に定義し、IR 生成の各箇所から呼び出す

## 残タスク

### 残 26 エラーの分類

| カテゴリ | 件数 | エラー内容 |
|---|---|---|
| E0425: 未定義型（他モジュールでインポート可能） | 5 | JSXNode, Context, HtmlEscapedString×3 |
| E0425: 未定義型（Hono 固有、出力に存在しない） | 8 | HTTPResponseError, __type, BufferSource, H, MessageFunction, RecursiveRecord, TemplateStringsArray, FC |
| E0425: 型パラメータの漏出 | 4 | T×2, K, TArrayBuffer×2 |
| E0425: Rust 標準型インポート不足 | 1 | HashMap |
| E0107: ジェネリクス不一致 | 6 | ArrayBufferView×2, ReadableStream×2, Uint8Array, Promise |
| E0433: 型パラメータスコープ | 1 | E (in E::Bindings) |
| E0425: TArrayBuffer | 1 | ArrayBufferView struct 内の型パラメータ（E0107 解消で同時解決） |

### T1-fix: フィールド名サニタイズを IR レベルに移動

- [ ] `sanitize_field_name` を共有ユーティリティ（例: `src/utils.rs` or `src/ir.rs`）に移動
- [ ] `src/pipeline/type_converter.rs`: `register_inline_struct` のフィールド名作成時に `sanitize_field_name` 適用
- [ ] `src/pipeline/type_resolver.rs`: 匿名 struct のフィールド名作成時に `sanitize_field_name` 適用
- [ ] `src/external_types.rs`: `convert_external_typedef` のフィールド名変換時に `sanitize_field_name` 適用
- [ ] `src/pipeline/external_struct_generator.rs`: `camel_to_snake` を `sanitize_field_name` に統合（camel_to_snake + sanitize の合成）
- [ ] `src/generator/mod.rs:74`: `sanitize_field_name` を `escape_ident` に戻す
- [ ] 単体テスト: ハイフン・ブラケット・`_` のフィールドが IR レベルで正しく変換されること
- [ ] `cargo test` パス確認

### T3: type_params 抽出パイプライン（7 エラー解消: E0107×6 + TArrayBuffer×1）

- [ ] `tools/extract-types/src/types.ts`: `ExternalInterfaceDef` に `type_params?: string[]` 追加
- [ ] `tools/extract-types/src/extractor.ts`: `extractInterface` で `node.typeParameters` 抽出
- [ ] JSON 再抽出: ecmascript.json + web_api.json
- [ ] `src/external_types.rs`: `ExternalInterfaceDef` に `type_params` フィールド追加、パース → `TypeParam` 変換
- [ ] FORMAT_VERSION インクリメント
- [ ] 単体テスト追加
- [ ] `cargo test` パス確認

**対象エラー**:
- E0107: ArrayBufferView<ArrayBuffer>×2, ReadableStream<serde_json::Value>×2, Uint8Array<ArrayBuffer>, Promise<Vec<...>>
- E0425: TArrayBuffer×2（ArrayBufferView と Uint8Array の struct 内）

### T5: types.rs インポート + スタブ生成（19 エラー解消）

T1-fix, T3 完了後に着手（IR レベルのサニタイズと type_params で解消される分を差し引いた残りを処理）。

#### T5a: Rust 標準型インポート（1 エラー）
- [ ] types.rs 内で `HashMap` が参照されていたら `use std::collections::HashMap;` を先頭に追加

#### T5b: 他モジュール定義型のインポート（5 エラー）
- [ ] `file_outputs` から定義済み型名とモジュールパスの対応表を構築
- [ ] types.rs 内の未定義型を対応表で解決 → `use crate::module::TypeName;` を生成
- 対象: JSXNode（jsx::base）, Context（jsx::dom::render）, HtmlEscapedString（utils::html）×3

#### T5c: 未定義型のスタブ struct 生成（13 エラー）
- [ ] T5b で解決されなかった未定義型に対して:
  - TypeRegistry に存在する型 → フル struct 生成
  - TypeRegistry に存在しない型 → 空スタブ `pub struct TypeName;` を生成
  - 型パラメータ（T, K, E, H, TArrayBuffer）→ 参照元の enum/struct にジェネリクスパラメータとして追加
- 対象: HTTPResponseError, __type, BufferSource, H, MessageFunction, RecursiveRecord, TemplateStringsArray, FC, T, K, E::Bindings

### T6: 最終検証 + ドキュメント更新

- [ ] `cargo test` 全パス
- [ ] `cargo clippy` 0 警告
- [ ] `cargo fmt --all --check` パス
- [ ] カバレッジ閾値維持
- [ ] Hono ベンチマーク: ディレクトリコンパイル 157/158 以上
- [ ] plan.md / TODO / backlog 最新化
