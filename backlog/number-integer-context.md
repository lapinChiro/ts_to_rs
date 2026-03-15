# number の整数コンテキスト対応と parseInt のパニック回避

## 背景・動機

2 つの関連する問題:

1. **配列インデックス**: `arr[idx]` で `idx: f64` になり、Rust の `usize` が必要な箇所でコンパイル不可。TS の `number` は整数としても使われる
2. **parseInt/parseFloat のパニック**: `parseInt("abc")` が TS では `NaN` を返すが、Rust では `.parse::<f64>().unwrap()` でパニック

関連コード:
- `src/generator/expressions.rs` の Index 式生成
- `src/transformer/expressions/mod.rs` の parseInt/parseFloat 変換（412-414行目付近）

## ゴール

- 配列インデックスで `f64` が使われる場合、`as usize` を自動挿入する
- `parseInt` / `parseFloat` が `Option<f64>` を返す（`.ok()` パターン）か、`unwrap_or(f64::NAN)` を使用する

## スコープ

### 対象

- generator の Index 式で、index が `f64` の場合に `as usize` キャストを挿入
- `parseInt` / `parseFloat` の変換を `.parse::<f64>().unwrap_or(f64::NAN)` に変更

### 対象外

- number → i64/i32 への型推論（初版は f64 固定の方針を維持）
- 全ての整数コンテキストの自動検出（配列インデックスのみ対応）

## 設計

### 技術的アプローチ

**配列インデックス:**

generator の `Expr::Index` 生成で、index 式を `{expr} as usize` でラップする。IR レベルでは変更不要（generator の責務）。

**parseInt/parseFloat:**

`convert_expr` の `parseInt(s)` 変換を:
- 現在: `s.parse::<f64>().unwrap()`
- 変更後: `s.parse::<f64>().unwrap_or(f64::NAN)`

### 影響範囲

- `src/generator/expressions.rs` — Index 式の生成
- `src/transformer/expressions/mod.rs` — parseInt/parseFloat の変換
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: `arr[idx]` で `idx: f64` の場合に `as usize` が出力されるテスト追加
- [ ] ステップ2（GREEN）: generator の Index 式に `as usize` キャスト挿入
- [ ] ステップ3（RED）: `parseInt("abc")` が `NaN` を返す（パニックしない）テスト追加
- [ ] ステップ4（GREEN）: parseInt/parseFloat の変換を `unwrap_or(f64::NAN)` に変更
- [ ] ステップ5: Quality check

## テスト計画

- `arr[idx]` → `arr[idx as usize]`
- `parseInt("123")` → `"123".parse::<f64>().unwrap_or(f64::NAN)`
- `parseFloat("3.14")` → 同上
- 回帰: 既存の配列アクセス、parseInt テスト

## 完了条件

- 配列インデックスがコンパイル可能な Rust を生成する
- parseInt/parseFloat が無効入力でパニックしない
- 全テスト pass、0 errors / 0 warnings
