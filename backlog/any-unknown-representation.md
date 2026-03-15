# any/unknown の実用的な型表現

## 背景・動機

`any`/`unknown` → `Box<dyn std::any::Any>` は式の中で直接使用できない（`x + 1` 等）。TS の `any` は任意の式で使えるため、変換後のコードがほぼ確実にコンパイル不可になる。

関連コード: `src/transformer/types/mod.rs:37-38`、`src/generator/types.rs:52`。

## ゴール

`any`/`unknown` を使うコードが、実用的にコンパイル可能な Rust を生成する。

## スコープ

### 対象

- `any`/`unknown` の型表現を `Box<dyn std::any::Any>` から実用的な代替に変更
- 関数パラメータ位置での `any` → ジェネリクス化の検討

### 対象外

- `any` を使うコード全体の完全な型安全化
- TS の `any` の暗黙的型変換の再現

## 設計

### 技術的アプローチ

`any`/`unknown` の出現位置によって最適な Rust 表現が異なる:

| 位置 | 現在 | 提案 |
|------|------|------|
| 関数パラメータ | `Box<dyn Any>` | ジェネリクス `T` + trait bound なし |
| 返り値型 | `Box<dyn Any>` | `serde_json::Value`（構造化データ）または `Box<dyn Any>` |
| 変数の型注記 | `Box<dyn Any>` | 型注記を省略（Rust の型推論に委ねる） |
| フィールドの型 | `Box<dyn Any>` | `serde_json::Value` |

最もシンプルなアプローチ: `any`/`unknown` → `serde_json::Value` に統一。`object` keyword と同じ扱い。`serde_json::Value` は:
- JSON 互換の値を表現可能
- 算術演算等は直接不可だが、`.as_f64()` 等のアクセサが利用可能
- `Box<dyn Any>` よりも実用的

### 影響範囲

- `src/transformer/types/mod.rs` — `TsAnyKeyword`/`TsUnknownKeyword` の変換
- `src/ir.rs` — `RustType::Any` バリアントの意味変更 or 削除
- `src/generator/types.rs` — `Any` の生成
- テストファイル

## 作業ステップ

- [ ] ステップ1（RED）: `any` → `serde_json::Value` のテスト追加
- [ ] ステップ2（GREEN）: `RustType::Any` の生成を `serde_json::Value` に変更
- [ ] ステップ3: `RustType::Any` バリアントを削除し `Named { name: "serde_json::Value" }` に統一（REFACTOR）
- [ ] ステップ4: 全テスト・スナップショット更新
- [ ] ステップ5: Quality check

## テスト計画

- `function f(x: any)` → `fn f(x: serde_json::Value)`
- `let x: unknown` → `let x: serde_json::Value`
- フィールド `data: any` → `data: serde_json::Value`
- 回帰: any を含む既存テスト

## 完了条件

- `any`/`unknown` が `serde_json::Value` として変換される
- 既存の `any` フォールバック（resilient モード）が引き続き動作する
- 全テスト pass、0 errors / 0 warnings
