# I-161 + I-171 — `&&=`/`||=` Compound Logical Assignment + `!<expr>` Generic Truthy Emission (Batch)

**Status**: Spec stage (SDCDF Beta)
**Created**: 2026-04-22
**Scope**: I-161 (TODO L3) + I-171 (TODO L3) batch
**Derived from**: I-144 Dual verdict framework (TS ✓ / Rust ✗ cells spun off as pre-existing defects)

## Background

I-144 (control-flow narrowing analyzer) の T1 per-cell E2E probe で、observation は
TS runtime ✓ preserved だったが Rust emission は compile error (RED) となる 2 件の
defect が `Dual verdict (TS / Rust)` framework で本 PRD scope に分離された:

- **I-161**: TS `x &&= y` / `x ||= y` の素朴 `x = x && y` emission が Rust の
  `&&` / `||` (bool 専用) と非互換で E0308。仕様上は `if (x) x = y` / `if (!x) x = y`
  に等価だが structural な desugar 未実装。empirical: I-144 `cell-regression-r4-logical-assign-preserves-narrow.ts` 試行で発覚 (2026-04-19)。
- **I-171**: TS `if (!x)` 汎用 truthy/falsy 変換は T6-3 で `!<Ident>` on `Option<T>`
  + always-exit body のみ対応。以下 9 pattern (b-k, c) は fall-through で
  `!<non-bool>` emit → Rust E0600 compile error。empirical: `/tmp/t63-investigate/pattern-{b,d,e,f,g,h,j,k}.ts` で全 pattern の compile error 確認 (2026-04-21)。

両者は `truthy_predicate_for_expr(expr, ty)` 汎用 helper を共有する structural fix で
解消可能なため、SDCDF Beta workflow に従い 1 PRD に batch する。

**最上位原則との関係**: `ideal-implementation-primacy.md` に従い、頻度 (現行 Hono で
直接 blocker なし) ではなく **正確性** を基準に structural fix を実装する。silent
semantic change は無いが (Tier 2 compile error)、`ideal transpiler` の完成条件として
全 valid TS → 等価 Rust を達成するため本 PRD は必須。

## Problem Space

`.claude/rules/problem-space-analysis.md` + `spec-first-prd.md` 準拠。本 PRD は
matrix-driven で、3 つの相関 sub-matrix (A / B / C) で問題空間を完全 enumerate する。

### 入力次元 (Dimensions)

**Matrix A: `&&=` / `||=` emission (I-161)**

| 次元 | variant | reference doc |
|------|---------|---------------|
| A.1 演算子 | `AndAssign` (`&&=`), `OrAssign` (`\|\|=`) | `ast-variants.md` §7 |
| A.2 LHS shape | `Ident`, `Member(Ident prop)`, `Member(Computed)`, `Member(PrivateName)` | `ast-variants.md` §9 SimpleAssignTarget |
| A.3 LHS effective type (narrow 適用後) | 12 equiv class (下記) | `rust-type-variants.md` §1 全 18 variant から導出 |
| A.4 context | `Stmt`, `Expr` (value observed) | `emission-contexts.md` §1, §2 |

**Matrix B: `!<expr>` emission 汎用 fix (I-171 Layer 1)**

| 次元 | variant | reference doc |
|------|---------|---------------|
| B.1 operand AST shape | 20 Tier 1 Expr variant (下記、Tier 2/3 は NA) | `ast-variants.md` §1 |
| B.2 operand effective type | 12 equiv class (Matrix A.3 共通) | `rust-type-variants.md` §1 |
| B.3 outer context | truthy-sensitive (if/while/do-while/for cond, ternary cond), typed (assignment/return/etc.), no-context | `emission-contexts.md` §3, §2, §6 |

**Matrix C: if-stmt narrow emission (I-171 Layer 2、T6-3 拡張)**

| 次元 | variant | reference doc |
|------|---------|---------------|
| C.1 test shape | `<expr>` (truthy), `!<expr>` (falsy) | `ast-variants.md` §6 |
| C.2 test operand shape | Ident, Member, OptChain, TsAs peek-through (unwrap inner), Other (BinExpr/UnaryExpr/LogicalAnd/Lit/Call など narrow 不可能 shape) | `ast-variants.md` §1 |
| C.3 body shape | always-exit, non-exit, mixed | IR (`ir_body_always_exits` 既存) |
| C.4 else branch | absent, present | `ast-variants.md` §2 If |
| C.5 operand type | Option<primitive>, Option<synthetic union>, Option<Named other>, Bool, F64, String, Primitive(int), Vec/Fn/Named/StdCollection (always truthy), Any (blocked I-050), TypeVar (blocked) | `rust-type-variants.md` §1 |

### LHS/operand effective type 12 equiv class (Matrix A.3 / B.2 共通)

narrow 適用後の実効型による grouping。`RustType::Option` は narrow 破れる前の宣言型
として現れ、narrow alive 時は inner に格納される。

| # | equiv class | Rust 型 | 典型 TS 由来型 | truthy predicate | falsy predicate |
|---|-------------|---------|--------------|------------------|-----------------|
| T1 | Bool | `bool` | `boolean` | `x` | `!x` |
| T2 | F64 | `f64` | `number` | `x != 0.0 && !x.is_nan()` | `x == 0.0 \|\| x.is_nan()` |
| T3 | String | `String` | `string` | `!x.is_empty()` | `x.is_empty()` |
| T4 | Primitive(int) | `i32`/`usize`/... | `number` (cast) | `x != 0` | `x == 0` |
| T5 | Option<primitive T> (T1-T4) | `Option<T>` | `T \| null/undefined` | `x.is_some_and(\|v\| <v T\# truthy>)` | `x.map_or(true, \|v\| <v T\# falsy>)` |
| T6 | Option<Named (synthetic union)> | `Option<U>` | `A \| B \| null` | match-based (per-variant) | match-based |
| T7 | Option<Named (other)> | `Option<U>` | `Interface \| null`, `Class \| null` | `x.is_some()` | `x.is_none()` |
| T8 | Named (struct/enum, non-union) / Vec / Fn / StdCollection / DynTrait / Tuple | various | interface, class, array, function, Record, Set, tuple literal | const `true` (always truthy) | const `false` |
| T9 | Any | `serde_json::Value` | `any`, `unknown` | **blocked I-050** | **blocked I-050** |
| T10 | TypeVar | `T` generic | `<T>` type param | **blocked** (bounds-dependent) | **blocked** |
| T11 | Never | `Infallible` | `never` | NA (unreachable) | NA |
| T12 | Unit / Ref(T) / QSelf / Option<Option<T>> / Tuple / Result | various | 各種 | case-specific (詳細は Matrix A/B 各セル) | 同上 |

**narrow 適用規則**: `get_expr_type(expr)` / `get_type_for_var(name, span)` は既に
narrow を適用した effective 型を返す (`type_resolution.rs:25-32, 42-47`)。本 PRD の
emission は effective 型に dispatch するだけで narrow-awareness を継承する。

### Matrix A — `&&=` / `||=` 直積 (24 primary cell × 4 orthogonal carrier)

**Primary cell (Op × effective type)**: 2 × 12 = 24 セル。以下表は `x &&= y` 形式
(LHS shape = Ident, context = Stmt、narrow alive 状態) で記述。他 carrier 組合せは
下表の直積 (詳細は Task T1-T2 unit test で網羅)。

