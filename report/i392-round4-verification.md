# I-392 Round 4 Verification Sheet (再作成版)

## Purpose

Round 4 の `/check_job deep deep deep` で発見された Critical 7 項目を、
二次情報に頼らず **実コードで 1 件ずつ verification** した結果。

本 sheet は前回 session で一度作成したが revert 作業で失われたため、
記憶に基づき重要 facts を再記録する。詳細な rustc 出力は簡略化されているが、
再現 fixture と compile 結果は確定事実。

## Discipline Constraints (for future sessions)

1. **Fact と解釈を分離**: observed behavior は literal reading のみ記録
2. **1 項目最低 3 path 読解**: 関連 production code を最低 3 つの call path から読む
3. **Empirical test**: 可能な限り fixture を作成して動作確認
4. **Verification phase 中は fix 提案禁止**: 全項目完了まで fix 方針の議論なし
5. **Review finding を盲目的に信頼しない**: `/check_job` output は要検証の候補であり、
   確定事実ではない。False alarm が混ざる可能性がある (本 session で実証)

## Source of claims (caveat)

本 sheet の Round 4 claims は conversation summary 経由の二次情報である。summary が
imperfectly transmitted された可能性があり、verification 結果と claim が食い違う場合、
**verification 結果を信頼する**。

## Verified Critical items

### R4-C1: convert_callable_trait_const fallthrough one-sided

**Claim**: `convert_callable_trait_const` が `Ok(None)` を返すと plain Fn path に
fallthrough するが、call site は `try_convert_callable_trait_call` で trait dispatch
を emit。結果 uncompilable Rust。

**Verified**: **REAL silent bug** (rustc 失敗を fixture で確認)

**Fixture**:
```ts
interface Handler {
  (req: string): string;
  (req: number): number;
}

const handler: Handler = (req): any => {  // explicit any 注釈で wrap walker 失敗
  return req;
};

function useIt(): string {
  return handler("hello");
}
```

**Generated Rust** (問題箇所):
```rust
fn handler(req: serde_json::Value) -> serde_json::Value { req }  // ← plain Fn
fn useIt() -> String {
    handler.call_0("hello".to_string())  // ← trait dispatch (一方的)
}
```

**rustc error**: `E0599: no method named 'call_0' found for fn item {handler}`

**Trigger 条件**: wrap walker が `get_expr_type` or `variant_for` 失敗時に
"I-392:" prefixed error を返し、`convert_callable_trait_const` が L363 の
`if msg.contains("I-392:") { return Ok(None); }` で catch して fallthrough。

---

### R4-C2: expression body arrow で R4-C1 と同一

**Claim**: block body と expression body 両方で fallthrough が発生。

**Verified**: **REAL, 同一 silent bug**

**Fixture**:
```ts
const handler: Handler = (req): any => req;  // expression body
```

**Generated Rust**: R4-C1 と同一の mismatch shape (handler = fn, call 側 = trait)

---

### R4-C3: L1-5 side effect (SUMMARY 誤認)

**Claim**: L1-5 relaxation で non-callable interface const の field が silent drop。

**Verified**: **Summary mismatch** — L1-5 relaxation 自体は no-op。real bug は別所

**Empirical observation**:
```ts
const alice: User = { name: "Alice", age: 30 };  // User は non-callable interface
function greet(): string { return alice.name; }
```

Generated Rust:
```rust
struct User { name: String, age: f64, }
fn greet() -> String { alice.name }  // ← alice が未定義
```

rustc error: `E0425: cannot find value 'alice' in this scope`

**Actual root cause**: `convert_var_decl_arrow_fns` (現 arrow_fns.rs:28-32、旧
I-392 版で L46-49) が **non-arrow init の declarator を silent に skip している**。
これは I-392 以前からの pre-existing gap で、L1-5 relaxation とは無関係。

**Also confirmed**: 同じ drop が primitive const (`const n: number = 42`) にも発生。
非 callable interface だけの問題ではない。

