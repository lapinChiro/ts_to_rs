# Generator から意味的変換を transformer へ移動

## 背景・動機

Generator の責務は「IR をそのまま Rust テキストにする 1:1 マッピング」である。しかし現在、以下の 2 箇所で generator が IR の意味的な変換（1:N 文展開、位置依存の文形式変更）を行っている:

1. **VecSpread 展開**: `Stmt::Let { init: Expr::VecSpread }` や `Stmt::Return(Expr::VecSpread)` を検出し、generator 内で `Vec::new()` + `push`/`extend` の複数文に展開している
2. **Tail expression 判定**: `is_last_in_fn` フラグを受け取り、関数末尾の `Stmt::Return(expr)` を `return expr;` ではなく tail expression `expr` として出力している

これらは「割れ窓」であり、今後 generator に新しい特殊処理が追加される前に解消すべきアーキテクチャ上の責務違反。

## ゴール

- Generator が IR ノードを 1:1 でテキスト化するだけになる（`is_last_in_fn` パラメータ廃止、VecSpread 特殊分岐廃止）
- IR に `Stmt::TailExpr` が追加され、transformer が関数末尾の return を変換する
- IR から `Expr::VecSpread` が廃止され、transformer が `Stmt` レベルで展開する
- 生成される Rust コードが変更前と同一である（振る舞い変更なし）

## スコープ

### 対象

- `Expr::VecSpread` を IR から廃止し、transformer の `convert_stmt_list` で `Stmt` 列に展開する
- `Stmt::TailExpr(Expr)` を IR に追加し、transformer の関数本体変換で末尾 return を変換する
- Generator から `is_last_in_fn` パラメータを削除する
- Generator の `statements.rs` から VecSpread 特殊処理（`generate_vec_spread_let_stmts`, `generate_vec_spread_stmts`）を削除する

### 対象外

- `Expr::Vec` の変更（spread なしの配列リテラルは現状通り）
- Generator の `expressions.rs` 内の `generate_vec_spread`（式位置での VecSpread。これも廃止対象だが、transformer での展開に含まれる）

## 設計

### 技術的アプローチ

#### 1. VecSpread 展開の transformer 移動

Transformer の `convert_stmt_list` で、各 `Stmt` を変換した後に VecSpread を検出・展開する:

```rust
// convert_stmt_list 内、convert_stmt の結果を処理した後
for stmt in converted {
    match stmt {
        Stmt::Let { name, mutable, ty, init: Some(Expr::VecSpread { segments }) } => {
            // 展開: let mut name = Vec::new(); name.extend(...); name.push(...);
            result.extend(expand_vec_spread_let(&name, &ty, &segments));
        }
        Stmt::Return(Some(Expr::VecSpread { segments })) => {
            // 展開: let mut __v = Vec::new(); __v.extend(...); return __v;
            result.extend(expand_vec_spread_return(&segments));
        }
        Stmt::Expr(Expr::VecSpread { segments }) => {
            // 展開: 同様
            result.extend(expand_vec_spread_expr(&segments));
        }
        other => result.push(other),
    }
}
```

展開後、`Expr::VecSpread` と `VecSegment` を IR から削除する。

#### 2. TailExpr の導入

IR に新しい `Stmt` バリアントを追加:

```rust
pub enum Stmt {
    // ... 既存 ...
    /// 関数末尾の tail expression（`return` キーワードなし）
    TailExpr(Expr),
}
```

Transformer の関数本体変換で、末尾の `Stmt::Return(Some(expr))` を `Stmt::TailExpr(expr)` に変換する。`convert_fn_decl` の body 生成後に適用:

```rust
fn convert_last_return_to_tail(body: &mut Vec<Stmt>) {
    if let Some(Stmt::Return(Some(expr))) = body.last() {
        let expr = expr.clone();
        *body.last_mut().unwrap() = Stmt::TailExpr(expr);
    }
}
```

Generator の `generate_stmt` は `Stmt::TailExpr(expr)` を `{pad}{expr}` として出力し、`is_last_in_fn` パラメータは廃止する。

### 影響範囲