| # | Op | LHS type | Ideal Rust (stmt) | 現状 emission | 判定 | Scope |
|---|----|----------|-------------------|---------------|------|-------|
| A-1 | `&&=` | T1 Bool | `if x { x = y; }` | `x = x && y;` ✗ | ✗ | 本 PRD |
| A-2 | `&&=` | T2 F64 | `if x != 0.0 && !x.is_nan() { x = y; }` | `x = x && y;` ✗ (E0308) | ✗ | 本 PRD |
| A-3 | `&&=` | T3 String | `if !x.is_empty() { x = y; }` | `x = x && y;` ✗ | ✗ | 本 PRD |
| A-4 | `&&=` | T4 Primitive(int) | `if x != 0 { x = y; }` | `x = x && y;` ✗ | ✗ | 本 PRD |
| A-5 | `&&=` | T5 Option<F64> | `if x.is_some_and(\|v\| v != 0.0 && !v.is_nan()) { x = Some(y); }` | `x = x && y;` ✗ | ✗ | 本 PRD |
| A-5s | `&&=` | T5 Option<String> | `if x.as_ref().is_some_and(\|v\| !v.is_empty()) { x = Some(y); }` | `x = x && y;` ✗ | ✗ | 本 PRD |
| A-6 | `&&=` | T6 Option<union enum> | `if match &x { Some(U::F64(v)) if v != 0.0 && !v.is_nan() => true, Some(U::String(s)) if !s.is_empty() => true, _ => false } { x = Some(y); }` (SG-2 統一 2026-04-22、Design section predicate-helper form に一致) | `x = x && y;` ✗ | ✗ | **narrow-alive 版は I-177 依存で deferred**、non-narrow 版は本 PRD |
| A-7 | `&&=` | T7 Option<Named other> | `if x.is_some() { x = Some(y); }` | `x = x && y;` ✗ | ✗ | 本 PRD |
| A-8 | `&&=` | T8 always-truthy (Named/Vec/Fn/HashMap/DynTrait) | const-fold: `x = y;` | `x = x && y;` ✗ | ✗ | 本 PRD (const-fold) |
| A-9 | `&&=` | T9 Any | **blocked** — runtime truthy on `Value` + RHS coerce | `x = x && y;` ✗ | ✗ | **別 PRD (I-050)** |
| A-10 | `&&=` | T10 TypeVar | **blocked** — bounds 依存 | `x = x && y;` ✗ | ✗ (defer) | **別 PRD** |
| A-11 | `&&=` | T11 Never | NA (IR invariant: `Never` 型変数は到達不能、assignment が到達する前に関数が exit) | NA | NA | — |
| A-12a | `&&=` | T12 Unit | T8 const-fold: `x = y;` (TS `void` 変数の runtime は undefined = falsy、`&&=` は no-op) → IR level Unit には truthy semantic 不在で decision が undefined。**本 PRD scope out**: Unit LHS はユースケース未発見、Hono bench で 0 件。発見時は UnsupportedSyntaxError で surface | — | defer | **別 PRD** (empirical 発火時起票) |
| A-12b | `&&=` | T12 Ref(T) | T8 const-fold `x = y;` (Rust `&T` / `&mut T` 変数は構造的に非 null で常に truthy、JS reference truthy semantic と整合。変数位置での Ref LHS は ownership 推論経由でしか発生せず、発生時は const-fold 適用で正しく動作。I-048 依存ではない) | — | defer | **本 PRD (T8 経路共用)、ただし Ref(T) LHS の user-facing pattern 未発見のため empirical 発火なし** |
| A-12c | `&&=` | T12 Option<Option<T>> | NA (IR invariant: `RustType::Option::wrap_optional()` は idempotent で double-wrap 不可、valid TS input から到達不能) | NA | NA | — |
| A-12d | `&&=` | T12 Tuple | T8 const-fold: `x = y;` (Tuple `[f64, String]` は JS Array と同じ always-truthy、`&&=` は直接 assign) | `x = x && y;` ✗ | ✗ | **本 PRD** (T8 const-fold 経路共用) |
| A-12e | `&&=` | T12 Result | NA (empirical verified 2026-04-22: `RustType::Result` 出現箇所を grep した結果、user TS input から emission される local 変数型としての Result は **存在しない**。唯一の Result 型 local は `_try_result` (try/catch 内部 emission、`src/transformer/statements/error_handling.rs:94`) で、これは ts_to_rs 自身が挿入する internal var であり user AST に対応する SimpleAssignTarget として `&&=`/`||=` の LHS にならない。ユーザーが `let r = try_something()` と書いた場合、ts_to_rs は Result を `try/match` で即分解し変数に Success 型を束縛するため、Result 型変数は emission されない) | NA | NA | — |
| A-12f | `&&=` | T12 QSelf | NA (associated type `<T as Trait>::Item` は type position 専用、value position 変数型として IR emit されず — conditional type infer が value 化する経路なし) | NA | NA | — |
| A-13 | `&&=` | PatternAssignTarget LHS (destructure `[a, b] &&= y`) | NA (ECMA-262 仕様: logical assignment operators は SimpleAssignTarget 限定、PatternAssignTarget は syntax error。SWC ast-variants.md §9 で Tier 2 unsupported 表記) | — (parser reject) | NA | — |
| A-14a | `&&=` | SimpleAssignTarget::SuperProp (`super.x &&= y`) | NA (ast-variants.md §9 SimpleAssignTarget Tier 2、`convert_assign_expr` の target match arm は Ident/Member のみ受付、他は `UnsupportedSyntaxError` 返却) | ✗ unsupported | NA | — |
| A-14b | `&&=` | SimpleAssignTarget::Paren (`(x) &&= y`) | NA (同 A-14a、Tier 2、実際の使用では parse が Paren を unwrap する場合も多いが AssignTarget 位置では SimpleAssignTarget::Paren として拒否) | ✗ unsupported | NA | — |
| A-14c | `&&=` | SimpleAssignTarget::OptChain (`x?.y &&= z`) | NA (同 A-14a、ECMA-262 でも OptChain を assignment target として使うのは short-circuit semantics により不自然、Tier 2) | ✗ unsupported | NA | — |
| A-14d | `&&=` | SimpleAssignTarget::TsAs (`(x as T) &&= y`) | NA (同 A-14a、TsAs 経由 LHS は Tier 2、peek-through 適用しても Ident/Member に行き着くため A-1〜A-12 で cover) | ✗ unsupported | NA | — |
| A-14e | `&&=` | SimpleAssignTarget::TsSatisfies (`(x satisfies T) &&= y`) | NA (同 A-14a、TsSatisfies 自体が I-115 unsupported) | ✗ unsupported | NA | — |
| A-14f | `&&=` | SimpleAssignTarget::Invalid | NA (parser error marker、SWC が syntax error 時に produce する sentinel、到達不能) | — (parser reject) | NA | — |
| O-1 | `\|\|=` | T1 Bool | `if !x { x = y; }` | `x = x \|\| y;` ✗ | ✗ | 本 PRD |
| O-2 | `\|\|=` | T2 F64 | `if x == 0.0 \|\| x.is_nan() { x = y; }` | `x = x \|\| y;` ✗ (E0308) | ✗ | 本 PRD |
| O-3 | `\|\|=` | T3 String | `if x.is_empty() { x = y; }` | `x = x \|\| y;` ✗ | ✗ | 本 PRD |
| O-4 | `\|\|=` | T4 Primitive(int) | `if x == 0 { x = y; }` | `x = x \|\| y;` ✗ | ✗ | 本 PRD |
| O-5 | `\|\|=` | T5 Option<F64> | `if !x.is_some_and(\|v\| v != 0.0 && !v.is_nan()) { x = Some(y); }` | `x = x \|\| y;` ✗ | ✗ | 本 PRD |
| O-5s | `\|\|=` | T5 Option<String> | `if !x.as_ref().is_some_and(\|v\| !v.is_empty()) { x = Some(y); }` | `x = x \|\| y;` ✗ | ✗ | 本 PRD |
| O-6 | `\|\|=` | T6 Option<union enum> | `if !match &x { Some(U::F64(v)) if v != 0.0 && !v.is_nan() => true, Some(U::String(s)) if !s.is_empty() => true, _ => false } { x = Some(y); }` (SG-2 統一 2026-04-22、Design section predicate-helper form に一致) | `x = x \|\| y;` ✗ | ✗ | **narrow-alive 版は I-177 依存で deferred**、non-narrow 版は本 PRD |
| O-7 | `\|\|=` | T7 Option<Named other> | `if x.is_none() { x = Some(y); }` | `x = x \|\| y;` ✗ | ✗ | 本 PRD |
| O-8 | `\|\|=` | T8 always-truthy (Named/Vec/Fn/HashMap/DynTrait/Tuple) | const-fold: no-op (empty stmt) — always truthy は `\|\|=` の assign branch を発動しない | `x = x \|\| y;` ✗ | ✗ | 本 PRD (const-fold to no-op) |
| O-9 | `\|\|=` | T9 Any | **blocked** — runtime falsy on `Value` + RHS coerce | `x = x \|\| y;` ✗ | ✗ | **別 PRD (I-050)** |
| O-10 | `\|\|=` | T10 TypeVar | **blocked** — bounds 依存 | `x = x \|\| y;` ✗ | ✗ (defer) | **別 PRD** |
| O-11 | `\|\|=` | T11 Never | NA (同 A-11: Never 型変数は到達不能) | NA | NA | — |
| O-12a | `\|\|=` | T12 Unit | defer (同 A-12a: Unit LHS ユースケース未発見、発見時は UnsupportedSyntaxError) | — | defer | **別 PRD** |
| O-12b | `\|\|=` | T12 Ref(T) | T8 const-fold no-op (Ref 常に truthy → `\|\|=` assign branch 不発動。同 A-12b の analysis、I-048 非依存) | — | defer | **本 PRD (T8 経路共用)、empirical 発火なし** |
| O-12c | `\|\|=` | T12 Option<Option<T>> | NA (同 A-12c: IR invariant `wrap_optional()` idempotent) | NA | NA | — |
| O-12d | `\|\|=` | T12 Tuple | T8 const-fold no-op (Tuple 常に truthy、`\|\|=` assign branch 不発動) | `x = x \|\| y;` ✗ | ✗ | **本 PRD** (T8 経路共用) |
| O-12e | `\|\|=` | T12 Result | NA (同 A-12e: user-visible Result 変数は emission されない) | NA | NA | — |
| O-12f | `\|\|=` | T12 QSelf | NA (同 A-12f: associated type は value position 変数型として emit されず) | NA | NA | — |
| O-13 | `\|\|=` | PatternAssignTarget LHS | NA (同 A-13: ECMA-262 syntax error) | — | NA | — |
| O-14a | `\|\|=` | SimpleAssignTarget::SuperProp (`super.x \|\|= y`) | NA (同 A-14a、`convert_assign_expr` target match arm は Ident/Member 限定、Tier 2 は UnsupportedSyntaxError) | ✗ unsupported | NA | — |
| O-14b | `\|\|=` | SimpleAssignTarget::Paren (`(x) \|\|= y`) | NA (同 A-14b) | ✗ unsupported | NA | — |
| O-14c | `\|\|=` | SimpleAssignTarget::OptChain (`x?.y \|\|= z`) | NA (同 A-14c) | ✗ unsupported | NA | — |
| O-14d | `\|\|=` | SimpleAssignTarget::TsAs (`(x as T) \|\|= y`) | NA (同 A-14d) | ✗ unsupported | NA | — |
| O-14e | `\|\|=` | SimpleAssignTarget::TsSatisfies (`(x satisfies T) \|\|= y`) | NA (同 A-14e) | ✗ unsupported | NA | — |
| O-14f | `\|\|=` | SimpleAssignTarget::Invalid | NA (同 A-14f、parser error marker) | — (parser reject) | NA | — |

**注釈**:
- `T8 always-truthy`: `&&=` は const-fold で `x = y;`、`\|\|=` は const-fold で `// no-op` (empty stmt)
- `T5 Option<F64>` の RHS wrap: RHS `y` が `f64` のとき `Some(y)` に wrap
- `T5 Option<String>` の RHS wrap: RHS `y` が `String` のとき `Some(y)`
- LHS shape = Member: `convert_member_expr_for_write` 経由で target 生成 (既存 helper)、それ以外は Ident と同形 — predicate dispatch は effective 型が決定し、Member の target expr は Expr::FieldAccess として predicate 内 operand に passthrough
- ~~narrow alive: `get_expr_type` が narrow 適用型を返すため、effective 型に dispatch すれば narrow は自動継承~~ **(SG-3 訂正 2026-04-22)**: narrow alive 状態の compound assign は I-144 T6-3 narrow emission の mutation propagation gap に block される (pre-existing structural defect)。outer `Option<T>` への mutation 伝播が shadow binding 経由で失われる。narrow × compound assign cells は本 PRD scope 外、**新 PRD I-177 (narrow emission v2)** で structural fix 後に再開。本 PRD は non-narrow scope の primary cell のみを対象とする。

#### Matrix A.4: narrow × compound assign sub-matrix (SG-3 追加 2026-04-22、I-177 依存で deferred)

narrow alive (enclosing `if (x !== null) { ... }` scope 内) × Matrix A primary cells の cross
product。ideal emission は I-177 で narrow emission v2 設計完了後に確定。本 PRD 完了条件からは
**暫定的に scope out**、I-177 完了時に本 PRD の再開 task (T3-N) として実装する。

| cell pattern | outer declared type | narrow alive type | 本 PRD での扱い |
|-------------|---------------------|-------------------|----------------|
| narrow × A-1 Bool | Option<Bool> | Bool | I-177 依存 deferred |
| narrow × A-2 F64 | Option<F64> | F64 | I-177 依存 deferred (empirical: T7-1 cell で return-from-inside-narrow form のみ pass) |
| narrow × A-3 String | Option<String> | String | I-177 依存 deferred |
| narrow × A-4 Primitive(int) | Option<Primitive(_)> | Primitive(_) | I-177 依存 deferred |
| narrow × A-5/A-5s | Option<Option<inner>> — NA (IR invariant wrap_optional idempotent) | NA | NA |
| narrow × A-6 | Option<synthetic union> | synthetic union | I-177 依存 deferred (cell-a6 empirical 2026-04-22: outer x unchanged) |
| narrow × A-7 | Option<Named other> | Named other | I-177 依存 deferred |
| narrow × A-8 | Option<always-truthy> | always-truthy | I-177 依存 deferred (const-fold 経路でも outer propagation に narrow emission v2 が必要) |
| narrow × O-* | Option<...> | ... | 同 A-* (symmetric) |

#### Matrix A.5: Expr-context × Copy/non-Copy cross product (primary × 2 = 全 24 in-scope cell の expr-context 版)

Matrix A の primary cell (A-1〜A-8 + A-12d + O-1〜O-8 + O-12d) は全て stmt-context の ideal
emission。expr-context (= `const z = (x &&= y);` のように値を観測する context) では block
expression に tail expr を付加する必要があり、tail expr の形は `is_copy_type(lhs_type)`
で決定する:

| # | primary cell | effective type | Copy? | Ideal expr-context emission |
|---|--------------|---------------|-------|----------------------------|
| A-1x | A-1 | Bool | Copy | `{ if x { x = y; } x }` |
| A-2x | A-2 | F64 | Copy | `{ if x != 0.0 && !x.is_nan() { x = y; } x }` |
| A-3x | A-3 | String | !Copy | `{ if !x.is_empty() { x = y; } x.clone() }` |
| A-4x | A-4 | Primitive(int) | Copy | `{ if x != 0 { x = y; } x }` |
| A-5x | A-5 | Option<primitive Copy> | Copy (inner Copy) | `{ if x.is_some_and(..) { x = Some(y); } x }` |
| A-5sx | A-5s | Option<String> | !Copy | `{ if x.as_ref().is_some_and(..) { x = Some(y); } x.clone() }` |
| A-6x | A-6 | Option<union enum> | !Copy (Named enum, may contain String variant) | `{ if match &x { Some(U::F64(v)) if v != 0.0 && !v.is_nan() => true, Some(U::String(s)) if !s.is_empty() => true, _ => false } { x = Some(y); } x.clone() }` (SG-2 統一 2026-04-22) |
| A-7x | A-7 | Option<Named other> | !Copy | `{ if x.is_some() { x = Some(y); } x.clone() }` |
| A-8x | A-8 | always-truthy (Copy / !Copy) | type-dependent | const-fold: `{ x = y; x.clone() }` (!Copy) / `{ x = y; x }` (Copy) |
| A-12dx | A-12d | Tuple | varies by elements | if all elements Copy → Copy; else !Copy. const-fold: `{ x = y; x.clone() }` 等 |
| O-1x〜O-8x / O-12dx | O-1〜O-8 + O-12d | 同 effective type | 同 Copy-ness | `&&=` 対応行の falsy predicate 版、同 tail |

**注**: Copy-ness 判定は `RustType::is_copy_type()` (`src/ir/types.rs`) を source of truth
とする。`TypeVar` の Copy-ness は bounds-dependent で blocked (O-10x / A-10x)。

### Matrix B — `!<expr>` 汎用 fix 直積 (20 AST shape × 12 type = 240 primary cell)

**Primary cell (operand AST shape × operand effective type)**: 詳細は全 shape × 全 type の
cross を下記グループ表で網羅。

#### Matrix B.1: operand AST shape 分類

