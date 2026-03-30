# ts_to_rs

TypeScript コードを等価な Rust コードに変換する CLI ツール。

## インストール

```bash
cargo install --path .
```

## 使い方

```bash
# 単一ファイルの変換（出力は input.rs に書き出される）
ts-to-rs input.ts

# 出力先を指定
ts-to-rs input.ts -o output.rs

# ディレクトリの一括変換（全 .ts ファイルを変換し、mod.rs を自動生成）
ts-to-rs src/ -o out_rs/

# 未対応構文を検出して JSON レポートを stdout に出力
ts-to-rs input.ts --report-unsupported

# ビルトイン Web API 型定義を無効化
ts-to-rs input.ts --no-builtin-types

# 外部型定義の解決（Docker 経由で tsc を使用）
ts-to-rs resolve-types --tsconfig tsconfig.json
```

`--report-unsupported` を指定すると、未対応構文でエラー終了せず全ファイルを処理し、未対応構文の一覧を JSON で出力します:

```json
[
  { "kind": "ExportDefaultExpr", "location": "input.ts:8:1" }
]
```

### ディレクトリモード

ディレクトリを入力に指定すると、全 `.ts` ファイルを一括変換します:

- 全ファイルの型定義を共有レジストリに統合（クロスファイル型参照を解決）
- `ModuleGraph` が import/export を事前解析し、re-export チェーンを解決
- `mod.rs` をディレクトリ構造に基づいて自動生成
- ハイフン入りファイル名はアンダースコアに自動変換

## 対応する変換

### 型

| TypeScript | Rust |
|-----------|------|
| `interface` / `type` (オブジェクト型) | `struct` |
| `interface` (メソッドのみ) | `trait` |
| `interface` (混合: フィールド + メソッド) | `struct` + `trait` + `impl` |
| `interface Child extends Parent` | `trait` の継承 / 交差型 trait 合成 |
| `string`, `number`, `boolean` | `String`, `f64`, `bool` |
| `T \| null` / `T \| undefined` / `?` | `Option<T>` |
| `T[]` / `Array<T>` | `Vec<T>` |
| `[string, number]` (tuple 型) | `(String, f64)` |
| `type X = "a" \| "b"` (string literal union) | `enum X { A, B }` + `as_str()` |
| `type X = 200 \| 404` (numeric literal union) | `enum X { V200 = 200, V404 = 404 }` |
| `type X = string \| number` (primitive union) | `enum X { String(String), F64(f64) }` |
| discriminated union (`type \| "kind"` フィールド) | `enum` + match パターン |
| `A & B` (intersection type) | フィールド統合 `struct` |
| ジェネリクス (`<T>`, `<T extends U>`) | ジェネリクス (`<T>`, `<T: U>`) |
| `any` / `unknown` | `Box<dyn std::any::Any>` |
| `never` | `!` |
| `void` | `()` |
| `enum` (数値) | `enum` + `#[repr(i64)]` |
| `enum` (文字列) | `enum` + `as_str()` メソッド |
| `x as T` (type assertion) | `x`（assertion 除去） |
| `async function foo(): Promise<T>` | `async fn foo() -> T` |
| 関数型 (`(x: number) => number`) | `Box<dyn Fn(f64) -> f64>` |
| `(x: number) => void` (コールバック型) | `Box<dyn Fn(f64)>` |

### 文

| TypeScript | Rust |
|-----------|------|
| 関数宣言 | `fn` |
| デフォルト引数 (`x: number = 0`) | `Option<T>` + `unwrap_or(値)` |
| `const` / `let` | `let` / `let mut` |
| `if` / `else` | `if` / `else` |
| `while (cond) { ... }` | `while cond { ... }` |
| `for (const x of items) { ... }` | `for x in items { ... }` |
| `for (let i = 0; i < n; i++)` | `for i in 0..n { ... }` |
| `for (let i = n; i >= 0; i--)` (一般形) | `loop { if !(cond) { break; } ... }` |
| `do { body } while (cond)` | `loop { body; if !(cond) { break; } }` |
| `switch` / `case` | `match` |
| `break` / `continue` | `break` / `continue` |
| `break label` / `continue label` | `break 'label` / `continue 'label` |
| `label: for` / `label: while` (ラベル付きループ) | `'label: for` / `'label: while` |
| `throw new Error("msg")` | `return Err("msg".to_string())` |
| `try { ... } catch (e) { ... }` | try ブロック本体を展開 |
| `return` (Option 型関数内) | `Some()` で自動ラップ |
| `export` | `pub` |

### 式

