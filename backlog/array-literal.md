# 配列リテラル変換

## 背景・動機

TS の配列リテラル（`[1, 2, 3]`）は頻出構文だが、現在の変換ツールでは未対応。既存の設計で `T[]` / `Array<T>` → `Vec<T>` にマッピング済みのため、配列リテラルは `vec![...]` マクロに変換するのが一貫性のある設計。

## ゴール

TS の配列リテラル式を Rust の `vec![...]` マクロ呼び出しに変換できる。

### 変換例

**数値配列:**
```typescript
const nums = [1, 2, 3];
```
→
```rust
let nums = vec![1.0, 2.0, 3.0];
```

**文字列配列:**
```typescript
const names = ["alice", "bob"];
```
→
```rust
let names = vec!["alice".to_string(), "bob".to_string()];
```

**式を含む配列:**
```typescript
const values = [x, y + 1, foo()];
```
→
```rust
let values = vec![x, y + 1.0, foo()];
```

## スコープ

### 対象

- リテラル要素の配列（`[1, 2, 3]`、`["a", "b"]`、`[x, y + 1, foo()]`）
- 変数初期化、関数引数、return 文での配列リテラル

### 対象外

- スプレッド構文（`[...arr, 4]`）
- 分割代入（`const [a, b] = arr`）
- 空配列の型推論（`[]` 単体で型が決まらないケース）

## 設計

### 技術的アプローチ

1. **IR 拡張**: `Expr` に `Vec` バリアントを追加（要素のリストを持つ）
2. **transformer 追加**: SWC の `ArrayLit` を解析し、各要素の式を再帰的に変換
3. **generator 更新**: `Expr::Vec` を `vec![elem1, elem2, ...]` 形式で出力

### 影響範囲

- `src/ir.rs` — `Expr` に `Vec` バリアント追加
- `src/transformer/expressions.rs` — `ArrayLit` のハンドリング追加
- `src/generator.rs` — `Expr::Vec` の生成ロジック追加
- `tests/fixtures/` — 配列リテラル用の fixture 追加

## 作業ステップ

- [ ] ステップ1: IR 拡張 — `Expr::Vec { elements: Vec<Expr> }` を追加
- [ ] ステップ2: transformer — SWC `ArrayLit` を `Expr::Vec` に変換
- [ ] ステップ3: generator — `Expr::Vec` を `vec![...]` として出力
- [ ] ステップ4: スナップショットテスト — fixture ファイルで E2E 検証

## テスト計画

- 正常系: 数値配列、文字列配列、ブール配列、式を含む配列、ネストした配列（`[[1, 2], [3, 4]]`）
- 正常系: 関数引数・return 文での配列リテラル
- 異常系: 空配列（要素なし）の扱い
- 境界値: 1 要素の配列
- スナップショット: `tests/fixtures/array-literal.input.ts` で E2E 検証

## 完了条件

- 上記変換例が正しく変換される
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
- スナップショットテストが追加されている
