# 式中ネスト位置の配列スプレッド対応

## 背景・動機

`foo([...arr, 1])` のように配列スプレッドが式中のネスト位置（関数引数、代入の右辺の一部等）にある場合、「spread array in expression position is not supported」エラーになる。

現在のスプレッド展開は `convert_stmt` レベルで `try_expand_spread_*` 関数群が処理しているが、これらは文の直接の子位置（`let x = [...arr, 1]`、`return [...arr, 1]`）のみ対応。式中のネスト位置は `convert_expr` 内で処理する必要がある。

## ゴール

- `foo([...arr, 1])` が式中位置でエラーにならず正しく変換される
- ブロック式を使って式の位置を保持し、親の文構造を変更しない
- 全テスト pass、clippy 0 警告、fmt 通過

## スコープ

### 対象

- IR に `Expr::Block(Vec<Stmt>)` を追加（ブロック式: `{ stmt1; stmt2; tail_expr }`)
- Generator に `Expr::Block` のレンダリングを追加
- `convert_array_literal` でスプレッドを検出した場合、ブロック式に展開:
  ```rust
  // [1, ...arr, 2] →
  {
      let mut _v = vec![1.0];
      _v.extend(arr.iter().cloned());
      _v.push(2.0);
      _v
  }
  ```

### 対象外

- 文レベルのスプレッド展開の変更（既存の `try_expand_spread_*` はそのまま維持）

## 設計

### 技術的アプローチ

#### IR 追加

```rust
/// A block expression: `{ stmt1; stmt2; tail_expr }`
Block(Vec<Stmt>),
```

最後の `Stmt` が `TailExpr` であればブロック式の値として返される。

#### Generator

```rust
Expr::Block(stmts) => {
    let mut out = "{\n".to_string();
    for s in stmts {
        out.push_str(&generate_stmt(s, indent + 1));
        out.push('\n');
    }
    out.push_str(&format!("{pad}}}"));
    out
}
```

#### Transformer

`convert_array_literal` でスプレッドを検出した場合:

1. 最初の非スプレッド要素群を `vec![...]` で初期化
2. スプレッド要素を `.extend(arr.iter().cloned())` で展開
3. 後続の非スプレッド要素を `.push(elem)` で追加
4. 全体を `Expr::Block` でラップし、最後に `TailExpr(Ident("_v"))` を追加

### 影響範囲

- `src/ir.rs` — `Expr::Block` 追加
- `src/generator/expressions.rs` — `Block` レンダリング追加
- `src/transformer/expressions/mod.rs` — `convert_array_literal` のスプレッド処理変更

## 作業ステップ

### Part A: IR + Generator

- [ ] ステップ1（RED）: `Expr::Block` のレンダリングテスト
- [ ] ステップ2（GREEN）: IR に `Block` 追加 + Generator 実装

### Part B: Transformer

- [ ] ステップ3（RED）: `foo([...arr, 1])` が正しくブロック式に展開されるテスト
- [ ] ステップ4（GREEN）: `convert_array_literal` のスプレッド処理変更
- [ ] ステップ5: 既存の文レベルスプレッド展開との回帰確認

### Part C: 統合

- [ ] ステップ6: コンパイルテスト + Quality check

## テスト計画

### Generator テスト

- `Expr::Block([Let, TailExpr])` → `{ let x = 1; x }`

### Transformer テスト

- `[...arr, 1]` in expression position → `Expr::Block` containing extend + push
- `[1, ...arr]` → `Expr::Block` containing vec init + extend
- `[1, ...arr, 2, ...brr, 3]` → 複数スプレッドの展開

### 回帰テスト

- 既存の文レベルスプレッド（`let x = [...arr, 1]`）が変わらないこと
- `array-spread` fixture のスナップショット

## 完了条件

- 式中位置の配列スプレッドがエラーにならず、ブロック式で正しく変換される
- 既存の文レベルスプレッド展開に回帰がない
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
