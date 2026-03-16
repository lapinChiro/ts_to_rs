# 型ガードの式変換（I-45）

## 背景・動機

`typeof x === "string"` や `x instanceof ClassName` は TS で頻出する条件式パターンだが、現在は変換エラーになる。`typeof` は `UnaryOp::TypeOf` が未対応、`instanceof` は `BinaryOp::InstanceOf` が未対応のためである。

`resolve_bin_expr_type` では `InstanceOf` を `Bool` 型として認識しているが、式変換（`convert_binary_op`, `convert_unary_expr`）が対応していないため、型解決と式変換の間に不整合がある。

型推論基盤（I-38）が完了しており、TypeEnv で変数の型が取得できるため、`typeof` チェックを型情報ベースでコンパイル時解決できる下地がある。

## ゴール

1. `typeof x === "string"` パターンが Rust の条件式に変換される:
   - TypeEnv で `x` の型が判明している場合: コンパイル時に `true`/`false` に解決
   - 型が不明の場合: コメント付きのプレースホルダー
2. `typeof x` 単体（比較なし）が変換される
3. `x instanceof ClassName` が Rust の条件式に変換される
4. これらを含む条件式（if 文の条件、三項演算子の条件等）が変換エラーにならない
5. 既存テストに退行がない

## スコープ

### 対象

- `typeof x === "type"` パターンの変換（6 種: "string", "number", "boolean", "undefined", "object", "function"）
- `typeof x !== "type"` パターンの変換
- `typeof x` 単体の変換（比較式外で使われるケース）
- `x instanceof ClassName` の変換
- IR の `UnOp`/`BinOp` への新バリアント追加（必要に応じて）
- Generator での Rust コード生成

### 対象外

- 型 narrowing（typeof/instanceof 後の変数型の絞り込み）→ 別 PRD
- `in` 演算子の変換 → I-47 で対応
- ユーザー定義型ガード（`x is Type` 構文）→ 別 PRD
- typeof の結果を変数に保存して後で比較するパターン（`const t = typeof x; if (t === "string")`）

## 設計

### 技術的アプローチ

#### 1. `typeof x === "type"` パターン

TS の `typeof` は実行時にオペランドの型を文字列で返す。Rust にはこの直接対応がないため、型情報を活用してコンパイル時に解決する。

**パターン認識:** `convert_bin_expr` で、二項演算の片方が `typeof expr` で他方が文字列リテラルである場合を検出する。

**TypeEnv で型が判明している場合:**

| TS typeof 結果 | 対応する RustType | 変換結果 |
|---|---|---|
| `"string"` | `String` | `true` |
| `"number"` | `F64` | `true` |
| `"boolean"` | `Bool` | `true` |
| `"undefined"` | `Option(_)` | `x.is_none()` |
| `"object"` | `Named { .. }` / `Vec(_)` | `true` |
| `"function"` | `Fn { .. }` | `true` |

型が一致しない場合は `false`。`!==` の場合は結果を反転。

**TypeEnv で型が不明の場合:**

型に依存しない汎用的な表現として、コメント付きプレースホルダーを生成:
```rust
/* typeof x === "string" */ true
```

#### 2. `typeof x` 単体

typeof 単体（比較式の外）は Rust に直接対応がないため、型情報から文字列リテラルに解決:

```typescript
const t = typeof x;
```
→ TypeEnv で `x` が `String` なら:
```rust
let t = "string";
```

型が不明の場合:
```rust
let t = "unknown"; // TODO: typeof not resolved (type of x is unknown)
```

#### 3. `x instanceof ClassName`

`instanceof` は TS のプロトタイプチェーンベースの型チェック。Rust には直接対応がないが、TypeEnv の型情報を使って解決できる:

**TypeEnv で型が判明している場合:**
- `x` の型が `ClassName` と一致 → `true`
- `x` の型が `Option<ClassName>` → `x.is_some()`
- `x` の型が異なる → `false`

**型が不明の場合:**
```rust
/* x instanceof ClassName */ true
```

#### 4. IR の拡張

新しい IR ノードは不要。typeof/instanceof パターンは変換時に直接 `Expr::BoolLit`, `Expr::MethodCall`（`is_none`/`is_some`）, または `Expr::Comment` + `Expr::BoolLit` に解決する。

ただし `typeof x` 単体のために、`convert_unary_expr` で `UnaryOp::TypeOf` を処理する必要がある。

### 影響範囲

- `src/transformer/expressions/mod.rs` — `convert_bin_expr` にパターン認識追加、`convert_unary_expr` に typeof 追加
- `src/generator/expressions.rs` — 変更なし（既存の IR ノードで表現可能）
- `tests/` — 新規テスト追加
- `tests/fixtures/` — typeof/instanceof のスナップショットテスト追加

## 作業ステップ

- [ ] ステップ 1: typeof パターン認識のヘルパー関数
  - 二項演算で `typeof x === "type"` / `typeof x !== "type"` パターンを検出する関数
  - 左右どちらに typeof があっても検出（`"string" === typeof x` も対応）
  - テスト: パターン検出の単体テスト

- [ ] ステップ 2: typeof === の変換（型判明時）
  - TypeEnv で型が判明 → `true`/`false` にコンパイル時解決
  - 6 種の typeof 文字列と RustType の対応表
  - `!==` は結果反転
  - テスト: 各 typeof 文字列 × 一致/不一致

- [ ] ステップ 3: typeof === の変換（型不明時）
  - コメント付きプレースホルダーを生成
  - テスト: TypeEnv 空の状態での typeof 変換

- [ ] ステップ 4: typeof 単体の変換
  - `convert_unary_expr` で `UnaryOp::TypeOf` を処理
  - TypeEnv で型判明 → 対応する文字列リテラルに解決
  - 型不明 → `"unknown"` + TODO コメント
  - テスト: `typeof x` の各型パターン

- [ ] ステップ 5: instanceof の変換
  - `convert_binary_op` に `InstanceOf` を追加、または `convert_bin_expr` でパターン認識
  - TypeEnv で型判明 → `true`/`false` / `is_some()`
  - 型不明 → コメント付きプレースホルダー
  - テスト: instanceof の型一致/不一致/Option

- [ ] ステップ 6: 条件式との統合
  - if 文の条件で typeof/instanceof が使われるケースの統合テスト
  - テスト: `if (typeof x === "string") { ... }` の E2E 変換

## テスト計画

- **単体テスト**: typeof パターン認識（左右どちらでも検出）
- **単体テスト**: typeof === 変換（6 種 × 一致/不一致 × ===/!==）
- **単体テスト**: typeof 単体変換（各型 + 不明）
- **単体テスト**: instanceof 変換（一致/不一致/Option/不明）
- **統合テスト（スナップショット）**: typeof/instanceof を含む if 文の変換
- **回帰テスト**: 既存テスト全通過
- **境界値**: typeof のオペランドが複雑な式（`typeof obj.field`）

## 完了条件

1. `cargo test` 全テスト通過
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
3. `cargo fmt --all --check` 通過
4. `typeof x === "type"` の 6 種パターンが型判明時にコンパイル時解決される
5. `typeof x !== "type"` が正しく反転される
6. `typeof x` 単体が型情報から文字列リテラルに解決される
7. `instanceof` が TypeEnv の型情報で解決される
8. 型不明時にコメント付きプレースホルダーが生成される（変換エラーにならない）
9. 既存テストに退行がない
