# Emission Context Catalog (Beta)

**Version snapshot**: transformer code base (2026-04-17)
**Pilot validated**: I-050-a (2026-04-17) — #1 (let-init) と #3 (return) を matrix 列挙に使用、漏れなし

本ドキュメントは `spec-first-prd.md` の grammar-derived matrix 作成時に参照する。
PRD の入力次元 (outer context / emission context) を列挙する際、本カタログの
全 context について「この機能が当該 context で発生し得るか」を判定する。

**更新トリガー**: `propagate_expected` / `convert_expr_with_expected` の経路変更時、
新 emission context の追加時に同時更新。

---

## 凡例

- **expected type propagation**: TypeResolver が式の型期待値を子式に伝播する経路
- **conversion behavior**: Transformer が expected type に基づいて emit 方法を変える箇所

---

## 1. Statement-Level Contexts

式が文の一部として出現する context。

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 1 | **Variable declaration init** (annotation あり) | 型注釈 | `statements/mod.rs::convert_var_decl` | `let x: T = expr` |
| 2 | **Variable declaration init** (annotation なし) | None (型推論) | `statements/mod.rs::convert_var_decl` | `let x = expr` |
| 3 | **Return statement** | 関数の return type | `statements/mod.rs::convert_stmt` (Return arm) | `return expr` |
| 4 | **Expression statement** | None | `statements/mod.rs::convert_stmt` (Expr arm) | `expr;` (standalone) |
| 5 | **Throw statement** | `Result::Err` 型 | `statements/error_handling.rs::convert_throw_stmt` | `throw expr` → `Err(...)` |

---

## 2. Operator Contexts

式が演算子のオペランドとして出現する context。

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 6 | **Assignment RHS** (`=`) | LHS の型 | `expressions/assignments.rs::convert_assign_expr` | `x = expr` |
| 7 | **Compound assign RHS** (`+=` 等) | LHS の型 | `expressions/assignments.rs::convert_assign_expr` | `x += expr` |
| 8 | **NullishAssign RHS** (`??=`) | LHS inner type | `expressions/assignments.rs` / `statements/nullish_assign.rs` | `x ??= expr` |
| 9 | **Binary operator LHS/RHS** | 対向の型 / None | `expressions/binary.rs::convert_bin_expr` | `a + b`, `a && b` |
| 10 | **Nullish coalescing LHS** | `Option<T>` | `type_resolver/expected_types.rs::propagate_expected` NC arm | `expr ?? d` |
| 11 | **Nullish coalescing RHS** | inner T | `type_resolver/expected_types.rs::propagate_expected` NC arm | `x ?? expr` |
| 12 | **Unary operand** | 演算子依存 | `expressions/binary.rs::convert_unary_expr` | `!expr`, `-expr`, `typeof expr` |
| 13 | **Update operand** | numeric | `expressions/assignments.rs::convert_update_expr` | `expr++`, `--expr` |

---

## 3. Conditional / Control Flow Contexts

式が制御構造内で出現する context。

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 14 | **If condition** | bool (implicit) | `statements/control_flow.rs::convert_if_stmt` | `if (expr)` |
| 15 | **While condition** | bool (implicit) | `statements/loops.rs::convert_while_stmt` | `while (expr)` |
| 16 | **Do-while condition** | bool (implicit) | `statements/loops.rs::convert_do_while_stmt` | `do {} while (expr)` |
| 17 | **For condition** | bool (implicit) | `statements/loops.rs::convert_for_stmt` | `for (;expr;)` |
| 18 | **For init** | None | `statements/loops.rs::convert_for_stmt` | `for (expr;;)` |
| 19 | **For update** | None | `statements/loops.rs::convert_for_stmt` | `for (;;expr)` |
| 20 | **For-of iterable** | iterable 型 | `statements/loops.rs::convert_for_of_stmt` | `for (x of expr)` |
| 21 | **For-in iterable** | object 型 | `statements/loops.rs::convert_for_in_stmt` | `for (k in expr)` |
| 22 | **Switch discriminant** | match 対象型 | `statements/switch.rs::convert_switch_stmt` | `switch (expr)` |
| 23 | **Switch case test** | discriminant 型 | `statements/switch.rs::convert_switch_stmt` | `case expr:` |
| 24 | **Ternary condition** | bool | `expressions/mod.rs::convert_cond_expr` | `expr ? a : b` |
| 25 | **Ternary consequent** | outer expected type | `type_resolver/expected_types.rs::propagate_expected` Cond arm | `c ? expr : b` |
| 26 | **Ternary alternate** | outer expected type | `type_resolver/expected_types.rs::propagate_expected` Cond arm | `c ? a : expr` |

---

## 4. Function / Call Contexts

