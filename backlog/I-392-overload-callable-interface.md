# I-392: Overloaded callable interface の完全な型保持変換

## Background

`convert_interface_as_fn_type` (interfaces.rs:158-161) が `max_by_key(|s| s.params.len())`
で最長 overload のみを採用し、他の overload を silent に破棄している。

```typescript
interface GetCookie {
    (c: Context): Cookie;                                    // overload 1
    (c: Context, key: string): string | undefined;           // overload 2
    (c: Context, key: string, prefix?: PrefixOpts): string | undefined; // overload 3
}
```

現状の変換: `type GetCookie = Box<dyn Fn(Context, String, Option<PrefixOpts>) -> Option<String>>`
→ overload 1 の `Cookie` return type が消失。overload 1 で呼び出すコードが silent semantic change。

Hono で 4 つの overloaded callable interface が実在（GetCookie, GetSignedCookie, SetHeaders, SetMetric）。

## Goal

Multi-overload callable interface を、各 overload の return type を正確に保持した
struct + impl 表現に変換する。call site で overload resolution を行い、正しい method と
return type を生成する。

## Scope

### In Scope

- `convert_interface_as_fn_type` の multi-overload パスを struct + impl 生成に変更
- TypeResolver の Ident callee パスで call_signatures ベースの overload resolution
- Transformer の Ident callee パスで multi-overload callable の method call 生成
- `resolve_fn_type_info` の arg_count=0 固定バグ修正
- Bug-affirming test (`test_convert_interface_call_signature_overload_uses_longest`) 修正
- Single-overload callable interface は現状維持（`Item::TypeAlias { ty: Fn }` — ideal）

### Out of Scope

- I-181 (call signature generic type params) — I-392 の struct 表現確定後に再評価
- Overloaded method signatures（`interface I { method(x: string): void; method(x: number): void; }`）— 別の code path (`convert_method_signature`)
- `type_aliases.rs` 内の call signature overload 処理（line 262）— 同パターンだが別 PRD

## Design

### Technical Approach

#### 分岐条件

`convert_interface_as_fn_type` 内で overload 数を判定:
- **Single overload (call_sigs.len() == 1)**: 現行の `Item::TypeAlias { ty: Fn }` 生成（変更なし）
- **Multi overload (call_sigs.len() >= 2)**: 新パス — struct + impl 生成

#### Multi-overload の生成物

```typescript
interface GetCookie {
    (c: Context): Cookie;
    (c: Context, key: string): string | undefined;
}
```

→ 3 つの IR Item を生成:

**1. Widest Fn の union return type (synthetic enum)**

全 overload の return type → `register_union` → synthetic enum `CookieOrOptionString`

**2. Wrapper struct**

```rust
pub struct GetCookie {
    pub inner: Box<dyn Fn(Context, Option<String>) -> CookieOrOptionString>,
}
```

- params: 最長 overload の全パラメータ。短い overload で存在しない位置は `Option<T>` ラップ
- return: 全 overload の return type の union (synthetic enum)

**3. Impl block (overload 別 method)**

```rust
impl GetCookie {
    pub fn call(&self, c: Context) -> Cookie {
        match (self.inner)(c, None) {
            CookieOrOptionString::Cookie(v) => v,
            _ => unreachable!("guaranteed by TypeScript type checker"),
        }
    }
    pub fn call_with_1(&self, c: Context, key: String) -> Option<String> {
        match (self.inner)(c, Some(key)) {
            CookieOrOptionString::OptionString(v) => v,
            _ => unreachable!("guaranteed by TypeScript type checker"),
        }
    }
}
```

- method 命名: `call` (shortest overload), `call_with_N` (N = 追加 param 数)
  - 同一引数数の overload → `call_N_0`, `call_N_1` (index suffix)
- 各 method: self.inner を overload の引数パターンで呼び出し → return type を narrowing

#### TypeResolver 修正

`resolve_fn_type_info` (helpers.rs:314-322) が `select_overload(call_signatures, 0, &[])` と
arg_count=0 を固定渡ししている。呼び出し元 (`resolve_call_expr` line 46) から arg_count と
arg_types を伝播する。

`resolve_call_expr` の Ident callee パス (line 37-61) を拡張:
```
lookup_var → Named("GetCookie") →
  registry.get("GetCookie") → TypeDef::Struct { call_signatures: [...], ... } →
  call_signatures が非空 →
    select_overload(call_signatures, call.args.len(), &arg_types) →
    specific overload の return_type を返す
```

