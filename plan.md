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

```
ts_to_rs/
├── src/
│   ├── main.rs             # CLIエントリポイント
│   ├── lib.rs              # ライブラリエントリポイント
│   ├── parser.rs           # SWCでTSファイルをAST化
│   ├── transformer/
│   │   ├── mod.rs          # AST → IR 変換のエントリ
│   │   ├── types.rs        # 型変換 (TS型 → Rust型)
│   │   ├── functions.rs    # 関数変換
│   │   ├── statements.rs   # 文の変換
│   │   └── expressions.rs  # 式の変換
│   ├── generator.rs        # IR → Rust ソースコード生成
│   └── ir.rs               # 中間表現の型定義
├── tests/
│   ├── fixtures/           # 変換テスト用の .ts / .rs ペア
│   │   ├── basic-types.input.ts
│   │   ├── basic-types.expected.rs
│   │   └── ...
│   └── integration_test.rs # E2Eテスト
├── Cargo.toml
├── CLAUDE.md
└── plan.md
```

### 作業ステップ

#### Step 0: 開発環境整備 ✅ 完了
- [x] CLAUDE.md をRust用に書き換え
- [x] `.claude/rules/` をRust用に書き換え（testing.md, quality-check.md, tdd.md）

#### Step 1: プロジェクト基盤セットアップ
- [ ] Cargo.toml（swc_ecma_parser, swc_ecma_ast, clap, insta, anyhow 等）
- [ ] `src/main.rs`, `src/lib.rs` の雛形作成
- [ ] `cargo build` が通ることを確認
- [ ] `.gitignore` にRust用設定を追加

#### Step 2: IR（中間表現）の型定義 — RED → GREEN
- [ ] Rustコードを表現するための IR 型を定義（enum / struct）
- [ ] テスト: IR型が必要な構造を表現できることを確認

#### Step 3: 型変換 — RED → GREEN
- [ ] テスト: `interface Foo { name: string; age: number; }` → `struct Foo { name: String, age: f64 }`
- [ ] SWC AST の型宣言ノード → IR 変換を実装
- [ ] プリミティブ型マッピング
- [ ] Optional (`?` / `| null`) → `Option<T>`

#### Step 4: 関数変換 — RED → GREEN
- [ ] テスト: `function add(a: number, b: number): number { return a + b; }` → `fn add(a: f64, b: f64) -> f64 { a + b }`
- [ ] 関数シグネチャの変換
- [ ] 基本的な式・文の変換（return, 変数宣言, if/else）

#### Step 5: コード生成 — RED → GREEN
- [ ] テスト: IR → Rust ソースコード文字列
- [ ] インデント、セミコロン、ブレースの整形

#### Step 6: E2Eパイプライン結合
- [ ] テスト: `.ts` ファイル入力 → `.rs` ファイル出力
- [ ] fixture ベースのスナップショットテスト（insta）

#### Step 7: CLI実装
- [ ] `ts-to-rs <input.ts> -o <output.rs>` コマンド（clap）
- [ ] エラーメッセージ、ヘルプ表示

#### Step 8: 品質チェック・リリース準備
- [ ] clippy 0警告、テスト全パス
- [ ] README.md 更新

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
