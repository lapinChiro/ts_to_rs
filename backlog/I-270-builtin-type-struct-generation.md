# I-270: 参照されるビルトイン型の struct 定義自動生成

## 背景・動機

変換パイプラインが union enum バリアントや struct フィールドで外部型（`ArrayBuffer`, `Date`, `Error`, `RegExp` 等）を参照するが、Rust 側に対応する struct 定義が存在せずコンパイルエラーになる。

外部型は `web_api.json`（105 型）/ `ecmascript.json`（57 型）から `TypeRegistry` に読み込まれ、メソッド戻り値型やパラメータ型の解決に利用されている。しかし、これらは **型情報の登録のみ** であり、IR の `Item::Struct` としては生成されない。

Hono ベンチマークのディレクトリコンパイルで `types.rs`（共有 synthetic モジュール）に 36 のエラーが発生。根本原因分析: `report/i-270-types-rs-errors-analysis.md`

## ゴール

1. 変換出力で参照されるビルトイン外部型に対し、JSON 定義のフィールド情報に基づく struct 定義が自動生成される
2. types.rs の全 36 エラーが解消される
3. Hono ディレクトリコンパイルが 157/158 以上になる
4. コンパイルテスト `instanceof-builtin` のスキップ理由のうち「struct 定義不在」が解消される

## スコープ

### 対象

- 外部型 struct の自動生成（参照される型のみ、JSON フィールド情報ベース）
- TypeRegistry の外部型トラッキング（`register_external` / `is_external`）
- 推移的依存の解決（生成 struct のフィールドが参照する外部型も生成）
- type_params 抽出パイプライン（TS 抽出スクリプト → JSON → Rust ローダー）
- 再帰型の自動 Box ラップ
- フィールド名サニタイズ（ハイフン・ブラケット・予約識別子）
- types.rs へのインポート自動生成（他モジュール定義型 + Rust 標準型）
- 未定義型のスタブ生成 + 型パラメータのジェネリクス化

### 対象外

- メソッドの impl ブロック生成（I-270c として TODO に記録済み）
- `instanceof-builtin` コンパイルテストのスキップ完全解除（メソッド呼び出しが存在し impl なしではコンパイル不可）
- 型エイリアス（`BufferSource` 等）の TypeRegistry 登録（I-263 の範囲。本 PRD ではスタブ struct で対処）

## 設計

### 技術的アプローチ

#### 1. 外部型 struct 生成（実装済み）

`src/pipeline/external_struct_generator.rs` に `collect_undefined_type_references` + `generate_external_struct` を実装。`TypeRegistry.is_external()` で外部型のみを対象。per-file の `all_items` と共有 `synthetic_items` の両方で生成。

#### 2. 推移的依存の固定点計算

外部 struct 生成を 1 回で終わらせず、生成された struct のフィールドが参照する型も走査し、新しい未定義外部型がなくなるまでループ。最大 10 回のイテレーション制限。

#### 3. type_params 抽出パイプライン

TypeScript の interface 宣言から type parameters を抽出し、JSON スキーマに追加。Rust 側のローダーが `TypeDef::Struct.type_params` に反映。生成 struct が正しいジェネリクス（`<R>`, `<TArrayBuffer>`）を持つ。

**変更箇所**:
- `tools/extract-types/src/types.ts`: `ExternalInterfaceDef` に `type_params?: string[]`
- `tools/extract-types/src/extractor.ts`: `extractInterface` で `node.typeParameters` 抽出
- `src/external_types.rs`: `ExternalInterfaceDef` に `type_params` フィールド追加、パース → `TypeParam` 変換
- JSON 再抽出

#### 4. フィールド名サニタイズ

`src/generator/mod.rs` の struct フィールド生成で `sanitize_field_name` を適用:
- ハイフン → アンダースコア
- ブラケット除去
- `_` のみ → `_field`
- 先頭数字 → `_` プレフィクス

#### 5. 再帰型 Box ラップ

`generate_external_struct` でフィールド型が struct 自身を参照する場合に `Box` でラップ。

#### 6. types.rs インポート + スタブ生成

types.rs 内で参照されるが未定義の型に対し:
1. 他モジュールで定義されている → `use crate::module::TypeName;` 生成
2. Rust 標準型（HashMap 等） → `use std::collections::HashMap;` 生成
3. TypeRegistry に存在する型 → フル struct 生成
4. TypeRegistry にない型 → 空スタブ `pub struct TypeName;` 生成
5. 型パラメータ（単一大文字 `T`, `K`, `E` 等） → 参照元の enum/struct をジェネリクス化

### 設計整合性レビュー

- **高次の整合性**: 変換パイプラインの設計方針に沿っている。外部型 struct 生成は既存アーキテクチャの自然な拡張。TypeRegistry の外部型トラッキングは型の出自を正しくモデル化
- **DRY / 直交性 / 結合度**: `external_struct_generator.rs` が struct 生成を担当し、OutputWriter はファイル書き出しに集中。type_params 抽出は TS → JSON → Rust の各層で責務分離
- **割れ窓**: `escape_ident` がハイフン等を処理しない既存問題を本 PRD で修正

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/pipeline/external_struct_generator.rs` | 推移的依存 + 再帰型 Box + スタブ生成 |
| `src/pipeline/mod.rs` | 固定点ループ + synthetic_items 後処理 |
| `src/pipeline/output_writer.rs` | types.rs インポート生成 |
| `src/generator/mod.rs` | フィールド名サニタイズ |
| `src/external_types.rs` | type_params パース |
| `src/registry.rs` | (実装済み) register_external / is_external |
| `src/lib.rs` | (実装済み) transpile_with_builtins |
| `tools/extract-types/src/types.ts` | type_params スキーマ追加 |
| `tools/extract-types/src/extractor.ts` | type_params 抽出 |
| `src/builtin_types/ecmascript.json` | 再抽出（type_params 追加） |
| `src/builtin_types/web_api.json` | 再抽出（type_params 追加） |
| `tests/` | テスト追加・スナップショット更新 |

## タスク一覧

詳細は `tasks.md` 参照。

| タスク | 内容 | 解消エラー数 |
|---|---|---|
| T1 | フィールド名サニタイズ | 6 |
| T2 | Uint8Array 除外修正 + 推移的依存 | 5 |
| T3 | type_params 抽出パイプライン | 10 |
| T4 | 再帰型 Box ラップ | 1 |
| T5 | types.rs インポート + スタブ生成 | 14 |
| T6 | 最終検証 + ドキュメント更新 | — |

## テスト計画

### 単体テスト

- フィールド名サニタイズ: ハイフン・ブラケット・`_`・先頭数字の各パターン
- 推移的依存: A→B→C の依存チェーンで C が外部型
- 再帰型: `Function.caller: Function` → `Box<Function>`
- type_params: JSON 定義に type_params → Item::Struct の type_params に反映
- スタブ生成: TypeRegistry にない型 → 空 struct
- 型パラメータ検出: 単一大文字 → ジェネリクス化

### 統合テスト

- `instanceof-builtin` with builtins: struct 定義が出力に含まれる（スナップショット）
- `external-type-struct`: union + 外部型の struct 生成（スナップショット）

### ベンチマーク

- Hono ディレクトリコンパイル: types.rs エラー 0、157/158 以上

## 完了条件

1. types.rs の 36 エラーが全て解消
2. Hono ディレクトリコンパイルが 157/158 以上
3. 全テストパス、clippy 0 警告、fmt チェックパス
4. カバレッジ閾値を維持
5. type_params が JSON スキーマに追加され、JSON が再抽出済み