| TypeScript | Rust |
|-----------|------|
| `foo(x, y)` | `foo(x, y)` |
| `obj.method(x)` | `obj.method(x)` |
| `new Foo(x)` | `Foo::new(x)` |
| テンプレートリテラル | `format!()` |
| `a > 0 ? a : b` (三項演算子) | `if a > 0.0 { a } else { b }` |
| `!x` / `-x` (単項演算子) | `!x` / `-x` |
| `===` / `!==` (厳密等価) | `==` / `!=` |
| `x ?? y` (nullish coalescing) | `x.unwrap_or_else(\|\| y)` |
| `x?.y` (optional chaining) | `x.as_ref().map(\|_v\| _v.y)` |
| `await expr` | `expr.await` |
| `const s: string = "hello"` | `let s: String = "hello".to_string()` |
| `/pattern/flags` (正規表現リテラル) | `Regex::new(r"pattern").unwrap()` |

### クラス

| TypeScript | Rust |
|-----------|------|
| `class` | `struct` + `impl` |
| `class Child extends Parent` | `struct` + `trait` + `impl Trait for Struct` |
| `abstract class` | `trait` |
| `super(args)` | 親フィールドの初期化 |
| `get foo(): T { ... }` | `fn foo(&self) -> T { ... }` |
| `set foo(v: T) { ... }` | `fn set_foo(&mut self, v: T) { ... }` |
| パラメータプロパティ (`constructor(public x: T)`) | フィールド + コンストラクタ代入 |

### リテラル・データ構造

| TypeScript | Rust |
|-----------|------|
| `[1, 2, 3]` (配列リテラル) | `vec![1.0, 2.0, 3.0]` |
| `{ x: 1, y: 2 }` (型注記付きオブジェクトリテラル) | `Point { x: 1.0, y: 2.0 }` |
| `{ x, y }` (shorthand property) | `Point { x, y }` |
| `{ origin: { x: 0, y: 0 } }` (ネストしたオブジェクト) | `Rect { origin: Origin { x: 0.0, y: 0.0 } }` |
| `draw({ x: 0, y: 0 })` (関数引数のオブジェクト) | `draw(Point { x: 0.0, y: 0.0 })` |
| `[...arr, 4]` (配列 spread) | `let mut v = Vec::new(); v.extend(arr...); v.push(4.0);` |
| `{...p, x: 10}` (オブジェクト spread、型注記付き) | `Point { x: 10.0, y: p.y }` |
| `const { x, y } = obj` (分割代入) | `let x = obj.x; let y = obj.y;` |
| `const { x: newX } = obj` (リネーム) | `let newX = obj.x;` |
| `const [a, b] = arr` (配列分割代入) | `let a = arr[0]; let b = arr[1];` |
| `Color.Red` (enum メンバーアクセス) | `Color::Red` |

### 型 narrowing

| TypeScript | Rust |
|-----------|------|
| `typeof x === "string"` | `if let` パターン |
| `x instanceof Foo` | `if let` パターン |
| `x !== null` / `x != null` | `if let Some(v) = x` |
| truthy チェック (`if (x)`) | `if let Some(v) = x` |
| `typeof x` (switch 文) | `match` + 型バリアント |
| 複合条件 (`&&`) | ネスト `if let` |
| `any` 型の narrowing | `enum` 自動生成 + `match` |

### 文字列メソッド

| TypeScript | Rust |
|-----------|------|
| `s.length` | `s.len() as f64` |
| `s.includes(x)` | `s.contains(x)` |
| `s.startsWith(x)` / `s.endsWith(x)` | `s.starts_with(x)` / `s.ends_with(x)` |
| `s.trim()` | `s.trim().to_string()` |
| `s.toLowerCase()` / `s.toUpperCase()` | `s.to_lowercase()` / `s.to_uppercase()` |
| `s.split(x)` | `s.split(x).collect::<Vec<&str>>()` |
| `s.replace(a, b)` | `s.replace(a, b)` |
| `s.replaceAll(a, b)` | `s.replace(a, b)` |
| `s.substring(a, b)` | `s[a..b].to_string()` |
| `s.slice(a, b)` | `s[a..b].to_string()` |

### 配列メソッド

| TypeScript | Rust |
|-----------|------|
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
| `arr.join(sep)` | `arr.join(sep)` |

### 正規表現メソッド

| TypeScript | Rust |
|-----------|------|
| `/pattern/` (リテラル) | `Regex::new(r"pattern").unwrap()` |
| `regex.test(str)` | `regex.is_match(str)` |
| `regex.exec(str)` | `regex.captures(str)` |
| `str.match(regex)` | `regex.captures(str)` |
| `str.replace(regex, repl)` | `regex.replace[_all](str, repl).to_string()` |

