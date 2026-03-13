# パラメータ位置のインラインオブジェクト型リテラル対応

## 背景・動機

Hono では `constructor(init: { routers: Router<T>[] })` のようにパラメータの型注記にインラインオブジェクト型リテラルが使われている。現在、型エイリアスとしての `TsTypeLit`（`type Foo = { x: number }`）は対応済みだが、パラメータ型注記の位置では `convert_ts_type` に `TsTypeLit` の match arm がないため変換エラーになる。

## ゴール

`function foo(opts: { x: number, y: string }): void` が以下の Rust コードに変換される:

```rust
pub struct FooOpts {
    pub x: f64,
    pub y: String,
}

pub fn foo(opts: FooOpts) {}
```

型リテラルから自動生成された struct 名は `<関数名><パラメータ名>` のパスカルケースで命名される（例: `FooOpts`）。

## スコープ

### 対象

- パラメータの型注記にある `TsTypeLit` を検出し、名前付き struct を自動生成する
- 関数パラメータ・メソッドパラメータで動作する
- 生成される struct 名は `<関数名><パラメータ名>` のパスカルケースとする

### 対象外

- 戻り値型のインライン型リテラル
- ネストしたインライン型リテラル（`{ inner: { x: number } }`）
- 型リテラルのオプショナルメンバー（`{ x?: number }`）の特別な処理

## 設計

### 技術的アプローチ

1. `convert_ts_type` に `TsTypeLit` の match arm を追加するのではなく、関数パラメータの変換時（`convert_param` / `convert_param_pat`）に `TsTypeLit` を検出し、struct の `Item` を副作用として生成する
2. 生成された struct は関数の `Item` と一緒に返す必要があるため、変換コンテキストに「追加で生成された Item」を蓄積する仕組みが必要
3. 具体的には `convert_fn_decl` の戻り値を拡張するか、蓄積用の `Vec<Item>` を引数で渡す

### 影響範囲

- `src/transformer/types/mod.rs` — `TsTypeLit` → struct フィールドの変換ヘルパー追加
- `src/transformer/functions/mod.rs` — `convert_param` でインライン型リテラルを検出し struct 生成
- `src/transformer/mod.rs` — 生成された追加 Item の出力への統合

## 作業ステップ

- [ ] ステップ1（RED）: `function foo(opts: { x: number }): void` の変換テストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: `convert_param` で `TsTypeLit` を検出し、struct Item を生成する最小実装
- [ ] ステップ3: 生成された struct Item を出力に統合する仕組みの実装
- [ ] ステップ4（RED→GREEN）: 複数フィールドのケースに対応
- [ ] ステップ5: E2E テスト（fixture）を追加
- [ ] ステップ6（REFACTOR）: コードの整理

## テスト計画

- 正常系: 単一フィールド `{ x: number }` → struct 生成
- 正常系: 複数フィールド `{ x: number, y: string }` → struct 生成
- 正常系: 複数パラメータのうち一部だけが型リテラルの場合
- 正常系: メソッドパラメータでの動作
- 境界値: フィールドなし `{}` の場合

## 完了条件

- パラメータ位置の `TsTypeLit` が名前付き struct として変換される
- 生成された struct が関数定義の前に出力される
- `cargo fmt --all --check` / `cargo clippy` / `cargo test` が 0 エラー・0 警告
