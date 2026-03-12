# 三項演算子変換

## 背景・動機

TS の三項演算子（`condition ? a : b`）は頻出構文だが、現在の変換ツールでは未対応。Rust では `if` が式として使えるため、`if condition { a } else { b }` に変換できる。

## ゴール

TS の三項演算子を Rust の `if` 式に変換できる。

### 変換例

**基本:**
```typescript
const x = a > 0 ? a : -a;
```
→
```rust
let x = if a > 0.0 { a } else { -a };
```

**ネスト:**
```typescript
const s = x > 0 ? "positive" : x < 0 ? "negative" : "zero";
```
→
```rust
let s = if x > 0.0 { "positive".to_string() } else { if x < 0.0 { "negative".to_string() } else { "zero".to_string() } };
```

**関数引数:**
```typescript
foo(flag ? x : y);
```
→
```rust
foo(if flag { x } else { y });
```

## スコープ

### 対象

- 三項演算子（`condition ? consequent : alternate`）
- ネストした三項演算子
- 変数初期化、関数引数、return 文での使用

### 対象外

- 特になし

## 設計

### 技術的アプローチ

1. **IR 拡張**: `Expr` に `If` バリアントを追加（条件式、then 式、else 式）
2. **transformer 追加**: SWC の `CondExpr` を解析し、`Expr::If` に変換
3. **generator 更新**: `Expr::If` を `if cond { then } else { else }` 形式で出力

### 影響範囲

- `src/ir.rs` — `Expr` に `If` バリアント追加
- `src/transformer/expressions.rs` — `CondExpr` のハンドリング追加
- `src/generator.rs` — `Expr::If` の生成ロジック追加
- `tests/fixtures/` — 三項演算子用の fixture 追加

## 作業ステップ

- [ ] ステップ1: IR 拡張 — `Expr::If { condition: Box<Expr>, then_expr: Box<Expr>, else_expr: Box<Expr> }` を追加
- [ ] ステップ2: transformer — SWC `CondExpr` を `Expr::If` に変換
- [ ] ステップ3: generator — `Expr::If` を `if cond { then } else { else }` として出力
- [ ] ステップ4: スナップショットテスト — fixture ファイルで E2E 検証

## テスト計画

- 正常系: 基本的な三項演算子、ネストした三項演算子、関数引数内での使用
- 正常系: 条件部に比較演算子・論理演算子を含むケース
- 境界値: 深くネストした三項演算子（3段以上）
- スナップショット: `tests/fixtures/ternary.input.ts` で E2E 検証

## 完了条件

- 上記変換例が正しく変換される
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
- スナップショットテストが追加されている