### 数値・グローバル関数

| TypeScript | Rust |
|-----------|------|
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
| `console.log(x)` | `println!("{:?}", x)` |
| `console.error(x)` / `console.warn(x)` | `eprintln!("{:?}", x)` |

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

**多パス AST-to-AST 変換パイプライン** を採用している。

```
TS source
  → Parser (SWC AST)
  → ModuleGraph (import/export 解析・re-export 解決)
  → TypeCollector + TypeConverter (TypeRegistry 構築)
  → TypeResolver (式の型・期待型・narrowing を事前計算)
  → Transformer (AST + 型情報 → IR)
  → Generator (IR → Rust ソースコード)
  → OutputWriter (ファイル出力・mod.rs 生成)
```

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
├── main.rs              # CLI エントリポイント
├── lib.rs               # ライブラリエントリポイント（transpile 関数）
├── parser.rs            # SWC で TS ファイルを AST 化
├── ir/                  # 中間表現の型定義
├── registry/            # TypeRegistry（型定義の事前収集）
├── external_types/      # ビルトイン型定義のロード・変換（バイナリ埋め込み JSON）
├── builtin_types/       # ビルトイン型 JSON（web_api.json + ecmascript.json）
├── directory.rs         # ディレクトリモードのユーティリティ
├── pipeline/            # 変換パイプラインの各パス
│   ├── module_resolver.rs       # import specifier → ファイルパス解決
│   ├── module_graph/            # ModuleGraph（import/export 解析・re-export 解決）
│   ├── type_converter/          # TS 型注釈 → RustType 変換
│   ├── type_resolver/           # 式の型・期待型・narrowing の事前計算
│   ├── type_resolution.rs       # FileTypeResolution（型解決結果のデータ構造）
│   ├── synthetic_registry.rs    # SyntheticTypeRegistry（合成型の重複排除）
│   ├── external_struct_generator/ # 外部型 struct 定義の自動生成
│   ├── any_narrowing.rs         # any 型の typeof/instanceof 制約収集ユーティリティ
│   ├── any_enum_analyzer.rs     # any-narrowing enum の分析
│   ├── output_writer.rs         # ファイル出力・mod.rs 生成
│   └── types.rs                 # パイプライン共通の型定義
├── transformer/         # AST → IR 変換
│   ├── mod.rs           # 変換エントリポイント
│   ├── context.rs       # TransformContext（変換コンテキスト）
│   ├── type_position.rs # TypePosition（型位置に基づく trait ラップ）
│   ├── classes/         # クラス変換
│   ├── functions/       # 関数変換
│   ├── statements/      # 文の変換（mutability 推論含む）
│   └── expressions/     # 式の変換
├── generator/           # IR → Rust ソースコード生成
│   ├── mod.rs           # 公開 API + Item 生成
│   ├── types.rs         # 型の生成
│   ├── statements/      # 文の生成
│   └── expressions/     # 式の生成
tests/
├── fixtures/            # 変換テスト用 .ts 入力ファイル（84 件）
├── snapshots/           # insta スナップショット（自動生成）
├── integration_test.rs  # スナップショットテスト
├── compile_test.rs      # 生成 Rust のコンパイル検証テスト
├── cli_test.rs          # CLI の統合テスト
├── e2e_test.rs          # E2E テスト（変換→コンパイル→実行→出力検証）
├── e2e/                 # E2E テスト用リソース
└── compile-check/       # コンパイルチェック用 Cargo プロジェクト
doc/
├── completed-features.md  # 完了済み機能一覧
└── design-decisions.md    # 設計判断の記録
scripts/
├── hono-bench.sh        # Hono フレームワーク変換率ベンチマーク
├── analyze-bench.py     # ベンチマーク結果のエラー分類・集計
├── bench_categories.py  # ベンチマークエラーカテゴリ定義
├── inspect-errors.py    # ベンチマークエラー詳細分析ツール
└── check-file-lines.sh  # .rs ファイル行数チェック（閾値 1000 行）
```

## 開発

```bash
cargo build                # ビルド
cargo test                 # テスト実行
cargo clippy --all-targets --all-features -- -D warnings  # lint
cargo fmt --all --check    # フォーマットチェック
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89  # カバレッジ計測
cargo llvm-cov --html      # HTML レポート（target/llvm-cov/html/）
./scripts/hono-bench.sh    # Hono 変換率ベンチマーク
```
