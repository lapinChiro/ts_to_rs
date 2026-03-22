# test_var_type_alias_arrow 失敗原因調査レポート

**基準コミット**: a160a92（未コミットの Phase 2.5-A 変更を含む状態で調査）
**ステータス**: **修正済み** — 方針 A を採用し、`visit_var_decl` 再構成 + `resolve_arrow_expr` / `resolve_fn_expr` に expected type 読み取りを追加

## 要約

`test_var_type_alias_arrow` の失敗原因は、**TypeResolver が関数型エイリアス（`type GetConnInfo = (host: string) => ConnInfo`）で注釈された変数の arrow function に対して、return type を body に伝搬できないこと**にある。Transformer 側はこの経路を `extract_fn_return_type` で処理しているが、TypeResolver には対応するロジックが存在しない。

## テストケース

```typescript
interface ConnInfo { remote: RemoteInfo; }
interface RemoteInfo { address: string; }
type GetConnInfo = (host: string) => ConnInfo;
export const getConnInfo: GetConnInfo = (host: string) => ({
    remote: { address: host },
});
```

エラー: `unsupported syntax: object literal requires a type annotation to determine struct name at byte 144`
（byte 144 = `{ address: host }`、ネストされたオブジェクトリテラル）

## 根本原因

### 必要な伝搬チェーン

`{ address: host }` に `RemoteInfo` が expected type として設定されるために必要な伝搬チェーン:

1. 変数 `getConnInfo` の型注釈 `GetConnInfo` → `Named("GetConnInfo")`
2. `Named("GetConnInfo")` → TypeRegistry lookup → `TypeDef::Function { return_type: Some(Named("ConnInfo")) }`
3. Arrow function の return type = `ConnInfo`
4. Arrow expression body `({ remote: { address: host } })` に expected = `ConnInfo` を設定
5. `propagate_expected` が `ConnInfo` の struct fields を lookup → `remote: RemoteInfo`
6. `{ address: host }` に expected = `RemoteInfo` を設定

### TypeResolver での断絶箇所

**Step 2-3 が欠落**している。

TypeResolver の `visit_var_decl`（`src/pipeline/type_resolver.rs:211-214`）では:

```rust
if let (Some(ann_ty), Some(init)) = (&annotation_type, &decl.init) {
    self.propagate_expected(init, ann_ty);
}
```

`ann_ty` = `Named("GetConnInfo")` で `propagate_expected` が呼ばれるが、`propagate_expected` の match 式で Arrow 式は `_ => {}` に該当し、何もしない。

TypeResolver の `resolve_arrow_expr`（`src/pipeline/type_resolver.rs:1213`）では:

```rust
if let Some(return_ann) = &arrow.return_type {
    // ...
}
```

arrow 自体に return type アノテーションがないため（型は変数の型注釈にのみ存在）、`current_fn_return_type` が設定されず、Phase 2.5-A で追加した expression body への return type 伝搬も発動しない。

### Transformer 側の対応コード（参考）

Transformer は `src/transformer/functions/mod.rs:1133-1147` で同等の処理を行っている:

```rust
let var_rust_type = ident.type_ann.as_ref()
    .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, reg).ok());
let ret = var_rust_type.as_ref()
    .and_then(|ty| extract_fn_return_type(ty, tctx, reg)); // ← TypeRegistry lookup
```

`extract_fn_return_type`（`src/transformer/functions/mod.rs:1278-1301`）は `Named("GetConnInfo")` から TypeRegistry で `TypeDef::Function { return_type: Some(Named("ConnInfo")), .. }` を取得し、`ConnInfo` を `override_return_type` として arrow に渡す。

## 影響範囲

この問題は以下の全てのパターンに影響する:

- `type Handler = (req: Request) => Response; const h: Handler = (req) => ({ ... });`
- `type Callback = (data: string) => Result; const cb: Callback = (data) => ({ ... });`
- 関数型エイリアスで型注釈された変数に代入された arrow function で、body が object literal を含む場合

Hono フレームワークでは `type GetConnInfo` がこのパターンで使用されており、実用上の影響がある。

## 修正方針

TypeResolver に以下の 2 つの処理を追加する:

### 方針 A: `visit_var_decl` で arrow initializer に return type を設定

`visit_var_decl` で initializer が Arrow 式で、変数の型注釈が関数型エイリアス（TypeRegistry に `TypeDef::Function` として登録）の場合:

1. TypeRegistry から return type を取得
2. `current_fn_return_type` に設定してから arrow body を walk する

具体的には、`resolve_arrow_expr` を呼ぶ前に `current_fn_return_type` を設定するか、`resolve_arrow_expr` に `override_return_type` パラメータを追加する。

### 方針 B: `propagate_expected` に Arrow 式のパターンを追加

`propagate_expected` で expected が `Named(X)` かつ TypeRegistry で `X` が `TypeDef::Function` の場合、内部の Arrow 式に return type を伝搬する。

### 推奨

**方針 A** を推奨。理由:

- `propagate_expected` は子式への型伝搬を担当する関数であり、arrow function の内部構造（return type, scope）の管理は `resolve_arrow_expr` の責務
- `resolve_arrow_expr` に override_return_type を渡す設計は Transformer 側の `convert_arrow_expr_with_return_type` と同じ構造であり、一貫性がある
- `visit_var_decl` から `resolve_expr(init)` が呼ばれる時点で arrow の return type が `current_fn_return_type` に設定されていれば、Phase 2.5-A で追加した expression body への return type 伝搬が自動的に発動する

### 実装の概要（方針 A）

`visit_var_decl` に以下を追加:

```rust
// Before resolving the initializer, if the annotation is a function type alias
// and the initializer is an arrow, set the return type for the arrow resolver.
if let (Some(RustType::Named { name, .. }), Some(ast::Expr::Arrow(_))) =
    (&annotation_type, decl.init.as_deref())
{
    if let Some(TypeDef::Function { return_type: Some(ret_ty), .. }) = self.registry.get(name) {
        self.current_fn_return_type = unwrap_promise_and_unit(ret_ty.clone());
    }
}
```

これを `resolve_expr(init)` 呼び出しの前に配置する。`resolve_arrow_expr` が呼ばれると `current_fn_return_type` が使用され、expression body への伝搬が発動する。

ただし、`visit_var_decl` では `resolve_expr(init)` が 2 回呼ばれている（line 184 と line 220）ため、適切な位置への配置が必要。

## 関連ファイル

| ファイル | 関連内容 |
|---|---|
| `tests/fixtures/var-type-alias-arrow.input.ts` | テスト入力 |
| `tests/integration_test.rs:416-419` | テスト定義 |
| `src/pipeline/type_resolver.rs:177-230` | `visit_var_decl` |
| `src/pipeline/type_resolver.rs:1207-1260` | `resolve_arrow_expr` |
| `src/pipeline/type_resolver.rs:626-710` | `propagate_expected` |
| `src/transformer/functions/mod.rs:1133-1147` | Transformer 側の return type 取得 |
| `src/transformer/functions/mod.rs:1278-1301` | `extract_fn_return_type` |
| `src/transformer/expressions/functions.rs:221-380` | `convert_arrow_expr_with_return_type` |
| `src/registry.rs:861-888` | `try_collect_fn_type_alias` |
