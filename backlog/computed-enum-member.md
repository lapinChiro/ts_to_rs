# enum の computed member 対応

## 背景・動機

`enum Flags { Read = 1 << 0, Write = 1 << 1 }` のようなビット演算による enum 値指定が未対応。現在の `convert_ts_enum`（`src/transformer/mod.rs` line ~611）は `Expr::Lit(Num)` と `Expr::Lit(Str)` のみ処理しており、`Expr::Bin`（binary expression like `1 << 0`）は `None` になり値が消える。

TypeScript のビットフラグパターンは頻出であり、変換時に値が失われると Rust 側で手動修正が必要になる。

## ゴール

`1 << 0` のようなビット演算式を値として保持し、Rust の enum で `Read = 1 << 0` として出力する。

## スコープ

### 対象

- enum メンバー初期化子のバイナリ式（`1 << 0`, `1 | 2` 等）の処理
- IR の `EnumValue` に式を保持するバリアント `EnumValue::Expr(String)` の追加
- Generator での `EnumValue::Expr` の出力対応

### 対象外

- 複雑な式（関数呼び出し、変数参照、三項演算子等）
- enum メンバー間の参照（`B = A + 1` のようなパターン）
- 定数畳み込み（式の評価は行わず、文字列表現として保持する）

## 設計

### 技術的アプローチ

- `EnumValue` enum に `Expr(String)` バリアントを追加する
- `convert_ts_enum` の初期化子処理で `Expr::Bin` にマッチし、SWC AST のバイナリ式を Rust の式文字列に変換する
- Generator は既に `EnumValue::Number` で `repr` 属性付き出力を行っているため、`EnumValue::Expr` も同様のパスで出力する

### 影響範囲

- `src/transformer/mod.rs` — `convert_ts_enum` のマッチアーム追加
- `src/ir.rs`（または IR 定義ファイル）— `EnumValue::Expr(String)` 追加
- `src/generator/` — `EnumValue::Expr` の出力処理追加
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: `enum Flags { Read = 1 << 0, Write = 1 << 1 }` の変換テストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: `EnumValue::Expr(String)` バリアントを IR に追加
- [ ] ステップ3（GREEN）: `convert_ts_enum` で `Expr::Bin` を処理し、式文字列を生成
- [ ] ステップ4（GREEN）: Generator で `EnumValue::Expr` を出力
- [ ] ステップ5: Quality check

## テスト計画

- `enum Flags { Read = 1 << 0 }` → `Read = 1 << 0`（シフト演算）
- `enum Flags { Both = 1 | 2 }` → `Both = 1 | 2`（ビット OR）
- 回帰: 数値 enum（`A = 1`）が変更なく動作すること
- 回帰: 文字列 enum（`A = "a"`）が変更なく動作すること

## 完了条件

- ビット演算式を含む enum メンバーが Rust コードとして正しく出力される
- 既存の enum テストがすべてパスする
- `cargo test`, `cargo clippy`, `cargo fmt --check` が 0 エラー・0 警告
