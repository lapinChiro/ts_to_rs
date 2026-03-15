# switch 文の改善（fall-through 検出 + 文字列 match + パターン IR 改善）

## 背景・動機

switch 変換に 3 つの問題がある:

1. **fall-through 誤検出**: `return`/`throw` で終わる case が fall-through パスに入り、unreachable な `_fall = true;` が生成される
2. **文字列 match 非対応**: clean match で文字列 discriminant に `.as_str()` が付与されず、Rust の `match` が `String` を `&str` パターンでマッチできない
3. **パターン表現の型安全性不足**: `MatchArm::patterns` が `Vec<Expr>` 型で、Rust の match パターン（リテラル・ワイルドカード・変数）を型安全に表現できない

## ゴール

- `return`/`throw`/`continue` で終わる case が clean match パスで処理される
- 文字列 discriminant に `.as_str()` が自動付与され、`match s.as_str() { "a" => ... }` が生成される
- 数値 discriminant で整数値の場合は `IntLit` パターンが使用される
- IR の `MatchArm` がリテラル・ワイルドカードを型安全に表現する `MatchPattern` enum を使用する
- 非リテラル case（変数）は fall-through パス（if-chain）にフォールバックする
- switch fixture がコンパイルテストを通る
- 全テスト pass、clippy 0 警告、fmt 通過

## スコープ

### 対象

- `convert_switch_stmt` の fall-through 検出を改善（`return`/`throw`/`continue` を終端に追加）
- IR に `MatchPattern` enum を追加（`Literal(Expr)`, `Wildcard`）
- `MatchArm::patterns` を `Vec<MatchPattern>` に変更
- Generator で文字列パターン検出時に discriminant に `.as_str()` を付与
- Generator で `MatchPattern::Literal(IntLit)` を整数パターンとしてレンダリング
- 非リテラル case を検出し、fall-through パスにフォールバック
- switch fixture の拡充（文字列 switch、return で終わる case 等）

### 対象外

- match ガード（`if` 条件付きパターン）
- destructuring パターン

## 設計

### 技術的アプローチ

#### IR 変更

```rust
/// A pattern in a match arm.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    /// A literal value pattern (e.g., `1`, `"hello"`)
    Literal(Expr),
    /// A wildcard pattern (`_`)
    Wildcard,
}
```

`MatchArm` を変更:
```rust
pub struct MatchArm {
    pub patterns: Vec<MatchPattern>,  // was Vec<Expr> + is_wildcard: bool
    pub body: Vec<Stmt>,
}
```

`is_wildcard` フィールドは `MatchPattern::Wildcard` に統合して削除。

#### fall-through 検出の改善

```rust
fn is_case_terminated(stmts: &[ast::Stmt]) -> bool {
    stmts.last().is_some_and(|s| matches!(s,
        ast::Stmt::Break(_) | ast::Stmt::Return(_) | ast::Stmt::Throw(_) | ast::Stmt::Continue(_)
    ))
}
```

#### Generator: 文字列パターン検出

clean match 生成時に、パターンに `StringLit` が含まれるかを検出。含まれる場合、discriminant に `.as_str()` を付与:

```rust
let has_string_patterns = arms.iter().any(|arm|
    arm.patterns.iter().any(|p| matches!(p, MatchPattern::Literal(Expr::StringLit(_))))
);
let discriminant_str = if has_string_patterns {
    format!("{}.as_str()", generate_expr(expr))
} else {
    generate_expr(expr)
};
```

#### 非リテラル case のフォールバック

case の test 式がリテラルでない場合（変数参照等）、switch 全体を fall-through パス（if-chain）で生成する。

### 影響範囲

- `src/ir.rs` — `MatchPattern` 追加、`MatchArm` 変更
- `src/generator/statements.rs` — `Match` レンダリング改善
- `src/transformer/statements/mod.rs` — fall-through 検出改善、`MatchPattern` 使用
- テストファイル全般

## 作業ステップ

### Part A: IR + Generator

- [ ] ステップ1（RED）: `MatchPattern` ベースのレンダリングテスト（文字列パターン → `.as_str()` 付与）
- [ ] ステップ2（GREEN）: IR に `MatchPattern` 追加 + Generator 改善 + 既存コードの `is_wildcard` → `Wildcard` 移行

### Part B: Transformer

- [ ] ステップ3（RED）: `return` で終わる case が clean match に入るテスト
- [ ] ステップ4（GREEN）: fall-through 検出の改善
- [ ] ステップ5（RED）: 文字列 switch が正しく変換されるテスト
- [ ] ステップ6（GREEN）: `MatchPattern::Literal(StringLit)` の生成

### Part C: 統合

- [ ] ステップ7: switch fixture 拡充 + スナップショット + コンパイルテスト
- [ ] ステップ8: Quality check

## テスト計画

### Generator テスト

- 文字列パターン → `match s.as_str() { "a" => ... }`
- 整数パターン → `match x { 1 => ... }`（IntLit 使用）
- Wildcard → `_ => { ... }`

### Transformer テスト

- `case 1: return "one";` → clean match の `1 => { return "one"; }`
- `case "a": return "alpha";` → `match s.as_str() { "a" => ... }`
- `case 1: doA(); case 2: doB(); break;` → fall-through パス（既存動作維持）

### 回帰テスト

- switch fixture のスナップショット
- コンパイルテスト

## 完了条件

- `return`/`throw`/`continue` で終わる case が clean match パスで処理される
- 文字列 switch が `.as_str()` 付きで生成され、コンパイルが通る
- `MatchArm` が `MatchPattern` enum を使用し、`is_wildcard` フィールドが廃止されている
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
