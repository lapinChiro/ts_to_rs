# I-392 P0.1: IfLet/Match 発生確認調査

## 調査結果

### Q1: typeof narrowing + callable interface arrow body で Expr::IfLet が生成されるか?

**YES — 発生する。**

- **Expression body (ternary)**: `(x: any): number => typeof x === "string" ? x.length : 0`
  のようなパターンで `Expr::IfLet` が生成される
- **構築箇所**: `src/transformer/expressions/mod.rs:236-241` (single guard),
  `src/transformer/expressions/mod.rs:268-280` (compound guards)
- **Evidence**: 既存 fixture `any-type-narrowing.input.ts:31` の snapshot
  (`integration_test__any_type_narrowing.snap:52-54`) で確認済:
  ```rust
  fn exprArrow(x: ExprArrowXType) -> f64 {
      if let ExprArrowXType::String(x) = x { x.len() as f64 } else { 0.0 }
  }
  ```

- **Block body**: `if (typeof x === "string") { ... }` は `Stmt::IfLet` (statement レベル)
  に変換される。式レベルの `Expr::IfLet` ではない

### Q2: switch inside callable-interface arrow body で Expr::Match が生成されるか?

**NO — 発生しない。**

- Switch 文は `Stmt::Match` (statement レベル) に変換される
  (`src/transformer/statements/switch.rs:172-300`)
- Arrow expression body は式のみ受け付けるため、switch は含まれない
- Arrow block body 内の switch は `Stmt::Match` になるが、`Expr::Match` にはならない

### Q3: return 位置で発生するか?

**Expr::IfLet のみ return 位置で発生する。**

- Ternary (`? :`) が arrow expression body にある場合、`Expr::IfLet` が return 値として
  生成される
- Generator は `Expr::IfLet` を式位置で出力可能
  (`src/generator/expressions/mod.rs:307-312`)
- `Expr::Match` は return 式としては発生しない

### 結論: Phase 6 設計への影響

1. **`wrap_expr_tail` は `Expr::IfLet` の then/else branch を再帰 wrap する必要がある**
   - ternary inside callable interface arrow body で発生する
   - then/else の各 branch が異なる overload の return type を持つ可能性がある
2. **`Expr::Match` の wrap は不要** (YAGNI)
   - 現状 arrow body で `Expr::Match` は生成されない
   - `wrap_expr_tail` の Match arm は `unreachable!()` とする
3. **`Stmt::IfLet` / `Stmt::Match` は wrap 不要**
   - Block body の statement レベル narrowing は `Stmt::Return` 内の式を wrap すればよい
   - statement 自体を wrap する必要はない
