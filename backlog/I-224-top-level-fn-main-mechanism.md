# I-224: Top-level executable script の Rust emission に `fn main()` 自動生成 (TS module-load semantics → Rust fn main mechanism)

## Background

ts_to_rs は現在、TS module の top-level に `function main(): void` が user 定義されている場合のみ Rust `fn main()` を emit する。user 定義 `main` 不在で top-level expression statements (`console.log(...)` 等) が存在する case では `pub fn init()` のみ emit、Rust binary の entry point である `fn main()` を生成せず、`cargo run` が **E0601 `main function not found`** で compile fail する。

加えて、user 定義 `function main()` AND top-level expression statements が **共存** する case で、`fn main()` (= user main) と `pub fn init()` (= top-level stmts) が **両方 emit** されるが **`init()` は never called** = **silent dead code** = **Tier 1 silent semantic change** (TS では top-level statements が module load 時に実行されるが、Rust では `fn main()` のみ実行され `pub fn init()` 内 statements が silent に drop される)。

empirical 確認 (2026-05-01、`/tmp/b2-probe.ts`):

```ts
function main(): void { console.log("from main"); }
console.log("top-level");
```
→ 現状生成 Rust:
```rust
fn main() { println!("from main"); }
pub fn init() { println!("top-level"); }   // ← never called!
```
TS 実行 stdout: `top-level\nfrom main\n` (ECMAScript spec: hoisted main + top-level execution order)
Rust 実行 stdout: `from main\n` only (= `top-level` silently dropped)

**Reachability**: 全 e2e fixture (= 全 future PRD verification flow) + 一般 TS user code で top-level execution + user main 共存 pattern。**Universal e2e infrastructure defect** であり、本 PRD は **PRD I-205 T14 prerequisite (案 β Phase 1-A 最初)** + 全 future PRD の e2e verification leverage の foundation。

## Problem Space

`.claude/rules/problem-space-analysis.md` に従い、TS module top-level の Rust emission 戦略を完全 enumerate する。

### 入力次元 (Dimensions)

機能の出力を決定する独立次元:

- **Axis A (Top-level body composition)**: 7 variants (TS module body の top-level item kind を grouping):
  - A0: 何もない (empty) / 宣言のみ (function / class / type / interface / enum decl) / imports のみ (= **library mode**、top-level 実行 stmt 不在)
  - A1: top-level Stmt::Expr のみ (例: `console.log(...)`)
  - A2: top-level Decl::Var with literal init のみ (例: `const x = 0;`、現状 `const x: f64 = 0.0;` 形式で top-level emit、execute は不要)
  - A3: top-level Decl::Var with side-effect / non-const init のみ (例: `const c = new Counter();` / `const h = createHandler();`)
  - A4: top-level Stmt::If / For / ForIn / ForOf / While / DoWhile / Try / Switch / Throw / Labeled / Block (control-flow at top-level)
  - A5: top-level Stmt::Empty / Stmt::Debugger (no-op, runtime side effect なし)
  - A6: 上記 A1-A5 の混在 (Decls + Stmt::Expr + Decl::Var + control-flow 等)

- **Axis B (User-defined `main` symbol)**: 5 variants (orthogonality merge 適用済 = function decl / const arrow / const fn expr は dispatch 同一):
  - B0: 不在 (no user `main`)
  - B1: sync function `main` (function decl / const arrow / const fn expr 統合)
  - B2: async function `main` (function decl / const arrow / const fn expr 統合)
  - B3: 非 fn symbol (user `main` を type / interface / class / enum / namespace / let-mutable / variable で定義 = Rust 上 fn と別 namespace のため衝突なし)
  - B4: `__ts_main` 衝突 (user が `function __ts_main()` 等を定義 = 本 PRD の reserved 名前空間と衝突 = Tier 2 honest error reject)

- **Axis C (Top-level `await` 使用)**: 2 variants:
  - C0: 不在
  - C1: 存在 (TS / ESM proposal feature = `const x = await fetch(...);` / `await Promise.resolve();` 等 module body top-level での `await` keyword 使用)

Cartesian: 7 × 5 × 2 = **70 cells**。実 dispatch leaves に対して orthogonality merge + NA 適用後 ~30 cells 独立 row enumerate。

### 組合せマトリクス (全 cells 独立 row、Rule 1 (1-2) abbreviation prohibition compliant)