- `src/ir.rs` — `Expr::VecSpread` / `VecSegment` 廃止、`Stmt::TailExpr` 追加
- `src/transformer/statements/mod.rs` — VecSpread 展開ロジック追加
- `src/transformer/functions/mod.rs` — 末尾 return → TailExpr 変換
- `src/transformer/classes.rs` — メソッド本体の末尾 return → TailExpr 変換
- `src/transformer/mod.rs` — `convert_var_decl_arrow_fns` の本体末尾変換
- `src/transformer/expressions/mod.rs` — `convert_array_lit` が `Expr::VecSpread` ではなく別の表現を返す
- `src/generator/statements.rs` — VecSpread 特殊処理削除、`is_last_in_fn` 廃止、`TailExpr` 追加
- `src/generator/expressions.rs` — `generate_vec_spread` 削除
- `src/generator/mod.rs` — `is_last` 計算の削除
- テストファイル全般

## 作業ステップ

### Part A: TailExpr 導入（先に行う — VecSpread より影響範囲が小さい）

- [ ] ステップ1: IR に `Stmt::TailExpr(Expr)` を追加
- [ ] ステップ2（RED）: `Stmt::TailExpr(Expr::Ident("x"))` が `    x` として生成されるテスト追加
- [ ] ステップ3（GREEN）: generator に `TailExpr` の出力を実装
- [ ] ステップ4: transformer の `convert_fn_decl` で末尾 return → TailExpr 変換を実装
- [ ] ステップ5: transformer の classes / mod.rs のメソッド・クロージャ本体にも同様の変換を適用
- [ ] ステップ6: generator から `is_last_in_fn` パラメータを削除し、全呼び出し元を更新
- [ ] ステップ7: 全テスト通過を確認

### Part B: VecSpread 廃止

- [ ] ステップ8（RED）: transformer が `[...arr, 1]` を `[Stmt::Let(Vec::new), Stmt::Expr(extend), Stmt::Expr(push)]` に展開するテスト追加
- [ ] ステップ9（GREEN）: `convert_stmt_list` に VecSpread 展開ロジックを追加（`expand_vec_spread_let` 等）
- [ ] ステップ10: `convert_array_lit` を変更し、spread 付き配列を `Expr::VecSpread` ではなく展開済みの形で返す（呼び出し元の `convert_stmt_list` で展開）
- [ ] ステップ11: generator の `statements.rs` から `generate_vec_spread_let_stmts` / `generate_vec_spread_stmts` を削除
- [ ] ステップ12: generator の `expressions.rs` から `generate_vec_spread` を削除
- [ ] ステップ13: IR から `Expr::VecSpread` / `VecSegment` を削除
- [ ] ステップ14: 全テスト通過・Quality check

## テスト計画

### TailExpr

- `Stmt::TailExpr(Expr::Ident("x"))` の generator 出力が `    x`（セミコロンなし）
- 関数末尾の `return x;` が tail expression `x` に変換される（既存のスナップショットテストで回帰確認）
- 関数末尾以外の `return x;` は `Stmt::Return` のまま（変換されない）
- if/while/for 内の return は `Stmt::Return` のまま
- `Stmt::Return(None)` は `Stmt::TailExpr` に変換しない（`return;` は tail expression にできない）

### VecSpread

- `let x = [...arr, 1]` が transformer で `[Let(Vec::new), Expr(extend), Expr(push)]` に展開される
- `return [...arr, 1]` が transformer で `[Let(Vec::new), Expr(extend), Expr(push), Return(ident)]` に展開される
- `[...arr]`（単一 spread）が `Let { init: clone }` に最適化される
- 既存のスナップショットテストで生成コードが変更前と同一であることを確認
- IR に `Expr::VecSpread` が存在しないことをコンパイルで確認（削除すればコンパイルエラーで保証）

## 完了条件

- IR に `Expr::VecSpread` / `VecSegment` が存在しない
- IR に `Stmt::TailExpr` が存在する
- Generator の `generate_stmt` に `is_last_in_fn` パラメータがない
- Generator の `statements.rs` に `generate_vec_spread_let_stmts` / `generate_vec_spread_stmts` がない
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
- 生成される Rust コードが変更前と同一（スナップショットテストで確認）
