# クロージャ・関数呼び出し修正

対象 TODO: I-80, I-81, I-82

## 背景・動機

クロージャと関数呼び出しに関連する 3 件の変換バグがコンパイルエラーを引き起こす。

1. **I-80**: クロージャが外部変数を変更する場合、`let mut` が必要だが生成されない
2. **I-81**: optional chaining 内の JS メソッド名（`toUpperCase` 等）が Rust メソッド名にマッピングされない
3. **I-82**: 高階関数の引数に関数名を渡す際、`Box::new()` ラッピングが生成されない

## ゴール

- 可変キャプチャするクロージャのバインディングが `let mut` で生成される
- optional chaining 内のメソッド呼び出しが `map_method_call` を通り、正しい Rust メソッド名に変換される
- `Box<dyn Fn>` 型の引数に関数名を渡す際、`Box::new(fn_name)` が生成される
- 各修正に対応する E2E テストが PASS する

## スコープ

### 対象

- I-80: クロージャ変換（`src/transformer/expressions/mod.rs`）で外部変数への代入を検出し `let mut` を生成
- I-81: optional chaining 変換で `map` クロージャ内のメソッド呼び出しを `map_method_call` に通す
- I-82: 関数呼び出し変換（`src/transformer/expressions/mod.rs`）で `Box<dyn Fn>` 型パラメータへの引数を `Box::new()` でラップ

### 対象外

- クロージャの `FnMut` vs `Fn` の自動判定
- optional chaining の非メソッド呼び出し（プロパティアクセスは対応済み）
- `Rc<dyn Fn>` 等の他のスマートポインタ

## 設計

### 技術的アプローチ

#### I-80: 可変キャプチャ

クロージャが変数を変更するかは、クロージャの body 内に代入式 (`=`, `+=` 等) で外部変数名が左辺に出現するかで判定。出現する場合、クロージャの `let` バインディングを `let mut` にする。

#### I-81: optional chaining メソッドマッピング

`convert_optional_chaining` で生成する `map(|_v| _v.method())` のクロージャ内メソッド呼び出しを、`map_method_call` に通してメソッド名を変換する。

#### I-82: Box::new ラッピング

`convert_call_expr` で引数の型情報（TypeEnv/TypeRegistry）から `Box<dyn Fn>` 型を検出し、引数を `Box::new(arg)` でラップ。

### 影響範囲

- `src/transformer/expressions/mod.rs` — I-80, I-81, I-82
- `tests/e2e/scripts/closures.ts` — I-80 E2E 拡充
- `tests/e2e/scripts/optional_chaining.ts` — I-81 E2E 拡充
- `tests/e2e/scripts/functions.ts` — I-82 E2E 拡充

## 作業ステップ

- [ ] ステップ 1: I-80 — 可変キャプチャ検出 + `let mut` 生成。ユニットテスト + `closures.ts` E2E 拡充
- [ ] ステップ 2: I-81 — optional chaining メソッドマッピング。ユニットテスト + `optional_chaining.ts` E2E 拡充
- [ ] ステップ 3: I-82 — `Box::new()` ラッピング。ユニットテスト + `functions.ts` E2E 拡充
- [ ] ステップ 4: 全テスト退行チェック

## テスト計画

- **I-80**: クロージャ内で外部変数を変更するスクリプトがコンパイル・実行可能
- **I-81**: `s?.toUpperCase()` が `.to_uppercase()` として実行され正しい値を返す
- **I-82**: 関数を引数に渡す高階関数がコンパイル・実行可能

## 完了条件

- [ ] 3 件すべて修正済み、E2E テスト PASS
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] `cargo fmt --all --check` が PASS
- [ ] `cargo test` が全 PASS