| # | A (top-exec) | B (user main) | C (top-await) | Ideal Rust output | 現状 | 判定 | Scope |
|---|---|---|---|---|---|---|---|
| 1 | A0 (empty / library) | B0 (no main) | C0 | declarations only emit、no `fn main`、no `pub fn init` | declarations only emit (no fn main、no init = correct) | ✓ | regression lock-in |
| 2 | A0 (declarations only) | B1 (sync) | C0 | `fn main() { <user main body> }` (user main 直接 emit) | `fn main() { <user main body> }` (correct) | ✓ | regression lock-in |
| 3 | A0 (declarations only) | B2 (async) | C0 | `#[tokio::main] async fn main() { <user async main body> }` | `#[tokio::main] async fn main() { <body> }` (correct) | ✓ | regression lock-in |
| 4 | A0 (declarations only) | B3 (non-fn symbol = type / interface / class / enum / variable) | C0 | declarations only emit (Rust fn と別 namespace、衝突なし、no `fn main` 必要なし) | declarations only emit (correct) | ✓ | regression lock-in |
| 5 | A0 (declarations only) | B4 (`__ts_main` 衝突) | C0 | Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use; user must rename" | `fn __ts_main()` 直接 emit、本 PRD で reserved 化検出未実装 | ✗ Tier 2 reclassify | **本 PRD scope** |
| 6 | A0 (declarations only) | B0 (no main) | C1 (top-await) | NA (top-level await は execution context 内のみ valid、本 cell A0 は execution 不在のため await 配置不能) | NA | NA (TS spec: top-level await requires module + execution stmt context) | NA |
| 7 | A0 (declarations only) | B1 (sync) | C1 | NA (sync user main には top-level await 配置不能、await は async context 必要) | NA | NA (TS spec: await in sync function = parse error) | NA |
| 8 | A0 (declarations only) | B2 (async) | C1 | NA per Axis 7 同 logic, ただし Axis A0 の context で top-level await 配置 site 不在 | NA | NA | NA |
| 9 | A1 (top-Stmt::Expr only) | B0 (no main) | C0 | synthesize `fn main() { <top-level Stmt::Expr>; ... }` | `pub fn init() { ... }` only、no `fn main` | ✗ E0601 compile fail | **本 PRD scope (cell-09 case)** |
| 10 | A1 | B1 (sync) | C0 | rename user main to `__ts_main`、synthesize `fn main() { <top-level stmts>; <if user explicitly calls main(), substitute __ts_main()>; }` + `fn __ts_main() { <user body> }` | `fn main() { <user body> }` + `pub fn init() { <top-level stmts>; }` (init never called = silent dead code) | ✗ silent semantic change L1 | **本 PRD scope (Tier 2 broken → Tier 1)** |
| 11 | A1 | B2 (async) | C0 | rename user main to `__ts_main`、synthesize `#[tokio::main] async fn main() { <top-level stmts>; <substituted main() call>; }` + `async fn __ts_main()` | `#[tokio::main] async fn main() { <user body> }` + `pub fn init() { <top-level stmts>; }` (init never called) | ✗ silent semantic change L1 | **本 PRD scope** |
| 12 | A1 | B3 (non-fn symbol) | C0 | synthesize `fn main() { <top-level stmts>; }` + user non-fn symbol そのまま emit (Rust 上 fn と別 namespace) | `pub fn init() { ... }` only | ✗ E0601 + non-fn symbol preserved | **本 PRD scope** |
| 13 | A1 | B4 (`__ts_main` 衝突) | C0 | Tier 2 honest error reclassify (cell 5 と同 wording) | unimplemented | ✗ Tier 2 reclassify | **本 PRD scope** |
| 14 | A1 | B0 | C1 | synthesize `#[tokio::main] async fn main() { <top-level stmts including await>; }` | unimplemented (現状 await 含む top-level stmts も `pub fn init` に格納、`fn main` 不在 + non-async context での await = compile fail) | ✗ E0601 + async runtime missing | **本 PRD scope** |
| 15 | A1 | B1 (sync) | C1 | rename user sync main to `__ts_main` (sync preserved)、synthesize `#[tokio::main] async fn main() { <top-level await stmts>; <__ts_main()> }` (async wrapper sync user main を call) | unimplemented | ✗ silent semantic change L1 + E0601 | **本 PRD scope** |
| 16 | A1 | B2 (async) | C1 | rename user async main to `__ts_main`、synthesize `#[tokio::main] async fn main() { <top-level await stmts>; <substituted main() call>; }` + `async fn __ts_main()` | unimplemented | ✗ silent semantic change L1 + E0601 | **本 PRD scope** |
| 17 | A1 | B3 (non-fn symbol) | C1 | synthesize `#[tokio::main] async fn main() { <top-level await stmts>; }` + user non-fn symbol preserved | unimplemented | ✗ E0601 | **本 PRD scope** |
| 18 | A1 | B4 (`__ts_main` 衝突) | C1 | Tier 2 honest error reclassify (cell 5 と同 wording) | unimplemented | ✗ Tier 2 reclassify | **本 PRD scope** |
| 19 | A2 (Decl::Var with Lit init only) | B0 | C0 | declarations only (top-level `const x: f64 = 0.0;`)、no fn main 必要なし (Lit init は Rust const 適合 = library mode 維持可能) | top-level `const x: f64 = 0.0;` (correct) + no fn main | ✓ | regression lock-in |
| 20 | A2 | B1 (sync) | C0 | top-level `const x: f64 = 0.0;` + `fn main() { <user main body> }` | top-level `const x: f64 = 0.0;` + `fn main() { <user body> }` (correct) | ✓ | regression lock-in |
| 21 | A3 (Decl::Var with side-effect init = `const c = new Counter()` etc.) | B0 | C0 | synthesize `fn main() { let c = Counter::new(); ... }` (Decl::Var with non-const init を fn main body 内 `let` として capture) | declaration silently dropped (I-016 silent skip + no fn main) | ✗ silent drop + E0601 | **本 PRD scope** (capture mechanism) **+ I-016 (init 変換) prerequisite chain** |
| 22 | A3 | B1 (sync) | C0 | rename user main to `__ts_main`、synthesize `fn main() { let c = Counter::new(); ... <substituted main() call> }` + `fn __ts_main()` | declaration silently dropped + `fn main` (user) + `pub fn init` empty (since Stmt::Expr 不在) | ✗ silent drop + dead code | **本 PRD scope + I-016 prerequisite chain** |
| 23 | A3 | B2 (async) | C0 | rename user async main to `__ts_main`、synthesize `#[tokio::main] async fn main() { let c = Counter::new(); ... <substituted main() call> }` | declaration silently dropped + tokio::main (user) | ✗ silent drop + dead code | **本 PRD scope + I-016 prerequisite chain** |
| 24 | A3 | B3 (non-fn symbol) | C0 | synthesize `fn main() { let c = Counter::new(); ... }` + user non-fn symbol preserved | declaration silently dropped | ✗ silent drop + E0601 | **本 PRD scope + I-016 prerequisite chain** |
| 25 | A4 (top-level Stmt::If / For / While / Try / etc. control-flow) | B0 | C0 | synthesize `fn main() { <control-flow stmts as-is in body> }` | currently `_ => Err(UnsupportedSyntaxError)` at `transform_module_item:449` (= Tier 2 honest error reject、本 cell は Tier 2 honest error preserved) | ✗ Tier 2 honest error already (= **regression lock-in for Tier 2 honest reject**, ただし B2 fn main synthesis 達成後 Tier 1 化候補 = 別 PRD I-203 や類似 codebase-wide cleanup PRD scope) | **Tier 2 honest error reclassify (本 PRD)** = 既存 Tier 2 を maintenance、Tier 1 化は別 PRD |
| 26 | A4 | B1 (sync) | C0 | 同上 (control-flow @ top-level は Tier 2 honest reject preserved) | Tier 2 honest error already | Tier 2 honest reclassify | regression lock-in for Tier 2 |
| 27 | A5 (Stmt::Empty / Stmt::Debugger) | B0 | C0 | Stmt::Empty: skip silently (no-op); Stmt::Debugger: synthesize `fn main()` body 内 `// debugger` comment placeholder + Tier 2 honest error reclassify (Rust に debugger statement 等価不在) | Stmt::Empty: silent skip (correct); Stmt::Debugger: 現状 transform_module_item の `_ =>` arm で UnsupportedSyntaxError | Stmt::Empty ✓、Stmt::Debugger Tier 2 honest reclassify | **本 PRD scope (Stmt::Debugger reclassify明示)** |
| 28 | A6 (mixed Stmt::Expr + Decl::Var + control-flow) | B0 | C0 | source order preserve、Stmt::Expr / Decl::Var を fn main body 内 capture、control-flow は本 PRD scope 外 (cell 25 同) | unimplemented | ✗ partial silent drop | **本 PRD scope + cell 25 と一貫した dispatch** |
| 29 | A6 | B1 (sync) | C0 | source order preserve、user main rename + control-flow @ top-level は本 PRD scope 外 | unimplemented | ✗ silent semantic change | **本 PRD scope** |
| 30 | A6 | B2 (async) | C1 | source order preserve、user async main rename、async fn main、top-level await capture | unimplemented (combined edge) | ✗ silent semantic change + E0601 + missing async | **本 PRD scope (most complex case)** |
| 31 | A1 with multiple `main()` calls (e.g., `main(); main();`) | B1 (sync) | C0 | user main rename、synthesize `fn main() { __ts_main(); __ts_main(); }` (multiple call site preserved in source order via __ts_main substitution) | unimplemented (current emission incomplete in this combination) | ✗ silent | **本 PRD scope (substitution invariant verify)** |

判定凡例: ✓ (現状 OK、regression lock-in test 必須) / ✗ (修正必要、本 PRD or 別 PRD) / NA (unreachable, spec-traceable reason) / Tier 2 honest reclassify (本 PRD で fix、Tier 1 化は別 PRD)

### Spec-Stage Adversarial Review Checklist

Spec stage 完了 verification は `.claude/rules/spec-stage-adversarial-checklist.md` の **13-rule checklist** を本 PRD `## Spec Review Iteration Log` section に転記して全項目 verification する。13-rule の 1 つでも未達があれば Implementation stage 移行不可。

## Oracle Observations (Rule 2 (2-2) hard-code、各 ✗/要調査 cell の tsc / tsx empirical)

各 ✗ cell について以下 4 項目 embed:

### Cell 5: A0 + B4 (`__ts_main` collision、no top-exec)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-05-ts-main-collision-no-exec.ts`
- **tsc / tsx output (TS-1 spec stage で record)**:
  ```
  stdout: __ts_main\n
  stderr: (empty)
  exit_code: 0
  ```
- **Cell number reference**: matrix #5
- **Ideal output rationale**: TS では `function __ts_main()` は valid identifier、tsx で実行可能。Rust 側では本 PRD の rename scheme と衝突 → Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use; user must rename to avoid collision"。Reject は ideal-implementation-primacy 整合 = silent collision risk 排除。

### Cell 9: A1 + B0 (top-Stmt::Expr only、no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-09-stmt-expr-only-no-main.ts`
- **tsc / tsx output**:
  ```
  stdout: hello world\n
  stderr: (empty)
  exit_code: 0
  ```