| # | Shape | 扱い方針 | 備考 |
|---|-------|---------|------|
| B.1.1 | `Ident` | 直接 falsy_predicate_for_expr(ir_expr, ty) | T6-3 既存、確認 |
| B.1.2 | `Lit(Null)` | const-fold → `true` | TS TS2873 warning と整合 |
| B.1.3 | `Lit(Undefined Ident)` | const-fold → `true` | `!undefined` |
| B.1.4 | `Lit(Bool(true))` | const-fold → `false` | |
| B.1.5 | `Lit(Bool(false))` | const-fold → `true` | |
| B.1.6 | `Lit(Num(0))` | const-fold → `true` | JS 0 is falsy |
| B.1.7 | `Lit(Num(non-0))` | const-fold → `false` | |
| B.1.8 | `Lit(Num(NaN))` | const-fold → `true` | `!NaN = true`。**Empirical note (T4 2026-04-23)**: SWC は `NaN` を `ast::Expr::Ident(sym="NaN")` として parse、`Lit::Num(Number::NAN)` は produce しない。`try_constant_fold_bang` の `Lit::Num` arm (`n.value.is_nan()` check) は到達不能。runtime semantic は Layer 4 F64 dispatch (`<NaN> == 0.0 \|\| <NaN>.is_nan()` = `true`) で等価に確保。unit test `bang_const_fold_num_nan` は AST-level 到達不能性を lock-in、E2E `cell-b-bang-f64-in-ret` で typed runtime verify |
| B.1.9 | `Lit(Str(""))` | const-fold → `true` | |
| B.1.10 | `Lit(Str(non-empty))` | const-fold → `false` | |
| B.1.11 | `Lit(BigInt(0))` | const-fold → `true` | `!0n = true` |
| B.1.12 | `Lit(BigInt(non-0))` | const-fold → `false` | |
| B.1.13 | `Lit(Regex)` | const-fold → `false` | Regex object always truthy |
| B.1.14 | `Paren` | unwrap inner, recurse | |
| B.1.15 | `Member` (Ident/Computed prop) | falsy_predicate_for_expr(ir, member_ty) | narrow propagation は I-165 scope 外 |
| B.1.16 | `OptChain` | unwrap to `chain.is_none() \|\| <inner falsy>` 展開 | OptChain `x?.v` falsy は `x == null \|\| <v falsy>`。**Implementation equivalence (T4 empirical)**: `falsy_predicate_for_expr(optchain_ir, Option<Inner>)` 経路で `!<optchain>.is_some_and(\|v\| <truthy(v)>)` が emission され、上記 ideal と semantically equivalent (None→true、Some(falsy)→true、Some(truthy)→false が一致)。unit test `bang_optchain_on_option_field_emits_option_falsy` で lock-in |
| B.1.17 | `TsAs` | peek-through to inner (TS as は型のみ) | shared helper `peek_through_type_assertions` |
| B.1.18 | `TsNonNull` | peek-through to inner | 同上 |
| B.1.19 | `Unary(!)` (double negation) | `!!x` → truthy_predicate_for_expr(x, ty) | De Morgan fold。**T4 IG-1 fix (2026-04-23)**: inner operand が literal / literal-equivalent (null / undefined / Lit(Bool/Num/Str/BigInt/Regex) / Arrow / Fn) の場合、Layer 3 で `try_constant_fold_bang(inner) = Some(BoolLit(b))` 判定後 `BoolLit(!b)` を recursive const-fold 返却。TypeResolver 非依存で decidable。pre-fix は Layer 5 fallback で raw `!!<lit_ir>` emission (Rust compile error for non-bool literal)。**T4 IG-6 fix (2026-04-23 /check_problem)**: inner operand が `Assign` / `Bin(LogicalAnd/LogicalOr)` の場合、direct `truthy_predicate_for_expr` は無効 Rust を emit (Assign は `()` を返す、非 bool Logical は `&&`/`\|\|` 型エラー)。`needs_bang_recurse` 判定で `convert_bang_expr(&inner.arg)` を recurse し outer `Not` で wrap、Layer 3b/3c dispatch 経由で valid Rust を emit。unit test `bang_double_negation_on_{null,undefined,truthy_string,zero,peek_through_wrappers}_literal` (IG-1) + `bang_double_negation_on_{assign_inverts_layer_3c_block,logical_and_inverts_de_morgan,logical_or_inverts_de_morgan,arithmetic_compound_assign}` (IG-6) で 9 variant lock-in |
| B.1.20 | `Unary(-)`, `Unary(+)`, `Unary(TypeOf)` | falsy_predicate_for_expr(ir, result_ty) | Unary 結果型は内部既知 |
| B.1.21 | `Bin(Arithmetic)` (+,-,*,/,%) | tmp bind + falsy predicate on F64 | BinExpr 再評価回避 |
| B.1.22 | `Bin(Comparison)` (==,===,<,>,...) | 既に Bool → `!inner` | 現状動作 |
| B.1.23 | `Bin(LogicalAnd)` (`x && y`) | De Morgan: `!<x falsy> \|\| !<y falsy>` ... see note | result 型は union。**Dual verdict (T4 E2E empirical 2026-04-23)**: E2E fixture `cell-b-bang-logical-and.ts` (`if (!(x && y)) return; /* post-narrow use of x, y */`) は **TS ✓ / Rust ✗ (I-177 blocker)**。T4 Bang arm emission 自体は correct (`!<x falsy> \|\| !<y falsy>` De Morgan emission)。blocker は post-return narrow materialisation 不足 (narrow-scope、I-177 scope)。本 PRD Completion Criteria は De Morgan emission までを scope とし、narrow 連動は I-177 完了後の sub-task で回帰 |
| B.1.24 | `Bin(LogicalOr)` (`x \|\| y`) | De Morgan: `<x falsy> && <y falsy>` | 同上 |
| B.1.25 | `Bin(Bitwise)` (&,\|,^,<<,>>,>>>) | 結果 F64 → falsy predicate on F64 | |
| B.1.26 | `Bin(InstanceOf)` | 既に Bool → `!inner` | 現状動作 |
| B.1.27 | `Bin(In)` | 既に Bool → `!inner` | 現状動作 |
| B.1.28 | `Bin(NullishCoalescing)` | tmp bind + falsy predicate on NC result type | 結果 type depends |
| B.1.29 | `Call` | tmp bind + falsy predicate on return type | 副作用を 1 回に |
| B.1.30 | `Cond` (ternary) | tmp bind + falsy predicate on result type | |
| B.1.31 | `New` | const-fold → `false` (constructor returns object, always truthy) | |
| B.1.32 | `Await` | tmp bind + falsy predicate on awaited type | **Dual verdict (T4 E2E empirical 2026-04-23)**: E2E fixture `cell-b-bang-await.ts` は **TS ✓ / Rust ✓ emission / E2E ✗ (I-180 blocker)**。T4 Bang arm emission (`{ let tmp = <fut>.await; <F64 falsy> }`) は正しい。blocker は tsx が top-level `main();` で async fn を 2 回実行する harness 側の issue (TS side の fixture design)。unit test `bang_await_falls_through_without_type` で Layer 5 fallback lock-in、typed path は emission empirical verify 済 |
| B.1.33 | `Assign` | tmp bind + falsy predicate on value type (assignment returns RHS) | **T4 Layer 3c implementation (2026-04-23)**: `convert_bang_assign` で `{ let __ts_tmp_assign_N = <rhs>; <target> = __ts_tmp_assign_N; <falsy(tmp)> }` 形式で desugar。compound assign (`+=` 等) は normalised plain-assign 形で desugar 経路を通過 (`&&=`/`\|\|=`/`??=` は T3 で earlier-lowered)。unit test `bang_assign_desugar_{primitive_f64_rhs, skips_compound_assign, unresolved_rhs_type_falls_back}` 3 case で lock-in。**Dual verdict**: E2E fixture `cell-b-bang-assign.ts` は **TS ✓ / Rust ✓ desugar emission / E2E ✗ (I-181 blocker)**。blocker は fixture 内の tuple destructuring `[l,x] = f()` + ternary `"str"` emission の pre-existing defects (I-181)、Assign desugar 自体は正しい |
| B.1.34 | `Array` / `Object` / `Tpl` / `Arrow` / `Fn` | const-fold → `false` (always truthy) | |
| B.1.35 | `This` | falsy_predicate_for_expr on self type | class context |
| B.1.36 | `Update` (`i++`) | tmp bind + falsy predicate on F64 | side effect。**Dual verdict (T4 E2E empirical 2026-04-23)**: E2E fixture `cell-b-bang-update.ts` は **TS ✓ / Rust ✓ emission / E2E ✗ (I-181 blocker、cell-b-bang-assign と共有)**。T4 Bang arm emission `{ let _old = i; i = i+1; <F64 falsy(_old)> }` は正しい。unit test `bang_update_expression_on_f64_with_type_tmp_binds` で lock-in |
| B.1.37a | `Seq` (comma expr `a, b, c`) | NA (`ast-variants.md` §1 Tier 2: ts_to_rs は Seq を unsupported として I-114 で起票済、convert_expr が UnsupportedSyntaxError 返却) |
| B.1.37b | `Yield` | NA (generator は ts_to_rs scope 外、ast-variants.md §1 Tier 2 "generator (ts_to_rs 未対応)") |
| B.1.37c | `MetaProp` (`import.meta`, `new.target`) | NA (meta property は ast-variants.md §1 Tier 2、convert_expr で UnsupportedSyntaxError) |
| B.1.37d | `Class` (class expression) | NA (class expression は I-093 unsupported、ast-variants.md §1 Tier 2) — もっとも `!<class>` は常に truthy (class 自体はオブジェクト参照) なので const-fold する価値はあるが、convert_expr の制約で到達しない |
| B.1.37e | `TaggedTpl` (tagged template literal) | NA (I-110 unsupported) |
| B.1.37f | `SuperProp` (`super.x`) | NA (super property access ast-variants.md §1 Tier 2、convert_expr で error) |
| B.1.37g | `TsTypeAssertion` (`<T>e` 旧 syntax) | **本 PRD scope in** — `peek_through_type_assertions` helper に TsTypeAssertion arm を含める (Design section 記載済)。`!<x as unknown>` は peek-through で inner を `convert_expr` に通す経路。standalone TsTypeAssertion の convert_expr 対応は本 PRD scope 外 (別 TODO) |
| B.1.37h | `TsSatisfies` (`e satisfies T` TS 4.9+) | NA (`ast-variants.md` §1 Tier 2 で TsSatisfies は I-115 unsupported、`convert_expr` が UnsupportedSyntaxError を返す。本 PRD の peek-through helper は runtime 無効果の type wrapper 全てを unwrap する設計思想だが、TsSatisfies は I-115 convert_expr 対応を前提とする構造的依存があるため scope out — 本 PRD の peek-through が inner を convert_expr に通した結果、TsSatisfies は I-115 依存。peek-through 前提で TsSatisfies peek-through は I-115 完了後に自動 extent 可能 (peek_through_type_assertions 側に TsSatisfies arm 追加のみで済む) が、現 PRD scope では I-115 依存で blocked)) |
| B.1.37i | `TsConstAssertion` (`e as const`) | **本 PRD scope in** — `peek_through_type_assertions` helper に TsConstAssertion arm を含める (Design section 記載済)。`!<x as const>` は peek-through で inner を `convert_expr` に通す |
| B.1.37j | `TsInstantiation` (`f<T>` TS 4.7+) | NA (generic instantiation expr unsupported、runtime 無効果) |
| B.1.37k | `PrivateName` (`#field` standalone) | NA (class 外での standalone PrivateName は syntax error、ast-variants.md §1 Tier 2) |
| B.1.37l | `Invalid` | NA (parser error marker、TS syntax error で SWC が produce する sentinel — `!<Invalid>` は syntax error により到達不能) |

#### Matrix B.2: effective type × ideal emission

Matrix A.3 と同一の 12 equiv class × 上記 shape の product。具体的な emission shape は:

| # | effective type | ideal falsy expression (operand = `<e>`) | 備考 |
|---|---------------|-----------------------------------------|------|
| B-T1 | Bool | `!<e>` | 直接 |
| B-T2 | F64 | `<e> == 0.0 \|\| <e>.is_nan()` (Ident), tmp bind for non-Ident | |
| B-T3 | String | `<e>.is_empty()` (Ident), `<e>.as_str().is_empty()` for expressions | |
| B-T4 | Primitive(int) | `<e> == 0` | |
| B-T5 | Option<primitive> Copy | `!<e>.is_some_and(\|v\| v != 0.0 && !v.is_nan())` など | `is_some_and(\|v: T\| ...)` で T は by-value (Rust 1.70+); SG-1 訂正 2026-04-22 |
| B-T5s | Option<String> | `!<e>.as_ref().is_some_and(\|v\| !v.is_empty())` | `.as_ref()` で borrow |
| B-T6 | Option<union enum> | `match &<e> { Some(U::X(v)) if <v truthy> => false, ..., _ => true }` | per-variant。**Dual verdict (T4 E2E empirical 2026-04-23)**: E2E fixture `cell-b-bang-option-union.ts` は **TS ✓ / Rust ✓ per-variant match emission / E2E ✗ (I-179 blocker)**。T4 Bang arm per-variant emission は正しい。blocker は fixture の caller side (`f(NaN)` / `f("")`) で NaN / string literal が `F64OrString::F64(...)` / `F64OrString::String(...)` に wrap されず emit される pre-existing defect (I-179 synthetic union literal coercion)。B-T6 emission 単体は T2 helper unit test + E2E other cells で verify 済 |
| B-T7 | Option<Named other> | `<e>.is_none()` | Named always truthy when Some |
| B-T8 | always-truthy | const-fold → `false` | |
| B-T9 | Any | blocked I-050 | |
| B-T10 | TypeVar | blocked | |
| B-T11 | Never | NA (operand 型 never は到達不能) | |
| B-T12 | Unit | `true` (Unit は JS void と対応、`void x` → undefined → falsy) | 稀 |

