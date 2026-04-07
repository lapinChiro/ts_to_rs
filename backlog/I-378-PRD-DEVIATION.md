# I-378 PRD Deviation Log

本ファイルは PRD `I-378-expr-path-structuring.md` の実装中に発見した、PRD spec と実装の差分および設計 defect を記録する。

---

## D-1: PRD T2 — `is_trivially_pure` / `is_copy_literal` の戻り値 spec が defect

### PRD spec (該当箇所)

> ### T2: `Expr` への 3 新 variant 追加
>
> - **Work**: `src/ir/expr.rs` `enum Expr` に `EnumVariant` / `PrimitiveAssocConst` / `StdConst` を追加。`is_trivially_pure` / `is_copy_literal` の網羅性を維持（**3 variant とも `false`**）

### 実装した値

| 関数 | EnumVariant | PrimitiveAssocConst | StdConst |
|---|---|---|---|
| `is_trivially_pure` | **`true`** | **`true`** | **`true`** |
| `is_copy_literal` | `false` | **`true`** | **`true`** |

### 調査と根拠

PRD T2 spec が「3 variant とも false」と書いているのは**既存の `Expr::Ident("f64::NAN")` が `Expr::Ident(_) => true` 経由で `is_trivially_pure() == true` を返している事実を見落とした defect**。3 種の構築サイト書き換え (T6) 後にこの戻り値が `true → false` に静かに反転すると、以下の **silent semantic change と生成出力 regression** が発生する:

#### `is_trivially_pure` 呼び出しサイトトレース

`grep -n "is_trivially_pure" src/` で 3 サイト存在:

1. **`src/transformer/expressions/data_literals.rs:263`** — object spread の temp binding 判定
   ```rust
   if !expr.is_trivially_pure() {
       // Generate `let __spread_obj_N = expr;` to evaluate once
   }
   ```
   - **現状**: `Expr::Ident("f64::NAN")` は pure (true) → temp binding 不要
   - **PRD spec 採用時**: `PrimitiveAssocConst` が pure (false) → **不必要に `let __spread_obj_0 = f64::NAN;` を生成**

2. **`src/transformer/expressions/data_literals.rs:332`** — overridden explicit field の副作用保持
   ```rust
   if !used_indices.contains(&idx) && !value.is_trivially_pure() {
       // Generate `let _ = value;` to preserve side effects
   }
   ```
   - **現状**: `Expr::Ident("Color::Red")` / `Expr::Ident("f64::NAN")` は pure → スキップ
   - **PRD spec 採用時**: **不必要に `let _ = Color::Red;` / `let _ = f64::NAN;` を生成**

3. **`src/transformer/expressions/data_literals.rs:398`** — spread 前 dropped explicit の副作用保持（同 (2)）

#### `is_copy_literal` 呼び出しサイトトレース

`grep -n "is_copy_literal" src/` で 1 サイト存在:

- **`src/transformer/mod.rs:814`** — Option default の eager / lazy 判定
   ```rust
   if default_ir.is_copy_literal() {
       .unwrap_or(default_ir)        // eager
   } else {
       .unwrap_or_else(|| default_ir) // lazy
   }
   ```
   - **現状**: `Expr::Ident("f64::NAN")` は `is_copy_literal() == false` → `unwrap_or_else(|| f64::NAN)` を生成
   - **本実装 (true)**: `unwrap_or(f64::NAN)` を生成。**意味論的に等価かつ idiomatic な改善**だが byte-diff 発生

### 採用した値の意味論的根拠

#### `is_trivially_pure: true` (3 variant 全て)

- `Expr::EnumVariant { Color, Red }` — enum unit variant 参照は副作用ゼロの定数
- `Expr::PrimitiveAssocConst { F64, NAN }` — プリミティブ associated const は副作用ゼロの定数
- `Expr::StdConst(F64Pi)` — std module const は副作用ゼロの定数
- いずれも既存の `Expr::Ident("...")` 経路と同じ意味論であり、Phase 2 構築サイト書き換えで戻り値を反転させることは Tier 1 silent semantic change

#### `is_copy_literal: true` (PrimitiveAssocConst / StdConst のみ)

- `f64::NAN` / `f64::INFINITY` / `i32::MAX` 等プリミティブ associated const は **その型自体が `Copy`** であり、eager 評価で安全
- `std::f64::consts::PI` 等も同様に `f64: Copy`
- `EnumVariant` は親 enum の `Copy` 実装が unknown のため保守的に `false` を維持（既存 `Expr::Ident("Color::Red")` も false）
- Phase 2 構築サイト書き換え後、Hono fixture / E2E test で Option デフォルトに `f64::NAN` 等を使う箇所では生成出力が `unwrap_or_else(|| f64::NAN)` → `unwrap_or(f64::NAN)` に変化する **(byte-diff 発生)**。**意味論的に等価**で副作用ゼロのため Tier 1 silent semantic change ではない（Tier 0 strict improvement）

### Phase 2 の必須事項

- **T11 (Hono ベンチ + baseline 比較)**: byte-diff 発生時、`unwrap_or_else(|| f64::NAN)` → `unwrap_or(f64::NAN)` 系の差分のみが出ていることを確認する。それ以外の差分は要調査。具体的には `f64::NAN` / `f64::INFINITY` / `Math.PI`/`Math.E` 等が Option default で使われている fixture を grep で先に列挙し、想定差分セットを確定する
- **PRD T2 spec の修正**: 本ファイル D-1 を参照する形で PRD T2 の "(3 variant とも false)" 記述を修正

### PRD への反映

PRD T2 の該当行を以下に書き換えた:

```
- **Work**: `src/ir/expr.rs` `enum Expr` に `EnumVariant` / `PrimitiveAssocConst` / `StdConst` を追加。
  `is_trivially_pure` / `is_copy_literal` を実意味論に合わせて拡張（PRD-DEVIATION D-1 参照）:
  - `is_trivially_pure`: 3 variant とも `true`（定数参照、副作用ゼロ。`Expr::Ident("f64::NAN")` の
    既存挙動を維持し silent semantic change を防ぐ）
  - `is_copy_literal`: `PrimitiveAssocConst` / `StdConst` は `true`（プリミティブ Copy 値）、
    `EnumVariant` は親 enum の Copy 性 unknown のため保守的に `false`
- **Completion criteria**: `cargo check` pass。`is_trivially_pure` / `is_copy_literal` の各 3 variant
  に対する単体テストが期待値（D-1 表）と一致して pass
```
