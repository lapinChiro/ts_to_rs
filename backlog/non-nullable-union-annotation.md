# 非 nullable union の型注記位置対応

## 背景・動機

Hono で `Response | Promise<Response>` のような null/undefined を含まない union が型注記位置（パラメータ型、返り値型、プロパティ型）で 7 件出現する。現在の `convert_union_type`（`src/transformer/types/mod.rs` line ~110）は `T | null` パターンのみ対応しており、非 nullable union は `"non-nullable union types are not supported"` エラーになる。

型注記位置では Rust に匿名 enum を直接定義する方法がないため、完全な変換は不可能だが、エラーで止まるよりもコンパイル可能なフォールバック出力を生成する方が実用的である。

## ゴール

非 nullable union が型注記位置でエラーにならず、フォールバック型（最初のメンバー型 + TODO コメント）として変換される。

## スコープ

### 対象

- `convert_union_type` で非 nullable union をエラーにせず、最初の型を返す
- 変換時に情報が失われることを示す TODO コメントを付与する

### 対象外

- 型注記位置での匿名 enum 自動定義
- union の全メンバーを完全に表現する変換
- `type X = A | B` のようなトップレベル型エイリアス位置の union（これは既存の enum 変換が対応済み）

## 設計

### 技術的アプローチ

- `convert_union_type` の非 nullable union 分岐を変更する
- 現在のエラー返却を、最初のメンバー型への変換に置き換える
- 返却する `RustType` に TODO コメント情報を付与する方法は、既存の IR 構造に依存する（コメントフィールドがなければ、ログ出力またはコード生成時のコメント挿入で対応する）
- プロジェクトの初版方針に沿い、情報損失を許容してコンパイル可能な出力を優先する

### 影響範囲

- `src/transformer/types/mod.rs` — `convert_union_type` の非 nullable 分岐変更
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: `fn f(x: string | number)` の変換テストを追加し、エラーではなくフォールバック型を期待するテストを書く
- [ ] ステップ2（GREEN）: `convert_union_type` を拡張し、非 nullable union で最初の型を返す
- [ ] ステップ3: E2E テスト追加
- [ ] ステップ4: Quality check

## テスト計画

- `x: string | number` → `x: String`（最初の型にフォールバック）
- `x: Response | Promise<Response>` → `x: Response`（最初の型にフォールバック）
- 回帰: `x: string | null` → `Option<String>` が変更なく動作すること
- 回帰: `type X = A | B` のトップレベル union enum 変換が変更なく動作すること

## 完了条件

- 非 nullable union が型注記位置でエラーにならない
- フォールバック型として最初のメンバー型が使用される
- 既存のテストがすべてパスする
- `cargo test`, `cargo clippy`, `cargo fmt --check` が 0 エラー・0 警告
