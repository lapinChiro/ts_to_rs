# 型エイリアスパターンの変換拡張

対象 TODO: I-25, I-37

## 背景・動機

TYPE_ALIAS_UNSUPPORTED（48 インスタンス）は Hono ベンチマーク第 2 のエラーカテゴリ。`type` 宣言の body で未対応の SWC 型パターンが残存している。

最大は条件型 `A extends B ? C : D`（Discriminant 3、22 件）。条件型は TypeScript の型レベルプログラミングの中核機能であり、Hono で多用される。

I-37（TYPE_LITERAL_MEMBER、2 インスタンス）は型リテラル内のメソッドシグネチャが未対応。I-137 で名前付き型参照の交差型は対応済みだが、インライン型リテラル `type X = { foo(): string }` は未対応。同じ型エイリアス変換コードパスのため同時に対応する。

## ゴール

1. 条件型 `A extends B ? C : D` が Rust の型に変換される（22 + 5 = 27 件）
2. `keyof typeof X` 演算子が変換される（1 件）
3. 型リテラル内のメソッドシグネチャが trait メソッドとして変換される（2 件）
4. Hono ベンチマークの TYPE_ALIAS_UNSUPPORTED が 48 から大幅に減少する
5. 既存テストに退行がない

## スコープ

### 対象

- 条件型 `A extends B ? C : D` → Rust の型変換（True 分岐を採用するヒューリスティクス or 型パラメータ境界として表現）
- 条件型の亜種 Discriminant(15) の対応
- `keyof typeof X` → 型リテラルのキー union への変換
- 型リテラル内のメソッドシグネチャ → trait メソッド
- ユニットテスト + スナップショットテスト

### 対象外

- マップ型 `{ [K in keyof T]: V }` — スコープ外（9 + 6 = 15 件、proc macro が必要）
- テンプレートリテラル型 — スコープ外（5 件）
- 条件型のネスト（`A extends B ? C extends D ? E : F : G`）
- infer キーワード（`T extends Array<infer U> ? U : never`）

## 設計

### 条件型の変換戦略

TypeScript の条件型 `T extends U ? X : Y` は Rust に直接対応する構文がない。変換戦略:

1. **具体型が判明する場合**: tsc が型を解決済み（ビルトイン型経由）なら、解決後の具体型を使用
2. **ジェネリック型パラメータの場合**: `T` が unresolved のとき、True 分岐（`X`）をデフォルトとして採用。理由: 条件型はほとんどの場合「型がマッチする前提」で使われる
3. **never 分岐**: False 分岐が `never` の場合、True 分岐のみを出力

### keyof typeof の変換

`keyof typeof X` → `X` のフィールド名を文字列 union として列挙。TypeRegistry から `X` の TypeDef を取得し、フィールド名を文字列リテラル union に変換。

### 型リテラルのメソッドシグネチャ

`type X = { foo(): string }` → メソッドが存在する場合 trait として変換（I-137 で interface に実装済みの 3 分類ロジックを型リテラルにも適用）。

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/types/mod.rs` | 条件型変換、keyof typeof、型リテラルメソッド対応 |
| `src/transformer/types/tests.rs` | ユニットテスト |
| `tests/fixtures/` | スナップショットフィクスチャ |

## 作業ステップ

- [ ] 1: 条件型の基本変換テスト（RED）— `T extends string ? T : never` → `T` (True 分岐)
- [ ] 2: 条件型変換の実装（GREEN）
- [ ] 3: Discriminant(15) 亜種のテストと実装
- [ ] 4: `keyof typeof` のテスト（RED）
- [ ] 5: `keyof typeof` の実装（GREEN）
- [ ] 6: 型リテラルメソッドのテスト（RED）
- [ ] 7: 型リテラルメソッドの実装（GREEN）
- [ ] 8: Hono ベンチマーク実行、TYPE_ALIAS_UNSUPPORTED の変化確認
- [ ] 9: 退行チェック

## テスト計画

| テスト | 入力 | 期待出力 |
|-------|------|---------|
| 条件型 True | `type X = string extends string ? number : boolean` | `type X = f64` |
| 条件型 パラメータ | `type X<T> = T extends string ? T : never` | `type X<T> = T`（True 分岐） |
| 条件型 never | `type X = number extends string ? number : never` | `type X = Never`（False 分岐が具体型、True が never でない場合） |
| keyof typeof | `const obj = { a: 1, b: 2 }; type K = keyof typeof obj` | 文字列 union |
| 型リテラルメソッド | `type X = { foo(): string }` | `trait X { fn foo(&self) -> String; }` |

## 完了条件

- [ ] 条件型（Discriminant 3/15）が変換される
- [ ] `keyof typeof` が変換される
- [ ] 型リテラルのメソッドシグネチャが trait に変換される
- [ ] Hono ベンチの TYPE_ALIAS_UNSUPPORTED が 48 から減少
- [ ] TYPE_LITERAL_MEMBER が 2 から 0 になる
- [ ] 既存テストに退行がない
- [ ] clippy 0 警告、fmt PASS、全テスト PASS