式が関数呼び出しの一部として出現する context。

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 27 | **Call argument** (positional) | callee の param type | `type_resolver/call_resolution.rs` | `f(expr)` |
| 28 | **Call argument** (rest param) | Vec\<T\> の element type | `type_resolver/call_resolution.rs` | `f(..., expr)` trailing |
| 29 | **Method call argument** | method signature param type | `type_resolver/call_resolution.rs` | `obj.m(expr)` |
| 30 | **New expression argument** | constructor param type | `expressions/calls.rs::convert_new_expr` | `new C(expr)` |
| 31 | **Method call receiver** | None (receiver 型は LHS から) | `expressions/member_access.rs::convert_member_expr` | `expr.method()` |
| 32 | **Callback body** (arrow/fn expr) | 外側の Fn return type | `type_resolver/expected_types.rs::propagate_expected` | `arr.map(x => expr)` |
| 33 | **Default parameter value** | parameter type (Option unwrap) | `type_resolver/expected_types.rs::propagate_expected` Assign arm | `function f(x = expr)` |

---

## 5. Data Structure Contexts

式がデータ構造の要素として出現する context。

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 34 | **Object literal field value** | struct field type / HashMap value type | `type_resolver/expected_types.rs::propagate_expected` Object arm | `{ key: expr }` |
| 35 | **Array literal element** | Vec\<T\> の element type / Tuple positional type | `type_resolver/expected_types.rs::propagate_expected` Array arm | `[expr, ...]` |
| 36 | **Spread source** (array) | Vec\<T\> 型 | `expressions/data_literals.rs::convert_array_lit` | `[...expr]` |
| 37 | **Spread source** (object) | struct 型 | `expressions/data_literals.rs::convert_object_lit` | `{ ...expr }` |
| 38 | **Template literal interpolation** | String (implicit) | `expressions/mod.rs::convert_template_literal` | `` `${expr}` `` |
| 39 | **Struct init field** | field type (IR level) | `generator/expressions.rs` | IR `StructInit { fields }` |
| 40 | **Destructuring source** | destructure target 型 | `statements/mod.rs::convert_var_decl` | `const { a } = expr` |
| 41 | **Destructuring default** | field type (Option unwrap) | `type_resolver/expected_types.rs::propagate_expected` | `const { a = expr } = obj` |

---

## 6. Type Assertion / Cast Contexts

式が型アサーション内で出現する context。

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 42 | **TsAs inner** | asserted type | `expressions/mod.rs::convert_expr` TsAs arm | `expr as T` |
| 43 | **TsNonNull inner** | inner type (non-null) | `expressions/mod.rs::convert_expr` TsNonNull arm | `expr!` |
| 44 | **Parenthesized inner** | outer expected (passthrough) | `type_resolver/expected_types.rs::propagate_expected` Paren arm | `(expr)` |

---

## 7. Member Access Contexts

式がメンバーアクセスの一部として出現する context。

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 45 | **Member access receiver** | None | `expressions/member_access.rs::convert_member_expr` | `expr.field` |
| 46 | **Computed index** | usize / String (key type) | `expressions/member_access.rs::convert_member_expr` | `expr[key]` |
| 47 | **OptChain receiver** | Option unwrap 後の型 | `expressions/member_access.rs::convert_opt_chain_expr` | `expr?.field` |

---

## 8. Special Contexts

| # | Context | expected type source | 主要 handler | 備考 |
|---|---------|---------------------|-------------|------|
| 48 | **Await operand** | Promise\<T\> → T unwrap | `expressions/mod.rs::convert_expr` Await arm | `await expr` |
| 49 | **Match arm body** | match return type | `statements/switch.rs::convert_switch_stmt` | `case: expr` body |
| 50 | **Class field initializer** | field type annotation | `classes/members.rs` | `field = expr` |
| 51 | **Class static block** | None | `classes/members.rs` | `static { expr; }` |

---

## 9. Expected Type Transformation Rules (`convert_expr_with_expected`)

expected type が `Option<T>` の場合の変換挙動:

| 入力式 | 変換結果 | 条件 |
|--------|---------|------|
| `null` / `undefined` | `None` | — |
| non-Option literal | `Some(convert(expr, T))` | wrap |
| already-Option expr | passthrough (no double-wrap) | `produces_option_result()` |
| `.find()` / `.pop()` / `.get().cloned()` | passthrough | structural Option producer |

expected type が trait 型の場合:
- `Box<dyn Trait>` wrapping が post-conversion で適用される

---

## PRD 作成時のチェックポイント

matrix の「outer context」次元を列挙する際、上記 51 context 全てについて
「この機能が当該 context で発生し得るか」を判定する。特に見落としやすい context:

- **#8 NullishAssign RHS**: `??=` の右辺は通常の代入 RHS と異なる propagation
- **#10/#11 NC LHS/RHS**: `??` の各辺は Option context で propagation
- **#33 Default parameter value**: Optional unwrap 後の型が propagate
- **#38 Template interpolation**: String への implicit coercion
- **#41 Destructuring default**: Option unwrap 後の型が propagate
- **#49 Match arm body**: switch case body の expected type 伝播は不完全 (I-143-f)
