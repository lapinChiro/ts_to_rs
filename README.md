# ts_to_rs

TypeScript コードを等価な Rust コードに変換する CLI ツール。

## インストール

```bash
cargo install --path .
```

## 使い方

```bash
# 基本的な使い方（出力は input.rs に書き出される）
ts-to-rs input.ts

# 出力先を指定
ts-to-rs input.ts -o output.rs
```

## 対応する変換

| TypeScript | Rust |
|-----------|------|
| `interface` / `type` (オブジェクト型) | `struct` |
| `string`, `number`, `boolean` | `String`, `f64`, `bool` |
| `T \| null` / `T \| undefined` / `?` | `Option<T>` |
| `T[]` / `Array<T>` | `Vec<T>` |
| 関数宣言 | `fn` |
| `const` / `let` | `let` / `let mut` |
| `if` / `else` | `if` / `else` |
| テンプレートリテラル | `format!()` |
| ジェネリクス (`<T>`, `<A, B>`) | ジェネリクス (`<T>`, `<A, B>`) |
| `class` | `struct` + `impl` |
| アロー関数 (`(x) => x + 1`) | クロージャ (`\|x\| x + 1`) / `fn` |
| 関数型 (`(x: number) => number`) | `Box<dyn Fn(f64) -> f64>` |
| `foo(x, y)` | `foo(x, y)` |
| `obj.method(x)` | `obj.method(x)` |
| `new Foo(x)` | `Foo::new(x)` |
| `throw new Error("msg")` | `return Err("msg".to_string())` |
| `try { ... } catch (e) { ... }` | try ブロック本体を展開 |
| `while (cond) { ... }` | `while cond { ... }` |
| `for (const x of items) { ... }` | `for x in items { ... }` |
| `for (let i = 0; i < n; i++)` | `for i in 0..n { ... }` |
| `[1, 2, 3]` (配列リテラル) | `vec![1.0, 2.0, 3.0]` |
| `{ x: 1, y: 2 }` (型注記付きオブジェクトリテラル) | `Point { x: 1.0, y: 2.0 }` |
| `enum` (数値) | `enum` + `#[repr(i64)]` |
| `enum` (文字列) | `enum` + `as_str()` メソッド |
| `export` | `pub` |

## 例

入力 (TypeScript):

```typescript
interface User {
    name: string;
    age: number;
    active: boolean;
}

function greet(user: User): string {
    return `Hello, ${user.name}`;
}
```

出力 (Rust):

```rust
pub struct User {
    pub name: String,
    pub age: f64,
    pub active: bool,
}

pub fn greet(user: User) -> String {
    format!("Hello, {}", user.name)
}
```

## 技術的アプローチ

**AST-to-AST変換** を採用している。

1. TSの解析: [SWC](https://swc.rs/) の Rust クレートで TS ソースを AST 化
2. 変換: SWC AST → 独自 IR（中間表現）
3. コード生成: IR → Rust ソースコード文字列

| 項目 | 技術 |
|------|------|
| 言語 | Rust |
| TS解析 | swc_ecma_parser + swc_ecma_ast |
| CLI | clap |
| テスト | cargo test + insta (スナップショット) |
| Lint | clippy |
| フォーマット | rustfmt |

## ディレクトリ構成

```
src/
├── main.rs             # CLIエントリポイント
├── lib.rs              # ライブラリエントリポイント（transpile 関数）
├── parser.rs           # SWCでTSファイルをAST化
├── transformer/        # AST → IR 変換
│   ├── mod.rs          # 変換エントリポイント
│   ├── types.rs        # 型変換 (TS型 → Rust型)
│   ├── functions.rs    # 関数変換
│   ├── classes.rs      # クラス変換
│   ├── statements.rs   # 文の変換
│   └── expressions.rs  # 式の変換
├── generator/          # IR → Rust ソースコード生成
│   ├── mod.rs          # 公開 API + Item 生成
│   ├── types.rs        # 型の生成
│   ├── statements.rs   # 文の生成
│   └── expressions.rs  # 式の生成
└── ir.rs               # 中間表現の型定義
tests/
├── fixtures/           # 変換テスト用 .ts 入力ファイル
├── snapshots/          # insta スナップショット（自動生成）
└── integration_test.rs # E2Eテスト（insta snapshot）
```

## 開発

```bash
cargo build          # ビルド
cargo test           # テスト実行
cargo clippy         # lint
cargo fmt --check    # フォーマットチェック
cargo llvm-cov --fail-under-lines 85   # カバレッジ計測（閾値85%）
cargo llvm-cov --html                  # HTMLレポート（target/llvm-cov/html/）
```