**注釈**:
- **non-Ident operand + tmp bind**: F64/String などで `<e>` が BinExpr/Call/Cond などの side-effect-prone expr の場合、2 回評価を避けるため block 式で tmp binding: `{ let _tmp = <e>; _tmp == 0.0 \|\| _tmp.is_nan() }`
- **Peek-through shapes** (B.1.14/17/18): TsAs / TsNonNull / Paren は inner に委譲。再帰 recursion でネスト対応
- **Double negation** (B.1.19): `!!x` → `truthy_predicate_for_expr(x, ty)`、式の型は常に Bool

### Matrix C — if-stmt narrow emission (T6-3 拡張)

**Primary cell**: C.2 operand shape × C.5 operand type × C.3 body shape × C.4 else branch の
subset (T6-3 既存 ✓ セル + 拡張 ✗ セル)。C.1 は `!<expr>` (falsy) に focus (truthy 側は
既に `convert_if_stmt` が narrow event 経由でカバー済)。

#### Matrix C.1: `if (!<operand>) { body }` narrow 材料化

凡例: ✓ = 既に ideal、✗ = 本 PRD scope、🔒 = 依存 PRD (out-of-scope)、NA = 不可

| # | operand shape | operand type | body | else | Ideal emission | 現状 | 判定 |
|---|---------------|-------------|------|------|---------------|------|------|
| C-1 | Ident | Option<primitive> | always-exit | — | T6-3 consolidated match (`let x = match x { Some(v) if truthy => v, _ => { exit } }`) | ✓ T6-3 | ✓ |
| C-2 | Ident | Option<union enum> | always-exit | — | T6-3 per-variant match | ✓ T6-3 | ✓ |
| C-3 | Ident | Option<Named other> | always-exit | — | T6-3 `Some(v) => v, None => exit` | ✓ T6-3 | ✓ |
| C-4 | Ident | Option<primitive> | non-exit | absent | predicate form `if <x falsy> { body }` — narrow 材料化なし (body 後は Option<T> のまま) | ✗ fall-through `!Option` | ✗ **本 PRD** |
| C-5 | Ident | Option<primitive> | any | present | consolidated match 拡張: `match x { Some(v) if truthy => { else_body }, _ => { then_body } }` | ✗ fall-through | ✗ **本 PRD** |
| C-6 | Ident | primitive (Bool/F64/String/int) | any | any | Layer 1 fallback: `if <x falsy> { body } else { ... }` | ✓ T6-3 `try_generate_primitive_truthy_condition` | ✓ |
| C-7 | Lit(null) | — | any | any | const-fold: `if true { body }` → `body` | ✗ fall-through | ✗ **本 PRD** |
| C-8 | Lit(false) | — | any | any | const-fold: `if true { body }` → `body` | ✗ fall-through | ✗ **本 PRD** |
| C-9 | Lit(true) | — | any | any | const-fold: `if false { body }` → `else_body` or empty | ✗ fall-through | ✗ **本 PRD** |
| C-10 | Lit(num≠0), Lit(str≠""), Lit(bool=true) | — | any | any | const-fold → `false` → body not taken | ✗ | ✗ **本 PRD** |
| C-11 | Paren | (recurse) | any | any | unwrap & recurse | partial | ✗ **本 PRD** |
| C-12 | TsAs | (recurse on inner) | any | any | peek-through & recurse | ✗ fall-through | ✗ **本 PRD** |
| C-13 | TsNonNull | (recurse on inner) | any | any | peek-through & recurse | ✗ fall-through | ✗ **本 PRD** |
| C-14 | Unary(!) | (recurse, double neg) | any | any | `!!x` = truthy of x → invert dispatch | ✗ fall-through | ✗ **本 PRD** |
| C-15 | Member (Ident/Computed prop) | Option<T> | always-exit | — | `if <e>.is_none() { exit }` (narrow 不可) | ✗ fall-through `!Option` | ✗ **本 PRD (Layer 1 only)** |
| C-15n | Member | Option<T> | any | any | narrow 材料化 (後続 `u.v` 参照) | NA (本 PRD) | 🔒 **I-165** (field-path VarId) |
| C-16a | OptChain | Option<T> | always-exit | — | `if <e>.is_none() \|\| <inner>.is_none() { exit }` (compile error fix at if-cond) | ✗ fall-through | ✗ **本 PRD (Layer 1)** |
| C-16b | OptChain base | Option<T> (x typed) | always-exit | — | post-if で **`x` → non-null narrow 材料化** (e.g. `x.v` を subsequent 参照で valid 化)。`guards.rs::detect_early_return_narrowing` の Bang arm に OptChain case を追加、`extract_optchain_base_ident` (narrowing_patterns.rs:100 既存 pub(crate) helper) で base Ident を取得、`NarrowEvent::Narrow` を `NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant)` で push | ✗ narrow 未記録 | ✗ **本 PRD (T6 broken window 拡張)**。I-165/I-143-a 非依存 — I-144 T6-4 (`x?.v !== undefined`) と symmetric な ~10 LOC 追加 |
| C-16n | OptChain field (`x.v` や `x?.v` の field 側 narrow 材料化) | Option<inner> | any | any | field-path narrow (`x.v` → non-null 後続参照) | NA (本 PRD) | 🔒 **I-165** (field-path VarId、base narrow は C-16b で解消済) |
| C-17 | Bin(arith) | F64 | any | any | tmp bind + falsy predicate on F64 | ✗ fall-through | ✗ **本 PRD** |
| C-18 | Bin(LogicalAnd) | union | any | any | De Morgan 展開: `!<x truthy> \|\| !<y truthy>` | ✗ fall-through | ✗ **本 PRD** |
| C-19 | Call / Cond / Await / New | depends | any | any | tmp bind + falsy predicate | ✗ fall-through | ✗ **本 PRD** |
| C-20 | Ident | Option<Any> | any | any | Value runtime truthy branch | ✗ | 🔒 **I-050** |
| C-21 | Ident | TypeVar (`T` bounds unknown) | any | any | bounds-dependent emission | ✗ | 🔒 **別 PRD** |
| C-22 | Ident | Never | — | — | NA (unreachable operand) | NA | — |
| C-23 | LogicalOr `<e> \|\| <f>` in `!(e \|\| f)` | union | any | any | De Morgan: `<e falsy> && <f falsy>` | ✗ | ✗ **本 PRD** |
| C-24 | Array / Object / Tpl / Arrow / Fn / This / New | always-truthy | any | any | const-fold: `!<truthy> = false` → body not taken | ✗ fall-through | ✗ **本 PRD** |

### Matrix Completeness Audit

実装完了宣言前に以下を全チェック (本 audit は `problem-space-analysis.md` 準拠で
後続 `/check_job` Review insight 発見時に更新):

- [x] 全 AST shape variant を `ast-variants.md` から網羅列挙した (Matrix B.1 の 37 行、うち Tier 2/3 は NA)
- [x] 全 RustType variant を `rust-type-variants.md` から網羅列挙した (12 equiv class で 18 variant カバー、RustType 18 全 variant を equiv class へ明示 map: Unit→T12a, String→T3, F64→T2, Bool→T1, Option→T5/T6/T7, Vec→T8, Fn→T8, Result→T12e, Tuple→T12d, Any→T9, Never→T11, Named→T6/T7/T8, TypeVar→T10, Primitive→T4, StdCollection→T8, Ref→T12b, DynTrait→T8, QSelf→T12f)
- [x] 全 emission context を `emission-contexts.md` から網羅列挙した (Matrix A.4 / B.3)
- [x] 未カバーセル・「多分 OK」セルが残っていない (blocked / NA は明示)
- [x] 「稀」「低頻度」を理由にした省略がない
- [x] Matrix A.12b (Ref), A.12c (Option<Option>) 等の低頻度 cell も明示的に defer/NA 判定
- [x] Dual verdict (TS / Rust) を全 ✗ cell で明示
  - Matrix A: tsc observation `tests/observations/i161-i171/a1-a7.ts` で TS runtime ✓ 確認済 (tsc exit=0)
  - Matrix B: tsc observation `tests/observations/i161-i171/b1-b14.ts` で TS runtime ✓ 確認済
  - Rust emission: convert_unary_expr / convert_assign_expr で structural fix 未実装 ✗
- [x] 全セルに test (unit / integration / E2E) が対応する (Task T1-T5 の test 責務で網羅)

### Spec-Stage Adversarial Review Checklist (spec-first-prd.md 準拠、v4 最終状態)

Spec stage 完了時、以下 5 項目を全 [x] にする。1 つでも未達なら Implementation stage
移行不可。

- [x] **Matrix completeness**: 全セルに ideal output が記載されている (空欄 / TBD なし) — Matrix A (A-1〜A-14f + A-12a〜f 全列挙、13 primary + Tier 2 SimpleAssignTarget 6 + PatternAssignTarget NA)、Matrix O (O-1〜O-14f + O-12a〜f 全列挙、AndAssign と完全対称)、A.5 expr-context × Copy/non-Copy 12 cell、Matrix B (B.1.1〜B.1.37l、B.1.37 を 12 Tier 2 variant に個別展開)、Matrix C (C-1〜C-24、C-16 を C-16a Layer 1 / C-16b OptChain base narrow in-scope / C-16n field-narrow I-165 scope out に split)。blocked cell は I-050/I-165 link 明示 (v4 で I-143-a 依存を誤分類修正 — C-16 base narrow は I-143-a 非依存であることを empirical 確認)
- [x] **Oracle grounding**: ✗ / 要調査 セルの ideal output が tsc observation log と cross-reference されている — tsc observation 26 fixture (`tests/observations/i161-i171/`: v1 a1-a7 + b1-b14 21 件 + v2 a8-and/a8-or/a9-or-narrow/b15-bigint-regex/b16-logical-or 5 件) で全 ✗ cell を TS runtime 観測済。v4 追加 8 Matrix B shape + 1 C-16b + 5 T7 cell は JS spec + E2E fixture runtime 記録で grounding
- [x] **NA justification**: NA セルの理由が spec-traceable — A-11 / O-11 (IR invariant: Never 型は到達不能)、A-12a〜f / O-12a〜f (Unit/Ref/Option<Option>/Tuple/Result/QSelf の IR invariant / emission path 不在 / ECMA syntax error 等、v3 で A-12d Tuple + v4 で A-12b/O-12b Ref(T) を T8 const-fold in-scope に reclassify、A-12e Result は empirical trace で `_try_result` internal var のみ確認)、A-13 / O-13 (PatternAssignTarget → ECMA-262 syntax error)、**A-14a〜f / O-14a〜f** (v4 追加: Tier 2 SimpleAssignTarget SuperProp/Paren/OptChain/TsAs/TsSatisfies/Invalid は `convert_assign_expr` match arm で UnsupportedSyntaxError)、B.1.37a〜l (12 Tier 2 variant 個別理由、TsTypeAssertion/TsConstAssertion は peek-through in-scope)、C-22 (Never operand 到達不能)、**C-16n** (v4 で field-path narrow のみ I-165 scope out、base narrow は C-16b で in-scope 化)、C-20 (Any → I-050)、C-21 (TypeVar → 別 PRD)。「稀」「多分」「頻度が低い」等の曖昧理由ゼロ
- [x] **Grammar consistency**: matrix に reference doc に未記載の variant が存在しない — Matrix B.1.1〜B.1.37l の全 AST shape は `ast-variants.md` §1 Tier 1/2/3 と完全整合、Matrix A の AssignOp (AndAssign/OrAssign) は §7 準拠、SimpleAssignTarget Tier 2 全 variant を Matrix A-14 / O-14 で enumerate、Matrix A/B の effective type (T1-T12) は `rust-type-variants.md` §1 の 18 variant を全マップ。Emission context は `emission-contexts.md` §1-§8 の 51 context を明示
- [x] **E2E readiness**: 各セルに対応する E2E fixture が (red 状態で) 準備されている — `tests/e2e/scripts/i161-i171/cell-*.ts` 計 **60 fixture** (3 GREEN + 57 RED `#[ignore]`) 作成済: Matrix A primary 9 + A supplementary 2 (Member/expr) + Matrix O primary 8 + Matrix B 20 (v4: +8 NC/Cond/Await/Assign/This/Update/TsTypeAssertion/TsConstAssertion) + Matrix C 15 (v4: +1 C-16b OptChain base narrow) + T7 classifier regression 5 (v4 additions) + regression 1。全 fixture に `*.expected` oracle 記録済、`tests/e2e_test.rs` に 60 test function 登録済。T3 / T4 unit test plan は個別 cell を明示列挙 (T3 140 case、T4 ~95 case)。red-state empirical 検証は `report/i161-i171-t1-red-state.md` v4 に記録

