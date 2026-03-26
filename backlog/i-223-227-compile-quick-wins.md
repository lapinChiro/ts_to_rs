# コンパイルエラーのクイックウィン（I-223 / I-227）

## 背景・動機

ディレクトリコンパイル成功率が 98.7%（156/158）で、残り 2 ファイルの失敗原因は I-227（enum 名に `::` 混入）の 1 箇所のみ。また I-223（文字列リテラルのエスケープ不正）はファイル単位コンパイルで 3 ファイルのエラー原因。

いずれも Generator の出力処理の欠陥であり、小規模な修正でコンパイル成功率を改善できる。OBJECT_LITERAL_NO_TYPE ロードマップの前に片付けるクイックウィン。

## ゴール

1. ディレクトリコンパイル成功率 100%（158/158）
2. 文字列リテラル内のバックスラッシュ・引用符が Rust の文字列リテラルとして正しくエスケープされる
3. union enum 名・バリアント名に `::` 等の不正な文字が含まれない

## スコープ

### 対象

- **I-227**: `variant_name_for_type` が `RustType::Named { name: "serde_json::Value" }` をそのまま返し、enum 名 `StringOrserde_json::Value` が不正な識別子になる
- **I-223**: `Expr::StringLit` の Generator 出力が値をそのまま `format!("\"{s}\"")` で出力し、バックスラッシュや引用符がエスケープされない

### 対象外

- I-236（余分な `}`）: 調査の結果、文字列リテラル内の波括弧が原因の誤検知。構造的な波括弧の不均衡は存在しない
- 正規表現リテラル（`Expr::Regex`）のエスケープ: 既に raw string `r"..."` で出力済み
- `Expr::FormatMacro` 内の文字列: フォーマット文字列は別の構造（`{template}`）で、`StringLit` とは独立

## 設計

### 技術的アプローチ

#### T1: I-227 — union enum 名の識別子サニタイズ

`src/pipeline/synthetic_registry.rs:296` の `variant_name_for_type` で `RustType::Named { name, .. }` のケースが `name.clone()` をそのまま返す。`serde_json::Value` のようなパス区切り付き型名が enum 名・バリアント名に混入する。

修正: `variant_name_for_type` で `::` を除去し、最後のセグメントのみを使用する（`serde_json::Value` → `Value`）。これは Rust の型名パスから識別子を導出する標準的なアプローチ。

同様に `RustType::DynTrait(name)` も `name.clone()` を返しており、`dyn Foo` のような名前が混入する可能性がある。合わせて修正する。

#### T2: I-223 — 文字列リテラルの Rust エスケープ

`src/generator/expressions.rs:113` の `Expr::StringLit(s) => format!("\"{s}\"")` が値をエスケープせず出力する。

SWC の `Str.value` はデコード済み（TS の `"\n"` は実際の改行文字 `0x0A` として格納）。Generator は IR の意味的な値を Rust の文字列リテラルとして正しくエスケープする責務を持つ。

修正: `escape_rust_string` ヘルパーを追加し、以下の文字をエスケープする:
- `\` → `\\`
- `"` → `\"`
- 改行 `\n`、復帰 `\r`、タブ `\t`
- その他の制御文字（`\0`、`\x00`-`\x1F`）

`Expr::StringLit` の出力を `format!("\"{}\"", escape_rust_string(s))` に変更する。

### 設計整合性レビュー

- **高次の整合性**: Generator の出力責務に沿った変更。IR は意味的な値を保持し、出力形式は Generator が決定するという設計原則を維持
- **DRY / 直交性**: `escape_rust_string` は Generator 内のプライベートヘルパー。他のエスケープ処理（`Expr::Regex` の raw string 等）とは独立
- **割れ窓**: `variant_name_for_type` の `DynTrait` も同様の問題を持つ可能性があり、合わせて修正する

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/pipeline/synthetic_registry.rs` | `variant_name_for_type` の `Named`/`DynTrait` ケースを修正 |
| `src/generator/expressions.rs` | `escape_rust_string` ヘルパー追加、`StringLit` 出力を修正 |

## タスク一覧

### T1: I-227 — union enum 名の識別子サニタイズ

- **作業内容**: `src/pipeline/synthetic_registry.rs:296` の `variant_name_for_type` で `RustType::Named { name, .. }` のケースを、`name` に `::` が含まれる場合は最後のセグメントのみ使用するように変更する。`RustType::DynTrait(name)` も同様に処理する
- **完了条件**: (1) `serde_json::Value` を含む union の enum 名が `ValueOrString` のような有効な識別子になる (2) バリアント名も `Value(serde_json::Value)` のように有効になる (3) 既存テストが全て通る (4) テスト追加
- **依存**: なし

### T2: I-223 — 文字列リテラルの Rust エスケープ

- **作業内容**: `src/generator/expressions.rs` に `escape_rust_string(s: &str) -> String` ヘルパーを追加。`Expr::StringLit` の出力（`:113`）を `format!("\"{}\"", escape_rust_string(s))` に変更する。エスケープ対象: `\` → `\\`、`"` → `\"`、`\n`、`\r`、`\t`、制御文字
- **完了条件**: (1) バックスラッシュを含む文字列が `\\` として出力される (2) 引用符を含む文字列が `\"` として出力される (3) 制御文字が正しくエスケープされる (4) 通常の文字列は影響を受けない (5) 既存テストが全て通る (6) テスト追加
- **依存**: なし

### T3: ベンチマーク検証

- **作業内容**: `./scripts/hono-bench.sh` を実行し、ディレクトリコンパイル 100% を確認。ファイル単位コンパイルの改善も記録。TODO / plan.md を最新化
- **完了条件**: (1) ディレクトリコンパイル 158/158 (100%) (2) リグレッションなし (3) ドキュメント最新化
- **依存**: T1, T2

## テスト計画

### T1 テスト

- `test_variant_name_for_named_type_with_path_separator_uses_last_segment`: `serde_json::Value` → `"Value"`
- `test_variant_name_for_named_type_without_path_separator_unchanged`: `String` → `"String"`
- `test_generate_union_name_with_path_type_produces_valid_identifier`: `string | serde_json::Value` → `"StringOrValue"`
- `test_variant_name_for_dyn_trait_extracts_name`: trait 名に `::` が含まれる場合

### T2 テスト

- `test_escape_rust_string_backslash`: `a\b` → `a\\b`
- `test_escape_rust_string_double_quote`: `say "hello"` → `say \"hello\"`
- `test_escape_rust_string_newline_tab`: 改行・タブ → `\n`、`\t`
- `test_escape_rust_string_plain_text_unchanged`: 通常文字列は変化なし
- `test_generate_string_lit_with_backslash`: `Expr::StringLit` の出力検証
- E2E テスト: バックスラッシュを含む文字列の変換・実行

## 完了条件

1. ディレクトリコンパイル成功率 100%（158/158）
2. `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
3. `cargo fmt --all --check` がパス
4. `cargo test` が全テストパス
5. Hono ベンチマークでリグレッションがない
