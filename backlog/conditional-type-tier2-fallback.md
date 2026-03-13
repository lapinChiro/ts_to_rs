# conditional type — Tier 2: フォールバック出力

## 背景・動機

Tier 1 で自動変換できない conditional type（約 24 件）は現在エラーで変換が停止する。Hono 全体の変換を進めるには、自動変換できないパターンでも変換を止めずに、手動修正のための情報を残す必要がある。

## ゴール

Tier 1 で変換できない conditional type に遭遇した場合:

1. 変換エラーで停止しない
2. 元の TypeScript コードをコメントとして Rust コードに残す
3. 型エイリアスのプレースホルダーを生成する（手動修正の起点）

出力例:
```rust
// TODO: Conditional type not auto-converted
// Original TS: type Foo<T> = T extends { kind: infer K } ? K extends "click" ? ClickEvent : HoverEvent : never;
type Foo<T> = (); // placeholder
```

## スコープ

### 対象

- Tier 1 のパターンマッチに該当しない conditional type のフォールバック処理
- 元の TS コードのコメント出力
- プレースホルダー型エイリアスの生成

### 対象外

- Tier 3 の高度パターンの自動変換（将来課題として TODO に記載済み）
- フォールバック出力の型を推測する高度なロジック

## 設計

### 技術的アプローチ

`convert_conditional_type` で Tier 1 のパターンにマッチしなかった場合に、エラーを返す代わりにフォールバック Item を生成する:

1. `TsConditionalType` の元の TS コードをソースマップ情報またはスパン情報から復元する（困難な場合は AST から再構築）
2. コメント付きの型エイリアス `Item` を生成する
3. 変換を続行する

IR への影響:
- `Item::TypeAlias` はすでに存在する場合はそれを利用
- コメントの付加方法: `Item` に `comments: Vec<String>` フィールドを追加するか、コメント専用の `Item::Comment` を追加する

### 影響範囲

- `src/transformer/types.rs` — `convert_conditional_type` のフォールバックパス
- `src/ir.rs` — コメント付加の仕組み（必要に応じて）
- `src/generator/mod.rs` — コメント付き Item の出力

## 作業ステップ

- [ ] ステップ1（RED）: Tier 1 に該当しない conditional type がフォールバック出力されることを検証するテストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: フォールバックパスの最小実装（コメント + プレースホルダー型エイリアス）
- [ ] ステップ3: 元の TS コードをコメントに含める処理を実装
- [ ] ステップ4: E2E テスト（fixture）を追加
- [ ] ステップ5（REFACTOR）: コードの整理

## テスト計画

- 正常系: Tier 1 に該当しない conditional type → コメント + プレースホルダー出力
- 正常系: フォールバック後も後続の型定義の変換が続行される
- 正常系: 元の TS コードがコメントに含まれる
- 境界値: 非常に長い conditional type の表現がコメントに収まること

## 完了条件

- Tier 1 に該当しない conditional type が変換エラーを起こさない
- 元の TS コードがコメントとして出力に含まれる
- プレースホルダー型エイリアスが生成される
- 変換が停止せず、後続の定義も正常に変換される
- `cargo fmt --all --check` / `cargo clippy` / `cargo test` が 0 エラー・0 警告
