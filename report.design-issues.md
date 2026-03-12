# 設計上の根本課題 — 調査レポート

調査日: 2026-03-13

## 要約

TODO に記載された未対応構文の多くが、**同一の設計上のボトルネック**に起因している。個別の機能課題に見えるが、根本原因は 2 つに集約される:

1. **型コンテキストの欠如** — transformer が式を変換する際、周囲の型情報にアクセスできない
2. **1パス・ステートレスなパイプライン** — モジュール内の宣言（interface, enum, function）を参照する仕組みがない

## 課題 1: 型コンテキストの欠如

### 現状

`convert_expr(expr: &ast::Expr) -> Result<Expr>` は SWC の式ノードのみを受け取り、「この式がどの型として使われるか」を知らない。

### 影響を受ける TODO 項目

| TODO 項目 | 必要な型情報 |
|-----------|-------------|
| StringLit の `.to_string()` 付与 | 期待される型が `String` か `&str` か |
| オブジェクトリテラル: ネスト | 外側の struct のフィールド型（内側の struct 名） |
| 配列: 空配列の型推論 | 変数宣言の型注記から要素型を推論 |
| 配列: 文字列要素の `.to_string()` | `Vec<String>` が期待されているかどうか |

### 現状のワークアラウンド

- `convert_expr_with_type_hint(expr, type_hint: Option<&str>)` を追加（今回の object-literal 対応）
- `ensure_owned_string()` を `functions.rs` に追加（throw → `Ok()` ラップ時のみ）

これらは場当たり的な対応であり、新しい機能ごとにパラメータやヘルパーが増殖する構造になっている。

### 解決の方向性

`convert_expr` のシグネチャに「期待される型」を渡す仕組みを統一的に導入する:

```rust
pub fn convert_expr(expr: &ast::Expr, expected_type: Option<&RustType>) -> Result<Expr>
```

`type_hint: Option<&str>`（構造体名のみ）ではなく `Option<&RustType>`（完全な型情報）を渡すことで:
- `String` が期待される場所の `StringLit` → `Expr::MethodCall { "to_string" }` で包む
- `Vec<String>` が期待される場所の配列内文字列 → 同上
- ネストしたオブジェクトリテラル → `RustType::Named` からフィールド型を解決（課題 2 が前提）

## 課題 2: 1パス・ステートレスなパイプライン

### 現状

```
TS source → parser (SWC AST) → transformer (IR) → generator (Rust source)
```

`transform_module()` はモジュールの各アイテムを順に変換し、先に変換した結果を後のアイテムの変換に利用しない。つまり:

- 関数 `draw(p: Point)` の定義を見ても、呼び出し `draw({ x: 0, y: 0 })` の変換時にパラメータ型 `Point` を参照できない
- `interface Rect { origin: Origin; size: Size; }` を見ても、`Rect` のオブジェクトリテラル変換時にフィールド型 `Origin`/`Size` を参照できない
- `enum Color { Red, Green }` を見ても、`Color.Red` を `Color::Red` に変換すべきかどうか判別できない

### 影響を受ける TODO 項目

| TODO 項目 | 必要な参照先 |
|-----------|-------------|
| オブジェクトリテラル: 関数引数 | 関数宣言のパラメータ型 |
| オブジェクトリテラル: ネスト | struct（interface）定義のフィールド型 |
| enum メンバーアクセス | enum 宣言（何が enum で何が struct かの判別） |
| `console.log` 等の組み込み API | 組み込み関数の型定義（外部定義のルックアップ） |

### 解決の方向性

transformer の前に **型定義収集パス** を追加し、`TypeRegistry` を構築する:

```
TS source → parser → SWC AST → collect_types → TypeRegistry
                                                      ↓
                              SWC AST → transform(TypeRegistry) → IR → generate
```

`TypeRegistry` は `HashMap<String, TypeDef>` 程度の単純な構造:

```rust
enum TypeDef {
    Struct { fields: Vec<(String, RustType)> },
    Enum { variants: Vec<String> },
    Function { params: Vec<(String, RustType)>, return_type: Option<RustType> },
}
```

SWC AST を 1 回走査して interface/type alias/enum/function の型情報を収集し、2 回目の走査（transform）で参照する。

## 課題間の関係

```
課題 2 (TypeRegistry)
  ↓ struct/enum/fn の定義を提供
課題 1 (expected_type の伝播)
  ↓ 期待される型を式変換に渡す
┌─────────────────────────────────────────┐
│ StringLit .to_string()                  │
│ ネストしたオブジェクトリテラル            │
│ 関数引数のオブジェクトリテラル            │
│ enum メンバーアクセス                    │
│ 配列の型推論                            │
│ 組み込み API 変換                       │
└─────────────────────────────────────────┘
```

課題 1 は課題 2 がなくても一部（変数宣言の型注記からの推論）は改善できるが、フルに活用するには課題 2 が前提。

## 対応優先度の提案

### Phase 1: `expected_type: Option<&RustType>` の導入

- `convert_expr_with_type_hint(expr, Option<&str>)` を `convert_expr(expr, Option<&RustType>)` に統合
- `StringLit` + `String` 型期待 → `.to_string()` 自動付与
- 変数宣言の型注記からの推論を一般化
- **影響**: 比較的小さい変更。既存の `convert_expr` 呼び出し元すべてに `None` を渡すだけ

### Phase 2: `TypeRegistry` の導入

- `transform_module` を 2 パスに変更
- interface/type alias/enum/function の定義を事前収集
- `convert_expr` に `TypeRegistry` への参照を渡す
- **影響**: アーキテクチャ変更。transformer 全体のシグネチャが変わる

### Phase 3: 個別機能の実装

- TypeRegistry を活用して、関数引数の型推論、ネストしたオブジェクト、enum メンバーアクセス等を実装

## 判断のタイミング

Phase 1 は即座に着手可能（小さい変更で StringLit の問題を解決）。
Phase 2 は、Phase 1 だけでは解決できない機能（関数引数の型推論、enum メンバーアクセス）に着手する直前に実施する。
現時点では backlog の次のタスク（ternary-operator, unsupported-syntax-detection, break-continue）は TypeRegistry を必要としないため、急ぐ必要はない。
