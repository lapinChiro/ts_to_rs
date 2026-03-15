# TypeEnv を使った式の型解決関数

## 背景・動機

`type-env-introduction` で TypeEnv のデータ構造とシグネチャが導入された後、次のステップとして「任意の式の型を解決する関数」が必要になる。

`x?.y` を変換する際に `x` が `Option` 型かどうかを判定するには、`x` の型を TypeEnv から取得し、さらにフィールド連鎖やメソッド呼び出しの型を TypeRegistry から解決する必要がある。

## ゴール

- `resolve_expr_type(expr, type_env, registry) -> Option<RustType>` 関数が存在する
- 識別子（ローカル変数、パラメータ）の型を TypeEnv から解決できる
- フィールドアクセス `x.field` の型を TypeEnv + TypeRegistry から連鎖的に解決できる
- 解決できない場合は `None` を返す（エラーにはしない）

## スコープ

### 対象

- `resolve_expr_type` 関数の実装
- 対応する式パターン: `Ident`、`Member`（フィールドアクセス）、`OptChain`（メンバー）
- TypeEnv → 変数名の型解決、TypeRegistry → フィールド型の解決

### 対象外

- メソッド呼び出しの戻り値型の解決（builtin API ごとに異なり複雑）
- `resolve_expr_type` を使った変換ロジックの変更（次の PRD `type-env-opt-chain` で対応）
- 型パラメータの具体化（ジェネリクス）

## 設計

### 技術的アプローチ

```rust
/// 式の型を解決する。解決できない場合は None を返す。
pub fn resolve_expr_type(
    expr: &ast::Expr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<RustType> {
    match expr {
        ast::Expr::Ident(ident) => type_env.get(&ident.sym.to_string()).cloned(),
        ast::Expr::Member(member) => {
            let obj_type = resolve_expr_type(&member.obj, type_env, reg)?;
            resolve_field_type(&obj_type, &member.prop, reg)
        }
        ast::Expr::Paren(paren) => resolve_expr_type(&paren.expr, type_env, reg),
        ast::Expr::TsAs(ts_as) => {
            // type assertion の型を使う
            convert_ts_type(&ts_as.type_ann, &mut Vec::new()).ok()
        }
        _ => None,
    }
}

/// Named 型のフィールド型を TypeRegistry から解決する。
fn resolve_field_type(
    obj_type: &RustType,
    prop: &ast::MemberProp,
    reg: &TypeRegistry,
) -> Option<RustType> {
    let type_name = match obj_type {
        RustType::Named { name, .. } => name,
        RustType::Option(inner) => match inner.as_ref() {
            RustType::Named { name, .. } => name,
            _ => return None,
        },
        _ => return None,
    };
    let field_name = match prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let type_def = reg.get(type_name)?;
    match type_def {
        TypeDef::Struct { fields } => {
            fields.iter().find(|(name, _)| name == &field_name).map(|(_, ty)| ty.clone())
        }
        _ => None,
    }
}
```

### 影響範囲

- `src/transformer/mod.rs` または `src/transformer/expressions/mod.rs` — `resolve_expr_type` 関数の追加
- テストファイル

## 作業ステップ

- [ ] ステップ1（RED）: `resolve_expr_type` で識別子の型を解決するテスト追加
- [ ] ステップ2（GREEN）: `resolve_expr_type` の `Ident` アーム実装
- [ ] ステップ3（RED）: フィールドアクセス `x.field` の型解決テスト追加
- [ ] ステップ4（GREEN）: `Member` アーム + `resolve_field_type` 実装
- [ ] ステップ5（RED）: `Option<Named>` のフィールド解決テスト追加
- [ ] ステップ6（GREEN）: `Option` 内の Named 型のフィールド解決
- [ ] ステップ7: Quality check

## テスト計画

- `Ident("x")` + TypeEnv に `x: String` → `Some(String)`
- `Ident("y")` + TypeEnv に `y` なし → `None`
- `x.field` + TypeEnv に `x: Foo`, Registry に `Foo { field: String }` → `Some(String)`
- `x.field` + TypeEnv に `x: Foo`, Registry に `Foo` のフィールドに `field` なし → `None`
- `x.field` + TypeEnv に `x: Option<Foo>` → `Some(String)`（Option 内の Named を解決）
- パース不可の式 → `None`

## 完了条件

- `resolve_expr_type` が識別子・フィールドアクセス・Option 内フィールドを解決できる
- 解決不可の場合にエラーではなく `None` を返す
- 全テスト pass、0 errors / 0 warnings