#### Transformer 修正

`calls.rs` の Ident callee 分類 (line 111-116) を拡張:
```rust
Some(TypeDef::Struct { call_signatures, .. }) if call_signatures.len() >= 2 => {
    // Multi-overload callable interface → method call
    let overload_idx = determine_overload_index(call_signatures, args);
    let method_name = overload_method_name(overload_idx, call_signatures);
    // Generate: Expr::MethodCall { object: fn_name, method: method_name, args }
}
Some(TypeDef::Struct { call_signatures, .. }) if call_signatures.len() == 1 => {
    // Single-overload → existing behavior (direct Fn call)
    CallTarget::Free(fn_name)
}
```

### Design Integrity Review

- **Higher-level consistency**: struct + impl 生成は `convert_interface_as_struct_and_trait` (line 248-334) の既存パターンに準拠。新規パスではなく既存パターンの応用
- **DRY**: overload method 命名 / narrowing match 生成は `convert_interface_as_fn_type` 内に局所化。`select_overload` を TypeResolver + Transformer で共有（既存）
- **Coupling**: TypeResolver → registry (既存), Transformer → registry (既存)。新規依存なし
- **Broken windows**: `resolve_fn_type_info` の arg_count=0 固定バグを修正。`type_aliases.rs:262` の同パターンは別 PRD（I-392 は interface のみ）

### Impact Area

| File | Change |
|------|--------|
| `src/pipeline/type_converter/interfaces.rs` | multi-overload → struct + impl 生成 |
| `src/pipeline/type_resolver/helpers.rs` | `resolve_fn_type_info` arg_count 伝播 |
| `src/pipeline/type_resolver/call_resolution.rs` | Ident callee → call_signatures 参照 |
| `src/transformer/expressions/calls.rs` | multi-overload → method call 生成 |
| `src/pipeline/type_converter/tests/interfaces.rs` | テスト修正 + 追加 |
| `src/pipeline/type_resolver/tests/complex_features.rs` | overload resolution テスト |

### Semantic Safety Analysis

**型フォールバックの分析:**

1. **Widest Fn の return type (union)**: struct の `inner` フィールドが union return。各 overload method が narrowing で特定型を返す → **Safe**（method の return type が正確）
2. **Optional params**: 短い overload にないパラメータを `Option<T>` でラップ → **Safe**（None で呼ぶことが overload 選択と等価）
3. **`unreachable!()` in narrowing**: TS type checker が implementation signature の互換性を保証。正しい TS からの変換では到達しない → **Safe**（TS type checker 保証）

**Verdict**: Safe — 各 overload method の return type は TS の overload 宣言と 1:1 対応。union は `inner` field のみ。

## Task List

### T1: Multi-overload struct + impl 生成

- **Work**: `src/pipeline/type_converter/interfaces.rs` の `convert_interface_as_fn_type` を修正:
  1. `call_sigs.len() == 1` → 現行ロジック維持
  2. `call_sigs.len() >= 2` → `generate_overloaded_callable_struct` 新関数:
     - Widest params 計算: 位置ごとに全 overload のパラメータ型を収集、異なれば union、短い overload で不在の位置は Option
     - Union return type: 全 overload の return type → `synthetic.register_union`
     - `Item::Struct { fields: [StructField { name: "inner", ty: Box<dyn Fn(widest_params) -> UnionReturn> }] }`
     - `Item::Impl { methods: [Method per overload] }` — 各 method は inner を呼んで narrowing match
  3. `vec![struct_item, impl_item]` を返す（返り値型を `Result<Vec<Item>>` に統一）
- **Completion criteria**:
  - `test_convert_interface_call_signature_overload_generates_struct` pass
  - `test_convert_interface_single_overload_still_fn_type` pass
  - `cargo check` pass
- **Depends on**: None

### T2: TypeResolver の overload resolution 修正

- **Work**:
  1. `helpers.rs` の `resolve_fn_type_info` を修正: arg_count, arg_types を引数追加。`select_overload(call_signatures, arg_count, arg_types)` に伝播
  2. `call_resolution.rs` の Ident callee パス (line 37-61) を拡張:
     - `lookup_var` → `Named(name)` → `registry.get(name)` → `TypeDef::Struct { call_signatures, .. }` で call_signatures 非空の場合
     - `select_overload(call_signatures, call.args.len(), &arg_types)` で overload 選択
     - 選択された overload の return_type を返す
  3. `set_call_arg_expected_types` も同様に call_signatures 対応