## Goal

以下を満たす structural fix を完了すること:

1. **I-161 完全解消**: 全 AST shape (`AssignOp::AndAssign`, `AssignOp::OrAssign`) × LHS shape (`Ident`, `Member`) × effective type (T1-T8 all cells of Matrix A) で ideal Rust を emit。Any/TypeVar/Never/Unit は NA / 別 PRD の判定明示
2. **I-171 Layer 1 完全解消**: `convert_unary_expr` Bang arm が全 operand AST shape (B.1.1-B.1.34) × effective type (T1-T8) で ideal Rust を emit。const-fold は `!null`/`!undefined`/`!lit`/`!<always-truthy>` で適用
3. **I-171 Layer 2 拡張**: `try_generate_option_truthy_complement_match` が C-4 (non-exit body) / C-5 (else branch) / C-7〜C-10 (const-fold) / C-11〜C-19 (non-Ident operand for Layer 1 feed-through)で ideal Rust を emit。narrow 材料化は Ident に限定 (非 Ident は Layer 1 のみ)
4. **共有 helper**: `truthy_predicate_for_expr(expr: &Expr, ty: &RustType, tmp_binder: &mut TempBinder) -> Expr` + `falsy_predicate_for_expr(...) -> Expr` を `src/transformer/helpers/truthy.rs` に新設、既存 `truthy_predicate(name, ty)` を内包する形にリファクタ
5. **peek-through helper**: `peek_through_type_assertions(expr: &ast::Expr) -> &ast::Expr` を `src/transformer/helpers/` に新設 (Paren / TsAs / TsNonNull / TsTypeAssertion / TsConstAssertion unwrap)
6. **narrow_analyzer 拡張**: `detect_early_return_narrowing` Bang arm が peek-through 適用 inner を Ident として narrow 記録 (Member/OptChain 直接は I-165 依存で scope out)
7. **全 quality gate 維持**: cargo test 全 pass、cargo clippy 0 warnings、cargo fmt 0 diffs、cargo llvm-cov 閾値 ≥ 90 維持、`check-file-lines.sh` pass
8. **Hono bench 非後退**: 直接 blocker ではないため clean 数値向上は goal でない (`ideal-implementation-primacy.md` 通り)。ただし regression (error 増加) が無いこと

## Scope

### In Scope (non-narrow)

- **I-161 complete (non-narrow)**: Matrix A の T1-T8 × {Ident, Member} × {Stmt, Expr} × **not-narrow-alive** 全セル
- **I-171 Layer 1 (`convert_unary_expr` 型別 dispatch)**: Matrix B 全 shape × T1-T8 セル、const-fold 含む
- **I-171 Layer 2 (if-stmt narrow emission 拡張)**: Matrix C の C-4/C-5/C-7〜C-19/C-23/C-24 (narrow 非依存または Ident narrow のみ)
- **共有 helper 導入**: `truthy_predicate_for_expr` / `falsy_predicate_for_expr` / `peek_through_type_assertions`
- **narrow_analyzer Bang arm 拡張**: peek-through 対応 (TsAs/TsNonNull/Paren の内側が Ident の場合 narrow 記録)
- **classifier `ResetCause::CompoundLogical` の挙動確認**: 新 emission path では reset 記録の有無が emission に影響しないことを structural 検証 (タスク T7 で empirical 確認)

### Deferred (split-off PRDs、完全性保持のため scope out ではなく分割)

**開発コストではなく、専任 PRD 化による正確性維持のため分割**:

- **I-177 (narrow emission v2)**: I-144 T6-3 inherited の narrow mutation propagation pre-existing defect。`if let Some(x) = x { body }` shadow 経由の body mutation が outer `Option<T>` に propagate しない structural gap。I-161 の narrow-alive cells (Matrix A.4 全行、Matrix A-6/O-6 の narrow-scope pattern、T7-1/T7-2/T7-3/T7-5 の narrow interaction regression) は I-177 完了後に本 PRD の T3-N / T7-N sub-task として再開。詳細は `backlog/I-177-narrow-emission-v2.md`。
- **I-178 (spec-first-prd Matrix/Design integrity check)**: Spec-Stage Adversarial Review Checklist に 6 項目目「Matrix ideal column と Design section の emission shape 整合性」を追加する rule 改善。本 PRD の SG-2 (Matrix A-6 per-variant match form と Design section predicate helper form の不整合) が framework gap として発見され、今後の matrix-driven PRD で同様の silent inconsistency を防止するため。

### Out of Scope (別 umbrella / blocked-by)

- **Matrix A-9, O-9 (Any LHS `&&=`/`||=`)**: 🔒 I-050 umbrella。`Value` runtime truthy + RHS coerce の別 PRD 管轄
- **Matrix A-10, O-10 (TypeVar LHS)**: 🔒 別 PRD (generic bounds 推論)。発火頻度極低
- **Matrix C-15n, C-16n (Member/OptChain narrow 材料化)**: 🔒 I-165 (per-binding VarId + field-path narrow)。本 PRD は Layer 1 compile error fix のみ
- **Matrix C-20 (Option<Any>)**: 🔒 I-050
- **Matrix C-21 (TypeVar)**: 🔒 別 PRD
- **I-143-a (`??` + NC peek-through)**: 🔒 独立 PRD。peek-through helper は共有するが I-143-a 本体の scope は NC
- **Hono bench 数値改善目標**: `ideal-implementation-primacy.md` により数値は signal のみ。goal に含めない

## Design

### Technical Approach

#### 新規コンポーネント

**1. `src/transformer/helpers/truthy.rs` 拡張 (expr 対応)**

```rust
/// Temp binder for side-effect-prone expressions in truthy/falsy contexts.
///
/// When the operand is a BinExpr / Call / Cond / etc., emitting the
/// truthy/falsy predicate naively would evaluate the operand multiple
/// times. `TempBinder` allocates a fresh `__ts_tmp_N` binding and emits
/// `{ let __ts_tmp_N = <e>; <predicate on __ts_tmp_N> }` block form.
pub(crate) struct TempBinder {
    counter: u32,
}

impl TempBinder {
    pub(crate) fn new() -> Self { Self { counter: 0 } }
    pub(crate) fn fresh(&mut self, prefix: &str) -> String {
        let name = format!("__ts_tmp_{prefix}_{}", self.counter);
        self.counter += 1;
        name
    }
}

/// JS truthy predicate for an arbitrary expression at the given effective type.
///
/// Returns a boolean-valued Rust expression that is true iff `<expr>`
/// would be truthy in JS. For side-effect-prone shapes, the returned
/// expression wraps a temporary binding to avoid double evaluation.
///
/// Dispatches per RustType:
/// - T1 Bool: passthrough
/// - T2 F64: `<e> != 0.0 && !<e>.is_nan()` (with tmp bind for non-Ident)
/// - T3 String: `!<e>.is_empty()` (with borrow for non-Ident)
/// - T4 Primitive(int): `<e> != 0`
/// - T5 Option<primitive>: `<e>.is_some_and(|v| <v truthy>)` (Copy) or
///   `<e>.as_ref().is_some_and(|v| <v truthy>)` (!Copy)
/// - T6 Option<synthetic union>: match-based per-variant guard
/// - T7 Option<Named other>: `<e>.is_some()`
/// - T8 always-truthy: const `true`
/// - T9-T10 (Any/TypeVar): returns None (caller errors out as I-050/別 PRD scope)
pub(crate) fn truthy_predicate_for_expr(
    expr: &Expr,
    ty: &RustType,
    binder: &mut TempBinder,
) -> Option<Expr>;

pub(crate) fn falsy_predicate_for_expr(
    expr: &Expr,
    ty: &RustType,
    binder: &mut TempBinder,
) -> Option<Expr>;
```

**既存 `truthy_predicate(name, ty)` / `falsy_predicate(name, ty)`** は内部で新 helper
を呼び出す薄い wrapper にリファクタする (既存 call site の互換維持)。

**2. `src/transformer/helpers/peek_through.rs` (新規)**

```rust
/// Unwraps type-only AST wrappers that do not affect runtime semantics.
///
/// Peeks through:
/// - `Paren(e)` — grouping
/// - `TsAs(e as T)` — type assertion (runtime no-op)
/// - `TsNonNull(e!)` — non-null assertion (runtime no-op)
/// - `TsTypeAssertion(<T>e)` — legacy type assertion
/// - `TsConstAssertion(e as const)` — const assertion
///
/// Returns the innermost expression. Recurses through nested wrappers.
pub(crate) fn peek_through_type_assertions(expr: &ast::Expr) -> &ast::Expr;
```

**3. `src/transformer/expressions/binary.rs::convert_unary_expr` Bang arm 拡張**

```rust
// Bang arm (current L272-283) を以下に置換:
ast::UnaryOp::Bang => {
    let unwrapped = peek_through_type_assertions(&unary.arg);
    // const-fold for literals
    if let Some(folded) = try_constant_fold_bang(unwrapped) {
        return Ok(folded);
    }
    // double negation: !!x → truthy_predicate_for_expr(x, ty)
    if let ast::Expr::Unary(inner) = unwrapped {
        if inner.op == ast::UnaryOp::Bang {
            let inner_inner = peek_through_type_assertions(&inner.arg);
            let ty = self.get_expr_type(inner_inner);
            let inner_ir = self.convert_expr(inner_inner)?;
            let mut binder = TempBinder::new();
            if let Some(ty) = ty {
                if let Some(pred) = truthy_predicate_for_expr(&inner_ir, ty, &mut binder) {
                    return Ok(pred);
                }
            }
            // fallback: !!x → !!inner_ir (Bool-returning expression)
        }
    }
    let operand_ty = self.get_expr_type(unwrapped);
    let operand_ir = self.convert_expr(unwrapped)?;
    let mut binder = TempBinder::new();
    if let Some(ty) = operand_ty {
        if let Some(pred) = falsy_predicate_for_expr(&operand_ir, ty, &mut binder) {
            return Ok(pred);
        }
    }
    // Type unresolved or blocked (Any/TypeVar) → error surface
    // (was silent `!<non-bool>` fall-through; now explicit)
    Expr::UnaryOp { op: UnOp::Not, operand: Box::new(operand_ir) }
}
```

**注**: Any 型の場合 `falsy_predicate_for_expr` は `None` 返却。現状互換性維持のため
fallback で `!<operand>` emit (Bool 以外で compile error、I-050 依存)。

**4. `src/transformer/expressions/assignments.rs::convert_assign_expr` AndAssign/OrAssign 置換**

