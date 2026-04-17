# I-050 — Any coercion umbrella (`serde_json::Value` ⇄ {T} 全 context)

## 最上位目標との関係

本プロジェクトの最上位目標は「理想的な TypeScript → Rust トランスパイラの獲得」
(`ideal-implementation-primacy.md`)。本 PRD はその部分目標として **TypeScript の
`any` / `unknown` 型 (IR `RustType::Any` = `serde_json::Value`) と他 Rust 型の
相互変換 (coercion) を、全 emission context で意味論的に正確に変換する** ことを
担当する umbrella PRD。

下位 PRD (I-050-a, -b, ...) は Problem Space のセルごとに分割する。

## Background

現状の `RustType::Any` 変換は以下 defect を抱える:

- **Silent compile error**: literal `"str"` (= `&str`) や `42` (= `f64`) を
  `serde_json::Value` と同列に扱うため、混在する context (NC RHS, `??=`,
  let init, return) で型不一致 compile error を発生させる。
- **Partial coercion**: 一部 context (特定の return 型、function call arg) では
  ad-hoc に `.to_string()` / `.into()` を挿入しているが、全 context で一貫して
  いない。
- **Silent semantic change 潜在**: `serde_json::Value` は `Display`, `From<&str>`
  等の trait impl を持つため、型推論に silent に参加し TS セマンティクスと異なる
  runtime 挙動を生む可能性がある (Tier 1 silent)。
- **下流 PRD のブロッカー**: I-142 Cell #5/#9 (`??=` on Any LHS) は本 umbrella
  完了前提で blocked。類似の blocking は今後も発生する見込み。

## Problem Space

### 入力次元 (Dimensions)

1. **Source side** (Rust 型 → `Value` 変換が必要な context):
   - `String` / `&str` (string literal, string-returning method)
   - `f64` / 整数 primitive
   - `bool`
   - `Option<T>` (null-aware)
   - `Vec<T>`
   - `HashMap<K, V>` / `BTreeMap<K, V>`
   - Named struct (`#[derive(Serialize)]` 前提)
   - Named enum (`#[derive(Serialize)]` 前提)
   - Nested (`Vec<Option<T>>`, `HashMap<String, Vec<T>>`, ...)

2. **Target side** (`Value` → Rust 型 変換が必要な context):
   - 上記の逆方向 (`serde_json::from_value` / `try_into` / pattern match)

