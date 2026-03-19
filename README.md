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

# 未対応構文を検出して JSON レポートを stdout に出力
ts-to-rs input.ts --report-unsupported
```

`--report-unsupported` を指定すると、未対応構文でエラー終了せず全ファイルを処理し、未対応構文の一覧を JSON で出力します:

```json
[
  { "kind": "ExportDefaultExpr", "location": "input.ts:8:1" }
]
```

## 対応する変換

| TypeScript | Rust |
|-----------|------|
| `interface` / `type` (オブジェクト型) | `struct` |
| `string`, `number`, `boolean` | `String`, `f64`, `bool` |
| `T \| null` / `T \| undefined` / `?` | `Option<T>` |
| `T[]` / `Array<T>` | `Vec<T>` |
| 関数宣言 | `fn` |
| デフォルト引数 (`x: number = 0`) | `Option<T>` + `unwrap_or(値)` |
| `const` / `let` | `let` / `let mut` |
| `if` / `else` | `if` / `else` |
| テンプレートリテラル | `format!()` |
| ジェネリクス (`<T>`, `<A, B>`) | ジェネリクス (`<T>`, `<A, B>`) |
| `class` | `struct` + `impl` |
| `class Child extends Parent` | `struct` + `trait` + `impl Trait for Struct` |
| `super(args)` | 親フィールドの初期化 |
| `get foo(): T { ... }` | `fn foo(&self) -> T { ... }` |
| `set foo(v: T) { ... }` | `fn set_foo(&mut self, v: T) { ... }` |
| アロー関数 (`(x) => x + 1`) | クロージャ (`\|x\| x + 1`) / `fn` |
| アロー関数 型注釈なし (`(x) => x + 1`) | `\|x\| x + 1` (型推論) |
| 関数型 (`(x: number) => number`) | `Box<dyn Fn(f64) -> f64>` |
| `foo(x, y)` | `foo(x, y)` |
| `obj.method(x)` | `obj.method(x)` |
| `new Foo(x)` | `Foo::new(x)` |
| `throw new Error("msg")` | `return Err("msg".to_string())` |
| `try { ... } catch (e) { ... }` | try ブロック本体を展開 |
| `while (cond) { ... }` | `while cond { ... }` |
| `for (const x of items) { ... }` | `for x in items { ... }` |
| `for (let i = 0; i < n; i++)` | `for i in 0..n { ... }` |
| `for (let i = n; i >= 0; i--)` (一般形) | `loop { if !(cond) { break; } ... }` |
| `do { body } while (cond)` | `loop { body; if !(cond) { break; } }` |
| `[1, 2, 3]` (配列リテラル) | `vec![1.0, 2.0, 3.0]` |
| `{ x: 1, y: 2 }` (型注記付きオブジェクトリテラル) | `Point { x: 1.0, y: 2.0 }` |
| `{ x, y }` (shorthand property) | `Point { x, y }` |
| `const { x, y } = obj` (分割代入) | `let x = obj.x; let y = obj.y;` |
| `const { x: newX } = obj` (リネーム) | `let newX = obj.x;` |
| `const [a, b] = arr` (配列分割代入) | `let a = arr[0]; let b = arr[1];` |
| `{ origin: { x: 0, y: 0 } }` (ネストしたオブジェクト) | `Rect { origin: Origin { x: 0.0, y: 0.0 } }` |
| `draw({ x: 0, y: 0 })` (関数引数のオブジェクト) | `draw(Point { x: 0.0, y: 0.0 })` |
| `[...arr, 4]` (配列 spread) | `let mut v = Vec::new(); v.extend(arr...); v.push(4.0);` |
| `{...p, x: 10}` (オブジェクト spread、型注記付き) | `Point { x: 10.0, y: p.y }` |
| `Color.Red` (enum メンバーアクセス) | `Color::Red` |
| `a > 0 ? a : b` (三項演算子) | `if a > 0.0 { a } else { b }` |
| `break` / `continue` | `break` / `continue` |
| `break label` / `continue label` | `break 'label` / `continue 'label` |
| `label: for` / `label: while` (ラベル付きループ) | `'label: for` / `'label: while` |
| `const s: string = "hello"` | `let s: String = "hello".to_string()` |
| `enum` (数値) | `enum` + `#[repr(i64)]` |
| `enum` (文字列) | `enum` + `as_str()` メソッド |
| `!x` / `-x` (単項演算子) | `!x` / `-x` |
| `function foo(): void {}` | `fn foo() {}` (戻り値型省略) |
| `(x: number) => void` (コールバック型) | `Box<dyn Fn(f64)>` |
| `async function foo(): Promise<T>` | `async fn foo() -> T` |
| `await expr` | `expr.await` |
| `console.log(x)` | `println!("{:?}", x)` |
| `console.error(x)` / `console.warn(x)` | `eprintln!("{:?}", x)` |
| `s.length` | `s.len() as f64` |
| `s.includes(x)` | `s.contains(x)` |
| `s.startsWith(x)` / `s.endsWith(x)` | `s.starts_with(x)` / `s.ends_with(x)` |
| `s.trim()` | `s.trim().to_string()` |
| `s.toLowerCase()` / `s.toUpperCase()` | `s.to_lowercase()` / `s.to_uppercase()` |
| `s.split(x)` | `s.split(x).collect::<Vec<&str>>()` |
| `s.replace(a, b)` | `s.replace(a, b)` |
| `arr.map(fn)` | `arr.iter().map(fn).collect::<Vec<_>>()` |
| `arr.filter(fn)` | `arr.iter().filter(fn).collect::<Vec<_>>()` |
| `arr.find(fn)` | `arr.iter().find(fn)` |
| `arr.some(fn)` / `arr.every(fn)` | `arr.iter().any(fn)` / `arr.iter().all(fn)` |
| `arr.forEach(fn)` | `arr.iter().for_each(fn)` |
| `arr.reduce(fn, init)` | `arr.iter().fold(init, fn)` |
| `arr.indexOf(x)` | `arr.iter().position(\|item\| *item == x)` |
| `arr.sort(fn)` | `arr.sort_by(fn)` |
| `arr.slice(a, b)` | `arr[a..b].to_vec()` |
| `arr.splice(a, n)` | `arr.drain(a..a+n).collect::<Vec<_>>()` |
| `Math.floor(x)` / `Math.ceil(x)` / `Math.round(x)` | `x.floor()` / `x.ceil()` / `x.round()` |
| `Math.abs(x)` / `Math.sqrt(x)` / `Math.trunc(x)` | `x.abs()` / `x.sqrt()` / `x.trunc()` |
| `Math.max(a, b)` / `Math.min(a, b)` | `a.max(b)` / `a.min(b)` |
| `Math.pow(x, y)` | `x.powf(y)` |
| `Math.sign(x)` / `Math.log(x)` | `x.signum()` / `x.ln()` |
| `Math.PI` / `Math.E` | `std::f64::consts::PI` / `std::f64::consts::E` |
| `parseInt(s)` / `parseFloat(s)` | `s.parse::<f64>().unwrap()` |
| `isNaN(x)` / `Number.isNaN(x)` | `x.is_nan()` |
| `Number.isFinite(x)` | `x.is_finite()` |
| `Number.isInteger(x)` | `x.fract() == 0.0` |
| `[string, number]` (tuple 型) | `(String, f64)` |
| `type X = "a" \| "b"` (string literal union) | `enum X { A, B }` + `as_str()` |
| `type X = 200 \| 404` (numeric literal union) | `enum X { V200 = 200, V404 = 404 }` |
| `type X = string \| number` (primitive union) | `enum X { String(String), F64(f64) }` |
| `any` / `unknown` | `Box<dyn std::any::Any>` |
| `never` | `!` |
| `x as T` (type assertion) | `x`（assertion 除去） |
| `x ?? y` (nullish coalescing) | `x.unwrap_or_else(\|\| y)` |
| `x?.y` (optional chaining) | `x.as_ref().map(\|_v\| _v.y)` |
| `===` / `!==` (厳密等価) | `==` / `!=` |
| `export` | `pub` |

### 変換不可能な構文

以下の TypeScript 構文は Rust の型システム上で等価な表現が存在しないため、変換できない。

| TypeScript | 理由 |
|-----------|------|
| `==` / `!=` (抽象等価) | JS の型強制（`1 == "1"` → `true`）は Rust の静的型システムで表現不可能。`===` を使用すること |

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
├── registry.rs         # TypeRegistry（型定義の事前収集）
├── transformer/        # AST → IR 変換
│   ├── mod.rs          # 変換エントリポイント
│   ├── types/          # 型変換 (TS型 → Rust型)
│   ├── functions/      # 関数変換
│   ├── classes.rs      # クラス変換
│   ├── statements/     # 文の変換
│   └── expressions/    # 式の変換
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
