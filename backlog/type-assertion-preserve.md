# type assertion の型情報保持

## 背景・動機

`x as T` の type assertion で `as T` 部分が完全に削除され、型情報が失われる。TS で型を絞り込む用途のコードで、Rust 側で型推論が失敗しメソッド呼び出しがコンパイル不可になる。

関連コード: `src/transformer/expressions/mod.rs:44` — `ast::Expr::TsAs(ts_as) => convert_expr(&ts_as.expr, reg, expected)` で assertion を無視。

## ゴール

type assertion が Rust の型キャスト（`as T`）またはコメントとして保持される。

## スコープ

### 対象

- `convert_expr` の `TsAs` アームで assertion の型情報を IR に保持
- IR に `Expr::TypeCast { expr, ty }` バリアントを追加
- generator で `expr as T`（プリミティブ間キャスト）または型注記コメントを生成

### 対象外

- type guard（`typeof x === "string"` による型の絞り込み）
- `as const` assertions

## 設計

### 技術的アプローチ

TS の `x as T` には 2 つの用途がある:

1. **プリミティブ間キャスト**: `n as number` → Rust の `n as f64` に対応
2. **型の絞り込み**: `(response as JsonResponse).body` → Rust では不要（型推論 or turbofish）

方針:
- IR に `Expr::Cast { expr, ty }` を追加
- generator で `ty` がプリミティブなら `as T` を生成
- `ty` が Named 型なら、型注記コメント `/* as T */` を生成（Rust のダウンキャストは別の仕組み）
- `expected` 型情報として `ty` を内側の式変換に伝搬する（型推論の補助）

### 影響範囲

- `src/ir.rs` — `Expr::Cast` バリアント追加
- `src/transformer/expressions/mod.rs` — `TsAs` の変換ロジック
- `src/generator/expressions.rs` — `Cast` の生成
- テストファイル

## 作業ステップ

- [ ] ステップ1: IR に `Expr::Cast { expr, ty }` 追加
- [ ] ステップ2（RED）: `(x as number)` → `x as f64` のテスト追加
- [ ] ステップ3（GREEN）: プリミティブキャストの変換実装
- [ ] ステップ4（RED）: `(x as Foo).bar` で `expected` 型が伝搬されるテスト追加
- [ ] ステップ5（GREEN）: Named 型の type assertion で expected 伝搬
- [ ] ステップ6: Quality check

## テスト計画

- `x as number` → `x as f64`
- `x as string` → `x` + 型注記コメント
- `(response as JsonResponse).body` → `response.body`（expected 型伝搬）
- 回帰: 既存の type assertion テスト（assertion 削除パターン）

## 完了条件

- type assertion の型情報が IR に保持される
- プリミティブ間キャストがコンパイル可能な Rust を生成する
- 全テスト pass、0 errors / 0 warnings