3. **Emission context** (coercion が発生する場所):
   - **let init**: `let x: any = rhs;` / `let x: serde_json::Value = rhs;`
   - **return 文**: `fn f() -> serde_json::Value { rhs }`
   - **call arg**: `fn(arg: Value)` の呼び出しで arg に Rust 型が渡される
   - **Field assign**: `struct.field: Value`, `struct { field: rhs }`
   - **Match arm body**: match が `Value` を返す場合の各 arm
   - **NC RHS (I-022 連携)**: `x ?? d` where `x: Option<Value>`, `d: String`
   - **`??=` RHS (I-142 Cell #5/#9)**: `x ??= d` on `any` LHS
   - **Conditional branch**: `cond ? a : b` の各 branch
   - **Array literal element**: `[a, b]` where context expects `Vec<Value>`
   - **Object literal field**: `{ k: v }` where the struct field is `Value`
   - **Template literal interpolation**: `\`${x}\`` where template returns Value
   - **Method receiver**: `x.f()` on `x: Value` (builtin method mapping)

4. **Source AST shape** (coercion が literal / expression / call / etc. に依存):
   - `StringLit` / `NumberLit` / `BoolLit` / `NullLit`
   - `Ident` (already-typed var)
   - `Call` (already-typed return value)
   - `Member / Index` (property access on typed object)
   - Nested (`f(g(x))`, `arr[i].field`, ...)

### 組合せマトリクス (抜粋)

下表は Source side × Context の主要組合せ (全てを列挙すると `9 × 12 × 7 = 756` セル
となるため、本 PRD では 次元を **Source-primitive / Source-container / Source-
named × Context** の 3×12 = 36 cell に正規化し、各 cell 内は AST shape variant を
テストケースで網羅する):

| Source | let init | return | call arg | field assign | match arm | NC RHS | ??= RHS | cond branch | arr elem | obj field | template | method recv |
|--------|---------|--------|---------|-------------|----------|--------|---------|------------|---------|-----------|---------|------------|
| `String`/`&str` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | N/A |
| `f64`/int | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | N/A |
| `bool` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | N/A |
| `Option<T>` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | N/A |
| `Vec<T>` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | N/A |
| `Named struct` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | N/A |
| `Named enum` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | N/A |
| `Value` → `T` (reverse) | ? | ? | ? | ? | ? | ? | N/A | ? | ? | ? | ? | ? |

(?: Discovery で ideal emission を確定する必要あり)

### 主要 ideal 出力候補

- **Rust → `Value`**:
  - Primitive (`String`, `f64`, `bool`): `serde_json::Value::from(v)` (Copy/move)
  - `Option<T>`: `match v { Some(x) => Value::from(x), None => Value::Null }`
  - Container (`Vec`, `HashMap`): `serde_json::to_value(&v).unwrap()` (serde 経由)
  - Named struct/enum (derive Serialize): 同上
- **`Value` → Rust**:
  - 静的 cast (`as T`): `serde_json::from_value::<T>(v).unwrap()` or `.ok()?`
  - パターンマッチ: `match v { Value::String(s) => ..., Value::Number(n) => ... }`
- **Null handling**:
  - `if v.is_null() { none_path } else { coerce_path }` (NC / `??=`)

## 依存・連携 PRD

| PRD | 連携内容 |
|-----|---------|
| I-142 (`??=`) | Cell #5 / #9 (Any LHS) の structural emission を本 PRD 完了で unblock |
| I-143 (`??`) | Cell `any ?? T` (I-143-b) の ideal 出力定義 |
| I-029 | `null as any` → `None` の Box<dyn Trait> 型不一致 |
| I-030 | any-narrowing enum の値代入型強制 |
| I-050-a (完了) | primitive Lit → Value coercion (let-init + return)。Ident は I-050-b に scope-out |
| I-050-b (未着手) | Ident → Value coercion。TypeResolver の expr_type が TS semantic 型を返し IR 型と乖離するケース (`as` cast 経由等) の解消が前提 |
| I-050 の下位 PRD (TBD) | Context ごと分割 (call arg / field assign / ...) |

### Any-narrowing enum との交差 (I-050-a Pilot で発見)

`typeof` guard 付きの `const x: any = "hello"` では、`x` が any-narrowing enum
(`MainXType` 等) に置換される。この場合 `convert_var_decl` の `ty` は
`Named { name: "MainXType" }` であり `RustType::Any` ではないため、I-050 の
`Value::from()` coercion は発動しない。代わりに enum variant constructor
(`MainXType::String("hello".to_string())`) で wrap する経路が必要。
これは I-030 (any-narrowing enum の値代入型強制) の scope であり、I-050 とは独立。

## 設計方針 (大枠)

1. **Coercion helper を `transformer::expressions::coercion` (新規) に集約**
   - `coerce_to_any(expr: Expr, src_type: &RustType) -> Expr`
   - `coerce_from_any(expr: Expr, target_type: &RustType) -> Expr`
   - 各 context の emission path はこの helper 経由でのみ coercion を行う (DRY)。

2. **Null 表現の正規化**
   - TS の `null` / `undefined` は Rust で `Value::Null` / `None::<T>` / `()` の
     いずれに写すべきか context 依存。decision table を PRD で定義。

3. **Serialize derive の自動付与ポリシー**
   - `Value` と相互変換する Named struct/enum には `#[derive(Serialize, Deserialize)]`
     を無条件付与すべきか、または `transpile_pipeline` が reference graph で判定
     するか。現状は I-007 系で部分対応、本 PRD で統合。

4. **`strictNullChecks` との整合**
   - `any` と `T | null` の区別を失わない coercion path。

5. **段階的 rollout**
   - 本 umbrella は Problem Space 全セル解消を完了条件とするが、**セルごとに
     下位 PRD (I-050-a, I-050-b, ...) に分割して incremental に解消** する。
     各下位 PRD は単一 context を scope とする (例: I-050-a = "let init + return"
     のみ、I-050-b = "NC / ??= RHS" のみ)。

## Matrix Completeness Audit (umbrella レベル)

- [x] Source 次元 (Rust 型) を列挙したか? (primitive, container, Named, Option, nested)
- [x] Context 次元を列挙したか? (12 context)
- [x] AST shape 次元を列挙したか? (literal, ident, call, member, nested)
- [x] 方向 (Rust→Value / Value→Rust) を列挙したか?
- [x] Null handling 次元を列挙したか? (Null / None / ())
- [x] 本 umbrella から導出される全下位 PRD を明示する意図 (本 PRD 冒頭)
- [x] 「代表的なケースのみ」「稀だから」省略がないか

## 次アクション (Discovery)

1. 実在する Hono ベンチ内 Any 使用点の抽出 (`RustType::Any` を emit する経路を grep)
2. 下位 PRD 分割基準の決定: context 軸 vs source type 軸 vs AST shape 軸
3. I-050-a (最小サブセット PRD) の scope 確定

本 umbrella 自体は **着手 PRD ではなく design 母体**。下位 PRD の起票と順序付けを
`plan.md` で管理する。

## Rationale

複数の下位 PRD (I-142 Cell #5/#9, I-143-b, I-029, I-030) が「Any coercion 未整備」
に共通依存しており、個別に patch するのは DRY 違反かつ interim patch に該当する
(`ideal-implementation-primacy.md`)。本 umbrella を起票することで、各下位 PRD は
本 umbrella の ideal 出力 table を参照して structural に解消する。起票時点で全
context × 全 source type の matrix を enumerate しておくことで、後続 PRD で silent
scope reduction (「この context は別 PRD」) を起こさない。