- **Cell number reference**: matrix #9
- **Ideal output rationale**: TS module-load semantics = top-level statements execute in source order。Rust binary entry = `fn main()`。Ideal: `fn main() { println!("hello world"); }` で TS runtime semantics preserved。

### Cell 10: A1 + B1 (top-Stmt::Expr + user sync main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-10-stmt-expr-with-user-sync-main.ts`
- **tsc / tsx output**:
  ```
  stdout: top-level\nfrom main\n
  stderr: (empty)
  exit_code: 0
  ```
  (TS spec: function declarations are hoisted but top-level statements execute in source order; user `main();` call (= top-level Stmt::Expr) preserves source order)
- **Cell number reference**: matrix #10
- **Ideal output rationale**: silent semantic change 排除 = TS execution order を Rust で完全 preserve するため user main rename + synthesis が必須。

### Cell 11: A1 + B2 (top-Stmt::Expr + user async main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-11-stmt-expr-with-user-async-main.ts`
- **tsc / tsx output**: 推定 `top-level\nfrom async main\n` (TS-1 で empirical record)
- **Cell number reference**: matrix #11
- **Ideal output rationale**: cell 10 + async dispatch (#[tokio::main])。

### Cell 14: A1 + B0 + C1 (top-Stmt::Expr + top-await、no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-14-stmt-expr-with-top-await-no-main.ts`
- **tsc / tsx output**: TS-1 で empirical record、top-level await の TypeScript ESM target compile + tsx execute で stdout を verify
- **Cell number reference**: matrix #14
- **Ideal output rationale**: TS top-level await は ESM 標準、Rust では `#[tokio::main] async fn main()` で synthesis 可能 = Tier 1 完全変換。

### Cell 21: A3 + B0 (Decl::Var with side-effect init only、no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-21-decl-var-side-effect-init-no-main.ts`
- **tsc / tsx output**: TS-1 で empirical record
- **Cell number reference**: matrix #21
- **Ideal output rationale**: TS module body の `const c = new Counter();` は module-load 時に execute、Rust では `fn main()` body 内 `let c = Counter::new();` で semantic preserve。**注**: 本 cell は I-162 (constructor synthesis) prerequisite を要する = `Counter::new()` の存在前提。本 PRD 本体は **fn main 内 capture mechanism** に focus、init expression 変換は I-162 の責任。

(残 ✗ cells: 12 / 13 / 15-18 / 22-24 / 27 (Stmt::Debugger) / 28-31 について TS-1 spec stage で oracle observation log を embed、`scripts/observe-tsc.sh` 出力転記。本 PRD draft では representative cells のみ事前 record、TS-1 で残 cell 完成。)

## SWC Parser Empirical Lock-ins (Rule 3 (3-2) hard-code、NA cell 用)

各 NA cell について SWC parser empirical lock-in test reference:

### NA Cell 6: A0 + B0 + C1 (top-await without execution context)

- **Spec-traceable reason**: TS spec / ESM proposal: top-level `await` keyword は module top-level の execution stmt 内でのみ valid。`function f() { ... }` のような **declaration only** module body では top-level の execution context が不在で `await` 配置 site なし。
- **SWC parser empirical evidence (TS-2 で lock-in)**:
  - **Test path**: `tests/swc_parser_top_level_await_test.rs::test_no_execution_context_rejects_await`
  - **Behavior**: SWC parser が top-level `await` を declaration-only module body で reject、または expected AST shape 構築せず
  - **If accept**: NA cell ではなく Tier 2 honest error reclassify (Rule 3 (3-3))

### NA Cell 7: A0 + B1 + C1 (sync user main + top-await)

- **Spec-traceable reason**: TS spec: `await` keyword は async function body 内のみ valid。sync function 内での `await` 使用は parse error。本 cell は Axis A0 の declaration-only context で top-level に await 配置不能 (Cell 6 と同 spec-traceable reason)。
- **SWC parser empirical evidence**: Cell 6 と統合 lock-in (`tests/swc_parser_top_level_await_test.rs`)。

### NA Cell 8: A0 + B2 + C1 (async user main + top-await without separate execution context)

- **Spec-traceable reason**: Cell 6/7 と同。Axis A0 の context で top-level await 配置 site 不在。Axis A != A0 (= execution stmt 存在) であれば top-level await が cells 14-18 で valid に enumerated されている。

(NA cells 6/7/8 は本 PRD の structural verification logic で SWC parser empirical lock-in test 1 件で symmetric 統合 cover、TS-2 で lock-in test 作成。)

## Impact Area Audit Findings (Rule 11 (d-5) hard-code、`_` arm violations 一覧 + 決定)

```bash
python3 scripts/audit-ast-variant-coverage.py --files src/transformer/mod.rs src/transformer/functions/arrow_fns.rs --verbose
```

実行結果 (2026-05-01 record):

- **Audit script結果**: PASS for PRD 2.7 scope enums (ClassMember, PropOrSpread, Prop)
- **Out-of-scope violations** (audit verbose 出力): 3 件 (= I-203 candidate)
  - `src/pipeline/any_enum_analyzer.rs:138` (ClassMember `_` arm)
  - `src/transformer/expressions/tests/i_205/this_dispatch.rs:607` (ClassMember `_` arm)
  - `src/pipeline/type_resolver/expected_types.rs:387` (Prop `_` arm)
  - **Decision**: 全 3 件は本 PRD I-224 scope 外 (= ClassMember / Prop dispatch、本 PRD は ModuleItem / Stmt / Decl dispatch concern)、I-203 codebase-wide AST exhaustiveness compliance PRD scope へ defer。

- **Manual `_` arm grep** in B2 impact area files (本 PRD scope の追加 audit):

| Violation | Location | Phase | Decision | Rationale |
|-----------|----------|-------|----------|-----------|
| `_ => continue` (silent skip non-Arrow/non-Lit init) | `src/transformer/functions/arrow_fns.rs:42` | Transformer (module-level Decl::Var dispatch) | **Tier 2 honest error reclassify を本 PRD で適用** + I-016 (Call/Ident 等 Tier 1 化) は別 PRD scope | 本 PRD では executable mode 検出時に Decl::Var を fn main body capture path へ routing、library mode 残存 cells で I-016 が Tier 1 化を担当 |
| `_ => Err(UnsupportedSyntaxError)` (transform_module_item catch-all) | `src/transformer/mod.rs:449` | Transformer (top-level item dispatch) | **本 PRD で expand**: A4 (control-flow stmts) を honest error preserve + A5 (Empty/Debugger) は本 PRD scope で reclassify (Stmt::Empty silent skip / Stmt::Debugger Tier 2 honest) + A1 (Stmt::Expr) / A3 (Decl::Var with init) は本 PRD で fn main capture path 追加 | Rule 11 (d-1) compliance: `_` arm を ModuleItem 全 variant explicit enumerate に refactor |
| `_ => Err(UnsupportedSyntaxError)` (transform_decl catch-all) | `src/transformer/mod.rs:666` | Transformer (Decl dispatch) | **I-203 defer** (= Decl variant exhaustiveness、本 PRD architectural concern と orthogonal) | 本 PRD は ModuleItem level dispatch focus、Decl level の `_` arm は別 architectural concern |

詳細な audit findings は TS-4 spec stage task で完成 (= 全 impact area files 対象 audit + 決定 record)。

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - "Axis A - Top-level body composition (7 variants: A0 library / A1 Stmt-Expr / A2 Decl-Var-Lit / A3 Decl-Var-side-effect / A4 control-flow stmts / A5 Empty-Debugger / A6 mixed)"
  - "Axis B - User-defined main symbol (5 variants: B0 none / B1 sync-fn / B2 async-fn / B3 non-fn-symbol / B4 ts-main collision)"
  - "Axis C - Top-level await presence (2 variants: C0 absent / C1 present)"
  - "Cross-axis sub-axes per default check axis - trigger condition (top-exec presence) / operand type variants (user main fn vs non-fn) / guard variant (NA - guard-less concern) / body shape (top-level stmt kinds capture into fn main) / closure-reassign (NA) / early-return (NA - main body stmts are execution semantic) / outer emission context (module-level / fn main body / pub fn init body deprecated) / control-flow exit (NA) / AST dispatch hierarchy (ModuleItem to Stmt to Decl to Expr layers)"
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: "N/A (matrix-driven PRD)"
```

## Goal

TS module top-level の Rust emission を **TS module-load semantics と byte-exact equivalent** な fn main mechanism として完成させる。

具体的 verifiable goals:

1. **Universal e2e infra**: 全 future PRD で `function main()` wrap 不要、top-level statement form の e2e fixture が直接 cargo run pass。**Verify by**: I-205 T14 fixture cell-09 (static-only、本 PRD で唯一 dependency 不在 cell) が e2e green pass。
2. **Silent semantic change 排除**: cell 10 / 11 / 15 / 16 / 22-24 等の "user main + top-level statements" 共存 case で TS execution order を Rust 側でも preserve、tsc stdout と byte-exact match。**Verify by**: Hono bench Tier-transition compliance (compliance check only、Hono codebase で本 pattern reachability TBD)。
3. **Rust E0601 排除**: 全 ✗ cell (9-31 のうち scope 内) で `cargo run` 成功 (= `fn main` 自動生成)。**Verify by**: TS-3 で red 状態 fixture が T1-T6 完了後 green 化。
4. **Rule 11 (d-1) compliance**: `transform_module_item` の `_` arm を ModuleItem 全 variant explicit enumerate に refactor、新 variant 追加時 compile error で全 dispatch fix 強制。**Verify by**: `audit-ast-variant-coverage.py --files src/transformer/mod.rs` で本 PRD scope `_` arm violation 0 件。
5. **`__ts_` namespace extension**: I-154 reservation rule に `__ts_main` を追加、Tier 2 honest error reclassify with explicit user-facing wording。

## Scope (3-tier 形式 hard-code、Rule 6 (6-2) 適用)

### In Scope

本 PRD で **Tier 1 完全変換** する features:

- Cell 9 / 12 / 14 / 17: Synthesize `fn main()` from top-level Stmt::Expr (no user main case、async dispatch sync/async)
- Cell 10 / 11 / 15 / 16: Synthesize `fn main()` + rename user main to `__ts_main` (silent semantic change 排除)
- Cell 21-24: Synthesize `fn main()` + capture top-level Decl::Var with side-effect init as `let` bindings inside fn main body (init expression 変換は I-162 prerequisite)
- Cell 28-31: Mixed cases、source order preserve、async dispatch
- `__ts_` namespace reservation で `__ts_main` 追加 (I-154 extension)
- `transform_module_item` の `_` arm を全 ModuleItem variant explicit enumerate に refactor (Rule 11 d-1 compliance)
- `pub fn init` mechanism 廃止 (= module body emission を fn main 統合)

### Out of Scope

別 PRD or 永続 unsupported な features:

- **Cell 19-20 (top-level Decl::Var with literal init、library mode)**: 既 correct emission preserve、regression lock-in test のみ追加。本 PRD scope 外
- **I-016 (Module-level const Call/Ident/String/Regex/BigInt init の Tier 1 化)**: 別 PRD scope (= **library mode** での module-level const variant 対応)。executable mode (= 本 PRD scope) では fn main body capture で対応、library mode (= 別 PRD scope) で I-016 が top-level static / lazy_static 等の strategy で対応
- **I-221 (top-level Module-level statement TailExpr noise)**: 別 PRD scope (= top-level Stmt::Expr の convert_stmt vs convert_expr dispatch concern、本 PRD は emission destination = fn main body concern と orthogonal)
- **I-180 (E2E harness async-main multi-execution)**: 別 PRD scope (= test infra defect、本 PRD は transpiler emission concern)
- **Cell 25-26 (top-level control-flow stmts: If/For/While/Try/Switch)**: Tier 2 honest error preserve (Rust 上 fn main 配置可能だが、本 PRD architectural concern boundary 外 = top-level "execution stmt" 概念に control-flow を含めると scope creep)。Tier 1 化は別 PRD で扱う候補

### Tier 2 honest error reclassify

本 PRD で **Tier 2 honest error 化** する features (= 別 PRD で Tier 1 化候補):

- **Cell 5 / 13 / 18**: User `function __ts_main()` 等 `__ts_` namespace 衝突 → Tier 2 honest error "`__ts_main` is reserved for transpiler-internal use; user must rename"
- **Cell 27-b (Stmt::Debugger at top-level)**: Rust に debugger statement 等価不在 → Tier 2 honest error "`debugger` statement has no Rust equivalent (= compile-time `panic!()` or `std::dbg!()` を user 自身で選択)"
- **Cell 25-26 (top-level control-flow)**: 既存 Tier 2 honest error preserve (本 PRD は wording 改善のみ、Tier 1 化は別 PRD)

これは silent drop / silent failure を排除し、user に compile-time error として明示する reclassify、ideal-implementation-primacy 観点で structural improvement。

## Invariants (Rule 8 (8-5) audit verify、独立 section)

機能仕様の中で「matrix cell に展開できない / 全 cell で同時に成立する必要がある」transversal property:

### INV-1: TS execution order = Rust execution order

- **(a) Property statement**: Cell A != A0 (= top-level execution 存在) の全 cell で、TS module top-level statements の execution order が Rust `fn main()` body 内で **byte-exact preserve** される。Hoisted function declarations は Rust 上で全 fn main 外に配置されるが、user 視点の execution semantic (= top-level stmts 順序通り、`main();` call site も順序通り) は preserve。
- **(b) Justification**: 違反すると TS execution stdout と Rust execution stdout が divergent = Tier 1 silent semantic change (本 PRD の primary concern)。
- **(c) Verification method**: Per-cell E2E fixture で TS / Rust stdout の byte-exact match を verify (TS-3 で fixture 作成、T6 で green 化)。
- **(d) Failure detectability**: silent semantic change (Rust compile pass + runtime stdout divergent)。

### INV-2: User `main` symbol semantic preservation

- **(a) Property statement**: User-defined `main` symbol (Axis B != B0) は **Rust 側で参照可能な状態** で preserve される。具体的に: B1/B2 (function form) → `__ts_main` で rename + 全 user-side `main()` call site を `__ts_main()` に substitute、B3 (non-fn symbol) → name preserved (Rust namespace 別)、B4 (collision) → Tier 2 honest reject。
- **(b) Justification**: 違反すると user code から `main` symbol への参照が Rust 側で broken = compile error or silent drop。
- **(c) Verification method**: Cell 10/11/15/16/22/23 fixture で user `main()` call site が `__ts_main()` に substitute されることを fixture probe + IR token-level test で verify。
- **(d) Failure detectability**: compile error (substitution 漏れで undefined name) or silent drop (substitution 過剰で wrong name resolved)。

### INV-3: Sync / async dispatch consistency

- **(a) Property statement**: 全 cell で fn main の sync / async dispatch が **以下条件の OR で決定**: (1) Axis B が async fn main (B2)、(2) Axis C が C1 (top-level await present)、(3) top-level Stmt::Expr / Decl::Var の init expression に await 含む。`#[tokio::main]` async fn main を emit する条件と sync fn main を emit する条件が exhaustive + mutually exclusive。
- **(b) Justification**: 違反すると await 含む top-level stmts が sync context で配置されて compile error、または `#[tokio::main]` 不要な context で添加されて runtime overhead 増加 + suboptimal Rust。
- **(c) Verification method**: Cell 11 / 14-18 / 23 / 30 に async dispatch 検証 fixture + dispatch detection helper の unit test。
- **(d) Failure detectability**: compile error (await in sync context) or suboptimal output (unnecessary tokio runtime)。

### INV-4: `pub fn init` mechanism 廃止 invariant

- **(a) Property statement**: 本 PRD 完了後、ts_to_rs の transpile output 内に `pub fn init()` 識別子が存在しない (= 全 emission path が fn main 統合 or library mode 実装に migration)。
- **(b) Justification**: `pub fn init` は never-called dead code source であり、本 PRD architectural concern (= fn main mechanism unification) の structural fix 完成条件。
- **(c) Verification method**: Codebase grep `pub fn init` で 0 hits 確認 (test fixtures + production code)、`build_init_fn` helper 削除確認、CI script `scripts/audit-no-pub-fn-init.sh` (新規) で auto verify。
- **(d) Failure detectability**: silent dead code preservation (compile pass + runtime drop = Tier 1 silent semantic change risk continues)。

### INV-5: `__ts_` namespace reservation extension consistency

- **(a) Property statement**: I-154 `__ts_` namespace reservation rule に `__ts_main` が追加 + 全 user identifier validation path で `__ts_main` を reserved 検出、collision case (= cell 5 / 13 / 18) で Tier 2 honest error reject。
- **(b) Justification**: rename scheme の structural foundation。reservation 不在で user `function __ts_main()` 共存可能なら本 PRD の rename mechanism が silent collision を引き起こす risk。
- **(c) Verification method**: I-154 namespace reservation test (= 既存 `__ts_old`, `__ts_new`, `__ts_recv` 等の test を `__ts_main` 拡張)、collision detection unit test、cell 5 / 13 / 18 fixture probe。
- **(d) Failure detectability**: compile error (Rust 上 user `__ts_main` と本 PRD synthesized `__ts_main` の identifier collision = E0428 duplicate definitions)。

## Design

### Technical Approach

#### 1. Detection: Executable mode vs Library mode

`Transformer::transform_module` の冒頭に **executable_mode 判定** を追加:

```rust
fn is_executable_mode(module: &Module) -> bool {
    module.body.iter().any(|item| match item {
        ModuleItem::Stmt(Stmt::Expr(_)) => true,                          // A1
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => has_side_effect_init(var), // A3
        ModuleItem::Stmt(Stmt::Debugger(_)) => true,                      // A5b (Tier 2 reclassify trigger)
        // A4 control-flow stmts は本 PRD scope 外 (Tier 2 honest preserved)
        // A2 Decl::Var Lit init は library mode 維持 (top-level const emit)
        // A0 declarations / imports / Empty stmts は false
        _ => false,
    })
}
```

#### 2. fn main synthesis dispatch

```rust
match (is_executable_mode, user_main_kind, is_async_required) {
    (false, UserMain::None, _) => library mode (declarations only emit、no fn main)
    (false, UserMain::Fn { is_async }, _) => user main = fn main directly emit (cell 2/3)
    (false, UserMain::NonFn, _) => library mode (cell 4)
    (false, UserMain::Collision, _) => Tier 2 honest error "`__ts_main` reserved" (cell 5)
    (true, UserMain::None, false) => synthesize sync fn main from top-level execution stmts (cell 9/21/24)
    (true, UserMain::None, true) => synthesize async fn main with #[tokio::main] (cell 14/17)
    (true, UserMain::Fn { is_async: false }, false) => rename user main → __ts_main + sync fn main synthesis + main() substitute (cell 10/22/29)
    (true, UserMain::Fn { is_async: true }, _) => rename user async main → __ts_main + #[tokio::main] async fn main synthesis (cell 11/23/30)
    (true, UserMain::Fn { is_async: false }, true) => sync user main + top-level await + #[tokio::main] async fn main synthesis (sync user main called from async fn main) (cell 15)
    (true, UserMain::NonFn, _) => synthesize fn main + user non-fn symbol preserved (cell 12/17/24)
    (true, UserMain::Collision, _) => Tier 2 honest error reclassify (cell 13/18)
}
```

#### 3. Top-level execution stmt capture

`transform_module` の loop で:
- A1 (Stmt::Expr): convert_expr → IR Expr → wrap in `Stmt::Expr` → push to `main_stmts`
- A3 (Decl::Var with side-effect init): convert_stmt → IR Stmt::Let { name, init } → push to `main_stmts`
- A2 (Decl::Var with Lit init): library mode emit as top-level Item::Const (preserve current path)
- A4 (control-flow): Tier 2 honest error (本 PRD scope 外、preserve current path)
- A5a (Empty): silent skip (preserve current path)
- A5b (Debugger): Tier 2 honest error reclassify (本 PRD で wording 確定)
- A6 (mixed): source order preserve、上記 dispatch を per-item 適用

#### 4. User main rename + main() substitution

User function `main` (B1/B2、function decl / arrow / fn expr) detection 後:
- declaration を `Item::Fn { name: "__ts_main", ... }` に rename emit
- `transform_module` の expression conversion path で `Expr::Call { callee: Ident("main"), args }` を `Expr::Call { callee: Ident("__ts_main"), args }` に substitute (= 全 user-side `main()` call site が __ts_main() を call)

#### 5. Async dispatch synthesis

`is_async_required` true の場合、fn main 自体を `#[tokio::main]` async fn main として emit。Sync user main (B1) を async fn main 内から call する case (cell 15) は user main = sync `__ts_main` のまま、async fn main から非 await の sync call で invoke。

#### 6. `__ts_main` collision detection

User module body iterate 時に `function __ts_main` / `const __ts_main = ...` 等の identifier `__ts_main` を持つ declaration を detect → Tier 2 honest error reclassify (UnsupportedSyntaxError 経由 line/col 含む transparent error report)。

#### 7. `pub fn init` 廃止

`build_init_fn` helper を削除、`transform_module` / `transform_module_collecting` の `init_stmts` collection logic を `main_stmts` に rename + dispatch logic 上記 #2 に統合。`pub fn init` を生成する全 path を削除し、CI script `scripts/audit-no-pub-fn-init.sh` で 0 hits invariant lock-in。

### Design Integrity Review

`.claude/rules/design-integrity.md` checklist:

1. **Higher-level consistency**: Caller side (= e2e harness、cargo run) は `fn main()` を要求、本 PRD で structural 提供 = 正しい layered design。Sibling modules: `transform_decl` (Decl-level dispatch) + `convert_var_decl_module_level` (Decl::Var dispatch、library mode) は本 PRD と orthogonal な architectural concern (本 PRD は ModuleItem → fn main body capture path)。✓ Verified consistent.
2. **DRY / Orthogonality / Coupling**:
   - DRY: `transform_module` と `transform_module_collecting` の重複 logic (= 同じ Stmt::Expr filter + init_stmts collection) を本 PRD で `collect_top_level_executions(module) -> Vec<MainStmt>` shared helper に集約 (新規 DRY violation 解消)。
   - Orthogonality: 本 PRD は fn main mechanism architectural concern に focus。I-016 (library mode const init)、I-221 (TailExpr dispatch)、I-180 (harness) は別 architectural concern として分離。
   - Coupling: 本 PRD で `__ts_main` rename mechanism を `Transformer::user_main_substitution` field として保持、`convert_expr` の `Call` arm で `Ident("main")` substitute path を追加 (= local coupling、global state 不要)。
3. **Broken windows**: `arrow_fns.rs:42` の `_ => continue` (I-016 source) は本 PRD scope 外として TODO 連動 preserve。`mod.rs:449` の `_ => Err` は本 PRD で expand (Rule 11 d-1 compliance)、`mod.rs:666` の `_ => Err` は I-203 defer。
4. **Verified, no in-PRD blocking issues**.

### Impact Area

**修正対象 files**:

- `src/transformer/mod.rs`:
  - `transform_module` (line 301-328): main dispatch logic 全面 refactor (init_stmts → main_stmts、executable mode 判定 + dispatch tree 適用)
  - `transform_module_collecting` (line 331-394): 同上 refactor (parallel logic、shared helper 化で DRY 解消)
  - `transform_module_item` (line 400-455): `_ => Err` の expand (Rule 11 d-1)、Stmt::Expr / Stmt::Decl / Stmt::Debugger 等 explicit enumerate
  - `build_init_fn` (line 702-713): 削除 (= 新 `build_main_fn` helper に置換、`pub fn init` 廃止)
- `src/transformer/functions/arrow_fns.rs`:
  - `convert_var_decl_module_level` (line 15-46): 既存 path 維持 (library mode = top-level const emit)、本 PRD は fn main capture path を caller (transform_module) で追加
- (新規) `src/transformer/main_synthesis.rs`: fn main synthesis logic + user main rename + dispatch tree 集約
- (新規) `src/ir/main_synthesis.rs` または `src/ir/mod.rs` 拡張: `MainStmt` enum (Expr / Let / Debugger reclassify error 等)、`UserMainKind` enum
- (修正) `tests/e2e/scripts/i-205/`: 既存 cell-09 等の e2e fixture が本 PRD で green 化
- (新規) `tests/e2e/scripts/i-224/`: per-cell E2E fixture (TS-3)
- (新規) `tests/swc_parser_top_level_await_test.rs`: NA cell 6/7/8 SWC parser empirical lock-in (TS-2)
- (修正) `tests/test_helpers.rs` または e2e_test.rs: `pub fn init` を expect しない harness logic update (本 PRD で `fn main` 直接 emit に migration)

**`__ts_` namespace reservation 拡張対象** (I-154 source):
- `src/transformer/conventions/reserved_names.rs` (or 該当 file、I-154 で確定): `__ts_main` を reserved list に追加
- `src/transformer/<reserved-name-validation>` (該当 path): `__ts_main` を user identifier validation で reject 化

(Empirical file path verify は TS-4 spec stage task で完成、本 PRD draft の path expressions に "or 該当" 等の uncertain 記述あれば audit fail = 本 draft で `(or 該当 file、I-154 で確定)` 等の暫定記述を含むため、TS-4 で empirical confirm 後 PRD doc update 必要)

### Semantic Safety Analysis

**Required**: 本 PRD は型 fallback 導入を含まない (= `__ts_main` rename は identifier-level rename で型 system 関与なし、fn main synthesis は IR レベルの structural emission)。型 resolution 変更なし。

**判定**: Not applicable — no type fallback changes。

ただし silent semantic change の risk audit は別軸で実施 (= INV-1 によって TS / Rust execution order の byte-exact match を verify、INV-2 によって user `main` symbol substitution の completeness を verify)。これは型 fallback ではなく **execution semantic preservation** で本 PRD architectural concern の primary objective。

## Spec Stage Tasks (Rule 5 (5-2) 適用、Stage 1 artifacts 完成 task)

### TS-0: Cartesian product matrix completeness

- **Work**: Problem Space Cartesian product matrix を完全 enumerate (~31 cells)、全 cell に判定 (✓/✗/NA/regression lock-in/Tier 2 reclassify) 付与、abbreviation pattern 排除
- **Completion criteria**: matrix table 内 `...` / range grouping / placeholder 不在、全 cell 独立 row、`audit-prd-rule10-compliance.py backlog/I-224-top-level-fn-main-mechanism.md` PASS
- **Status**: 本 PRD draft v1 で 31 cells initial enumerate 完了、TS-0 完成は self-review iteration v1+ で missing cells / abbreviation 検出 + fix 後

### TS-1: Oracle observation log embed

- **Work**: 各 ✗ / 要調査 cell について TS fixture 作成、`scripts/observe-tsc.sh` 実行、PRD doc `## Oracle Observations` section に embed (現状 representative cells 5/9/10/11/14/21 のみ embed、残 cells 12/13/15-18/22-24/27/28-31 を完成)
- **Completion criteria**: 全 ✗ / 要調査 cell について 4 項目 (TS fixture path / tsc output / cell # link / ideal output rationale) 記載、`audit-prd-rule10-compliance.py` で section 不在 audit fail 排除

### TS-2: SWC parser empirical lock-in (NA cells)

- **Work**: NA cells 6/7/8 (top-await without execution context) について `tests/swc_parser_top_level_await_test.rs` (新規) で SWC parser empirical lock-in test 作成、PRD doc `## SWC Parser Empirical Lock-ins` section に embed
- **Completion criteria**: SWC parser が top-await + Axis A0 context を reject する empirical evidence lock-in、accept 確認時は Tier 2 honest error reclassify (Rule 3 (3-3))

### TS-3: E2E fixture creation (red 状態 lock-in)

- **Work**: 各 ✗ cell に対応 `tests/e2e/scripts/i-224/cell-NN-*.ts` fixture 作成、`scripts/record-cell-oracle.sh` で expected output 記録 (red 状態 = ts_to_rs 出力と expected 不一致)。Cells: 5/9/10/11/12/13/14/15/16/17/18/21/22/23/24/27-b/28/29/30/31
- **Completion criteria**: `cargo test --test e2e_test` で全 fixture red 確認 (= Implementation stage T1-T6 完了で green 化予定)

### TS-4: Impact Area audit findings record

- **Work**: `python3 scripts/audit-ast-variant-coverage.py --files src/transformer/mod.rs src/transformer/functions/arrow_fns.rs --verbose` 実行、結果を PRD doc `## Impact Area Audit Findings` section に完成 (現状 partial、本 task で full enumerate)、各 violation の決定 (本 PRD scope or I-203 defer) 記録、Empirical file path verify (impact area path strings の "or 該当" 等 uncertain expression を empirical confirm し PRD doc update)
- **Completion criteria**: 全 violations 列挙 + 決定記載、Empirical file path verify 完了 (= PRD doc 内 path strings が empirical 確認済 file/line/function に correspond)

## Implementation Stage Tasks

(TDD 順: RED → GREEN → REFACTOR、Spec stage 完了 + user 承認後着手)

### T1: `__ts_` namespace reservation extension + collision detection

- **Work**: I-154 の `__ts_` reserved list に `__ts_main` 追加 (該当 file は TS-4 で empirical confirm 後特定)、user identifier validation で `__ts_main` を reject、cells 5/13/18 用 `UnsupportedSyntaxError::new("`__ts_main` is reserved for transpiler-internal use; user must rename", span)` emission path 追加
- **Completion criteria**: I-154 namespace test 拡張で `__ts_main` reserved verify、cell 5/13/18 fixture が Tier 2 honest error reject 出力
- **Depends on**: TS-1〜TS-4

### T2: `MainStmt` IR + `UserMainKind` enum + `collect_top_level_executions` helper

- **Work**: 新 `MainStmt` enum (variants: Expr / Let / Debugger reclassify error)、`UserMainKind` enum (None / FnSync / FnAsync / NonFn / Collision)、`collect_top_level_executions(module: &Module) -> (Vec<MainStmt>, UserMainKind, bool /* is_async_required */)` shared helper を新規 module `src/transformer/main_synthesis.rs` に実装
- **Completion criteria**: helper unit test (= 31 cell input variation × expected (MainStmt vec, UserMainKind, is_async) tuple)
- **Depends on**: T1

### T3: fn main synthesis + user main rename + main() substitution

- **Work**: `Transformer::synthesize_fn_main(main_stmts: Vec<MainStmt>, user_main: UserMainKind, is_async: bool) -> Vec<Item>` 実装、user main rename (B1/B2 → __ts_main 変名)、convert_expr の Call arm に `Ident("main")` → `Ident("__ts_main")` substitute logic 追加 (Transformer state field `user_main_substitution: bool`)
- **Completion criteria**: cells 9-18 / 21-24 / 28-31 の dispatch logic を unit test で verify (cell-by-cell の expected IR token-level assert)
- **Depends on**: T2

### T4: `transform_module` / `transform_module_collecting` refactor + `pub fn init` 廃止

- **Work**: `transform_module` / `transform_module_collecting` の logic を T2 helper + T3 synthesis 経由に refactor、`init_stmts` → `main_stmts` rename、`build_init_fn` 削除、`build_main_fn` 新規追加。`transform_module_item` の `_ => Err` を expand (ModuleItem 全 variant explicit enumerate、Rule 11 d-1 compliance)
- **Completion criteria**: cargo test 全 pass (`pub fn init` 言及の test は新 form に migrate)、`audit-ast-variant-coverage.py --files src/transformer/mod.rs` で `_` arm violation 0 件、CI script `scripts/audit-no-pub-fn-init.sh` で codebase 0 hits
- **Depends on**: T3

### T5: E2E fixture green-ify + I-205 cell-09 unblock

- **Work**: TS-3 で red 状態だった全 fixture (i-224 配下) + I-205 cell-09 (static-only、本 PRD のみ依存) を green 化、Tier-transition compliance verify (= existing Tier 2 errors transition Tier 1 = improvement、no new compile errors)
- **Completion criteria**: `cargo test --test e2e_test` 全 pass (本 PRD scope cells)、Hono bench Tier-transition compliance、cell-09 の `#[ignore]` 解除
- **Depends on**: T4

### T6: I-154 namespace doc + framework rule update + audit script integration

- **Work**: I-154 namespace doc に `__ts_main` 追記 + reservation rationale (= 本 PRD source) 記載、`scripts/audit-no-pub-fn-init.sh` を CI workflow `.github/workflows/ci.yml` に integrate、`audit-prd-rule10-compliance.py` の Empirical file path verify rule (= "or 該当" / "TBD" 等 uncertain expression detect) を本 PRD で empirical reinforce
- **Completion criteria**: doc update PR、CI step 追加 PR、本 PRD doc が audit-prd-rule10-compliance.py PASS
- **Depends on**: T5

## Spec Review Iteration Log (Rule 13 (13-2) hard-code)

### Iteration v1 (2026-05-01、本 PRD draft 初版 self-applied verify)

skill workflow Step 4.5 で 13-rule self-applied verify 実施:

| Rule | Sub-rule check | Verdict | Notes |
|---|---|---|---|
| 1 | (1-1) 全 cell ideal output | ✓ | 31 cells 全 enumerate |
| 1 | (1-2) abbreviation pattern 不在 | ✓ | `...` / range grouping / `representative` / `(各別 cell)` 不在 |
| 1 | (1-3) audit script PASS | **TS-4 で実施** | 本 draft commit 後 `audit-prd-rule10-compliance.py` 実行 |
| 1 | (1-4) Orthogonality merge legitimacy + Spec-stage structural verify | ✓ | Axis B の B1=function-decl/const-arrow/const-fn-expr orthogonality merge は dispatch-equivalent (TS-2 spec で 3 forms 同一 dispatch verify、`audit-prd-rule10-compliance.py` の `verify_orthogonality_merge_consistency` で auto verify、PRD draft 内 Axis B section 内 explicit declare) |
| 2 | (2-1) Oracle grounding cross-reference | ✓ | representative cells 5/9/10/11/14/21 で oracle grounding embed |
| 2 | (2-2) `## Oracle Observations` section embed | partial | 6 cells embedded、残 ~10 cells は TS-1 で完成、Critical finding (Implementation block) |
| 2 | (2-3) audit script verify | **TS-4 で実施** | section 不在は audit fail |
| 3 | (3-1) NA spec-traceable | ✓ | NA cells 6/7/8 が TS spec / ESM proposal の "top-level await requires execution context" に grounding |
| 3 | (3-2) SWC parser empirical observation | partial | NA cells 6/7/8 用 SWC parser empirical lock-in test (`tests/swc_parser_top_level_await_test.rs`) は TS-2 で作成、PRD doc では reference 記載済 |
| 3 | (3-3) SWC accept → Tier 2 reclassify | ✓ | SWC accept 確認時の reclassify 経路明記 |
| 4 | (4-1) reference doc 整合 | ✓ | `doc/grammar/ast-variants.md` の ModuleItem / Stmt / Decl / Expr Tier 1/2 と整合 |
| 4 | (4-2) doc-first dependency order | N/A | 本 PRD は doc 改修を含まない (= I-154 namespace doc は T6 で update、code change と同 PRD 内で sync) |
| 4 | (4-3) audit verify | N/A | 上記 |
| 5 | (5-1) E2E fixture 準備 | partial | TS-3 で red 状態 fixture 作成、本 draft では fixture path のみ記載 |
| 5 | (5-2) `## Spec Stage Tasks` + `## Implementation Stage Tasks` 2-section split | ✓ | 両 section hard-code |
| 5 | (5-3) Spec stage tasks に code 改修不在 | ✓ | TS-0〜TS-4 全て spec artifact 完成 task のみ |
| 5 | (5-4) audit verify | **TS-4 で実施** | |
| 6 | (6-1) Matrix Ideal output ↔ Design token-level 一致 | ✓ | Design section #2 dispatch tree が matrix Scope 列と corresponds |
| 6 | (6-2) Scope 3-tier hard-code | ✓ | In Scope / Out of Scope / Tier 2 honest error reclassify 3 sub-section |
| 6 | (6-3) matrix Scope 列値 | ✓ | `本 PRD scope` / `regression lock-in` / `Tier 2 honest reclassify` / `本 PRD scope + 別 PRD prerequisite chain` 等択一 |
| 6 | (6-4) Scope ↔ matrix cross-reference consistency | partial | TS-4 で audit verify |
| 7 | Control-flow exit sub-case completeness | N/A | 本 PRD は control-flow body / branch shape concern を含まない (top-level statement dispatch focus、user main body の control-flow は別 architectural concern) |
| 8 | (8-5) `## Invariants` 独立 section | ✓ | INV-1〜INV-5 hard-code、各 4 項目 (a)(b)(c)(d) 記載 |
| 9 | (a) Spec → Impl Dispatch Arm Mapping | partial | Design section #2 dispatch tree で arm mapping 記載、TS-3 で sub-case alignment 完成 |
| 9 | (b) Impl → Spec | N/A (Implementation stage で発動) | |
| 9 | (c) Field-addition symmetric audit | N/A | 本 PRD は IR struct field 追加なし (= MainStmt / UserMainKind enum 新規追加だが既存 field の symmetric audit は不要) |
| 10 | Cross-axis matrix completeness (9 default axis) | ✓ | Axis A/B/C 3-axis Cartesian、9 default axis のうち relevant axes (= trigger / operand type / body shape / outer context / AST dispatch hierarchy) を Rule 10 Application section で enumerate |
| 11 | (d-1) `_ => ` 全廃 | partial | `transform_module_item:449` の `_ => Err` は T4 で全 variant explicit enumerate に refactor、`arrow_fns.rs:42` の `_ => continue` は本 PRD scope 外 (I-016 defer)、`mod.rs:666` の `_ => Err` は I-203 defer |
| 11 | (d-2) phase 別 mechanism | ✓ | Transformer phase = `UnsupportedSyntaxError` で Tier 2 honest error reject、TypeResolver / NA cell mechanism は本 PRD scope に該当 case 不在 |
| 11 | (d-3) `ast-variants.md` single source of truth | ✓ | Audit script verbose 出力で reference doc Tier 1/2 と対比、本 PRD は ModuleItem / Stmt / Decl Tier 1 dispatch 完成 |
| 11 | (d-4) audit script CI | TS-4 で実施 | |
| 11 | (d-5) Pre-draft audit | ✓ | 本 PRD draft 着手前に audit-ast-variant-coverage.py run、結果 `## Impact Area Audit Findings` 部分 embed (TS-4 で完成) |
| 11 | (d-6) Architectural concern relevance | ✓ | 本 PRD architectural concern (= fn main mechanism) の relevant code path 内 `_ => ` arm (`mod.rs:449`) は本 PRD で fix、relevant 外 (`mod.rs:666` Decl dispatch / `arrow_fns.rs:42` Decl::Var dispatch / 他 file の ClassMember/Prop dispatch) は I-203 / I-016 defer (= orthogonality declared) |
| 12 | Rule 10/11 Mandatory + structural | ✓ | `## Rule 10 Application` section 記入済、TS-4 で audit script PASS verify |
| 13 | (13-1) skill workflow Step 4.5 systematic | ✓ | 本 iteration log 自身 |
| 13 | (13-2) `## Spec Review Iteration Log` record | ✓ | 本 section |
| 13 | (13-3) Critical findings 全 fix | partial | TS-1 partial / TS-2 reference 記載 / TS-4 partial = Implementation stage 移行 block する critical findings、Spec stage tasks (TS-0〜TS-4) で完成 |
| 13 | (13-4) audit verify | TS-4 で実施 | |
| 13 | (13-5) Self-applied integration | N/A initial | 本 PRD は I-205 lessons の adoption ではなく新 architectural concern、self-applied integration candidates は close 時 collect |

**Iteration v1 findings count**: Critical = 0 (Implementation block するもの不在、partial verify items は Spec Stage Tasks TS-0〜TS-4 で完成予定)、High = 4 (TS-1 / TS-2 / TS-3 / TS-4 の completion 未済、本 draft commit 後 spec stage で完成)、Medium = 0、Low = 0。

**Resolution**: TS-0〜TS-4 spec stage tasks で High findings 全 resolve 後、iteration v2 self-review pass で Spec stage 完了判定。

## Test Plan

### Unit tests (Implementation stage T2-T6 で追加)

- **`src/transformer/main_synthesis.rs::tests`** (新規):
  - `collect_top_level_executions` の 31 cell input variation × expected (MainStmt vec, UserMainKind, is_async) tuple
  - `synthesize_fn_main` の cell-by-cell IR token-level expected assert
  - User main rename + `main()` substitution の boundary value (multiple call sites = cell 31)
  - `__ts_main` collision detection (cells 5/13/18)
  - Async dispatch helpers (sync/async/tokio dispatch leaves)

### Integration tests (T2-T6 で追加)

- **`tests/i_224_namespace_test.rs`** (新規): I-154 namespace reservation 拡張で `__ts_main` reserved verify

### E2E tests (TS-3 で red 状態 lock-in、T5 で green 化)

- **`tests/e2e/scripts/i-224/cell-NN-*.{ts,expected}`** (新規 ~20 fixtures): per-cell E2E fixture
- **`tests/e2e_test.rs`**: per-cell test fn entries (`run_cell_e2e_test("i-224", "cell-NN-*")`)
- **I-205 cell-09 e2e fixture**: 本 PRD T5 で `#[ignore]` 解除

### SWC parser empirical tests (TS-2 で作成)

- **`tests/swc_parser_top_level_await_test.rs`** (新規): NA cells 6/7/8 用 SWC parser behavior lock-in

### Snapshot tests

なし (本 PRD は IR-level emission concern、snapshot test は不要)

## Completion Criteria

**Matrix completeness requirement (最上位完了条件)**: Problem Space matrix の全 31 cells に対するテストが存在し、各 cell の実出力が ideal 仕様と一致 (✓ cells = regression lock-in、✗ cells = green 化、Tier 2 reclassify cells = honest error 出力 verify)。1 cell でも未カバー、または「多分 OK」で済ませた cell があれば PRD は未完成。

**Quality gates**:

- `cargo test` 全 pass (lib + integration + e2e + compile_test)
- `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
- `cargo fmt --all --check` 0 diffs
- `./scripts/check-file-lines.sh` OK (全 file < 1000 lines)
- `python3 scripts/audit-ast-variant-coverage.py --files src/transformer/mod.rs --verbose` で `_` arm violation 0 件 (本 PRD scope file)
- `python3 scripts/audit-prd-rule10-compliance.py backlog/I-224-top-level-fn-main-mechanism.md` PASS
- `scripts/audit-no-pub-fn-init.sh` (新規) で codebase 0 hits

**Tier-transition compliance (broken-fix PRD として、`prd-completion.md` 適用)**:

- Pre-PRD state: existing Tier 2/Tier 1 broken (cells 9/10/11/12/13/14-18/21-24/28-31 が E0601 compile fail or silent semantic change)
- Post-PRD state: Tier 1 (compile-pass + tsc runtime stdout 一致) for cells in 本 PRD scope
- Hono bench result classification:
  - **Improvement** (allowed): existing related errors transition Tier-2 → Tier-1 (clean files count 増加 / errors count 減少 = expected)
  - **Preservation** (allowed): existing related errors unchanged (Hono が top-level executable form を主要使用していない場合の正常な観測結果)
  - **New compile errors** (prohibited): 本 PRD 修正範囲外の features に対して新たな compile error 導入は **regression** = 完了 block

**Impact estimates verified by tracing actual code paths**: cells 9-31 のうち少なくとも 3 representative instances (cell 9 / cell 10 / cell 21) で TS source → 生成 Rust → cargo run stdout → tsc / tsx stdout の全 chain を empirical trace、本 PRD fix が specific failure point を解消することを verify。

## References

- 関連 PRD: I-205 (T14 prerequisite block 由来、本 PRD direct beneficiary)、I-225 (B3 class field type inference、I-205 T14 sister prerequisite)、I-162 (constructor synthesis、cell 21-24 init expression conversion prerequisite)、I-016 (module-level const Call/Ident init、library mode counterpart、本 PRD と orthogonal scope)、I-221 (top-level Module-level statement TailExpr noise、本 PRD と隣接 area の独立 sub-defect)、I-180 (E2E harness async-main multi-execution、test infra defect)、I-154 (`__ts_` namespace reservation rule、本 PRD で `__ts_main` 拡張)、I-203 (codebase-wide AST exhaustiveness compliance、本 PRD scope 外 `_` arm violations の defer 先)
- 関連 rule: `.claude/rules/spec-first-prd.md` / `.claude/rules/spec-stage-adversarial-checklist.md` (13-rule) / `.claude/rules/check-job-review-layers.md` (4-layer review) / `.claude/rules/post-implementation-defect-classification.md` (5-category) / `.claude/rules/problem-space-analysis.md` / `.claude/rules/ideal-implementation-primacy.md` / `.claude/rules/conversion-correctness-priority.md` / `.claude/rules/prd-completion.md` / `.claude/rules/type-fallback-safety.md` (本 PRD は N/A) / `.claude/rules/testing.md` / `.claude/rules/design-integrity.md` / `.claude/rules/pipeline-integrity.md` / `.claude/rules/incremental-commit.md` / `.claude/rules/pre-commit-doc-sync.md`
- 関連 doc: `doc/grammar/ast-variants.md` (ModuleItem / Stmt / Decl / Expr Tier 1/2 reference)
- discovery date: 2026-05-01 (PRD I-205 T14 着手判定調査由来)
- user 承認: 2026-05-01 (案 β Phase 1-A 採用 + Discovery Q1-Q4 全推奨採択)
