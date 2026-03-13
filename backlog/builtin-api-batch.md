# 組み込み API の特殊変換 一括追加

## 背景・動機

Hono のソースコードでは `.reduce()`, `.indexOf()`, `.join()`, `.reverse()`, `.sort()`, `.slice()`, `.splice()`, `Math.PI`, `Math.sign()`, `Math.trunc()`, `Math.log()`, `Number.isInteger()` 等の組み込み API が使われている。現在これらは未対応のため変換時にエラーになるか、メソッド名がそのまま出力される。

## ゴール

以下の 10 カテゴリの組み込み API が正しい Rust コードに変換される:

1. `.reduce(fn, init)` → `.iter().fold(init, fn)`
2. `.indexOf(s)` → `.iter().position(|x| x == s)` （配列）/ `.find(s)` （文字列）
3. `.join(sep)` → `.join(sep)`
4. `.reverse()` → `.reverse()` （in-place）/ `.iter().rev().collect()` （非破壊）
5. `.sort()` → `.sort()` / `.sort_by(fn)`
6. `.slice(a, b)` → `[a..b].to_vec()`
7. `.splice(start, count)` → `drain(start..start+count).collect()`
8. `Math.PI` 等の定数 → `std::f64::consts::PI`
9. `Math.sign/trunc/log` → `.signum()` / `.trunc()` / `.ln()`
10. `Number.isInteger(x)` → `x.fract() == 0.0`

## スコープ

### 対象

- 上記 10 カテゴリの変換ルール追加
- 各変換に対するユニットテストと E2E テスト

### 対象外

- `.substring(a, b)` / `.slice(a, b)`（文字列版）— UTF-8 バイト境界問題があり別途検討
- `.reduce()` のコールバック関数の型推論
- `.sort()` の比較関数の完全な変換（基本パターンのみ対応）

## 設計

### 技術的アプローチ

既存のアーキテクチャに従い、以下の 2 つの拡張ポイントに変換ルールを追加する:

1. **`map_method_call`**（`expressions/mod.rs:454`）: `.reduce`, `.indexOf`, `.join`, `.reverse`, `.sort`, `.slice`, `.splice` の match arm を追加
2. **`convert_math_call`**（`expressions/mod.rs:670`）: `sign`, `trunc`, `log` の match arm を追加
3. **`convert_call_expr`** のメンバーアクセス部分: `Math.PI` 等の定数参照を検出し `std::f64::consts::PI` に変換
4. **`convert_number_method`**: `isInteger` の match arm を追加

### 影響範囲

- `src/transformer/expressions/mod.rs` — `map_method_call`, `convert_math_call`, `convert_call_expr`, `convert_number_method` の拡張

## 作業ステップ

- [ ] ステップ1（RED）: `.reduce(fn, init)` の変換テストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: `map_method_call` に `reduce` → `iter().fold()` を実装
- [ ] ステップ3（RED→GREEN）: `.indexOf`, `.join` の変換を実装
- [ ] ステップ4（RED→GREEN）: `.reverse`, `.sort` の変換を実装
- [ ] ステップ5（RED→GREEN）: `.slice`, `.splice` の変換を実装
- [ ] ステップ6（RED→GREEN）: `Math` 定数（`PI`, `E`）の変換を実装
- [ ] ステップ7（RED→GREEN）: `Math.sign/trunc/log` の変換を実装
- [ ] ステップ8（RED→GREEN）: `Number.isInteger` の変換を実装
- [ ] ステップ9: E2E テスト（fixture）を追加
- [ ] ステップ10（REFACTOR）: `map_method_call` の肥大化に対するリファクタリング検討

## テスト計画

各 API について最低 1 つのユニットテスト:

- `arr.reduce((acc, x) => acc + x, 0)` → `arr.iter().fold(0.0, |acc, x| acc + x)`
- `arr.indexOf(x)` → `arr.iter().position(|item| item == x)`
- `arr.join(",")` → `arr.join(",")`
- `arr.reverse()` → `arr.reverse()`
- `arr.sort()` → `arr.sort()`
- `arr.slice(1, 3)` → `arr[1..3].to_vec()`
- `arr.splice(1, 2)` → `arr.drain(1..3).collect::<Vec<_>>()`
- `Math.PI` → `std::f64::consts::PI`
- `Math.sign(x)` → `x.signum()`
- `Number.isInteger(x)` → `x.fract() == 0.0`

異常系: 引数の数が不正なケース

## 完了条件

- 10 カテゴリ全ての変換が動作する
- 各変換に対するユニットテストが存在する
- E2E テスト（fixture）が存在する
- `cargo fmt --all --check` / `cargo clippy` / `cargo test` が 0 エラー・0 警告
