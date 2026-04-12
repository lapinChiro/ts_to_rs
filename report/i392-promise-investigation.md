# I-392 P0.2: Promise<T> 変換経路調査

## 調査結果

### Q1: RustType::unwrap_promise() method は存在するか?

**NO — 存在しない。**

`src/ir/types.rs:190-229` の `impl RustType` は以下 2 method のみ:
- `uses_param(&self, param: &str) -> bool`
- `wrap_optional(self) -> RustType`

代わりに 2 つのスタンドアロン関数が存在:
1. `unwrap_promise_type()` — `src/transformer/functions/helpers.rs:33-41`
2. `unwrap_promise_and_unit()` — `src/pipeline/type_resolver/helpers.rs:233-245`

### Q2: async arrow の変換

`src/transformer/functions/arrow_fns.rs:108-117`:
```rust
let item = Item::Fn {
    is_async: arrow.is_async,  // L111: SWC AST から直接コピー
    return_type: ret,          // Promise<T> は unwrap されない
    ...
};
```

- `is_async` マーカー: **YES** — `Item::Fn` に `is_async: bool` field あり (`src/ir/item.rs:166`)
- Promise<T> unwrap: **NO** — arrow 変換パスでは unwrap されない
- 注: `convert_fn_decl` (`src/transformer/functions/mod.rs:117`) では `unwrap_promise_type` が呼ばれる

### Q3: interface call signature での Promise<T> 処理

`convert_interface_as_fn_type` (`src/pipeline/type_converter/interfaces.rs:139-241`):
- **Promise<T> unwrap なし**: `convert_ts_type()` の結果をそのまま使用
- **async 判定なし**: call signature に async 属性を抽出・保持する処理なし
- Phase 4.2 で Promise<T> 判定 + unwrap ロジックを追加する必要あり

### Q4: unwrap_promise_and_unit() の動作

`src/pipeline/type_resolver/helpers.rs:233-245`:
```
Promise<T> → T (unwrap) → Unit filter (void → None)
String → Some(String)
Unit (void) → None
```
- TypeResolver 段階で使用 (fn_exprs.rs, visitors.rs)
- Promise unwrap + Unit (void) → None 変換を 1 関数で行う

### Q5: unwrap_promise_type() の動作

`src/transformer/functions/helpers.rs:33-41`:
```
Promise<T> → Some(T)
Other → Some(Other)   // passthrough
```
- Transformer 段階で使用 (`convert_fn_decl` の async fn body)
- Promise unwrap のみ。Unit フィルタはしない (caller 側で事前処理済)

## Phase 4.2 設計への影響

1. **`RustType::unwrap_promise()` method を新規追加** (`src/ir/types.rs`)
   - INV-6 (Promise 処理の単一関数集約) に必要
   - 既存 2 関数は別責務 (Resolver 用 / Transformer 用) だが、core logic は共通
   - `RustType::unwrap_promise()` を追加し、既存 2 関数はこの method を内部で使う
2. **async overload の判定**: call signature の return type が `Promise<T>` かを判定
   - `Named { name: "Promise", type_args: [inner] }` パターンマッチで判定
3. **trait method の async + unwrap**: `async fn call_N(...) -> T` (Promise<T> → T に unwrap)