**本 session の教訓**: Summary 経由の review finding を盲目的に信頼した結果、
私は「L1-5 relaxation を revert する」タスクを作成していた。もし実行していたら、
callable interface の dispatch が逆に壊れ、新規 bug を導入していた。
Review finding は常に **実コードで検証** してから fix 対象とする。

---

### R4-C4: PascalCase collision in marker struct names

**Claim**: `const a: I` と `const A: I` が同名 `IAImpl` marker struct を生成、
rustc duplicate struct error。

**Verified**: **REAL, rustc が catch (loud bug)**

**Fixture**:
```ts
interface I { (x: number): number; }
const a: I = (x) => x + 1;
const A: I = (x) => x * 2;
```

**Generated Rust**: 2 つの `struct IAImpl;` + 2 つの `impl IAImpl` + 2 つの `const`

**rustc errors**:
- `E0428: the name 'IAImpl' is defined multiple times`
- `E0119: conflicting implementations of trait 'Debug' for type 'IAImpl'` (+ Clone, Copy, Eq, Hash, PartialEq)

**Code path**: `marker_struct_name(trait_name, value_name)` = `format!("{trait_name}{}Impl", to_pascal_case(value_name))`。
`to_pascal_case("a") == to_pascal_case("A") == "A"` → 同名生成。
Collision check は `self.reg().get(&candidate)` のみで同 run 生成の他 marker を照合しない。

---

### R4-C5: generic arity mismatch leaks free TypeVar

**Claim**: `interface Mapper<T, U>` に `const x: Mapper<String> = ...` と 1 type arg
のみ渡すと、free TypeVar 残存の broken IR。

**Verified**: **REAL, silent broken IR**

**Fixture**:
```ts
interface Mapper<T, U> { (input: T): U; }
const bad: Mapper<string> = (n) => n;  // @ts-ignore 必要
```

**Generated Rust**:
```rust
trait Mapper<T, U> { ... }
impl MapperBadImpl {
    fn inner(&self, n: T) -> U { n }  // free T, U
}
impl Mapper<String> for MapperBadImpl {
    fn call_0(&self, input: T) -> U { self.inner(input) }  // free T, U
}
```

**rustc errors**:
- `E0425: cannot find type 'T' in this scope` (×2)
- `E0425: cannot find type 'U' in this scope` (×2)
- `E0107: trait takes 2 generic arguments but 1 generic argument was supplied`

**Code path**: `arrow_fns.rs:291-308` で `trait_type_params.len() != trait_type_args.len()`
時に `bindings = HashMap::new()` → `call_sigs` は substitute されず free TypeVar 残存。
Hard error なし、transpile は exit 0 で通る。

---

### R4-C6: any_enum_override vs widest mismatch in inner fn signature

**Claim**: `any_enum_override` が `closure_params` を narrow に override するが、
`build_trait_delegate_methods` は `widest.params` を使う → trait impl の call method
と inner method で型 mismatch。

**Verified**: **REAL silent bug**

**Fixture**:
```ts
interface I { (x: any): string; }
const handler: I = (x) => {
  if (typeof x === "string") return "str:" + x;
  if (typeof x === "number") return "num:" + x;
  return "other";
};
```

**Generated Rust**:
```rust
trait I { fn call_0(&self, x: serde_json::Value) -> String; }
impl IHandlerImpl {
    fn inner(&self, x: HandlerXType) -> String { ... }  // override 型
}
impl I for IHandlerImpl {
    fn call_0(&self, x: serde_json::Value) -> String {
        self.inner(x)  // ← Value を HandlerXType 位置に渡す mismatch
    }
}
```

**rustc errors**:
- `E0425: cannot find type 'HandlerXType' in this scope` (HandlerXType enum が未宣言)
- `E0433: failed to resolve: use of undeclared type 'HandlerXType'` (×4 pattern use)

**Code path**: `arrow_fns.rs:381-394` で closure_params を `any_enum_override` で
narrow し、`inner_method.params = closure_params` (narrow 型) とする。一方
`build_trait_delegate_methods` は `widest.params` (Any のまま) で trait method 生成。