- **Completion criteria**:
  - `test_resolve_call_overloaded_callable_selects_by_arg_count` pass
  - `test_resolve_fn_type_info_with_arg_count` pass
  - 既存 TypeResolver テスト全件 pass
- **Depends on**: T1

### T3: Transformer の method call 生成

- **Work**: `src/transformer/expressions/calls.rs` の Ident callee 分類 (line 111-116) を拡張:
  1. `TypeDef::Struct { call_signatures, .. }` で `call_signatures.len() >= 2` の場合:
     - `select_overload` で overload index を特定
     - method name を生成（`call`, `call_with_1`, ... or `call_N_0`, `call_N_1`）
     - `Expr::MethodCall { object: Expr::Ident(fn_name), method, args }` を生成
  2. `call_signatures.len() == 1` の場合: 現行 `CallTarget::Free` を維持
- **Completion criteria**:
  - `test_transform_overloaded_callable_generates_method_call` pass
  - 既存 Transformer テスト全件 pass
- **Depends on**: T1, T2

### T4: Bug-affirming test 修正 + テスト補完

- **Work**:
  - `test_convert_interface_call_signature_overload_uses_longest` を修正:
    `Item::TypeAlias` ではなく `[Item::Struct, Item::Impl]` を assert
  - 新規テスト:
    - 2 overload (異なる引数数 + 異なる return type)
    - 3 overload (Hono GetCookie パターン)
    - 2 overload (同一引数数、異なる引数型)
    - single overload → TypeAlias のまま（regression guard）
    - TypeResolver: overload 別 return type 解決
    - Transformer: method call 生成
- **Completion criteria**: 全テスト pass
- **Depends on**: T1, T2, T3

### T5: 既存テスト修正 + snapshot 更新

- **Work**: callable-interface fixture の snapshot が struct 生成に変わるため更新。
  `type_alias_forms.rs` の overload テスト (`TG-6`) も確認・修正
- **Completion criteria**: `cargo test` 全件 pass、`cargo insta review` 完了
- **Depends on**: T1

### T6: Quality check + Hono bench

- **Work**: `cargo clippy`, `cargo fmt`, `cargo test`, `./scripts/hono-bench.sh`
- **Completion criteria**: clippy 0 warning, fmt 0 diff, bench regression 0
- **Depends on**: T5

## Test Plan

### 新規テスト（機能変更由来）

| Test | Input | Expected | Task |
|------|-------|----------|------|
| multi-overload struct generation | `interface I { (x: string): string; (x: string, y: number): boolean }` | `[Struct { inner: Fn }, Impl { call, call_with_1 }]` | T1 |
| 3-overload (Hono pattern) | GetCookie 3 overloads | Struct + Impl with 3 methods | T1 |
| same-arity overload | `interface I { (x: string): string; (x: number): number }` | Struct + Impl with call_0, call_1 | T1 |
| single overload unchanged | `interface I { (x: string): void }` | `TypeAlias { ty: Fn }` | T1 |
| TypeResolver arg-count resolution | `getCookie(ctx)` vs `getCookie(ctx, "key")` | Cookie vs Option<String> | T2 |
| resolve_fn_type_info with arg_count | call_signatures + arg_count=1 | correct overload return | T2 |
| Transformer method call | `getCookie(ctx, "key")` | `get_cookie.call_with_1(ctx, key)` | T3 |

### 既存テスト修正

| Test | 問題 | 修正 |
|------|------|------|
| `test_convert_interface_call_signature_overload_uses_longest` | Bug-affirming | Assert Struct + Impl |
| `test_convert_type_alias_call_signature_overload_picks_most_params` | 同パターン | 確認（type_aliases.rs は scope 外だが影響確認） |

## Completion Criteria

1. Multi-overload callable interface が Struct + Impl (per-overload method) を生成
2. 各 overload method の return type が TS 宣言と 1:1 対応
3. Single-overload callable interface が TypeAlias { Fn } のまま（regression なし）
4. TypeResolver が call site で overload 別の return type を解決
5. Transformer が multi-overload callee を method call に変換
6. `cargo test` 全件 pass
7. `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
8. `cargo fmt --all --check` 0 diff
9. Hono bench regression 0
