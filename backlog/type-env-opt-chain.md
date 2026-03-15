# TypeEnv を使った optional chaining / nullish coalescing の型判定

## 背景・動機

`type-env-introduction` と `type-env-expr-resolution` により、式の型を解決するインフラが整った。これを使い、optional chaining (`x?.y`) と nullish coalescing (`x ?? y`) で対象の型が `Option` かどうかを判定し、適切な Rust コードを生成する。

現在は全ての `x?.y` を `x.as_ref().map(|_v| _v.y)` に変換するが、`x` が非 Option 型の場合はコンパイル不可になる。TS の `?.` は非 null 型に対しても有効で、単にアクセスするだけ。

## ゴール

- 非 Option 型への `x?.y` が `x.y`（通常のフィールドアクセス）に変換される
- Option 型への `x?.y` が従来通り `.as_ref().map()` パターンに変換される
- 非 Option 型への `x ?? y` が `x`（そのまま）に変換される
- Option 型への `x ?? y` が従来通り `.unwrap_or_else()` に変換される

## スコープ

### 対象

- `convert_opt_chain_expr` で `resolve_expr_type` を使い、対象の型を判定して分岐
- `convert_bin_expr` の nullish coalescing で同様の分岐
- ネスト optional chaining `x?.y?.z` で `.and_then()` を使ったフラット化

### 対象外

- メソッドチェーン `x?.method()?.method2()` の完全対応（メソッド戻り値型の解決が必要）
- 型推論インフラの拡張（`resolve_expr_type` の対応パターン追加は別 PRD）

## 設計

### 技術的アプローチ

`convert_opt_chain_expr` を以下のように変更:

```rust
fn convert_opt_chain_expr(opt_chain, reg, type_env) -> Result<Expr> {
    match opt_chain.base.as_ref() {
        OptChainBase::Member(member) => {
            let obj_type = resolve_expr_type(&member.obj, type_env, reg);
            let is_option = obj_type.as_ref().is_some_and(|ty| matches!(ty, RustType::Option(_)));

            if is_option {
                // 既存パターン: x.as_ref().map(|_v| _v.y)
                // ただしネストの場合は .and_then() でフラット化
            } else {
                // 非 Option: 通常のフィールドアクセス x.y
                convert_member_expr(member, reg)
            }
        }
    }
}
```

nullish coalescing も同様:
```rust
if bin.op == NullishCoalescing {
    let left_type = resolve_expr_type(&bin.left, type_env, reg);
    let is_option = left_type.as_ref().is_some_and(|ty| matches!(ty, RustType::Option(_)));
    if is_option {
        // 既存パターン: x.unwrap_or_else(|| y)
    } else {
        // 非 Option: x をそのまま返す
        convert_expr(&bin.left, reg, None, type_env)
    }
}
```

### 影響範囲

- `src/transformer/expressions/mod.rs` — `convert_opt_chain_expr`, `convert_bin_expr` の nullish coalescing 処理
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: 非 Option 型への `x?.y` が通常アクセスになるテスト追加
- [ ] ステップ2（GREEN）: `convert_opt_chain_expr` で型判定分岐を実装
- [ ] ステップ3（RED）: 非 Option 型への `x ?? y` が `x` になるテスト追加
- [ ] ステップ4（GREEN）: nullish coalescing の型判定分岐
- [ ] ステップ5（RED）: ネスト `x?.y?.z` で `Option<T>` を返すテスト追加（`Option<Option<T>>` ではなく）
- [ ] ステップ6（GREEN）: `.map()` → `.and_then()` への変更
- [ ] ステップ7: 回帰テスト・Quality check

## テスト計画

- 非 Option 型への optional chaining → 通常アクセス
- Option 型への optional chaining → `.as_ref().map()` パターン
- ネスト optional chaining → フラットな `Option<T>`
- 非 Option 型への nullish coalescing → そのまま返す
- Option 型への nullish coalescing → `.unwrap_or_else()`
- 型が不明（TypeEnv に登録なし）の場合 → 既存動作を維持（`.as_ref().map()` フォールバック）
- 回帰: 既存のスナップショットテスト

## 完了条件

- 非 Option 型への optional chaining / nullish coalescing がコンパイル可能な Rust を生成する
- Option 型への変換は従来通り動作する
- ネスト optional chaining が `Option<Option<T>>` を生成しない
- 全テスト pass、0 errors / 0 warnings