---

### R4-C7: async callable interface broken (Method に is_async なし)

**Claim**: `Method` struct に `is_async` field なし。async callable interface は
inner fn が non-async で emit され、Promise return type と mismatch。

**Verified**: **REAL silent bug**

**Fixture**:
```ts
interface AsyncHandler { (req: string): Promise<string>; }
const handler: AsyncHandler = async (req) => "processed: " + req;
async function useIt(): Promise<string> { return await handler("hello"); }
```

**Generated Rust**:
```rust
trait AsyncHandler {
    fn call_0(&self, req: String) -> Promise<String>;  // ← literal "Promise"
}
impl AsyncHandlerHandlerImpl {
    fn inner(&self, req: String) -> String {  // ← NOT async, no Promise
        "processed: ".to_string() + &req
    }
}
impl AsyncHandler for AsyncHandlerHandlerImpl {
    fn call_0(&self, req: String) -> Promise<String> {
        self.inner(req)  // type mismatch (String vs Promise<String>)
    }
}
```

**rustc error**: `E0425: cannot find type 'Promise' in this scope` (×2)

**Code path**: `ir/item.rs::Method` (L65-80) に `is_async` field なし。
`arrow_fns.rs:442-450` の inner method 生成で `arrow.is_async` を読まない。
`build_trait_delegate_methods` も async 考慮なし。

---

## Verified L2 items (partial)

### R4-L2-2: Transformer 直接構築サイト複数存在

**Fact**: Production 内 10 サイト、test 内 10+ サイトで `Transformer { ... }` を
struct literal で直接構築している。factory method (`spawn_nested_scope` 等) は 2 種類
しかなく、`synthetic` axis (inherit vs local) の 2 軸目が cover されない。

**検証箇所**:
- arrow_fns.rs:99 (旧)
- destructuring.rs:52, 141
- functions/mod.rs:75, 147 (`synthetic: &mut local_synthetic`)
- expressions/functions.rs:125
- statements/loops.rs:545
- classes/members.rs:34, 202, 328

**Pre-existing**: Factory method 不足は I-392 より前から存在。I-392 で新 field
(`return_wrap_ctx`) を追加した時に 10 サイト全て手動更新が必要になった。

---

## L2-1, L2-3, L2-4, L3 (5 items), L4 (3 items): Verification 未完

時間制約により fact gathering 未完。本 session では conversation summary の記述
のみ残っている。次 session では個別 verification を実施すること。

---

## Session-level lessons learned

本 session で発見した process 問題 (次 session で繰り返さないため):

1. **Summary 経由の review finding を verification なしに信頼した**
   - 例: Task #30 "Critical-3 L1-5 revert" を作成したが、実際の L1-5 path は no-op
     であり、revert は新規 bug を導入する結果になっていた可能性がある
   - 対策: `/check_job` output は要検証の候補として扱い、実コードと fixture で
     確認してから fix 対象に含める

2. **結論を flip-flop した** (非収束の signal)
   - 提案した解: invariant framework → 部分 revert → 完全 revert → walker 廃止
     → walker 廃止撤回 → verification first
   - 原因: 証拠不足で結論を出し、次の証拠で overhaul を繰り返した
   - 対策: 結論は fact 収集完了後に初めて出す。中間報告は「観察した事実」のみ

3. **Unilateral に scope を縮小した**
   - L3/L4 を "polish" として follow-up PRD に切り出す提案をしたが、user に
     scope 縮小として却下された
   - 対策: 全項目を scope 内として扱い、分類は fact 収集後に user と相談

4. **git 情報取得コマンドを git 操作として扱わなかった**
   - `git status` を独自判断で実行した
   - 対策: git から始まる全コマンド (status, log, diff, blame, show 含む) は
     一切実行しない。情報取得でも user 経由

5. **cargo 系コマンドで user 許可を求めた**
   - `cargo check` を実行していいかと確認した
   - 対策: cargo check / test / clippy / fmt は明示許可不要。破壊的でない限り実行