```rust
// Current L253-262 (AndAssign/OrAssign) を以下に置換:
ast::AssignOp::AndAssign | ast::AssignOp::OrAssign => {
    let lhs_type = match &assign.left {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) =>
            self.get_type_for_var(&ident.id.sym, ident.id.span).cloned(),
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) =>
            self.get_expr_type(&ast::Expr::Member(member.clone())).cloned(),
        _ => None,
    };
    let Some(lhs_type) = lhs_type else {
        return Err(UnsupportedSyntaxError::new(
            "compound logical assign on unresolved type",
            assign.span(),
        ).into());
    };

    // Build predicate on target (already computed `target: Expr`)
    let mut binder = TempBinder::new();
    let predicate = match assign.op {
        ast::AssignOp::AndAssign =>
            truthy_predicate_for_expr(&target, &lhs_type, &mut binder),
        ast::AssignOp::OrAssign =>
            falsy_predicate_for_expr(&target, &lhs_type, &mut binder),
        _ => unreachable!(),
    };

    // Convert RHS with expected = inner (for Option<T>) or LHS type itself
    let rhs_expected = match &lhs_type {
        RustType::Option(inner) => Some(inner.as_ref()),
        other => Some(other),
    };
    let right = self.convert_expr_with_expected(&assign.right, rhs_expected)?;

    // Wrap RHS for Option<T> LHS (Some(value))
    let assign_value = match &lhs_type {
        RustType::Option(_) => Expr::FnCall {
            target: CallTarget::Builtin(BuiltinVariant::Some),
            args: vec![right],
        },
        _ => right,
    };

    // Const-fold for always-truthy types (T8).
    // stmt-context / expr-context は caller 側が dispatch するが、const-fold 時の
    // semantic は JS spec に従う:
    //   - &&=: truthy → assign。emission は `x = y;` (stmt) / `{ x = y; x [.clone()] }` (expr)
    //   - ||=: falsy branch 不発動 → assign なし。emission は `/* no-op */` (stmt) /
    //     `x [.clone()]` (expr) — JS では `x ||= y` が falsy でないと assign せず原 x を返す
    if is_always_truthy_type(&lhs_type) {
        let is_copy = lhs_type.is_copy_type();
        let target_value = if is_copy { target.clone() } else {
            Expr::MethodCall {
                object: Box::new(target.clone()),
                method: "clone".to_string(),
                args: vec![],
            }
        };
        return Ok(match (assign.op, is_expr_context) {
            // &&=: always assign, stmt → `x = y;`
            (ast::AssignOp::AndAssign, false) => Expr::Assign {
                target: Box::new(target),
                value: Box::new(assign_value),
            },
            // &&=: expr context → `{ x = y; x [.clone()] }`
            (ast::AssignOp::AndAssign, true) => Expr::Block(vec![
                Stmt::Expr(Expr::Assign {
                    target: Box::new(target),
                    value: Box::new(assign_value),
                }),
                Stmt::TailExpr(target_value),
            ]),
            // ||=: stmt → no-op (empty stmt list)
            (ast::AssignOp::OrAssign, false) => Expr::Unit,
            // ||=: expr context → original x (not `()`)
            (ast::AssignOp::OrAssign, true) => target_value,
            _ => unreachable!(),
        });
    }

    let Some(predicate) = predicate else {
        return Err(UnsupportedSyntaxError::new(
            "compound logical assign on Any/TypeVar (I-050 / generic bounds)",
            assign.span(),
        ).into());
    };

    // Emit: if <predicate> { x = y; }
    // For stmt context: naked If statement
    // For expr context: block `{ if <pred> { x = y; } x.clone() }`
    // ... (実装詳細は Task T2)
}
```

**5. `src/transformer/statements/control_flow.rs::try_generate_option_truthy_complement_match` 拡張**

Matrix C の C-4 (non-exit), C-5 (else), C-7〜C-19 (const-fold / peek-through / Layer 1
feed-through) を追加する:

- `else_body.is_some()` path 追加: consolidated match の `_` arm に else_body 配置
- `!ir_body_always_exits(then_body)` path 追加: predicate form emission `if <pred> { body }`
- `peek_through_type_assertions` 適用で `!(x as T)` 等 TsAs/TsNonNull/Paren 対応
- const-fold path 追加: `!null`/`!undefined`/`!lit` で body 直挿入 or empty
- non-Ident operand path 追加 (Layer 1 fall-through): `convert_unary_expr` 経由で
  falsy_predicate_for_expr が emit されるため、narrow 不可能 shape は Layer 1 fallback

**6. `src/pipeline/narrowing_analyzer/guards.rs::detect_early_return_narrowing` Bang arm 拡張**

```rust
// guards.rs L351-364 を以下に置換:
ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::Bang => {
    let unwrapped = peek_through_type_assertions(unary.arg.as_ref());
    if let ast::Expr::Ident(ident) = unwrapped {
        // 既存ロジック
    }
    // Member/OptChain は I-165 依存で scope out (narrow event 非出力)
}
```

#### 新規 IR 検討

既存 IR (`Expr::If`, `Expr::Match`, `Expr::Block`, `Expr::Let`, `Expr::Assign`) で
十分表現可能。新規 IR variant は不要。`Expr::Unit` (空式) は既存。

### Design Integrity Review

`.claude/rules/design-integrity.md` checklist 実施:

**1. Higher-level design consistency**:
- `truthy_predicate_for_expr` / `falsy_predicate_for_expr` は既存 `truthy_predicate(name, ty)` を内包する自然な拡張。name → expr への汎化で既存 call site (T6-3 consolidated match, `try_generate_primitive_truthy_condition`, `generate_truthiness_condition`, `generate_falsy_condition`) を同一 helper で表現。
- `peek_through_type_assertions` は既存 `unwrap_parens` (helpers.rs:92) の拡張。Paren のみの unwrap → Paren + TsAs + TsNonNull + TsTypeAssertion + TsConstAssertion。
- narrow event 記録経路 (`guards.rs::detect_early_return_narrowing`) の Bang arm 拡張は既存 pattern を踏襲 (Ident のみ → peek-through 経由 Ident)。
- **Verified, no inconsistency.**

**2. DRY**:
- 既存 `unwrap_parens` (helpers.rs:92) は `Paren` 単体 unwrap。新 `peek_through_type_assertions` はその superset。既存 `unwrap_parens` を新 helper から呼び出す形にし、純 Paren unwrap 時は既存 helper を利用する (DRY 維持)
- `truthy_predicate` / `falsy_predicate` (既存) は新 `truthy_predicate_for_expr` の Ident 特化 shorthand として維持 (テストロックイン + 既存 call site 互換性)
- const-fold ロジック (`!null`, `!lit`, `![]` 等) は 1 箇所に集約: `try_constant_fold_bang(expr: &ast::Expr) -> Option<Expr>` helper (`src/transformer/helpers/truthy.rs` 内配置) で DRY

**3. Orthogonality / Cohesion**:
- `truthy.rs` 単一モジュールに truthy/falsy/const-fold/TempBinder を集約。責務一貫 (JS truthy/falsy semantic の Rust 表現)。
- `peek_through.rs` 新規モジュールは「型のみ wrapper の AST unwrap」責務に focus。truthy と独立して他 PRD (I-143-a) でも再利用可能。
- `convert_unary_expr` / `convert_assign_expr` は dispatch のみ。predicate 合成ロジックは helpers に委譲。

**4. Coupling**:
- 新 helper は `ir::Expr` と `ir::RustType` に依存。既存と同じ。new coupling なし。
- `TempBinder` は局所 state。新規モジュール `truthy.rs` 内に閉じる。
- peek-through は `swc_ecma_ast` 依存のみ (既存 helpers.rs と同じ)。

