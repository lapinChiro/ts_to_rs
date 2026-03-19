# Trait パラメータ型の変換

対象 TODO: I-187

## 背景・動機

I-137/I-127 で interface をメソッドの有無に応じて trait に変換するようになった。しかし、interface 型を関数パラメータや変数宣言で使用する場合、Rust では trait をそのまま値型として使えない（`Sized` でないため）。

現在 `function foo(g: Greeter)` は `fn foo(g: Greeter)` と出力されるが、`Greeter` が trait の場合これはコンパイルエラーになる。

## ゴール

1. 関数パラメータで interface（trait）型が使われている場合、`&dyn Trait` に変換される
2. 変数宣言で interface（trait）型が使われている場合、`Box<dyn Trait>` に変換される
3. 戻り値型で interface（trait）型が使われている場合、`Box<dyn Trait>` に変換される
4. フィールドのみ interface（struct）の場合は変更なし（値型のまま）
5. 既存テストに退行がない

## スコープ

### 対象

- TypeRegistry を参照し、型名が「メソッドを持つ interface」かどうかを判定するロジック
- 関数パラメータの型変換: `Greeter` → `&dyn Greeter`
- 変数宣言の型変換: `Greeter` → `Box<dyn Greeter>`
- 戻り値型の型変換: `Greeter` → `Box<dyn Greeter>`
- ユニットテスト + スナップショットテスト + E2E テスト

### 対象外

- ジェネリクス付き trait（`impl Trait<T>`）
- trait object の lifetime annotation（`&'a dyn Trait`）
- `Vec<dyn Trait>` の自動 `Box` 化

## 設計

### 判定ロジック

TypeRegistry で型名を検索し、`TypeDef::Struct` の `methods` が空でないか（= メソッドを持つ interface か）を判定する。

```rust
fn is_trait_type(name: &str, reg: &TypeRegistry) -> bool {
    if let Some(TypeDef::Struct { fields, methods, .. }) = reg.get(name) {
        !methods.is_empty() && fields.is_empty()
    } else {
        false
    }
}
```

注意: 混合 interface（フィールド + メソッド）の場合、struct 名は `{Name}Data` で trait 名は `{Name}`。型注釈に `{Name}` が使われた場合は trait への参照。

### 型変換

`convert_ts_type` 内で `Named` 型を生成する際に、trait 判定を行い:
- パラメータ位置: `RustType::Named { name: "dyn Greeter" }` + 参照で包む
- 変数/戻り値位置: `RustType::Named { name: "Box<dyn Greeter>" }`

あるいは IR レベルで新しい型表現を追加:
- `RustType::DynTrait { name }` → generator で `&dyn Name` / `Box<dyn Name>` に変換

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/types/mod.rs` | 型変換時の trait 判定・dyn 変換 |
| `src/ir.rs` | 必要に応じて `RustType` に新バリアント追加 |
| `src/generator/types.rs` | dyn trait 型の出力 |
| `src/transformer/expressions/tests.rs` | ユニットテスト |
| `tests/fixtures/` | スナップショットフィクスチャ |
| `tests/e2e/scripts/` | E2E スクリプト |

## 作業ステップ

- [ ] 1: trait 判定ヘルパーのユニットテスト（RED）
- [ ] 2: 判定ロジック実装（GREEN）
- [ ] 3: 関数パラメータの `&dyn Trait` 変換テスト（RED）
- [ ] 4: パラメータ型変換の実装（GREEN）
- [ ] 5: 変数宣言の `Box<dyn Trait>` 変換テスト（RED）
- [ ] 6: 変数型変換の実装（GREEN）
- [ ] 7: 戻り値型の変換テスト + 実装
- [ ] 8: E2E テスト
- [ ] 9: 退行チェック

## テスト計画

| テスト | 入力 | 期待出力 |
|-------|------|---------|
| パラメータ | `function foo(g: Greeter): void` | `fn foo(g: &dyn Greeter)` |
| 変数宣言 | `const g: Greeter = ...` | `let g: Box<dyn Greeter> = ...` |
| 戻り値 | `function make(): Greeter` | `fn make() -> Box<dyn Greeter>` |
| フィールドのみ interface | `function foo(p: Point): void` | `fn foo(p: Point)` （変更なし） |
| 混合 interface の struct | `function bar(d: GreeterData): void` | `fn bar(d: GreeterData)` （struct なので変更なし） |

## 完了条件

- [ ] trait 型が関数パラメータで `&dyn Trait` に変換される
- [ ] trait 型が変数宣言で `Box<dyn Trait>` に変換される
- [ ] trait 型が戻り値型で `Box<dyn Trait>` に変換される
- [ ] フィールドのみ interface は値型のまま変換される
- [ ] E2E テスト PASS
- [ ] 既存テストに退行がない
- [ ] clippy 0 警告、fmt PASS、全テスト PASS
