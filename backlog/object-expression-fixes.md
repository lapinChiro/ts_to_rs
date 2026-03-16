# オブジェクト初期化・式変換修正

対象 TODO: I-86, I-89

## 背景・動機

オブジェクト初期化と式変換に関連する 2 件の変換バグがコンパイルエラーを引き起こす。

1. **I-86**: オプショナルフィールドを省略した struct 初期化で `value: None` が生成されない
2. **I-89**: 数値と文字列の `+` 結合が Rust でコンパイルできない（`f64 + &str` は不正）

## ゴール

- オプショナルフィールドを省略した struct 初期化で `field: None` が自動補完される
- 数値と文字列の `+` が `format!` マクロに変換される
- 各修正に対応する E2E テストが PASS する

## スコープ

### 対象

- I-86: オブジェクトリテラル変換（`src/transformer/expressions/mod.rs`）で TypeRegistry から struct 定義を参照し、省略された `Option<T>` フィールドに `None` を補完
- I-89: 二項演算子変換（`src/transformer/expressions/mod.rs` の `convert_binary_with_string_concat`）で左辺が非文字列型・右辺が文字列型の場合に `format!` を生成

### 対象外

- オプショナルフィールドのデフォルト値（`value = 42` パターン — 別の問題）
- `null + "str"` や `boolean + "str"` 等の他の型との結合

## 設計

### 技術的アプローチ

#### I-86: Optional None 補完

`convert_object_lit` で struct 名が判明している場合、TypeRegistry から全フィールドを取得し、オブジェクトリテラルに含まれていない `Option<T>` フィールドに `field_name: None` を追加。

#### I-89: 数値 + 文字列結合

`convert_binary_with_string_concat` で、一方が文字列型で他方が非文字列型の場合、`format!("{}{}", left, right)` に変換。TypeEnv で型を判定。

### 影響範囲

- `src/transformer/expressions/mod.rs` — I-86, I-89
- `tests/e2e/scripts/object_ops.ts` — I-86 E2E 拡充
- `tests/e2e/scripts/` — I-89 E2E テスト

## 作業ステップ

- [ ] ステップ 1: I-86 — Optional None 補完。ユニットテスト + `object_ops.ts` / `destructuring.ts` E2E 拡充
- [ ] ステップ 2: I-89 — 数値 + 文字列 `format!` 変換。ユニットテスト + E2E テスト
- [ ] ステップ 3: 全テスト退行チェック

## テスト計画

- **I-86**: `{ name: "test" }` で `value?: number` 省略時に生成コードが `value: None` を含みコンパイル可能
- **I-89**: `x + "px"` が `format!("{}{}", x, "px")` に変換され正しい文字列を出力

## 完了条件

- [ ] 2 件すべて修正済み、E2E テスト PASS
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] `cargo fmt --all --check` が PASS
- [ ] `cargo test` が全 PASS