**5. Broken Windows 発見**:
- **P1**: `generate_falsy_condition` (`helpers.rs:170`) の fallback `!Ident(x)` は Named/Vec/Fn 型 while-cond で silent compile error。現状は Option 経由別 path (WhileLet) で回避されているが、将来 `while ((x = named_expr))` パターンで発火リスク。**Fix in PRD**: 本 PRD で `truthy_predicate_for_expr` を使った更なる型網羅に置換。
- **P2**: `try_generate_primitive_truthy_condition` (`control_flow.rs:324-340`) は Ident 限定。`if (x as T)` 等 TsAs 経由で発火せず fall-through。**Fix in PRD**: peek-through 適用で TsAs/Paren 対応。
- **P3**: `detect_early_return_narrowing` Bang arm `ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::Bang` の inner `ast::Expr::Ident` 限定 (`guards.rs:351-364`)。**Fix in PRD (v3 拡張)**: (a) peek-through で Paren/TsAs/TsNonNull 経由 Ident をサポート、(b) **OptChain case 追加** — `extract_optchain_base_ident` (narrowing_patterns.rs:100 既存 pub(crate) helper) で base Ident を取得、`Option<T>` なら `NarrowEvent::Narrow` を `NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant)` で push。これは C-16b の実装で I-144 T6-4 (`x?.v !== undefined` 正向 narrow) と symmetric。Member LHS の field-path narrow (C-16n) のみ I-165 依存で defer。
- **P4**: 単体 truthy/falsy helper に `TempBinder` 概念不在。既存 tests は Ident 名前のみで `BinExpr`/`Call` operand 非テスト。**Fix in PRD**: `TempBinder` + 非 Ident expr テスト追加。

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/helpers/truthy.rs` | `truthy_predicate_for_expr`/`falsy_predicate_for_expr`/`TempBinder`/`try_constant_fold_bang`/`is_always_truthy_type` 新設、既存 `truthy_predicate`/`falsy_predicate` を内包 wrapper に |
| `src/transformer/helpers/mod.rs` | `peek_through` サブモジュール宣言、re-export |
| `src/transformer/helpers/peek_through.rs` (新規) | `peek_through_type_assertions(&ast::Expr) -> &ast::Expr` |
| `src/transformer/expressions/binary.rs` | `convert_unary_expr` Bang arm の type-aware dispatch 化 |
| `src/transformer/expressions/assignments.rs` | `convert_assign_expr` の AndAssign/OrAssign arm desugar 化 (stmt/expr context 両対応)、`convert_expr_with_expected(rhs, Some(lhs_inner))` で expected override |
| `src/pipeline/type_resolver/expressions.rs` | **L126-148 の compound assign branch を拡張**: 現 `NullishAssign` のみ expected propagate する条件に `AndAssign` / `OrAssign` を追加 (plain `=` at L94-125 と symmetric)、`lhs_type` resolve + `expected_types.insert(rhs_span, lhs_type)` + `propagate_expected(rhs, lhs_type)` 実施。I-175 の compound logical assign subset を structural fix (T3-TR) |
| `src/transformer/statements/control_flow.rs` | `try_generate_option_truthy_complement_match` の scope 拡張 (C-4/C-5/C-7〜C-19)、`try_generate_primitive_truthy_condition` の peek-through 適用、const-fold dispatch |
| `src/transformer/statements/helpers.rs` | `generate_truthiness_condition`/`generate_falsy_condition` の helper delegation に全型 dispatch 化 (broken window P1 fix) |
| `src/pipeline/narrowing_analyzer/guards.rs` | `detect_early_return_narrowing` Bang arm の peek-through 適用 |
| `tests/e2e/scripts/i161-i171/*.ts` (新規) | per-cell E2E fixture (red state 作成 + T6 で green 化) |
| `tests/observations/i161-i171/*.ts` (既作成) | tsc observation fixture |
| `tests/e2e_test.rs` | `test_e2e_cell_i161_i171` 関数追加 |
| `src/transformer/helpers/tests/` (新規 or 既存 mod) | `truthy_predicate_for_expr`/`falsy_predicate_for_expr` の unit test 網羅 (Matrix B.2 全 cell) |
| `src/transformer/statements/tests/control_flow.rs` (既存) | Matrix C 新セル regression test 追加 |
| `src/transformer/expressions/tests/assignments.rs` (既存 or 新規) | Matrix A 全 cell の unit test |

### Semantic Safety Analysis

`.claude/rules/type-fallback-safety.md` 適用:

本 PRD は型 fallback を **導入しない**。既存 `get_expr_type` / `get_type_for_var` の
narrow 適用型を dispatch key として使うのみで、型解決方針の変更はない。

1. **新規 fallback pattern**: 無し (const-fold は fallback ではなく TS 意味論に準拠した
   compile-time computation)
2. **既存 fallback の影響**: `generate_falsy_condition` の `!Ident(var)` fallback (P1)
   は本 PRD で **除去** される。除去後は全型で predicate が明示的に dispatch されるか、
   または `UnsupportedSyntaxError` で失敗する (silent → explicit) 改善。
3. **silent semantic change risk**:
   - `convert_unary_expr` Bang arm の旧 emission `!<operand_ir>` は **compile error**
     で fail する。silent semantic change なし (Tier 2 → fixed、元から silent ではない)
   - `convert_assign_expr` の `x = x && y` emission も同様に compile error → fix
   - const-fold (`!null` → `true` 等) は TS runtime と bit-exact で一致 (observations で確認)
4. **Verdict**: 全 pattern Safe (compile error の fix または TS runtime 一致の const-fold)

## Task List

### T0: Spec stage artifacts (現 stage、本 PRD 自体 + 既作成 observation)

- **Work**:
  - 本 PRD document 作成 (Matrix A/B/C 全 cell enumerate、ideal 出力定義)
  - tsc observation fixture 作成済 (`tests/observations/i161-i171/a1-a7 + b1-b14`、全 21 fixture)
  - Spec-Stage Adversarial Review Checklist 5 項目 green 化
  - red-state E2E fixture 作成 (T1 で実施、本 T0 の範囲は PRD doc + observation のみ)
- **完了条件**: 本 PRD document が Step 2 (spec stage review) を pass。Checklist 5 項目全 [x]
- **Depends on**: None

### T1: per-cell E2E fixture (red state 作成)

- **Work**:
  - `tests/e2e/scripts/i161-i171/*.ts` 作成 (Matrix A/B/C の ✗ cell 毎、2026-04-22 時点で 20+ fixture)
  - `scripts/record-cell-oracle.sh` で expected output 記録 (`.expected` ファイル)
  - `tests/e2e_test.rs` に `test_e2e_cell_i161_i171` 追加 (`#[ignore = "I-161/I-171 red state — unignore at T6"]`)
  - red 状態確認: 全 ✗ cell で transpile fail (UnsupportedSyntaxError) or cargo run fail (compile/runtime mismatch)
- **完了条件**: 全 ✗ cell の fixture が red 状態確認、test harness 登録済、report 作成
- **Depends on**: T0

### T2: 共有 helper 実装 (`truthy_predicate_for_expr` + `peek_through_type_assertions` + `TempBinder` + const-fold)

- **Work**:
  - `src/transformer/helpers/truthy.rs`: `TempBinder` struct + `truthy_predicate_for_expr(expr, ty, binder)` + `falsy_predicate_for_expr(expr, ty, binder)` + `try_constant_fold_bang(expr) -> Option<Expr>` + `is_always_truthy_type(ty) -> bool`
  - `src/transformer/helpers/peek_through.rs` 新規: `peek_through_type_assertions(&ast::Expr) -> &ast::Expr`
  - 既存 `truthy_predicate(name, ty)` / `falsy_predicate(name, ty)` を新 helper の Ident 特化 wrapper にリファクタ (backward compat)
  - unit test: Matrix B.2 全 cell (effective type × AST shape グループ) を網羅 (#[test] 関数 20+、`testing.md` の AST variant exhaustiveness 準拠)
- **完了条件**: 新 helper の unit test 全 pass、既存 `truthy_predicate`/`falsy_predicate` の test 非回帰、cargo fmt/clippy 0 warnings
- **Depends on**: T0

### T3: I-161 実装 (`convert_assign_expr` AndAssign/OrAssign 置換)

- **Work**:
  - `src/transformer/expressions/assignments.rs` L253-262 を T2 helper 経由 desugar 路に置換
  - stmt context 判定: `convert_stmt` 側で `Stmt::Expr(Expr::Assign{...})` を intercept して naked `Stmt::If` に emit するか、`convert_assign_expr` 内で block 式にまとめて emit
  - expr context: `{ if <pred> { x = y; } x.clone() }` または Copy 型では `{ if <pred> { x = y; } x }`
  - T8 always-truthy 型 const-fold 適用 (Named struct/enum / Vec / Fn / HashMap / DynTrait / Tuple / Ref(T))
  - narrow preservation: 新 emission は conditional assign なので、既存 classifier `ResetCause::CompoundLogical` (invalidating) と整合しないケースを検証 (Task T7 で empirical)
  - RHS wrap for Option<T>: `rhs_expected = Some(inner)` で `convert_expr_with_expected` 経由
  - **T3-TR (TypeResolver side fix、Critical 2026-04-22 /check_problem で発覚 + empirical 検証済)**: `src/pipeline/type_resolver/expressions.rs` L126-148 の compound assign branch は現状 `??=` のみ RHS への expected type propagation を実装しており、**AndAssign/OrAssign は propagate しない**。Transformer 側の `convert_expr_with_expected(rhs, Some(lhs_inner))` override だけでは `propagate_expected` が深部まで propagate されず、object literal RHS (`x &&= { a: 99 }`) は synthetic `_TypeLit0` emission に退化する。
    - **Empirical verification (2026-04-22)**: 同一 TS コードで plain `=` と `&&=` の emission を比較 — plain `=`: `p = P { a: 99.0 };` (✓ Named 経路)、`&&=`: `x = x && _TypeLit0 { a: 99.0 };` (✗ synthetic)。L94-125 が plain `=` のみ propagate している事実を empirical に確認。
    - **Root cause (trace 特定)**: `type_resolver/expressions.rs::Expr::Object` arm (L290-408) で TypeResolver が RHS object literal を resolve する際、`expected_types[obj_span]` が事前に set されていれば synthesis skip (L355-358)、未 set なら auto-synthesize + auto-register expected (L405-407)。Compound assign の `??=` branch (L138-147) は `expected_types.insert` するため OK、`AndAssign`/`OrAssign` branch (L135-148) は insert なしで、Object resolve 時に `_TypeLit0` synthesis が先行してしまう。
    - **修正**: plain `=` (L94-125) と symmetric に AndAssign/OrAssign branch を追加し、`lhs_type` resolve + `expected_types.insert(rhs_span, lhs_type)` + `propagate_expected(rhs, lhs_type)` を実施。これにより `&&=`/`||=` × object literal RHS × Named/Option<Named> LHS の path が coerce 正常化し、結果として **I-175 TODO の compound logical assign subset が T3-TR で indirectly 解消** する (I-175 TODO 本体は他 compound assign `+=` / `-=` 等に残存、本 PRD scope 外)。cell-a7 の mkP workaround は T3-TR 実装後に empirical 検証で removal 可否判定
- **Unit test enumeration (明示列挙)**:
  - **Matrix A primary** (&&=): T1 Bool / T2 F64 / T3 String / T4 Primitive(int via bigint i128) / T5 Option<F64> / T5s Option<String> / T6 Option<synthetic union> / T7 Option<Named other> / T8 Named struct / T8 Vec / T8 HashMap / T8 DynTrait / **T12d Tuple** / **T12b Ref(T)** = **13 cell × {Ident LHS, Member LHS} × {stmt, expr} = 52 case**
  - **Matrix O primary** (||=): 同 13 type × 2 shape × 2 context = **52 case** (AndAssign と対称で falsy predicate 版)
  - **Matrix A.5 expr-context × Copy/non-Copy cross product**: 12 cell を individually:
    - A-1x Bool Copy: `{ if x { x = rhs; } x }` (trailing `x`、no clone)
    - A-2x F64 Copy: 同上 (F64 Copy)
    - A-3x String !Copy: `{ if !x.is_empty() { x = rhs.to_string(); } x.clone() }` (trailing clone)
    - A-4x Primitive(int) Copy: 同 A-2x 相当
    - A-5x Option<primitive Copy>: Copy (Option<Copy> is Copy)
    - A-5sx Option<String> !Copy: `{ ..; x.clone() }`
    - A-6x Option<union enum> !Copy: 同上
    - A-7x Option<Named> !Copy: 同上
    - A-8x Named !Copy (struct): const-fold `{ x = y; x.clone() }`
    - A-8x Vec !Copy: 同 A-8x Named
    - A-12dx Tuple (elements Copy): Copy `{ x = y; x }`
    - A-12dx Tuple (elements !Copy e.g. `[string, number]`): !Copy `{ x = y; x.clone() }`
    - A-12bx Ref(T) Copy: `&T` / `&mut T` は Copy、`{ x = y; x }` (const-fold always-truthy)
    - **合計 12 個別 cell + O 側対称 12 = 24 case**
  - **Matrix A-14a〜f / O-14a〜f Tier 2 SimpleAssignTarget NA**: `convert_assign_expr` match arm が `UnsupportedSyntaxError` を返却することを **error-path unit test** で確認 × 6 Tier 2 variant × 2 op = **12 case**
  - integration test: TS → IR pipeline snapshot (`tests/snapshot_test.rs` に fixture) で primary + expr-context を cover
- **完了条件**: Matrix A/O 全 in-scope cell (primary 26 + A.5 cross 24 + Tier 2 NA error-path 12) で ideal emission または expected error、unit test green、emission shape を integration snapshot で lock-in、**T3-TR TypeResolver fix 後の cell-a7 fixture の mkP workaround 除去可否を empirical 検証** (RHS `{ a: 99 }` 直接記述で P 型 emission が出るか確認、可能なら fixture を `x &&= { a: 99 };` 直書きに戻す)
- **Depends on**: T2

### T4: I-171 Layer 1 実装 (`convert_unary_expr` Bang arm type-aware dispatch)

- **Work**:
  - `src/transformer/expressions/binary.rs::convert_unary_expr` L272-283 を type-aware dispatch に置換
  - peek-through 適用 (Paren/TsAs/TsNonNull/TsTypeAssertion/TsConstAssertion)
  - const-fold 適用 (`!null` / `!undefined` / `!lit` / `!<always-truthy>`)
  - double negation `!!<e>` → `truthy_predicate_for_expr(<e>, ty)`
  - 非 Ident operand で tmp binding 発動 (BinExpr/Call/Cond 等)
- **Unit test enumeration (明示列挙)**:
  - **Matrix B.2 type dispatch** (falsy_predicate_for_expr): T1 Bool / T2 F64 / T3 String / T4 Primitive(int usize) / T4 Primitive(int i128 BigInt) / T5 Option<F64> / T5s Option<String> / T6 Option<union enum> / T7 Option<Named other> / T8 Named / T8 Vec / T8 HashMap / T8 DynTrait / T12d Tuple / T12b Ref(T) / T12 Unit = **16 cell × operand=Ident = 16 case**
  - **Matrix B.1 shape dispatch** (convert_unary_expr): 全 Tier 1 Expr variant を網羅
    - B.1.1 Ident (代表型: Option<F64>) → 1 case
    - B.1.2-B.1.13 Lit 12 variant × const-fold → 12 case
    - B.1.14 Paren (peek-through) → 1 case
    - B.1.15 Member (field access、対応型別) → 2 case
    - B.1.16 OptChain → 1 case
    - B.1.17 TsAs (peek-through) → 1 case
    - B.1.18 TsNonNull (peek-through) → 1 case
    - B.1.19 Unary(!) double neg → 2 case (Option/F64)
    - B.1.20 Unary(-)/(+)/(TypeOf) operand → 3 case
    - B.1.21 Bin(Arithmetic +/-/*/div/mod) → 2 case (F64/Primitive)
    - B.1.22 Bin(Comparison) → 1 case (Bool result)
    - B.1.23 Bin(LogicalAnd) → 1 case (De Morgan)
    - B.1.24 Bin(LogicalOr) → 1 case
    - B.1.25 Bin(Bitwise `&`/`\|`/`^`/`<<`/`>>`/`>>>`) → 1 case (F64 result)
    - B.1.26 Bin(InstanceOf) → 1 case (Bool)
    - B.1.27 Bin(In) → 1 case (Bool)
    - B.1.28 Bin(NullishCoalescing) → 2 case (result type varies)
    - B.1.29 Call → 1 case (tmp bind)
    - B.1.30 Cond (ternary) → 1 case (tmp bind)
    - B.1.31 New → 1 case (const-fold)
    - B.1.32 Await → 1 case (tmp bind)
    - B.1.33 Assign → 1 case (tmp bind、side effect)
    - B.1.34 Array/Object/Tpl/Arrow/Fn → 5 case (5 always-truthy variant 個別)
    - B.1.35 This → 1 case
    - B.1.36 Update → 2 case (prefix/postfix)
    - B.1.37g TsTypeAssertion (peek-through) → 1 case
    - B.1.37i TsConstAssertion (peek-through) → 1 case
    - 合計: **約 48 case**
  - **Matrix B Tier 2 NA error-path**: Seq/Yield/MetaProp/Class/TaggedTpl/SuperProp/TsSatisfies/TsInstantiation/PrivateName/Invalid の 10 variant で `convert_expr` が UnsupportedSyntaxError を返却することを error-path test で確認 → **10 case**
  - **TempBinder / try_constant_fold_bang 単体 test**: TempBinder fresh 名前生成 3 case + const-fold 個別 12 Lit variant + is_always_truthy_type 判定 6 case = **21 case**
  - 合計: 16 + 48 + 10 + 21 = **~95 unit case**
- **完了条件**: Matrix B.1 × B.2 の全 in-scope cell (~48 shape cell + 16 type cell) で ideal emission、Tier 2 NA error-path test green、unit test 全 green
- **Depends on**: T2

### T5: I-171 Layer 2 実装 (`try_generate_option_truthy_complement_match` 拡張)

