# Generator が行っている意味的変換の調査

**基準コミット**: `e7d2dc3`（未コミット変更なし）

**対応状況**: PRD 化済み → `backlog/generator-semantic-extraction.md`

## 結論

Generator が TypeScript の意味情報を別の意味に変換（情報損失・意味変更）しているわけではない。問題は **アーキテクチャ上の責務違反** であり、generator が「IR をそのまま Rust テキストにする」以上のことをしている、という設計負債（割れ窓）。

生成される Rust コードの正確性に影響する問題ではなく、保守性の問題である。

## 詳細分析

### 1. VecSpread 展開

**該当箇所**: `src/generator/statements.rs`

IR `Expr::VecSpread { segments }` は `[...arr, 1, 2]` のような配列スプレッドを表現する。Generator はこれを検出すると、1 つの IR 文 (`Stmt::Let` や `Stmt::Return`) を **複数の Rust 文** に展開する。

**問題の本質**: 「IR → テキスト」の 1:1 マッピングではなく、「1 つの IR ノードを複数の Rust 文に分解する」意味的な変換。本来は transformer が IR レベルで複数の `Stmt` に分解すべき。

### 2. Tail expression 判定

**該当箇所**: `src/generator/statements.rs`

`is_last_in_fn` フラグで「関数本体の最後の文かどうか」を判定し、最後の `Stmt::Return(Some(expr))` を Rust の tail expression（`return` キーワードなし）に変換。

**問題の本質**: generator が「どう書くか」を超えて「何を書くか」を判断している。

## 分類

| 項目 | 意味変更？ | 情報損失？ | 正確性影響？ | 問題の性質 |
|------|----------|----------|------------|----------|
| VecSpread 展開 | No | No | No | 責務配置（transformer で分解すべき） |
| Tail expression | No | No | No | コードスタイル最適化の責務配置 |
