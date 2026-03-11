# ts_to_rs 開発計画

## プロジェクト概要

TypeScriptコードを等価なRustコードに変換するCLIツール。
開発はTSで素早く行い、デプロイ時にRustに変換することで、開発効率と実行性能を両立する。

## 技術的アプローチ

**AST-to-AST変換** を採用する。

- TSの解析: [SWC](https://swc.rs/) の Rust クレートを直接使用
  - SWC自体がRust製のため、ネイティブAPIを直接利用でき最も高速
  - `swc_ecma_parser` でTS→AST、`swc_ecma_ast` でAST型を利用
- Rustコード生成: AST → 独自IR → Rust ソースコード文字列を生成
  - 初版では文字列テンプレートで生成（シンプルに始める）
  - 必要に応じて後から `syn`/`quote` ベースのAST生成に移行

## 技術スタック

| 項目 | 技術 |
|------|------|
| 言語 | Rust |
| TS解析 | swc_ecma_parser + swc_ecma_ast |
| CLI | clap |
| テスト | cargo test + insta (スナップショット) |
| Lint | clippy |
| フォーマット | rustfmt |

## フェーズ1: 初版プロトタイプ（現在のスコープ）

### 目標

「単一TSファイルの基本構造をRustに変換できること」を示すプロトタイプ。
完全性より**変換パイプラインが動くこと**を優先する。

### 変換対象（初版スコープ）

以下の TS 構文を Rust に変換する:

| # | TypeScript | Rust | 優先度 |
|---|-----------|------|--------|
| 1 | `type` / `interface` (プレーンオブジェクト型) | `struct` | 高 |
| 2 | プリミティブ型 (`string`, `number`, `boolean`) | `String`, `f64`, `bool` | 高 |
| 3 | `T \| null` / `T \| undefined` / `?` | `Option<T>` | 高 |
| 4 | 関数宣言（純粋関数） | `fn` | 高 |
| 5 | `const` / `let` 変数宣言 | `let` / `let mut` | 高 |
| 6 | `if` / `else` | `if` / `else` | 高 |
| 7 | 配列型 `T[]` | `Vec<T>` | 中 |
| 8 | `enum`（文字列enum） | `enum` + `Display`/`FromStr` | 中 |
| 9 | テンプレートリテラル | `format!()` | 中 |
| 10 | `export` | `pub` | 中 |

### 初版で対象外とするもの

- クラス（メソッド、継承）
- 非同期処理（async/await）
- ジェネリクス
- モジュールシステム（import/export の解決）
- 外部ライブラリの型マッピング
- エラーハンドリング（try/catch → Result）
- クロージャ / 高階関数
- 所有権・ライフタイムの最適化（初版は全て clone で逃げる）

### ディレクトリ構成

[README.md](README.md#ディレクトリ構成) を参照。

追加ファイル:
├── Cargo.toml
├── CLAUDE.md
└── plan.md
```

### 作業ステップ

#### Step 0: 開発環境整備 ✅ 完了
- [x] CLAUDE.md をRust用に書き換え
- [x] `.claude/rules/` をRust用に書き換え（testing.md, quality-check.md, tdd.md）

#### Step 1: プロジェクト基盤セットアップ ✅ 完了
- [x] Cargo.toml（swc_ecma_parser 35, swc_ecma_ast 21, swc_common 19, clap 4, insta 1, anyhow 1）
- [x] `src/main.rs`, `src/lib.rs` の雛形作成
- [x] `cargo build` が通ることを確認
- [x] `.gitignore` にRust用設定を追加

#### Step 2: IR（中間表現）の型定義 ✅ 完了
- [x] Rustコードを表現するための IR 型を定義（enum / struct）
  - `RustType`: String, F64, Bool, Option, Vec, Named（ユーザー定義型用に追加）
  - `Item`: Struct, Enum, Fn
  - `Stmt`: Let, If, Return, Expr
  - `Expr`: NumberLit, BoolLit, StringLit, Ident, FormatMacro, BinaryOp
- [x] テスト: IR型が必要な構造を表現できることを確認（15テスト）

#### Step 3: 型変換 ✅ 完了
- [x] テスト: `interface Foo { name: string; age: number; }` → `struct Foo { name: String, age: f64 }`
- [x] SWC AST の型宣言ノード → IR 変換を実装
- [x] プリミティブ型マッピング（string→String, number→f64, boolean→bool）
- [x] Optional (`?` / `| null` / `| undefined`) → `Option<T>`（二重ラップ防止込み）
- [x] 配列型 `T[]` / `Array<T>` → `Vec<T>`
- [x] ユーザー定義型参照 → `RustType::Named`

#### Step 4: 関数変換 ✅ 完了
- [x] テスト: `function add(a: number, b: number): number { return a + b; }` → `fn add(a: f64, b: f64) -> f64 { a + b }`
- [x] 関数シグネチャの変換
- [x] 基本的な式・文の変換（return, 変数宣言, if/else）
- [x] 二項演算（算術・比較・論理演算子）
- [x] テンプレートリテラル → `format!()`

#### Step 5: コード生成 ✅ 完了
- [x] テスト: IR → Rust ソースコード文字列（29テスト）
- [x] インデント（4スペース）、セミコロン、ブレースの整形
- [x] 関数末尾の return を Rust の末尾式（セミコロンなし）に変換

#### Step 6: E2Eパイプライン結合 ✅ 完了
- [x] `lib.rs` に `transpile()` 公開関数を追加（parse → transform → generate）
- [x] `export` 宣言の処理を追加
- [x] テスト: `.ts` ファイル入力 → `.rs` ファイル出力
- [x] fixture ベースのスナップショットテスト（insta、4 fixture）

#### Step 7: CLI実装 ✅ 完了
- [x] `ts-to-rs <input.ts> -o <output.rs>` コマンド（clap）
- [x] `-o` 省略時はデフォルトで `<input>.rs` に出力
- [x] エラーメッセージ（anyhow の Context 付き）、ヘルプ表示

#### Step 8: 品質チェック・リリース準備 ✅ 完了
- [x] clippy 0警告、fmt 0エラー、95テスト全パス
- [x] README.md 更新

## フェーズ2以降（将来）

- クラス → struct + impl 変換
- async/await → tokio 変換
- import/export → mod / use 変換
- ジェネリクス対応
- 所有権推論（clone 削減）
- 複数ファイル一括変換
- Cargo.toml 自動生成
- `rustfmt` 連携
- Watch モード

## 判断保留事項

以下は初版のフィードバックを受けてから決定する:

1. **number の変換先**: `f64` 固定か、用途に応じて `i32`/`i64`/`f64` を推論するか
2. **String vs &str**: 初版は全て `String`。パフォーマンス最適化は後回し
3. **エラー表現**: `throw` → `panic!` か `Result` か
4. **所有権モデル**: 初版は全 clone。借用の推論は将来課題
