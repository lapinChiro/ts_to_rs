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
│   ├── statements.rs   # 文の変換
│   └── expressions.rs  # 式の変換
├── generator.rs        # IR → Rust ソースコード生成
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
```