- **Work**:
  - `src/transformer/statements/control_flow.rs::try_generate_option_truthy_complement_match` を scope 拡張:
    - **C-4 (non-exit body)**: `!ir_body_always_exits(body)` 路で predicate form `if <x falsy> { body }` emission (narrow 材料化なし、Layer 1 helper feed-through)
    - **C-5 (else branch)**: `else_body.is_some()` 路で consolidated match に else_body を truthy arm body として配置: `match x { Some(v) if <truthy> => { else_body }, _ => { then_body } }`
    - **C-7〜C-10 (const-fold)**: Lit null/undefined/lit で body 直挿入 or empty stmt
    - **C-11〜C-14 (peek-through)**: Paren/TsAs/TsNonNull/Unary(!!) で inner recurse
    - **C-15 (Member, always-exit only)**: `if <member>.is_none() { exit }` (narrow 材料化なし、narrow event も記録しない)
    - **C-16a (OptChain, always-exit, Layer 1)**: OptChain の is_none 展開 `if <base>.is_none() || <inner>.is_none() { exit }`
    - **C-16b (OptChain base narrow、T6 connected)**: post-if で `x` → non-null narrow (`guards.rs` 側で narrow event 記録 → `convert_if_stmt` が narrow event を consume)
    - **C-17〜C-19 (Bin/Call/Cond/Await/New operand)**: Layer 1 helper feed-through (tmp bind + predicate)
    - **C-23 (LogicalOr inner)**: De Morgan `<x falsy> && <y falsy>`
    - **C-24 (always-truthy operand)**: const-fold `!<truthy> = false` → body not executed
  - `convert_if_stmt` dispatch guard (L119-125) の更新: `else_body.is_none()` guard 除去、`ir_body_always_exits` は Layer 2 optional
  - integration test: Matrix C 全 in-scope cell の snapshot
- **完了条件**: Matrix C の全 in-scope cell (C-4/C-5/C-7〜C-19/C-23/C-24) で ideal emission、既存 C-1/C-2/C-3 regression 非破壊。C-16b (OptChain base narrow) は T6 P3b guards.rs 拡張で guards 側から narrow event が push されれば既存 `narrowed_type` query 経由で自動 consume されるため T5 本体の emission 変更は不要。C-16b fixture の `#[ignore]` 解除は T6 責務
- **Depends on**: T2, T4

### T6: broken window fix (P1/P2/P3/P4) + narrow_analyzer 拡張

- **Work**:
  - **P1**: `generate_truthiness_condition` / `generate_falsy_condition` (`src/transformer/statements/helpers.rs:161, 169`) の fallback を `truthy_predicate_for_expr`(Expr::Ident) に置換、全型網羅化
  - **P2**: `try_generate_primitive_truthy_condition` (`src/transformer/statements/control_flow.rs:324-340`) に peek-through 適用 (現 `unwrap_parens` → `peek_through_type_assertions`)
  - **P3**: `detect_early_return_narrowing` Bang arm (`src/pipeline/narrowing_analyzer/guards.rs:351-364`) を 2 段階で拡張:
    - **P3a**: peek-through 適用 (inner が Paren/TsAs/TsNonNull/TsTypeAssertion/TsConstAssertion 経由 Ident の場合 narrow 記録)
    - **P3b (new、C-16b 対応)**: OptChain case 追加 — `extract_optchain_base_ident(unary.arg)` (narrowing_patterns.rs:100 既存 helper) で base Ident を取得、`Option<T>` なら `NarrowEvent::Narrow` を `NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant)` で push。I-144 T6-4 (`x?.v !== undefined` 正向 narrow) と symmetric、~10 LOC
  - **P4**: `src/transformer/helpers/truthy.rs` の既存 test を新 helper 経由にも expand、非 Ident expr を含む test 追加
  - E2E fixture un-ignore (T1 で作成した `#[ignore]` 削除、test_e2e_cell_i161_i171 を un-ignore)
- **完了条件**: broken window P1-P4 全 fix (P3 は P3a+P3b 両方)、E2E fixture 全 green 化、既存 regression 非破壊
- **Depends on**: T2, T3, T4, T5

### T7: classifier / emission 相互検証 (empirical、narrow × logical assign 網羅)

- **Work**:
  - `ResetCause::CompoundLogical` の扱いを `&&=`/`||=` 新 emission path で empirical 検証
  - 現 classifier は `&&=`/`||=` を `invalidates_narrow=true` と記録するが、新 emission では `if <pred> { x = y; }` 構造で RHS の値域が narrow 型と一致する場合 narrow は semantic に preserve
  - **検証 cell (網羅)**:
    - **T7-1 (R4 &&= on narrowed F64)**: `let x: number | null = 5; if (x !== null) { x &&= 3; return x; }` → narrow preserved、return 3
    - **T7-2 (R4 ||= on narrowed F64)**: `let x: number | null = 5; if (x !== null) { x ||= 99; return x; }` → narrow preserved、return 5 (truthy)
    - **T7-3 (&&= + closure reassign)**: narrow alive state で `const reset = () => { x = null; }; x &&= 3; reset();` のような closure capture interaction → I-144 T6-2 closure-reassign suppression との整合確認
    - **T7-4 (||= on narrowed + subsequent ??=)**: `x ||= default; x ??= other;` chain で narrow 伝播
    - **T7-5 (&&= on narrowed union)**: `if (x !== null) { x &&= "hello"; }` on synthetic union — RHS が string で assign 後 x: String narrow 継続するか
    - **T7-6 (narrow reset via &&= with incompatible RHS type)**: narrow alive の F64 に `x &&= "text"` — **unit test で検証** (E2E 不適、compile fail が期待結果)。emission path で type error 化するか / UnsupportedSyntaxError 返却するかを assertion
  - 必要なら classifier を `CompoundLogical` の subclass (type-aware preservation) 化、または emission 側で reset event 無視
  - report 作成 (`report/i161-i171-classifier-emission-cohesion.md`) に全 6 cell の 検証結果 + 設計判断記録
- **完了条件**: T7-1〜T7-5 は E2E regression fixture として全 green (`tests/e2e/scripts/i161-i171/cell-t7-{1..5}-*.ts`)、T7-6 は unit test (`src/transformer/expressions/tests/assignments.rs`) で incompatible RHS type の expected error-path 検証 pass、report 記述済
- **Depends on**: T3, T6

### T8: Hono bench 非後退 + file-line check + quality gate

- **Work**:
  - `./scripts/hono-bench.sh` 実行、既存 clean/errors 数値と比較 (≥ 後退なし)
  - `./scripts/check-file-lines.sh` pass (1000 LOC threshold)
  - `cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --all --check` / `cargo llvm-cov` 全 pass
  - 本 PRD の成果 summary 作成 (plan.md 更新 + TODO I-161/I-171 削除)
- **完了条件**: 全 quality gate pass、Hono bench regression 0、plan.md 更新
- **Depends on**: T6, T7

## Test Plan

### Unit tests (T2, T3, T4 derived)

- `src/transformer/helpers/truthy.rs::tests`: 
  - Matrix B.2 T1-T8 effective type × Ident operand 12 case
  - 非 Ident operand (BinExpr, Call, Cond, Member, OptChain) × 主要型 10 case
  - const-fold: `!null`/`!undefined`/`!0`/`!""`/`!false`/`!true`/`![]`/`!fn` 8 case
  - double negation `!!x` on Option/F64/String 6 case
  - peek-through: `!(x as T)` (TsAs) / `!(x!)` (TsNonNull) / `!(x)` (Paren) / `!(<T>x)` (TsTypeAssertion) / `!(x as const)` (TsConstAssertion) / nested `!(((x as T)!))` / peek-through 後に double-neg `!!(x as T)` = 7 case
  - TempBinder: fresh name generation 3 case
- `src/transformer/helpers/peek_through.rs::tests`: Paren/TsAs/TsNonNull/TsTypeAssertion/TsConstAssertion の単独/ネスト + non-peek-through expr は identity で返す negative test = 10 case
- `src/transformer/expressions/tests/assignments.rs`: Matrix A T1-T8 + T12 Tuple + T12 Ref(T) × {&&=, ||=} × {Ident, Member} × {stmt, expr} = **80 case** + Tier 2 SimpleAssignTarget NA error-path (A-14a〜f / O-14a〜f × 2 op = 12 case) + A.5 expr-context × Copy/non-Copy cross product 24 case + **T7-6 (narrow × incompatible RHS type の error-path unit test) 1 case** = 117 case
- `src/transformer/statements/tests/control_flow.rs`: Matrix C の新 ✗ cell (C-4/C-5/C-7〜C-19/C-23/C-24) を snapshot (insta) lock-in

### Integration tests (T3, T4, T5 derived)

- `tests/snapshot_test.rs`: 
  - I-161 fixture: `tests/fixtures/i161-and-assign-*.input.ts` + snapshot
  - I-171 fixture: `tests/fixtures/i171-bang-*.input.ts` + snapshot
  - narrow 材料化回帰: `tests/fixtures/narrowing-truthy-*.input.ts` 再検証

### E2E tests (T1, T6 derived)

- `tests/e2e/scripts/i161-i171/*.ts`: per-cell runtime verification (tsx stdout == rust stdout)
- 対象 cell: Matrix A の全 primary ✗ cell + Matrix B の主要 ✗ cell + Matrix C の新 ✗ cell = 30+ fixture

### Regression / lock-in (T6, T7 derived)

- T6-3 (I-144) cell-t4c/t4d/i024 の re-run で非後退確認
- R4 regression (I-144 で削除、本 PRD で再生成): narrow preservation empirical
- I-154 label hygiene / I-142 ??= emission との非干渉確認

### Hono bench (T8 derived)

- `scripts/hono-bench.sh` で clean/errors 数値測定、pre-PRD との diff を report
- `ideal-implementation-primacy.md` 通り goal は 0 regression (改善は signal として記録のみ)

## Completion Criteria

`.claude/rules/prd-completion.md` 準拠。以下全てを満たすこと:

1. **Matrix 全セルカバー (最上位条件)**:
   - [ ] Matrix A 全 in-scope (non-narrow) cell (A-1〜A-8 × {Ident, Member} × {stmt, expr} + O-1〜O-8) の実出力が ideal 仕様と一致
   - [ ] Matrix A.4 (narrow × compound assign) 全 cell に `I-177 依存 deferred` annotation + I-177 完了時の T3-N sub-task 定義済
   - [ ] Matrix B 全 in-scope cell (B.1.1〜B.1.34 × T1-T8) の実出力が ideal 仕様と一致 **(T4 完了 2026-04-23: B.1.23/B.1.32/B.1.33/B.1.36/B-T6 は Rust emission ✓ / E2E は外部 PRD blocker I-177/I-179/I-180/I-181 依存。emission 層の正しさは unit test + Matrix cell Empirical note で lock-in、外部 PRD 完了後に E2E un-ignore)**
   - [ ] Matrix C 全 in-scope cell (C-1〜C-3 regression + C-4/C-5/C-7〜C-19/C-23/C-24) の実出力が ideal 仕様と一致
   - [ ] Out-of-scope cell (A-9, O-9, A-10, O-10, C-15n, C-16n, C-20, C-21) に明示的 blocked annotation、対応する TODO/別 PRD link 済
   - [ ] Split-off (I-177/I-178/I-179/I-180/I-181) cell/rule に対応 TODO entry が立っている
   - [ ] 全セルに test (unit / integration / E2E) 対応
   - [ ] **Spec vs Design integrity**: Matrix ideal column と Design section emission form が一致している (SG-2 empirical lesson 2026-04-22、今後 I-178 で framework 化)
   
2. **Quality gate**:
   - [ ] `cargo test` 全 pass (現 2880 lib + 122 integration + 3 compile + 114 E2E)
   - [ ] `cargo clippy --all-targets -- -D warnings` 0 warnings
   - [ ] `cargo fmt --all --check` 0 diffs
   - [ ] `cargo llvm-cov --fail-under-lines 90` pass
   - [ ] `./scripts/check-file-lines.sh` pass (≤ 1000 LOC per file)

3. **regression 非破壊**:
   - [ ] I-144 T6-3 cell (t4c/t4d/i024) 全 green 維持
   - [ ] I-142 `??=` emission regression 0
   - [ ] I-154 label hygiene regression 0
   - [ ] 既存 snapshot test 全 pass (差分があれば意図的変更として review)

4. **Hono bench**:
   - [ ] clean/errors 数値 regression 0 (改善は signal のみ、goal ではない)

5. **Document sync**:
   - [ ] plan.md 更新 (I-161/I-171 を直近完了作業に移動、優先 table から削除)
   - [ ] TODO から I-161 / I-171 entry 削除
   - [ ] `doc/handoff/design-decisions.md` に本 PRD 設計判断 section 追加 (truthy_predicate_for_expr / peek_through_type_assertions 統合設計)
   - [ ] backlog/I-161-I-171-*.md を本 PRD 完了後に削除 (履歴は git log)

6. **Spec stage artifacts 保持**:
   - [ ] tsc observation fixtures (`tests/observations/i161-i171/*.ts`) 永続保存
   - [ ] E2E red→green 記録 report 作成

**Impact estimate verification**: 本 PRD は Hono bench 直接 blocker ではない (empirical
verified 2026-04-21、`/tmp/t63-investigate/*`)。error count 削減の estimate は出さない
(`ideal-implementation-primacy.md` + `problem-space-analysis.md` により正確性が metric)。
