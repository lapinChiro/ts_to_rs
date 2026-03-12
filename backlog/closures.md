# クロージャ / 高階関数

## 背景・動機

TS ではアロー関数やコールバックが多用されるが、現在は変換できずエラーになる。`(x) => x + 1` のようなアロー関数を Rust のクロージャ `|x| x + 1` に変換し、関数の引数にコールバックを取るパターンにも対応する。

## ゴール

- アロー関数リテラルが Rust のクロージャに変換される
- 関数パラメータでのコールバック型（`(x: number) => number`）が `Fn` trait に変換される

## スコープ

### 対象

- アロー関数（expression body）: `(x: number) => x + 1` → `|x: f64| x + 1`
- アロー関数（block body）: `(x: number) => { return x + 1; }` → `|x: f64| { x + 1 }`
- 変数へのアロー関数代入: `const double = (x: number): number => x * 2;`
- 関数型パラメータ: `fn: (x: number) => number` → `fn: impl Fn(f64) -> f64`
- アロー関数を引数に渡す: `arr.map((x: number) => x + 1)`

### 対象外

- `function` キーワードによる関数式（`const f = function(x) { ... }`）
- `this` バインディングの違い（アロー関数 vs 通常関数）
- ジェネリックなクロージャ型（`<T>(x: T) => T`）
- 非同期アロー関数（`async (x) => ...`）

## 設計

### 技術的アプローチ

1. **IR の拡張**:
   - `Expr::Closure` を追加:
     ```rust
     Expr::Closure {
         params: Vec<Param>,
         return_type: Option<RustType>,
         body: ClosureBody,
     }
     ```
   - `ClosureBody` は `Expr`（expression body）または `Vec<Stmt>`（block body）
   - `RustType::Fn` を追加: `RustType::Fn { params: Vec<RustType>, return_type: Box<RustType> }`

2. **Transformer の拡張**:
   - `ast::Expr::Arrow` → `Expr::Closure` に変換
   - `TsFnType`（関数型）→ `RustType::Fn` に変換

3. **Generator の拡張**:
   - `Expr::Closure` → `|params| body` または `|params| { body }` を出力
   - `RustType::Fn` → `impl Fn(T1, T2) -> R` を出力

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/ir.rs` | `Expr::Closure`, `ClosureBody`, `RustType::Fn` 追加 |
| `src/transformer/expressions.rs` | アロー関数の変換 |
| `src/transformer/types.rs` | `TsFnType` → `RustType::Fn` の変換 |
| `src/generator.rs` | クロージャと関数型の出力 |
| `tests/fixtures/` | テスト fixture |

## 作業ステップ

- [ ] Step 1: IR に `RustType::Fn` を追加し、generator で `impl Fn(T) -> R` を出力
- [ ] Step 2: transformer で `TsFnType` → `RustType::Fn` を変換
- [ ] Step 3: IR に `Expr::Closure` と `ClosureBody` を追加
- [ ] Step 4: generator でクロージャの出力を実装（expression body / block body）
- [ ] Step 5: transformer で `ast::Expr::Arrow` → `Expr::Closure` を変換
- [ ] Step 6: E2E fixture テスト追加

## テスト計画

| # | 入力 | 期待出力 | 種別 |
|---|------|----------|------|
| 1 | `(x: number) => x + 1` | `\|x: f64\| x + 1` | expression body |
| 2 | `(x: number) => { return x + 1; }` | `\|x: f64\| { x + 1 }` | block body |
| 3 | `const double = (x: number): number => x * 2;` | `let double = \|x: f64\| -> f64 { x * 2.0 };` | 変数代入 |
| 4 | プロパティ `fn: (x: number) => number` | `fn_field: impl Fn(f64) -> f64` | 関数型パラメータ |
| 5 | パラメータなしアロー関数 `() => 42` | `\|\| 42.0` | 境界値 |

## 完了条件

- 上記テストが全パス
- 既存テスト全パス
- 生成コードがコンパイル検証テストを通る
- `cargo clippy` 0警告、`cargo fmt --check` 0エラー
