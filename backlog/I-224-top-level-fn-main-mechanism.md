# I-224: Top-level executable script の Rust emission に `fn main()` 自動生成 (TS module-load semantics → Rust fn main mechanism、test harness ESM upgrade cohesive batch)

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

**Option β cohesive batch (iteration v3、2026-05-01 user 確定)**: 旧 iteration v2 で cells 14-18/30 (top-level await ✗) + cells 6/7/8 (top-level await NA) を「test harness limitation」を理由に新 PRD I-226 へ defer する設計だったが、第三者 `/check_job` review (H-2) で `Rule 12 (e-3)` Permitted reasons に該当しない gray zone violation と判定、ideal-implementation-primacy 観点で「実装範囲が広い (test harness 跨ぎ)」を defer 理由とする compromise を排除。本 PRD scope を **cohesive batch** (= "Top-level executable script form の Rust emission strategy + verify infrastructure") に拡張、test harness ESM upgrade (= `scripts/observe-tsc.sh --esm` flag、`tests/e2e/rust-runner/` の tokio runtime 依存追加、top-level await capture into `#[tokio::main] async fn main()`) を本 PRD scope に integrate。1 PRD = 1 architectural concern boundary は「top-level executable form の完全 verify」として再定義され、I-226 起票は撤回。詳細は `report/I-224-spec-stage-v3-review-handoff.md` Section 4.2 + Spec Review Iteration Log v3 entry 参照。

## Problem Space

`.claude/rules/problem-space-analysis.md` に従い、TS module top-level の Rust emission 戦略を完全 enumerate する。

### 入力次元 (Dimensions)

機能の出力を決定する独立次元:

- **Axis A (Top-level body composition)**: **8 variants** (iteration v3、A5 split per third-party review C-4):
  - A0: 何もない (empty) / 宣言のみ (function / class / type / interface / enum decl) / imports のみ (= **library mode**、top-level 実行 stmt 不在)
  - A1: top-level Stmt::Expr のみ (例: `console.log(...)`、関数 call、await expression statement、等)
  - A2: top-level Decl::Var with **literal init only** (例: `const x = 0;`、現状 `const x: f64 = 0.0;` 形式で top-level emit、execute は不要)
  - A3: top-level Decl::Var with side-effect / non-const init (例: `const c = new Counter();` / `const v = await fetch();`)
  - A4: top-level Stmt::If / For / ForIn / ForOf / While / DoWhile / Try / Switch / Throw / Labeled / Block (control-flow at top-level)
  - **A5a**: top-level Stmt::Empty (semicolon-only no-op、source order に影響しない、runtime stdout に影響なし)
  - **A5b**: top-level Stmt::Debugger (`debugger;` statement、Rust に直接対応なし)
  - A6: 上記 A1-A5 の混在 (= 「のみ」制約を満たさない top-level body)

- **Axis B (User-defined `main` symbol)**: 5 variants:
  - B0: 不在 (no user `main`)
  - B1: sync function `main` (function decl / const arrow / const fn expr 統合 — orthogonality merge legitimate per Rule 1 (1-4)、structural verify は本 section 後 "Axis B B1 Orthogonality Verification" sub-section 参照)
  - B2: async function `main` (function decl / const arrow / const fn expr 統合)
  - B3: 非 fn symbol (user `main` を type / interface / class / enum / namespace / let-mutable / variable で定義 = Rust 上 fn と別 namespace のため衝突なし)
  - B4: `__ts_main` 衝突 (user が `function __ts_main()` 等を定義 = 本 PRD の reserved 名前空間と衝突 = Tier 2 honest error reject)

- **Axis C (Top-level `await` 使用)**: 2 variants:
  - C0: 不在
  - C1: 存在 (TS / ESM proposal feature = `const x = await fetch(...);` / `await Promise.resolve();` 等 module body top-level での `await` keyword 使用)

- **Axis E (Module export presence、orthogonality merge declaration、third-party review M-2)**: 2 variants:
  - E0: export 不在 (= `module` の表面に export keyword なし)
  - E1: export 存在 (= `export function f()`, `export const X = ...`, `export {}`, `export default ...`)

  **Orthogonality merge declaration**: Axis E は本 PRD architectural concern (= fn main mechanism + executable mode dispatch) の dispatch logic に **直接影響しない**。Rust binary crate (= fn main 自動生成 mode) では Rust top-level item の `pub` keyword 有無は library 公開度の concern であり、module-load semantics + execution order の concern とは orthogonal。E0/E1 cells は同一 dispatch logic (= main_stmts 収集 + user main rename + fn main synthesis) を通過、E1 cells では既存 path で生成される `pub` modifier を preserve (= regression lock-in)。**Rule 1 (1-4) compliant orthogonality merge: matrix 内では Axis E sub-axis 化せず、本 sub-section で structural verify**。
  - **(1-4-a) Orthogonality verification statement**: Axis E E1 cells は E0 cells と orthogonality-equivalent dispatch (= Rust binary crate 内 `pub` modifier preserve は emission strategy / execution order 不変)、source cell # = E0 with same (A, B, C)。
  - **(1-4-b) Spec-stage structural consistency verify**: Axis E E1 形式は SWC parser で `Decl::Fn` / `Decl::Var` 等の AST shape が E0 と identical (export keyword は ModuleDecl::ExportDecl wrapper として外側 layer に reflect、内部 Decl shape 保存)。本 sub-section 後 "Axis E Orthogonality Probe" で structural verify。
  - **(1-4-c) Spec-stage referenced cell symmetry probe**: 各 reachable (A, B, C) について E0 と E1 が dispatch-symmetric を実装側 unit test で lock-in (Implementation Stage T3 で test_axis_e_export_preserve_symmetric で probe)。

**Cartesian**: 8 (A) × 5 (B) × 2 (C) × 2 (E) = **160 cells** が full Cartesian product、ただし Axis E orthogonality merge 適用後 **80 cells** が matrix table の独立 row enumerate target (Axis E は本 PRD scope で "structural orthogonality lock-in" として処理、各 cell に E0/E1 共通 ideal output を spec)。

### Axis A vs Axis C 構造的 mutual exclusion (NA cells 25 件、Rule 3 (3-1/3-2) 適用)

Axis C C1 (top-level await) は AST shape として **Stmt::Expr (Expr::Await)** または **Decl::Var (with init = Expr::Await)** を要求する。これは Axis A1 (top-level Stmt::Expr) または Axis A3 (Decl::Var with side-effect/non-const init) の partition に該当し、以下 5 partition と AST 構造的 mutually exclusive:

- A0 + C1 (cells 2, 4, 6, 8, 10): A0 = 「実行 stmt 不在」、C1 = 「await stmt 存在」 → AST 構造的 mutual exclusion
- A2 + C1 (cells 22, 24, 26, 28, 30): A2 = 「Decl::Var with **literal** init only」、C1 = 「await init は non-literal」 → AST 構造的 mutual exclusion
- A4 + C1 (cells 42, 44, 46, 48, 50): A4 = 「control-flow stmts のみ」、C1 = 「await は Stmt::Expr / Decl::Var」 → 「のみ」制約違反、A6 (mixed) に分類 = A4 + C1 partition は空集合
- A5a + C1 (cells 52, 54, 56, 58, 60): A5a = 「Empty stmts のみ」、C1 = 同上 → A6 に分類 = NA
- A5b + C1 (cells 62, 64, 66, 68, 70): A5b = 「Debugger stmts のみ」、C1 = 同上 → A6 に分類 = NA

empirical SWC parser lock-in test (本 PRD scope、Rule 3 (3-2) compliance): `tests/swc_parser_top_level_await_test.rs` の 4 tests で structural reasoning を verify (4 tests passing 2026-05-01):
- `test_top_level_bare_await_parses_as_stmt_expr_await_axis_a1`: `await x;` → `Stmt::Expr(Expr::Await)` で A1 partition
- `test_top_level_var_decl_with_await_init_parses_as_decl_var_axis_a3`: `const x = await y;` → `Decl::Var` with `Expr::Await` init で A3 partition
- `test_pure_axis_a0_source_contains_no_await_expression`: pure A0 source は `Expr::Await` を top-level に含まない
- `test_axis_c1_implies_a1_or_a3_partition_synthesis`: C1 forms (4 variations) は A1/A3 に partition される

### 組合せマトリクス (80 cells 独立 row enumerate、Rule 1 (1-2) abbreviation prohibition compliant)

Cell # は Axis A × Axis B × Axis C lexicographic order で sequential numbering (1-80)。Axis E は本 matrix 内 sub-axis 化せず、各 cell ideal output が E0/E1 共通 (orthogonality merge declaration、上記参照)。

**Fixture file mapping**: 既存 fixture file 名 (cell-NN-*.ts、旧 numbering 1-31) は file rename を回避するため keep。本 matrix の "fixture" 列で新 cell # → 既存 fixture file path への mapping を提供。新規追加 fixture は既存 numbering を続けて assign (cell-32+ 等) または partition-equivalent 既存 fixture へ orthogonality merge で参照。

| # | A (top-exec) | B (user main) | C (top-await) | Ideal Rust output | 現状 | 判定 | Scope | Fixture |
|---|---|---|---|---|---|---|---|---|
| 1 | A0 (empty / library) | B0 (no main) | C0 | declarations only emit、no `fn main`、no `pub fn init` | declarations only emit (no fn main、no init = correct) | ✓ | regression lock-in | cell-01-empty-library |
| 2 | A0 | B0 | C1 | NA per Axis A0 vs C1 mutual exclusion (= C1 implies A1 or A3 partition、本 cell の input shape は parse 不能) | NA | NA | NA — covered by `test_pure_axis_a0_source_contains_no_await_expression` | (no fixture, SWC parser test) |
| 3 | A0 (declarations only) | B1 (sync) | C0 | `fn main() { <user main body> }` (user main 直接 emit) | `fn main() { <user main body> }` (現状 observe-tsc.sh auto-append convention で stdout=`user main: 7\n` を expected として record、direct tsx fidelity では stdout=(empty)) | ✓ regression lock-in (auto-append convention; documented as convention divergence) | regression lock-in | cell-02-user-sync-main-only |
| 4 | A0 | B1 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 5 | A0 | B2 (async) | C0 | `#[tokio::main] async fn main() { <user async main body> }` | `#[tokio::main] async fn main() { <body> }` (correct, auto-append convention) | ✓ regression lock-in | regression lock-in | cell-03-user-async-main-only |
| 6 | A0 | B2 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 7 | A0 | B3 (non-fn symbol) | C0 | declarations only emit (Rust fn と別 namespace、衝突なし) | declarations only emit (correct) | ✓ | regression lock-in | cell-04-non-fn-main-no-exec |
| 8 | A0 | B3 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 9 | A0 | B4 (`__ts_main` collision) | C0 | Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use; user must rename" | `fn __ts_main()` 直接 emit、reserved 化検出未実装 | ✗ Tier 2 reclassify | **本 PRD scope** | cell-05-ts-main-collision-no-exec (iteration v3 で fidelity 修正済 = A0 spec 整合) |
| 10 | A0 | B4 | C1 | NA per Axis mutual exclusion (上記 A0 + C1 と同 reason、collision detection は cell 20 (A1+B4+C1) でカバー) | NA | NA | NA | (no fixture) |
| 11 | A1 (top-Stmt::Expr only) | B0 (no main) | C0 | synthesize `fn main() { <top-level Stmt::Expr>; ... }` | `pub fn init() { ... }` only、no `fn main` | ✗ E0601 compile fail | **本 PRD scope** | cell-09-stmt-expr-only-no-main |
| 12 | A1 | B0 | C1 | synthesize `#[tokio::main] async fn main() { <top-level stmts including await>; }` (top-await capture into async fn main body) | unimplemented (現状 await 含む top-level stmts も `pub fn init` に格納、`fn main` 不在 + non-async context での await = compile fail) | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch、Option β)** | cell-14-top-await-no-main |
| 13 | A1 | B1 (sync) | C0 | rename user main to `__ts_main`、synthesize `fn main() { <top-level stmts>; <substituted main() calls>; }` + `fn __ts_main() { <user body> }` (multi-call sub-case = INV-2 verification: 全 user-side `main()` call site を `__ts_main()` に substitute) | `fn main() { <user body> }` + `pub fn init() { <top-level stmts>; }` (init never called = silent dead code) | ✗ silent semantic change L1 | **本 PRD scope (Tier 2 broken → Tier 1)** | cell-10-stmt-expr-with-user-sync-main; multi-call boundary value = cell-31-multiple-main-calls (INV-2 sub-case、H-7 Fix B per third-party review) |
| 14 | A1 | B1 | C1 | rename user sync main to `__ts_main`、synthesize `#[tokio::main] async fn main() { <top-level await stmts>; <substituted main() call>; }` (async wrapper invokes sync user main as non-await call) | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | cell-15-top-await-sync-main |
| 15 | A1 | B2 (async) | C0 | rename user main to `__ts_main`、synthesize `#[tokio::main] async fn main() { <top-level stmts>; <substituted main() call>; }` + `async fn __ts_main()` | `#[tokio::main] async fn main() { <user body> }` + `pub fn init() { <top-level stmts>; }` (init never called) | ✗ silent semantic change L1 | **本 PRD scope** | cell-11-stmt-expr-with-user-async-main |
| 16 | A1 | B2 | C1 | rename user async main to `__ts_main`、synthesize `#[tokio::main] async fn main() { <top-level await stmts>; <substituted main() call>; }` + `async fn __ts_main()` (substituted call site is `__ts_main().await`) | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | cell-16-top-await-async-main |
| 17 | A1 | B3 (non-fn symbol) | C0 | synthesize `fn main() { <top-level stmts>; }` + user non-fn symbol そのまま emit (Rust 上 fn と別 namespace) | `pub fn init() { ... }` only | ✗ E0601 + non-fn symbol preserved | **本 PRD scope** | cell-12-stmt-expr-with-non-fn-main |
| 18 | A1 | B3 | C1 | synthesize `#[tokio::main] async fn main() { <top-level await stmts>; }` + user non-fn symbol preserved (Rust type position) | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | cell-17-top-await-non-fn-main |
| 19 | A1 | B4 (`__ts_main` collision) | C0 | Tier 2 honest error reclassify (cell 9 と同 wording) | unimplemented | ✗ Tier 2 reclassify | **本 PRD scope** | cell-13-stmt-expr-with-ts-main-collision |
| 20 | A1 | B4 | C1 | Tier 2 honest error reclassify (cell 9 と同 wording、harness ESM upgrade で oracle 取得済 2026-05-01) | unimplemented | ✗ Tier 2 reclassify (harness ESM 必要) | **本 PRD scope (cohesive batch)** | cell-18-top-await-ts-main-collision |
| 21 | A2 (Lit init only) | B0 | C0 | declarations only (top-level `const x: f64 = 0.0;`)、no fn main 必要なし (Lit init は Rust const 適合 = library mode 維持可能) | top-level `const x: f64 = 0.0;` (correct) + no fn main | ✓ | regression lock-in | cell-19-decl-var-lit-init-no-main |
| 22 | A2 | B0 | C1 | NA per Axis A2 vs C1 mutual exclusion (Lit init は non-await、await init は A3) | NA | NA | NA | (no fixture) |
| 23 | A2 | B1 | C0 | top-level `const x: f64 = 0.0;` + `fn main() { <user main body> }` | top-level `const x: f64 = 0.0;` + `fn main() { <user body> }` (correct) | ✓ | regression lock-in | cell-20-decl-var-lit-with-user-main |
| 24 | A2 | B1 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 25 | A2 | B2 (async) | C0 | top-level `const x: f64 = 0.0;` + `#[tokio::main] async fn main() { <user async main body> }` | (currently unimplemented if A2+B2 ever appears; library const + tokio::main 直接 emit) | ✗ implementation gap (本 PRD scope で確定 ideal lock-in、orthogonality with cell 5 + cell 21) | **本 PRD scope (orthogonality merge: source cell # = 5 + 21、dispatch-equivalent for separate axes A and B)** | (no fixture, orthogonality merged with cells 5/21) |
| 26 | A2 | B2 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 27 | A2 | B3 (non-fn symbol) | C0 | top-level `const x: f64 = 0.0;` + non-fn symbol preserved + no fn main | (likely correct via existing path: top-level const + namespace 別 preserved + library mode) | ✓ orthogonality with cells 7 + 21 | regression lock-in | (no fixture, orthogonality merged) |
| 28 | A2 | B3 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 29 | A2 | B4 (collision) | C0 | top-level `const x: f64 = 0.0;` + Tier 2 honest reject (collision detection identical to cell 9) | unimplemented | ✗ Tier 2 reclassify | **本 PRD scope (collision detection invariant per INV-5)** | (no fixture, orthogonality merged with cell 9 collision dispatch) |
| 30 | A2 | B4 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 31 | A3 (Decl::Var with side-effect / non-const init) | B0 | C0 | synthesize `fn main() { let c = Counter::new(); ... }` (Decl::Var with non-const init を fn main body 内 `let` として capture) | declaration silently dropped (I-016 silent skip + no fn main) | ✗ silent drop + E0601 | **本 PRD scope (capture mechanism)** | cell-21-decl-var-side-effect-init-no-main |
| 32 | A3 | B0 | C1 | synthesize `#[tokio::main] async fn main() { let v = await_init().await; ... }` (top-await Decl::Var with await init を fn main body 内 `let v = ....await` として capture) | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | (NEW fixture pending Implementation Stage TS-3) |
| 33 | A3 | B1 (sync) | C0 | rename user main to `__ts_main`、synthesize `fn main() { let c = Counter::new(); ... <substituted main() call> }` + `fn __ts_main()` | declaration silently dropped + `fn main` (user) + `pub fn init` empty | ✗ silent drop + dead code | **本 PRD scope** | cell-22-decl-var-with-user-sync-main |
| 34 | A3 | B1 | C1 | rename user sync main to `__ts_main`、synthesize `#[tokio::main] async fn main() { let v = ....await; ... <__ts_main()> }` | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | (NEW fixture pending) |
| 35 | A3 | B2 (async) | C0 | rename user async main to `__ts_main`、synthesize `#[tokio::main] async fn main() { let c = Counter::new(); ... <substituted main() call> }` | declaration silently dropped + tokio::main (user) | ✗ silent drop + dead code | **本 PRD scope** | cell-23-decl-var-with-user-async-main |
| 36 | A3 | B2 | C1 | rename user async main to `__ts_main`、synthesize `#[tokio::main] async fn main() { let v = ....await; ... <__ts_main().await> }` | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | (NEW fixture pending) |
| 37 | A3 | B3 (non-fn symbol) | C0 | synthesize `fn main() { let c = Counter::new(); ... }` + user non-fn symbol preserved | declaration silently dropped | ✗ silent drop + E0601 | **本 PRD scope** | cell-24-decl-var-with-non-fn-main |
| 38 | A3 | B3 | C1 | synthesize `#[tokio::main] async fn main() { let v = ....await; ... }` + non-fn symbol preserved | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | (NEW fixture pending) |
| 39 | A3 | B4 (collision) | C0 | Tier 2 honest reject (collision detection invariant、INV-5、orthogonality with cell 9) | unimplemented | ✗ Tier 2 reclassify | **本 PRD scope** | (no fixture, orthogonality merged with cell 9 collision dispatch) |
| 40 | A3 | B4 | C1 | Tier 2 honest reject (collision detection invariant、harness ESM upgrade context でも reject 同一) | unimplemented | ✗ Tier 2 reclassify (harness ESM 必要) | **本 PRD scope (cohesive batch)** | (NEW fixture pending) |
| 41 | A4 (control-flow stmts at top-level) | B0 | C0 | Tier 2 honest preserve (existing `_ => Err(UnsupportedSyntaxError)` path、本 PRD scope 外 control-flow に Tier 1 化は別 PRD I-203 候補) | currently `_ => Err(UnsupportedSyntaxError)` at `transform_module_item` (correct Tier 2 honest reject) | ✗→Tier 2 honest reclassify (本 PRD で wording 改善のみ、structural fix preserve) | **Tier 2 honest error reclassify (本 PRD scope)** | (NEW fixture: cell-41-control-flow-no-main、Implementation Stage TS-3) |
| 42 | A4 | B0 | C1 | NA per A4 + C1 mutual exclusion (control-flow + top-await = A6 mixed partition) | NA | NA | NA | (no fixture) |
| 43 | A4 | B1 | C0 | Tier 2 honest preserve (orthogonality merge with cell 41、B-axis dispatch identical for A4 = control-flow body always rejected regardless of user main) | currently `_ => Err` | Tier 2 honest reclassify | **本 PRD scope (orthogonality merged with cell 41)** | (no fixture, orthogonality merged with cell 41) |
| 44 | A4 | B1 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 45 | A4 | B2 | C0 | Tier 2 honest preserve (orthogonality merged with cell 41) | currently `_ => Err` | Tier 2 honest reclassify | **本 PRD scope (orthogonality merged)** | (no fixture, merged) |
| 46 | A4 | B2 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 47 | A4 | B3 | C0 | Tier 2 honest preserve (orthogonality merged) | currently `_ => Err` | Tier 2 honest reclassify | **本 PRD scope (orthogonality merged)** | (no fixture, merged) |
| 48 | A4 | B3 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 49 | A4 | B4 | C0 | Tier 2 honest reject collision (orthogonality merge with matrix # 9 collision dispatch、INV-5 highest priority precedence: identifier-level reservation invariant が control-flow A-axis dispatch より先行 reject = `## Design > 2. fn main synthesis dispatch > Collision precedence` 参照) | currently `_ => Err` (control-flow path) | Tier 2 honest reclassify | **本 PRD scope (orthogonality merged with cell 9 collision dispatch、INV-5 priority via dispatch tree collision arm)** | (no fixture, merged with cell 9 collision dispatch via INV-5 priority arm) |
| 50 | A4 | B4 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 51 | A5a (Stmt::Empty) | B0 | C0 | Stmt::Empty silent skip (no-op、emission 不要、A5a representative cell) | Stmt::Empty silent skip (correct via existing path) | ✓ | regression lock-in | cell-27a-empty-stmt (iteration v3 新規作成 2026-05-01) |
| 52 | A5a | B0 | C1 | NA per A5a + C1 mutual exclusion (Empty stmts のみ + top-await = A6) | NA | NA | NA | (no fixture) |
| 53 | A5a | B1 | C0 | Stmt::Empty silent skip + user sync main = fn main directly emit (orthogonality merge with cell 51 silent skip + cell 3 user main directly emit) | (likely correct via existing path) | ✓ | regression lock-in (orthogonality merged with cells 51 + 3) | (no fixture, merged) |
| 54 | A5a | B1 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 55 | A5a | B2 | C0 | Stmt::Empty silent skip + user async main = tokio::main directly emit (orthogonality merge with cell 51 + cell 5) | (likely correct) | ✓ | regression lock-in (merged) | (no fixture, merged) |
| 56 | A5a | B2 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 57 | A5a | B3 | C0 | Stmt::Empty silent skip + non-fn symbol preserved (orthogonality merge with cell 51 + cell 7) | (likely correct) | ✓ | regression lock-in (merged) | (no fixture, merged) |
| 58 | A5a | B3 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 59 | A5a | B4 | C0 | Stmt::Empty silent skip + Tier 2 honest reject collision (orthogonality merge with cell 51 + cell 9) | unimplemented (Empty path correct + collision detection unimpl) | ✗ Tier 2 reclassify | **本 PRD scope (collision detection invariant、orthogonality merged with cell 9 collision dispatch)** | (no fixture, merged) |
| 60 | A5a | B4 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 61 | A5b (Stmt::Debugger) | B0 | C0 | Tier 2 honest reclassify "`debugger` statement has no Rust equivalent" (本 PRD で wording 確定) | currently `_ => Err(UnsupportedSyntaxError)` (一般 wording) | ✗ Tier 2 reclassify (wording 改善) | **本 PRD scope** | cell-27b-debugger-stmt |
| 62 | A5b | B0 | C1 | NA per A5b + C1 mutual exclusion | NA | NA | NA | (no fixture) |
| 63 | A5b | B1 | C0 | Tier 2 honest reclassify (debugger 含む top-level body = top-level の wholesale reject、orthogonality merged with cell 61) | currently `_ => Err` | Tier 2 honest reclassify | **本 PRD scope (orthogonality merged with cell 61)** | (no fixture, merged) |
| 64 | A5b | B1 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 65 | A5b | B2 | C0 | Tier 2 honest reclassify (orthogonality merged) | currently `_ => Err` | Tier 2 honest reclassify | **本 PRD scope (merged)** | (no fixture, merged) |
| 66 | A5b | B2 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 67 | A5b | B3 | C0 | Tier 2 honest reclassify (orthogonality merged) | currently `_ => Err` | Tier 2 honest reclassify | **本 PRD scope (merged)** | (no fixture, merged) |
| 68 | A5b | B3 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 69 | A5b | B4 | C0 | Tier 2 honest reclassify + collision (orthogonality merged with cell 61 + cell 9) | currently `_ => Err` | Tier 2 honest reclassify | **本 PRD scope (merged)** | (no fixture, merged) |
| 70 | A5b | B4 | C1 | NA per Axis mutual exclusion | NA | NA | NA | (no fixture) |
| 71 | A6 (mixed) | B0 | C0 | source order preserve、Stmt::Expr / Decl::Var (Lit init は library const、side-effect init は capture into fn main body)、control-flow は本 PRD scope 外 (cell 41 と一貫した Tier 2 honest preserve)、Empty silent skip / Debugger Tier 2 reclassify | unimplemented | ✗ partial silent drop | **本 PRD scope (cell 41 と一貫した dispatch + 各 sub-component dispatch)** | cell-28-mixed-no-main |
| 72 | A6 | B0 | C1 | A6 + top-await synthesis (= cell 71 spec + #[tokio::main] async fn main + top-await capture) | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | (NEW fixture pending) |
| 73 | A6 | B1 (sync) | C0 | source order preserve、user sync main rename + control-flow は本 PRD scope 外 | unimplemented | ✗ silent semantic change | **本 PRD scope** | cell-29-mixed-with-user-sync-main |
| 74 | A6 | B1 | C1 | A6 + sync user + top-await synthesis | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | (NEW fixture pending) |
| 75 | A6 | B2 (async) | C0 | source order preserve、user async main rename + #[tokio::main] async fn main | unimplemented | ✗ silent | **本 PRD scope** | (NEW fixture pending、orthogonality with cell 73 + cell 35) |
| 76 | A6 | B2 | C1 | source order preserve、user async main rename、#[tokio::main] async fn main、top-level await capture | unimplemented (combined edge) | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch)** | cell-30-mixed-top-await-async-main |
| 77 | A6 | B3 (non-fn) | C0 | source order preserve + non-fn preserved + synthesize fn main (orthogonality merge with cell 71 + cell 7) | unimplemented | ✗ silent | **本 PRD scope (orthogonality merged)** | (NEW fixture pending) |
| 78 | A6 | B3 | C1 | A6 + non-fn + top-await synthesis (orthogonality merge with cell 76 wrapper + cell 7 non-fn + cell 32 await capture) | unimplemented | ✗ + harness ESM upgrade required | **本 PRD scope (cohesive batch、orthogonality merged)** | (NEW fixture pending) |
| 79 | A6 | B4 (collision) | C0 | A6 + Tier 2 honest reject collision (orthogonality with cell 9) | unimplemented | ✗ Tier 2 reclassify | **本 PRD scope (orthogonality merged)** | (NEW fixture pending) |
| 80 | A6 | B4 | C1 | A6 + collision + top-await context = collision Tier 2 reject 同一 (orthogonality with cell 9 + harness ESM context) | unimplemented | ✗ Tier 2 reclassify (harness ESM 必要) | **本 PRD scope (cohesive batch、orthogonality merged)** | (NEW fixture pending) |

判定凡例: ✓ (現状 OK、regression lock-in test 必須) / ✗ (修正必要、本 PRD or 別 PRD) / NA (unreachable, spec-traceable reason、SWC parser empirical で structural lock-in) / Tier 2 honest reclassify (本 PRD で fix、Tier 1 化は別 PRD)

**Cell # vs Fixture file numbering**: matrix cell # は 1-80 の新方式 (Axis A × Axis B × Axis C lex order)、fixture file 名は file rename を回避するため旧 numbering (cell-01 〜 cell-31) を keep、各 cell 行の `Fixture` 列で対応 fixture file path を提供。

**Multi-call boundary value (cell 13 sub-case、H-7 Fix B per third-party review)**: 旧 cell-31 fixture (`A1 + B1 + C0` with multiple `main()` calls) は本 matrix の cell # 13 (= A1+B1+C0 main partition) の **boundary value test fixture** として keep、INV-2 (User main symbol semantic preservation) verification で「multiple call substitution sub-case = cell-31-multiple-main-calls fixture が boundary value test として locked-in」と明示。Axis A1 を sub-axis (single vs multi call) に分離せず、INV-2 verification method 内で sub-case integration。

### Axis B B1 Orthogonality Verification (Rule 1 (1-4) compliance、third-party review H-1)

Axis B B1 = "function decl / const arrow / const fn expr 統合" の 3 forms は SWC parser で異なる AST shape を生成:

| Form | TS source | SWC AST shape |
|---|---|---|
| B1a (function decl) | `function main(): void { ... }` | `Decl::Fn { fn_decl: FnDecl { ident, function } }` |
| B1b (const arrow) | `const main = (): void => { ... };` | `Decl::Var(VarDecl { decls[0]: VarDeclarator { name: BindingIdent("main"), init: Some(Expr::Arrow(_)) } })` |
| B1c (const fn expr) | `const main = function(): void { ... };` | `Decl::Var(VarDecl { decls[0]: VarDeclarator { name: BindingIdent("main"), init: Some(Expr::Fn(_)) } })` |

**Orthogonality merge legitimacy** (Rule 1 (1-4-a)): 3 forms 全てが本 PRD architectural concern (= "user `main` symbol detection + rename + main() call substitute") の dispatch logic 内で **同一 emission path** を通過する。具体的に:

1. **User main detection**: `Transformer::detect_user_main(module: &Module) -> UserMainKind`. 3 forms 全て `name == "main"` の `BindingIdent` (B1a は `FnDecl.ident`、B1b/B1c は `VarDeclarator.name`) を identify、`UserMainKind::FnSync` (or `FnAsync` for B2) を return。
2. **Rename target**: 3 forms 全て `BindingIdent("main")` を `BindingIdent("__ts_main")` に rewrite。Decl shape (Fn / Var-Arrow / Var-Fn) は preserve、identifier-level operation のみ。
3. **Call substitute**: `convert_expr` の `Call` arm で `Ident("main")` を `Ident("__ts_main")` に substitute (本 PRD の `user_main_substitution` flag）。

**Spec-stage structural consistency verify** (Rule 1 (1-4-b)): 3 forms の rename target identifier が同一 `__ts_main` namespace に collapse、Implementation Stage T3 で `test_axis_b_b1a_b_c_rename_dispatch_symmetric` (3 forms それぞれ probe) が同一 `__ts_main` 識別子への transform を verify。

**Spec-stage referenced cell symmetry probe** (Rule 1 (1-4-c)): 各 (A, C) cell with B1 が 3 forms とも同一 dispatch (= `__ts_main` rename + main() call substitute) を生成することを Implementation Stage T3 dispatch unit test で structural lock-in。

### Axis E Orthogonality Probe (Rule 1 (1-4-c) compliance)

Axis E E1 cells (= `export function main()`, `export const X = ...` 等) は E0 cells と同一 dispatch logic を通過することを以下で structural verify:

1. SWC parser empirical: `export function main(): void {}` → `ModuleDecl::ExportDecl { decl: Decl::Fn(...) }`、export wrapper を unwrap すると Decl shape は E0 と identical。
2. Implementation Stage T3 で `test_axis_e_export_preserve_symmetric` (representative reachable cells 11, 13, 21 から E1 form を probe) が `pub` modifier preserve + dispatch logic invariant (= main_stmts collection / rename / synthesis 全 phase) を lock-in。

**Axis E `pub` modifier preservation rule** (third-party adversarial review High #2 fix):

Axis E は ideal output に対して以下の rule で `pub` modifier 扱いを spec する。本 rule は INV-5 (`__ts_main` namespace reservation invariant) と整合:

- **User-defined `export function f()` の non-main case**: Rust 側 `pub fn f()` で modifier preserve (= existing path 維持、library export semantic)。
- **User-defined `export function main()` (Axis B B1 + Axis E E1)**: rename target は `__ts_main`、Rust 側出力は **`fn __ts_main()` (private、`pub` 不付与)**。Rationale:
  - INV-5 で `__ts_main` は **transpiler-internal identifier** として reserved、external API として expose されるべきではない (= user code から `__ts_main` を import しても `__ts_main` は symbolically reserved、本 PRD で Tier 2 reject されるか、本 PRD scope 外で用途不在)。
  - Rust binary crate context では `pub fn main`/`pub fn __ts_main` どちらも binary 内で意味を持たず、cosmetic difference のみ。Rust library crate context (= 本 PRD scope 外、I-016 owner) では `__ts_main` は本 PRD synthesized identifier であり library exports に含めるべきではない。
  - 即ち rename mechanism は user の `export` keyword を **strip** する (= rename + visibility change)、ideal output は `fn __ts_main()` private で固定。
- **`fn main` (synthesized) 自身の visibility**: Rust binary entry point として `fn main()` (private、`pub` 不付与) で固定。`#[tokio::main] async fn main()` も同様。

Implementation Stage T3 の `test_axis_e_export_preserve_symmetric` で本 rule を structural lock-in (= E1 form input で `__ts_main` 出力に `pub` modifier 不付与を assert)。

### Spec-Stage Adversarial Review Checklist

Spec stage 完了 verification は `.claude/rules/spec-stage-adversarial-checklist.md` の **13-rule checklist** を本 PRD `## Spec Review Iteration Log` section に転記して全項目 verification する。13-rule の 1 つでも未達があれば Implementation stage 移行不可。

(iteration v3 で旧 31-cell duplicate matrix table + duplicate "Spec-Stage Adversarial Review Checklist" 見出しを削除、上記 80-cell matrix のみ canonical = third-party adversarial review Critical #1 fix)

## Oracle Observations (Rule 2 (2-2) hard-code、各 ✗/要調査 cell の tsc / tsx empirical)

iteration v3 (Option β cohesive batch、2026-05-01) で旧 Out of Scope cells (cells 14-18/30) を In Scope migration、ESM mode (`scripts/observe-tsc.sh --esm --no-auto-main`) で empirical oracle 取得。各 ✗ cell について 4 項目 embed:

### Cell 9 (matrix #9): A0 + B4 (`__ts_main` collision, no top-exec)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-05-ts-main-collision-no-exec.ts`
- **tsc / tsx output (iteration v3 で fidelity 修正、`--no-auto-main` で再 record 2026-05-01)**:
  ```
  stdout: (empty)
  stderr: (empty)
  exit_code: 0
  ```
  (旧 iteration v2 の record `stdout: __ts_main\n` は `observe-tsc.sh` auto-append convention 由来 = `function main` ではなく `function __ts_main` を declare する fixture でも auto-append が誤発火していた issue を、iteration v3 で `--no-auto-main` flag 導入 + cell-05 fixture 自体から user-side call site を削除する fidelity 修正で解消、third-party review C-3 fix)
- **Matrix cell #**: 9 (A0 + B4 + C0)
- **Ideal output rationale**: TS では `function __ts_main()` declaration only、user-side call site なし → tsx で実行 stdout=(empty)。Rust 側では本 PRD の rename scheme と衝突 → Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use; user must rename to avoid collision"。Reject は ideal-implementation-primacy 整合 = silent collision risk 排除。

### Cell 11 (matrix #11): A1 + B0 (top-Stmt::Expr only, no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-09-stmt-expr-only-no-main.ts`
- **tsc / tsx output**:
  ```
  stdout: hello world\n
  stderr: (empty)
  exit_code: 0
  ```
- **Matrix cell #**: 11 (A1 + B0 + C0)
- **Ideal output rationale**: TS module-load semantics = top-level statements execute in source order。Rust binary entry = `fn main()`。Ideal: `fn main() { println!("hello world"); }` で TS runtime semantics preserved。

### Cell 12 (matrix #12): A1 + B0 + C1 (top-Stmt::Expr only, no user main, top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-14-top-await-no-main.ts`
- **tsc / tsx output (iteration v3、`--esm --no-auto-main` で 2026-05-01 record)**:
  ```
  stdout: got 42\n
  stderr: (empty)
  exit_code: 0
  ```
- **Matrix cell #**: 12 (A1 + B0 + C1)
- **Ideal output rationale**: TS module-load semantics で `const v = await Promise.resolve(42); console.log("got", v);` を順次 execute、stdout=`got 42\n`。Rust ideal: `#[tokio::main] async fn main() { let v = some_promise(42).await; println!("got {}", v); }` で execution order preserve。

### Cell 13 (matrix #13): A1 + B1 (top-Stmt::Expr + user sync main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-10-stmt-expr-with-user-sync-main.ts`
- **tsc / tsx output**:
  ```
  stdout: top-level\nfrom main\n
  stderr: (empty)
  exit_code: 0
  ```
  (TS spec: function declarations are hoisted but top-level statements execute in source order; user `main();` call (= top-level Stmt::Expr) preserves source order)
- **Matrix cell #**: 13 (A1 + B1 + C0)
- **Ideal output rationale**: silent semantic change 排除 = TS execution order を Rust で完全 preserve するため user main rename + synthesis が必須。
- **Multi-call boundary value sub-case (INV-2 verification)**: 旧 cell-31 fixture (`tests/e2e/scripts/i-224/cell-31-multiple-main-calls.ts`、stdout=`called\ncalled\n`) を本 cell の boundary value test fixture として keep、user main の multiple call sites が全 `__ts_main()` に substitute されることを probe。

### Cell 14 (matrix #14): A1 + B1 + C1 (top-Stmt::Expr + user sync main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-15-top-await-sync-main.ts`
- **tsc / tsx output (iteration v3、`--esm --no-auto-main` で 2026-05-01 record)**:
  ```
  stdout: from sync user main\ngot 10\n
  stderr: (empty)
  exit_code: 0
  ```
- **Matrix cell #**: 14 (A1 + B1 + C1)
- **Ideal output rationale**: top-await + user sync main rename + sync call (await suspends module-load、続いて hoisted main() を invoke、最後に console.log)。Rust ideal: `#[tokio::main] async fn main() { let v = some_promise(10).await; __ts_main(); println!("got {}", v); }` (sync user main は async fn 内から非 await call で invoke)。

### Cell 15 (matrix #15): A1 + B2 (top-Stmt::Expr + user async main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-11-stmt-expr-with-user-async-main.ts`
- **tsc / tsx output (2026-05-01 record)**:
  ```
  stdout: top-level\nfrom async main\n
  stderr: (empty)
  exit_code: 0
  ```
- **Matrix cell #**: 15 (A1 + B2 + C0)
- **Ideal output rationale**: cell 13 + async dispatch (#[tokio::main]) for user async main。本 cell は Axis C0 (= top-level await 不在) なので test harness 制約 (cjs/ESM) は不発、empirical verify 可能。

### Cell 16 (matrix #16): A1 + B2 + C1 (top-Stmt::Expr + user async main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-16-top-await-async-main.ts`
- **tsc / tsx output (iteration v3、`--esm --no-auto-main` で 2026-05-01 record)**:
  ```
  stdout: from async user main\ngot 20\n
  stderr: (empty)
  exit_code: 0
  ```
  (旧 iteration v2 record の `from async user main` 重複は `observe-tsc.sh` auto-append が `await main();` を検出できなかった script bug 由来、iteration v3 で `--no-auto-main` 導入 + `^\s*(await\s+)?main\(\)\s*;` regex 拡張で解消)
- **Matrix cell #**: 16 (A1 + B2 + C1)
- **Ideal output rationale**: top-await + async user main rename + await call (await suspends, then await main() invokes async user main, then top-level console.log)。Rust ideal: `#[tokio::main] async fn main() { let v = some_promise(20).await; __ts_main().await; println!("got {}", v); }`。

### Cell 17 (matrix #17): A1 + B3 (top-Stmt::Expr + non-fn main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-12-stmt-expr-with-non-fn-main.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`42\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 17 (A1 + B3 + C0)
- **Ideal output rationale**: interface `main` は Rust type position、synthesized `fn main()` (value position) と別 namespace で衝突なし、interface preserved + `fn main()` 合成。

### Cell 18 (matrix #18): A1 + B3 + C1 (top-Stmt::Expr + non-fn main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-17-top-await-non-fn-main.ts`
- **tsc / tsx output (iteration v3、`--esm --no-auto-main` で 2026-05-01 record)**:
  ```
  stdout: got 7 30\n
  stderr: (empty)
  exit_code: 0
  ```
- **Matrix cell #**: 18 (A1 + B3 + C1)
- **Ideal output rationale**: interface `main` runtime erased、await + console.log execute in source order。Rust ideal: `#[tokio::main] async fn main() { ... let v = some_promise(30).await; println!("got {} {}", m_id, v); }` + interface preserved as Rust type。

### Cell 19 (matrix #19): A1 + B4 (top-Stmt::Expr + `__ts_main` collision)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-13-stmt-expr-with-ts-main-collision.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`top-level\nuser __ts_main\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 19 (A1 + B4 + C0)
- **Ideal output rationale**: TS では `function __ts_main()` valid identifier、tsx execute 可能 (`top-level\nuser __ts_main\n` 順)。Rust では本 PRD rename scheme と衝突 → Tier 2 honest error reclassify。

### Cell 20 (matrix #20): A1 + B4 + C1 (top-Stmt::Expr + `__ts_main` collision + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-18-top-await-ts-main-collision.ts`
- **tsc / tsx output (iteration v3、`--esm --no-auto-main` で 2026-05-01 record)**:
  ```
  stdout: user collision __ts_main\ngot 40\n
  stderr: (empty)
  exit_code: 0
  ```
- **Matrix cell #**: 20 (A1 + B4 + C1)
- **Ideal output rationale**: collision detection は top-await context でも identical (= identifier-level reservation invariant、INV-5)、Rust 側 Tier 2 honest reclassify (cell 9/19 と同 wording)。

### Cell 31 (matrix #31): A3 + B0 (Decl::Var with side-effect init only, no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-21-decl-var-side-effect-init-no-main.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`1 2\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 31 (A3 + B0 + C0)
- **Ideal output rationale**: TS module body の `const p = makePoint(1, 2);` は module-load 時に execute、Rust では `fn main()` body 内 `let p = make_point(1.0, 2.0);` で semantic preserve。**注**: 本 cell の fixture は class instantiation を avoid (= `function makePoint()` 経由) して I-162 dependency を切り離し、B2 architectural concern を独立 verify。

### Cell 33 (matrix #33): A3 + B1 (Decl::Var with side-effect init + user sync main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-22-decl-var-with-user-sync-main.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`1 2\nfrom user main\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 33 (A3 + B1 + C0)
- **Ideal output rationale**: source order preserve + user main rename to `__ts_main` + synthesize `fn main() { let p = make_point(1.0, 2.0); println!("{} {}", p.x, p.y); __ts_main(); }`。

### Cell 35 (matrix #35): A3 + B2 (Decl::Var with side-effect init + user async main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-23-decl-var-with-user-async-main.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`1 2\nfrom async main\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 35 (A3 + B2 + C0)
- **Ideal output rationale**: cell 33 + async dispatch (`#[tokio::main] async fn main()`)。

### Cell 37 (matrix #37): A3 + B3 (Decl::Var with side-effect init + non-fn main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-24-decl-var-with-non-fn-main.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`point 1 2\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 37 (A3 + B3 + C0)
- **Ideal output rationale**: synthesize `fn main()` + interface main preserved as Rust type、let bindings + println in source order。

### Cell 51 (matrix #51): A5a + B0 (Stmt::Empty at top-level, no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-27a-empty-stmt.ts` (iteration v3 新規作成 2026-05-01)
- **tsc / tsx output (iteration v3 で record)**: stdout=(empty)、stderr=(empty)、exit_code=0
- **Matrix cell #**: 51 (A5a + B0 + C0)
- **Ideal output rationale**: TS では `;` standalone statement = no-op (Stmt::Empty)、stdout 影響なし。Rust 側では emission 不要 (silent skip)、library mode 維持 (no fn main 強制)。Axis A5a は cells 53/55/57/59 にも orthogonality merge で適用。

### Cell 61 (matrix #61): A5b + B0 (Stmt::Debugger at top-level, no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-27b-debugger-stmt.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`after debugger\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 61 (A5b + B0 + C0)
- **Ideal output rationale**: TS では `debugger;` は no-op (debugger 不在の context = production runtime)、stdout には影響なし (本 fixture では `console.log("after debugger")` を含むため A6 mixed pattern に近い、ただし Stmt::Debugger 自体は no-op)。Rust では debugger statement 等価不在 → Tier 2 honest error reclassify "`debugger` statement has no Rust equivalent"。Axis A5b は cells 63/65/67/69 にも orthogonality merge で適用。

### Cell 71 (matrix #71): A6 + B0 (mixed top-level + no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-28-mixed-no-main.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`100 42\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 71 (A6 + B0 + C0)
- **Ideal output rationale**: source order preserve、Lit init `LIT_VAL = 100` は library mode (top-level const)、side-effect init `n = compute()` は fn main body 内 let、Stmt::Expr (console.log) は fn main body 内。

### Cell 73 (matrix #73): A6 + B1 (mixed + user sync main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-29-mixed-with-user-sync-main.ts`
- **tsc / tsx output (2026-05-01 record)**: stdout=`100 42\nfrom user main\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 73 (A6 + B1 + C0)
- **Ideal output rationale**: cell 71 + user main rename + `__ts_main()` substitution。

### Cell 76 (matrix #76): A6 + B2 + C1 (mixed + user async main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-30-mixed-top-await-async-main.ts`
- **tsc / tsx output (iteration v3、`--esm --no-auto-main` で 2026-05-01 record)**:
  ```
  stdout: got 100 42 50\nfrom async main\n
  stderr: (empty)
  exit_code: 0
  ```
  (旧 iteration v2 record の `from async main` 重複は cell 16 と同 root cause = auto-append script bug、iteration v3 で fix)
- **Matrix cell #**: 76 (A6 + B2 + C1)
- **Ideal output rationale**: source order preserve + Lit const top-level + side-effect init + await init + user async main rename + await main() call。Rust ideal: top-level `const LIT_VAL = 100;` + `#[tokio::main] async fn main() { let n = compute_sync(); let v = some_promise(50).await; println!("got {} {} {}", LIT_VAL, n, v); __ts_main().await; }`。

### Cell 32 (matrix #32): A3 + B0 + C1 (Decl::Var with await init only, no user main, top-level await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-32-decl-var-await-init-no-main.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`got 99\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 32 (A3 + B0 + C1)
- **Ideal output rationale**: top-await Decl::Var init を fn main async body 内 `let v = some_promise(99).await;` で capture + `println!`、Trigger 2 only async dispatch via `#[tokio::main]`。

### Cell 34 (matrix #34): A3 + B1 + C1 (Decl::Var with await init + sync user main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-34-decl-var-await-init-sync-main.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`from sync user main\ngot 11\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 34 (A3 + B1 + C1)
- **Ideal output rationale**: rename sync user main → `__ts_main` + `#[tokio::main] async fn main()` + sync `__ts_main()` 非 await call wrapping (INV-3 (c) edge sub-case、Trigger 2 only でも sync user main 共存可能)。

### Cell 36 (matrix #36): A3 + B2 + C1 (Decl::Var with await init + async user main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-36-decl-var-await-init-async-main.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`from async user main\ngot 22\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 36 (A3 + B2 + C1)
- **Ideal output rationale**: rename async user main → `__ts_main` + `#[tokio::main] async fn main()` + `__ts_main().await` substitute (Trigger 1 + Trigger 2 combined)。

### Cell 38 (matrix #38): A3 + B3 + C1 (Decl::Var with await init + non-fn user main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-38-decl-var-await-init-non-fn-main.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`got 33 33\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 38 (A3 + B3 + C1)
- **Ideal output rationale**: interface `main` Rust type position に preserve + `#[tokio::main] async fn main()` 値 namespace に synthesize、衝突なし。

### Cell 40 (matrix #40): A3 + B4 + C1 (Decl::Var with await init + `__ts_main` collision + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-40-decl-var-await-init-ts-main-collision.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`user collision __ts_main\ngot 44\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 40 (A3 + B4 + C1)
- **Ideal output rationale**: INV-5 collision priority arm = dispatch tree `(_, Collision, _)` が先行 reject、Tier 2 honest error reclassify (cell 9 と同 wording)。harness ESM context でも reject 同一。

### Cell 41 (matrix #41): A4 + B0 + C0 (top-level control-flow stmt + no user main)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-41-control-flow-no-main.ts`
- **tsc / tsx output (iteration v5、`--no-auto-main` で 2026-05-01 record)**: stdout=`control-flow ran: 7\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 41 (A4 + B0 + C0)
- **Ideal output rationale**: 本 PRD scope では Tier 2 honest reclassify (= `UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; lift to a named function or use I-203 future expansion", span)`)。Tier 1 化 (= Rust fn main body 内 control-flow capture) は別 PRD I-203 候補。本 cell は A4 representative + cells 43/45/47 orthogonality merged。

### Cell 72 (matrix #72): A6 + B0 + C1 (mixed top-level + no user main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-72-mixed-no-main-top-await.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`got 100 42 72\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 72 (A6 + B0 + C1)
- **Ideal output rationale**: top-level `const LIT_VAL = 100;` (Lit init partition、per-item runtime hoist) + `#[tokio::main] async fn main() { let n = compute(); let v = ....await; println!(...); }`。

### Cell 74 (matrix #74): A6 + B1 + C1 (mixed + sync user main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-74-mixed-sync-main-top-await.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`from sync user main\ngot 100 42 74\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 74 (A6 + B1 + C1)
- **Ideal output rationale**: top-level `const LIT_VAL = 100;` + rename sync user main → `__ts_main` + `#[tokio::main] async fn main() { let n = compute(); let v = ....await; __ts_main(); println!(...); }` (sync `__ts_main()` 非 await call wrapping、INV-3 (c) edge sub-case)。

### Cell 75 (matrix #75): A6 + B2 + C0 (mixed + async user main + no top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-75-mixed-async-main-no-top-await.ts`
- **tsc / tsx output (iteration v5、`--no-auto-main` で 2026-05-01 record)**: stdout=`got 100 42\nfrom async main\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 75 (A6 + B2 + C0)
- **Ideal output rationale**: top-level `const LIT_VAL = 100;` + rename async user main → `__ts_main` + synthesis adds `__ts_main().await` (synthesis-added、user fixture では fire-and-forget `main();` だが Rust 側は async dispatch 整合のため await 付加)、Trigger 1 only via FnAsync。

### Cell 77 (matrix #77): A6 + B3 + C0 (mixed + non-fn user main + no top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-77-mixed-non-fn-main-no-top-await.ts`
- **tsc / tsx output (iteration v5、`--no-auto-main` で 2026-05-01 record)**: stdout=`got 77 100 42\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 77 (A6 + B3 + C0)
- **Ideal output rationale**: top-level `const LIT_VAL = 100;` + interface `main` Rust type position preserve + plain `fn main() { let n = compute(); println!(...); }` (Sync, no trigger)。

### Cell 78 (matrix #78): A6 + B3 + C1 (mixed + non-fn user main + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-78-mixed-non-fn-main-top-await.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`got 78 100 42 78\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 78 (A6 + B3 + C1)
- **Ideal output rationale**: top-level `const LIT_VAL = 100;` + interface preserve + `#[tokio::main] async fn main() { let n = compute(); let v = ....await; println!(...); }` (Trigger 2 only)。

### Cell 79 (matrix #79): A6 + B4 + C0 (mixed + `__ts_main` collision + no top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-79-mixed-ts-main-collision-no-top-await.ts`
- **tsc / tsx output (iteration v5、`--no-auto-main` で 2026-05-01 record)**: stdout=`user collision __ts_main\ngot 100 42\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 79 (A6 + B4 + C0)
- **Ideal output rationale**: INV-5 collision priority arm 先行 reject、Tier 2 honest error reclassify (cell 9 と同 wording、A6 mixed body 内の Lit init 部分も含めて module 全体 reject)。

### Cell 80 (matrix #80): A6 + B4 + C1 (mixed + `__ts_main` collision + top-await)

- **TS fixture path**: `tests/e2e/scripts/i-224/cell-80-mixed-ts-main-collision-top-await.ts`
- **tsc / tsx output (iteration v5、`--esm --no-auto-main` で 2026-05-01 record)**: stdout=`user collision __ts_main\ngot 100 42 80\n`、stderr=(empty)、exit_code=0
- **Matrix cell #**: 80 (A6 + B4 + C1)
- **Ideal output rationale**: INV-5 collision priority arm 先行 reject、Tier 2 honest error reclassify (cell 9 と同 wording)。harness ESM context でも reject 同一。

(全 in-scope ✗ cells = representative + NEW = 33 cells で oracle empirical record 完了 = third-party adversarial re-review (3rd round) Compromise audit fix で Rule 5 (5-1) "Spec stage 完了時点で red 状態 fixture 準備済" 厳格 compliance 達成。残 in-scope cells は orthogonality merge representative dispatch test で cover、Implementation Stage T2/T3 unit test で 1-to-1 mapping verify。)

### Auto-append Convention Note (Spec Stage Fidelity)

旧 iteration v2 では `scripts/observe-tsc.sh` が `function main` declaration を検出して `main();` を auto-append、A0 + B1/B2 cells (matrix # 3/5) の oracle stdout が user main body 実行結果を含む形で record されていた。これは Rust binary の "fn main = entry point" convention に合わせた test harness behavior であり、**TS module-load semantics 上は `function main()` declaration only ≠ 自動 invoke**。iteration v3 で:

- `--no-auto-main` flag を追加 (script 改修)、Spec stage oracle observation 用 "fidelity mode" を導入
- 旧 cells 02/03 (matrix # 3/5) の .expected は **auto-append convention 由来 record** として keep (= Rust 側 ideal output が `fn main() { user_body }` で entry point として execute、TS 側 strict semantic とは divergence するが「user `function main()` を Rust binary entry point として treat する」project convention)
- 新規 oracle record (cells 14-18/30 + cell-05 fix) は `--no-auto-main` で取得 = strict TS semantic fidelity

このセクションで「auto-append convention」と「strict TS fidelity」両者の oracle record 同居を明示記録、第三者 review C-3 (cell-05 fixture content) を fidelity 側で fix 完了。

## SWC Parser Empirical Lock-ins (Rule 3 (3-2) hard-code、NA cells 用)

### Axis A vs Axis C1 mutual exclusion lock-in (cells 2/4/6/8/10/22/24/26/28/30/42/44/46/48/50/52/54/56/58/60/62/64/66/68/70 — 25 cells、本 PRD scope iteration v3)

iteration v3 (Option β cohesive batch、third-party review C-2 fix) で SWC parser empirical lock-in test を本 PRD scope 内で作成 (旧 iteration v2 では I-226 defer 設計 = Rule 3 (3-2) hard violation と判定)。

**実装場所**: `tests/swc_parser_top_level_await_test.rs` (新規 file 2026-05-01、4 tests passing)

**Test 構成**:

| Test fn | Verifies | Cell coverage |
|---|---|---|
| `test_top_level_bare_await_parses_as_stmt_expr_await_axis_a1` | `await x;` → `Stmt::Expr(Expr::Await)` で A1 partition、A0 ではない | A0+C1 cells (2/4/6/8/10) のうち bare-await form の structural exclusion |
| `test_top_level_var_decl_with_await_init_parses_as_decl_var_axis_a3` | `const x = await y;` → `Decl::Var` with `Expr::Await` init で A3 partition、A0/A2 ではない | A0+C1 + A2+C1 cells のうち var-decl-await-init form の structural exclusion |
| `test_pure_axis_a0_source_contains_no_await_expression` | pure A0 source (declarations only) は top-level に `Expr::Await` を含まない | A0+C1 (cells 2/4/6/8/10) 全体 |
| `test_axis_c1_implies_a1_or_a3_partition_synthesis` | C1 forms (4 variations: `await x;`, `const/let/var x = await y;`) 全てが A1 or A3 partition に collapse | A0+C1, A2+C1, A4+C1, A5a+C1, A5b+C1 全 25 cells (= C1 が「のみ」制約に非含 partition は AST 構造的に不可能) |

**Spec-traceable NA justification (本 PRD scope 内 lock-in 完成、Rule 3 (3-2) compliant)**:

Axis C1 (top-level await) は AST shape として **Stmt::Expr (Expr::Await)** または **Decl::Var with `Expr::Await` init** を要求する。これは Axis A1 / A3 の partition definition と一致するため、以下 5 partition と AST 構造的に mutually exclusive:

- **A0 + C1**: A0 = 「実行 stmt 不在 (declarations / imports only)」 → C1 implies Stmt::Expr (= 実行 stmt 存在) → contradiction
- **A2 + C1**: A2 = 「Decl::Var with literal init only」 → await init は `Expr::Await` (non-literal) → contradiction
- **A4 + C1**: A4 = 「control-flow stmts のみ」 → C1 implies Stmt::Expr / Decl::Var (non-control-flow) → 「のみ」制約違反、A6 partition に分類 = A4+C1 partition 自体が空集合
- **A5a + C1**: A5a = 「Stmt::Empty のみ」 → C1 implies Stmt::Expr / Decl::Var → 「のみ」違反 = 空集合
- **A5b + C1**: A5b = 「Stmt::Debugger のみ」 → 同上 = 空集合

**SWC parser accept = Tier 2 reclassify check** (Rule 3 (3-3)): SWC parser は `await x;` を **accept** する (本 lock-in test の `test_top_level_bare_await_parses_as_stmt_expr_await_axis_a1` で empirical 確認)。これは TS spec の "module context required" 制約 (TS1375) とは別の話で、SWC parser は寛容 parsing で AST 上は受理。受理された AST shape が **A1 or A3 partition** に分類されるため、本 PRD では A0/A2/A4/A5a/A5b + C1 cells を「partition 自体が空集合」(NA 構造的不可能) として扱い、reachable な C1 cells は A1/A3/A6 + C1 として Tier 1 完全変換 (Option β cohesive batch in-scope)。`unreachable!()` macro を使用する箇所は Implementation Stage T2 で `Stmt`/`Decl` exhaustive enumeration 内に明示記載。

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
  - "Axis A - Top-level body composition (8 variants iteration v3: A0 library / A1 Stmt-Expr / A2 Decl-Var-Lit / A3 Decl-Var-side-effect / A4 control-flow stmts / A5a Stmt-Empty / A5b Stmt-Debugger / A6 mixed)"
  - "Axis B - User-defined main symbol (5 variants: B0 none / B1 sync-fn / B2 async-fn / B3 non-fn-symbol / B4 ts-main collision; B1 has 3 sub-forms B1a/B1b/B1c orthogonality-merged with structural verify per Rule 1 (1-4))"
  - "Axis C - Top-level await presence (2 variants: C0 absent / C1 present)"
  - "Axis E - Module export presence (2 variants: E0 absent / E1 present; orthogonality merge declaration per Rule 1 (1-4) - matrix sub-axis 化せず、各 cell ideal output が E0/E1 共通、structural probe で Implementation stage validation)"
  - "Cross-axis sub-axes per default check axis - trigger condition (top-exec presence) / operand type variants (user main fn vs non-fn) / guard variant (NA - guard-less concern) / body shape (top-level stmt kinds capture into fn main) / closure-reassign (NA) / early-return (NA - main body stmts are execution semantic) / outer emission context (module-level / fn main body / pub fn init body deprecated) / control-flow exit (NA) / AST dispatch hierarchy (ModuleItem -> Stmt -> Decl -> Expr layers)"
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: "N/A (matrix-driven PRD)"
```

## Goal

TS module top-level の Rust emission を **TS module-load semantics と byte-exact equivalent** な fn main mechanism として完成させる (Option β cohesive batch、Axis C1 = top-level await 全 cells を含む完全 verify infrastructure 統合)。

具体的 verifiable goals:

1. **Universal e2e infra**: 全 future PRD で `function main()` wrap 不要、top-level statement form の e2e fixture (Axis C0 / C1 両 partition) が直接 cargo run pass。**Verify by**: I-205 T14 fixture cell-09 (= matrix #11、static-only、本 PRD で唯一 dependency 不在 cell) が e2e green pass。
2. **Silent semantic change 排除**: cells 13/15/14/16/33/35/73/74/75/76 等の "user main + top-level statements" 共存 case で TS execution order を Rust 側でも preserve、tsc stdout と byte-exact match。**Verify by**: Hono bench Tier-transition compliance (compliance check only、Hono codebase で本 pattern reachability TBD、grep `__ts_main` 0 hits + reasonably reachable surface area の qualitative scan)。
3. **Rust E0601 排除**: 全 ✗ cell (本 PRD scope) で `cargo run` 成功 (= `fn main` 自動生成)。**Verify by**: TS-3 で red 状態 fixture が T1-T9 完了後 green 化。
4. **Top-level await full coverage** (Option β goal): cells 12/14/16/18/20/32/34/36/38/40/72/74/76/78/80 (Axis C1 全 reachable cells) で top-await capture into `#[tokio::main] async fn main()` body + test harness ESM mode で oracle empirical match。**Verify by**: T7 で `scripts/observe-tsc.sh --esm` flag を CI integrate、T8 で top-await synthesis logic 実装、T9 で全 Axis C1 fixture green。
5. **Rule 11 (d-1) compliance**: `transform_module_item` の `_` arm を ModuleItem 全 variant explicit enumerate に refactor、新 variant 追加時 compile error で全 dispatch fix 強制。**Verify by**: `audit-ast-variant-coverage.py --files src/transformer/mod.rs` で本 PRD scope `_` arm violation 0 件。
6. **`__ts_` namespace extension**: I-154 reservation rule に `__ts_main` を追加、Tier 2 honest error reclassify with explicit user-facing wording。**Verify by**: codebase + Hono grep `__ts_main` で 0 hits empirical (R-4 audit task)。

## Scope (3-tier 形式 hard-code、Rule 6 (6-2) 適用)

### In Scope

本 PRD で **Tier 1 完全変換** する features (Option β cohesive batch、Axis C0 + Axis C1 全 reachable cells 統合):

**Axis C0 (top-await 不在) cells**:
- Matrix # 11 (旧 cell-09): Synthesize `fn main()` from top-level Stmt::Expr (no user main case)
- Matrix # 13/15 (旧 cells 10/11): Synthesize `fn main()` + rename user sync/async main to `__ts_main` (silent semantic change 排除)
- Matrix # 17 (旧 cell-12): Synthesize `fn main()` + non-fn user main preserved
- Matrix # 19 (旧 cell-13): `__ts_main` collision detection + Tier 2 honest reject
- Matrix # 31/33/35/37 (旧 cells 21-24): Synthesize `fn main()` + capture top-level Decl::Var with side-effect init as `let` bindings inside fn main body (init expression 変換は I-162 prerequisite chain、本 PRD は capture mechanism のみ scope)
- Matrix # 71/73/75/77 (旧 cell-28/29 + NEW cells): Mixed cases (Stmt::Expr + Decl::Var)、source order preserve、multiple `main()` calls substitution invariant verify
- **Matrix # 25 (A2 + B2 + C0)**: Lit init + async user main = top-level `Item::Const` emit + `#[tokio::main] async fn main` directly emit。**Fixture 不要 (orthogonality merged with cells 5 + 21)** = (cell 5: A0+B2+C0 = async user main directly emit) + (cell 21: A2+B0+C0 = top-level Item::Const emit) の orthogonal composition、Implementation Stage T3 で representative cells 5/21 fixture が cell 25 を cover (third-party adversarial re-review (3rd round) High 1 fix で cell 29 と分離、各 cell の actual merge sources を明確化)
- **Matrix # 29 (A2 + B4 + C0)**: Lit init + `__ts_main` collision = INV-5 collision priority arm (= dispatch tree `(_, Collision, _)` arm) が先行 reject、Lit init は per-item runtime decision で top-level Item::Const として emit されるが collision detection が module-level scan で先行 fire するため module 全体 Tier 2 reject。**Fixture 不要 (orthogonality merged with cell 9 collision dispatch + cell 21 Lit init partition)**、Implementation Stage T3 で representative cell 9 fixture が cell 29 を cover (third-party adversarial re-review (3rd round) High 1 fix で cell 25 と分離)
- Matrix # 29/39/40/49/59/69/79/80 (collision detection invariant、orthogonality merged with matrix # 9 collision dispatch via INV-5 priority arm = dispatch tree `(_, Collision, _)` 先行 reject。third-party adversarial re-review (3rd round) High 3 fix で cells 40/80 を本 list に追加、cell 40 (A3+B4+C1) と cell 80 (A6+B4+C1) は Axis C1 partition の collision cells)
- (Cell 41/43/45/47 = A4 control-flow cells は Tier 2 reclassify tier に classification = 本 In Scope 列ではなく後段 "Tier 2 honest error reclassify" sub-section 参照、Rule 6 (6-2) 3-tier mutual exclusivity 整合; cell 49 = A4+B4 は INV-5 collision priority で cell 9 collision arm に orthogonality merged)
- Matrix # 51 representative (Stmt::Empty silent skip、orthogonality merged with cells 53/55/57/59)
- Matrix # 61 representative (Stmt::Debugger Tier 2 reclassify、orthogonality merged with cells 63/65/67/69)

**Axis C1 (top-await 存在) cells (Option β cohesive batch、iteration v3 で In Scope migration)**:
- Matrix # 12 (旧 cell-14): top-await capture into `#[tokio::main] async fn main()` (no user main)
- Matrix # 14 (旧 cell-15): top-await + sync user main rename + async wrapper invokes sync `__ts_main()` non-await call
- Matrix # 16 (旧 cell-16): top-await + async user main rename + `__ts_main().await` substitute
- Matrix # 18 (旧 cell-17): top-await + non-fn main preserved
- Matrix # 20 (旧 cell-18): top-await + collision detection invariant
- Matrix # 32/34/36/38/40 (NEW、A3 + various B + C1): side-effect init with `Expr::Await` capture into async fn main body
- Matrix # 72/74/76/78/80 (NEW + 旧 cell-30): mixed top-level + top-await synthesis

**Test harness ESM upgrade (Option β cohesive batch infrastructure)**:
- `scripts/observe-tsc.sh --esm` flag 追加 (iteration v3 で実施済、`package.json {"type":"module"}` を temp dir に配置で tsx ESM mode、top-await accept) + `--no-auto-main` flag 追加 (Spec stage oracle observation fidelity 用)
- `tests/e2e/rust-runner/Cargo.toml` に tokio runtime 依存追加 (Implementation Stage T7)
- `tests/e2e_test.rs` runner の ESM-mode runner template (= top-level await capture を `#[tokio::main] async fn main()` で execute する Rust binary を build / cargo run)

**Common scope**:
- `__ts_` namespace reservation で `__ts_main` 追加 (I-154 extension、`src/transformer/expressions/mod.rs:57-98` の constants + `src/transformer/statements/mod.rs:39-48` の validator 拡張)
- `transform_module_item` の `_` arm を全 ModuleItem variant explicit enumerate に refactor (Rule 11 d-1 compliance)
- `pub fn init` mechanism 廃止 (= module body emission を fn main 統合)

### Out of Scope (= 本 PRD で **code 修正対象外** な features、third-party review Medium #3 fix で wording 訂正)

別 PRD or 永続 unsupported な features (= 本 PRD で source code modification は不要だが、本 PRD scope に test 追加が含まれる場合あり):

- **Matrix # 1/3/5/7/21/23/27 (regression lock-in cells、A0/A2 + various B + C0、existing correct emission)**: 既 correct emission preserve、本 PRD では **code 修正対象外** だが、Test Plan E2E section で regression lock-in test 追加対象 (= 本 PRD scope の `test_e2e_cell_i224_<NN>` entries に含む)。**Cell 27 (A2+B3+C0)** = third-party adversarial re-review (3rd round) High 2 fix で本 list に追加 (= Lit init + non-fn user main = top-level Item::Const + library mode 維持、orthogonality merge with cells 7 + 21、representative fixture 不要だが test_e2e entry 必須)
- **I-016 (Module-level const Call/Ident/String/Regex/BigInt init の Tier 1 化)**: 別 PRD scope (= **library mode** での module-level const variant 対応)。executable mode (= 本 PRD scope) では fn main body capture で対応、library mode (= 別 PRD scope) で I-016 が top-level static / lazy_static 等の strategy で対応
- **I-221 (top-level Module-level statement TailExpr noise)**: 別 PRD scope (= top-level Stmt::Expr の convert_stmt vs convert_expr dispatch concern、本 PRD は emission destination = fn main body concern と orthogonal)
- **I-180 (E2E harness async-main multi-execution)**: 別 PRD scope (= test infra defect、本 PRD は transpiler emission concern + harness ESM upgrade infra concern、I-180 は別 dimension)
- (Cell 41/43/45/47 = A4 control-flow cells は **本 PRD scope (Tier 2 honest reclassify)** に classification、後段 "Tier 2 honest error reclassify" sub-section 参照; cell 49 = A4+B4 は INV-5 collision priority で cell 9 collision dispatch に orthogonality merged。**Tier 1 化** (= control-flow を fn main body 内 capture して compile-pass + runtime semantic preserve) は別 PRD I-203 (codebase-wide AST exhaustiveness compliance) 候補 = 本 PRD architectural concern boundary 外、top-level "execution stmt" 概念に control-flow を含めると scope creep)

### Tier 2 honest error reclassify

本 PRD で **Tier 2 honest error 化** する features (= 別 PRD で Tier 1 化候補):

- **Matrix # 9/19/20 + collision-merged cells 29/39/40/49/59/69/79/80**: User `function __ts_main()` 等 `__ts_` namespace 衝突 → Tier 2 honest error "`__ts_main` is reserved for transpiler-internal use; user must rename" (third-party adversarial re-review (3rd round) High 3 fix で cell 40 = A3+B4+C1 を本 list に追加、INV-5 priority arm 整合)
- **Matrix # 61 + A5b-merged cells 63/65/67/69 (Stmt::Debugger at top-level)**: Rust に debugger statement 等価不在 → Tier 2 honest error "`debugger` statement has no Rust equivalent (= compile-time `panic!()` or `std::dbg!()` を user 自身で選択)"
- **Matrix # 41 (representative) + A4-merged cells 43/45/47 (top-level control-flow at top-level、Axis A4 × B0/B1/B2/B3 + C0)**: 既存 Tier 2 honest error preserve + wording 改善 (= `UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; lift to a named function or use I-203 future expansion", span)`)。**Cell 49 (A4+B4) は本 list から除外**: INV-5 collision priority に従い `dispatch tree (_, Collision, _)` arm が先行 reject (= cell 9 collision dispatch に orthogonality merge)。**Tier 1 化は別 PRD I-203 で扱う候補**

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
- **(c) Verification method**: in-scope cells 13/14/15/16/33/34/35/36/73/74/75/76 fixture で user `main()` call site が `__ts_main()` (sync) or `__ts_main().await` (async) に substitute されることを fixture probe + IR token-level test で verify。**Multi-call boundary value sub-case** (H-7 Fix B per third-party review): cell-31 fixture (`tests/e2e/scripts/i-224/cell-31-multiple-main-calls.ts`、A1+B1+C0 with `main(); main();` form) を cell #13 の boundary value test fixture として keep、user main の multiple call sites が全 `__ts_main()` に substitute されることを probe (single call vs multi-call sub-axis を Axis A1 内に分離せず INV-2 verification method 内で sub-case integration、Rule 1 (1-2) の axis 過剰膨張を避けつつ multi-call substitution 完全性を保証)。
- **(d) Failure detectability**: compile error (substitution 漏れで undefined name) or silent drop (substitution 過剰で wrong name resolved)。

### INV-3: Sync / async dispatch consistency (iteration v3 で Axis C1 in-scope 反映に wording revise)

- **(a) Property statement**: 全 in-scope cell で fn main の sync / async dispatch が **以下の trigger 集合のいずれか 1 つ以上** が満たされたら `#[tokio::main] async fn main` (= async dispatch)、全て不在なら sync `fn main` で exhaustive + mutually exclusive 決定:
  - **Trigger 1**: Axis B B2 (= user-defined async function main; B1/B3/B0/B4 は trigger しない)
  - **Trigger 2**: Axis C C1 (= top-level await present, Option β cohesive batch で in-scope 化、test harness ESM upgrade で empirical verify 可能)
- **(b) Justification**: 違反すると async context が sync で配置 (top-await が non-async fn 内 = compile error) または sync user main が tokio runtime で wrap (suboptimal Rust)。Trigger 1 + Trigger 2 を mutually-merge OR 条件で扱うことで cell 14 (sync user main + top-await) のような combination も `#[tokio::main] async fn main { ... __ts_main(); ... }` で正しく dispatch (sync user main を async fn から非 await call で invoke)。
- **(c) Verification method**: 
  - **Trigger 1 (B2) only** (= async user main + Axis C0、has_top_level_await=false): cells 5/15/25/35/55/75 (= 本 PRD scope の Axis B2 + C0 全 cells、ただし cell 55 は orthogonality merged with cell 51 + cell 5、representative dispatch test で cover) で `#[tokio::main] async fn main` 出力 verify (third-party adversarial re-review (3rd round) Critical 3 fix で cell 55 追加)
  - **Trigger 2 (C1) only** (= no/sync/non-fn user main + Axis C1、user_main_kind ≠ FnAsync): cells 12/14/18/32/34/38/72/74/78 (= 本 PRD scope の Axis B0/B1/B3 + C1 全 cells、ただし cell 14 は INV-3 (b) Trigger 2 only として B1+C1 sync edge 含む) で `#[tokio::main] async fn main` 出力 verify (third-party adversarial re-review (3rd round) Critical 3 fix で cells 32/34/38/72/74/78 追加)
  - **Trigger 1 + 2 combined** (= async user main + Axis C1): cells 16/36/76 (= 本 PRD scope の Axis B2 + C1 全 cells) で `#[tokio::main] async fn main` 単一発行 (重複 attribute 不在) + `__ts_main().await` substitute call site verify
  - **Sync (no trigger、`fn main` 出力 cells)** (= sync/non-fn user main + Axis C0 with `fn main` emission、user_main_kind ∈ {FnSync, NonFn} かつ has_top_level_await=false かつ executable mode、加えて library mode で `fn main directly emit` する cells): cells 3/11/13/17/23/31/33/37/71/73/77 で plain `fn main` (or `pub fn main` for Axis E E1 representative path) verify (third-party adversarial re-review (4th round) Medium 1 fix で library mode `fn main directly emit` cells 3/23 を追加 = INV-3 (a) Property "全 in-scope cell で `fn main` の sync / async dispatch...決定" の exhaustive coverage)
  - **Library mode no-fn-main cells (INV-3 scope 外)** (= library mode + B0/B3 user_main_kind = no `fn main` emission): cells 1/7/21/27 は `fn main` 自体 emit しない (declarations only library mode = Axis A0/A2 + B0/B3)、INV-3 sync/async dispatch invariant の application 対象外 (Property statement (a) の "fn main の sync / async dispatch" 概念に該当しない)。本 cells は別の invariant "no fn main emission in library mode" で structural lock-in (= Implementation Stage T2 helper unit test で `(false, B0/B3, false)` arms が library mode → no fn main emit を assert)
  - **Edge sub-case (Trigger 2 only with sync user main、INV-3 (c) 内 sub-case)**: cells 14/34/74 (B1 + C1 = sync user main + top-await cohabitation across A1/A3/A6) で `#[tokio::main] async fn main` + sync `__ts_main()` 非 await call wrapping (sync user main は async fn から非 await call で invoke 可能) verify。本 sub-case は third-party adversarial review High #4 fix で in-scope B1+C1 cells を exhaustive 列挙
  - **Note (exhaustivity verify)**: 本 sub-case lists は dispatch tree (Design section #2) の 12 reachable arms (= excluding Collision arm + unreachable arm) の matrix # listing から **exhaustively derive** されている。Rule 8 invariant verification の structural completeness を保証するため、Implementation Stage T2/T3 の helper unit test で本 4 sub-case lists を per-cell expected `is_async_required` value として fixture-driven assert
- **(d) Failure detectability**: compile error (async fn 内 sync user main を await で call、または sync fn main 内 top-await を配置 = 構造的 compile error) or suboptimal output (sync-only context での tokio runtime overhead)。

### INV-4: `pub fn init` mechanism 廃止 invariant

- **(a) Property statement**: 本 PRD 完了後、ts_to_rs の transpile output 内に `pub fn init()` 識別子が存在しない (= 全 emission path が fn main 統合 or library mode 実装に migration)。
- **(b) Justification**: `pub fn init` は never-called dead code source であり、本 PRD architectural concern (= fn main mechanism unification) の structural fix 完成条件。
- **(c) Verification method**: Codebase grep `pub fn init` で 0 hits 確認 (test fixtures + production code)、`build_init_fn` helper 削除確認、CI script `scripts/audit-no-pub-fn-init.sh` (新規) で auto verify。
- **(d) Failure detectability**: silent dead code preservation (compile pass + runtime drop = Tier 1 silent semantic change risk continues)。

### INV-5: `__ts_` namespace reservation extension consistency

- **(a) Property statement**: I-154 `__ts_` namespace reservation rule に `__ts_main` が追加 + 全 user identifier validation path で `__ts_main` を reserved 検出、collision case (= matrix # 9/19/20 + collision-merged cells 29/39/40/49/59/69/79/80) で Tier 2 honest error reject (third-party adversarial re-review (3rd round) High 3 fix で cell 40 を本 list に追加、INV-5 priority arm が cell 40 = A3+B4+C1 を含む全 reachable B4 cells を絡める)。
- **(b) Justification**: rename scheme の structural foundation。reservation 不在で user `function __ts_main()` 共存可能なら本 PRD の rename mechanism が silent collision を引き起こす risk。
- **(c) Verification method**: I-154 namespace reservation test (= 既存 `__ts_old`, `__ts_new`, `__ts_recv` 等の test を `__ts_main` 拡張)、collision detection unit test、matrix # 9/19/20 fixture probe、`__ts_main` empirical pre-existing user-code audit (R-4 task) で codebase + Hono grep `__ts_main` 0 hits 確認。
- **(d) Failure detectability**: compile error (Rust 上 user `__ts_main` と本 PRD synthesized `__ts_main` の identifier collision = E0428 duplicate definitions)。

### INV-6: TypeResolver layer unaffected (third-party review R-3)

- **(a) Property statement**: 本 PRD の fn main synthesis + user main rename + main() call substitute logic は **TypeResolver layer の type resolution flow に影響しない**。具体的に: TypeResolver はモジュール内の type binding / expr_type lookup / narrowing 等を処理する pipeline phase であり、本 PRD の identifier rename (`main` → `__ts_main`) は AST transform stage (post-TypeResolver) で完結。TypeResolver 入力 (= `Module` AST) の identifier text を user-defined のまま保持、TypeResolver は `main` を user fn として既に正しく resolve、本 PRD は post-resolution AST に rename を後付け。
- **(b) Justification**: TypeResolver phase で identifier rename を行うと type resolution table が user-source-text 基準で構築されているため key mismatch を起こす risk。本 invariant により TypeResolver 経由 path に変更が波及しないことを構造的に保証。
- **(c) Verification method**: 
  - 既存 TypeResolver unit tests が本 PRD changes (= `Transformer::transform_module` の dispatch + main_synthesis logic) で全 pass
  - Implementation Stage T2 着手前に empirical probe (= `cargo test --lib pipeline::type_resolver::` 全 pass、`fn detect_user_main` の input/output で TypeResolver field を touch しないことを review)
  - dispatch logic 内に TypeResolver 呼び出し (`type_registry.lookup` 等) が新規追加されていないことを Code review (Layer 1 Mechanical) で audit
- **(d) Failure detectability**: TypeResolver test failure (= 既存 type resolution path の regression、compile-time / runtime いずれか) or silent type mismatch (= type fallback 拡大、本 PRD では type fallback 不在のため発生なし)。

### INV-7: `pub fn init` mechanism 廃止の external API audit (third-party review R-2)

- **(a) Property statement**: `pub fn init` mechanism 廃止は ts_to_rs の generated Rust code の external API breaking change である (= user / downstream test が generated Rust 上 `init()` を call する case が存在すれば compile fail)。本 PRD で **codebase + Hono + 既存 e2e test 全体で `init()` call site の empirical audit を完了** し、breaking change の実 reachability を 0 件に確定。
- **(b) Justification**: INV-4 が `pub fn init` 識別子 generated code 内 0 hits を保証するが、**呼び出し側 (call site) の audit は別軸**。INV-7 は call site reachability を保証することで breaking change の actual impact を確定 (= 0 hits なら structural improvement、>0 hits なら本 PRD が affecting downstream を ackowledge し migration path を提供)。
- **(c) Verification method**: 
  - Codebase grep: `grep -rn '\\binit\\s*(' src/ tests/ tools/` で ts_to_rs side の `init()` call site enumerate + test 用 `init()` call (= 既存 test fixtures が generated Rust の `init()` を invoke する code) を全 list
  - Hono codebase grep: `grep -rn '\\binit\\s*(' /tmp/hono*` (Hono benchmark target) で 3rd party `init()` call site enumerate
  - e2e test runner: `tests/e2e_test.rs` 内で generated Rust の `init()` を expect する logic 検出 (= 本 PRD で migration が必要な harness boundary)
  - 実施タイミング: Spec stage で audit 開始 (= TS-7 task)、Implementation stage T4 で migration code 実装、T5 で全 hits 0 verify
- **(d) Failure detectability**: 本 PRD 完了後 `cargo run` 失敗 (= 既存 binary が `init()` を call する path が migration されていない compile error or runtime panic) or downstream Hono benchmark の Tier-transition compliance で compile fail 増加。

## Design

### Technical Approach

#### 1. Detection: Executable mode vs Library mode

`Transformer::transform_module` の冒頭に **executable_mode 判定** を追加。

**predicate spec は本 Design section #3 "Top-level execution stmt capture + per-item runtime decision" に hard-code される完全版を参照** (= `is_executable_mode` predicate 全 Stmt variants explicit enumerate per Rule 11 (d-1) self-applied compliance、A1 partition (Stmt::Expr) → true、A3 partition (Decl::Var with side-effect/await init via `has_side_effect_init`) → true、それ以外の全 variants (A0 declarations / A2 Lit init / A4 control-flow / A5a Empty / A5b Debugger / ModuleDecl) → false)。

旧 iteration v5 までの本 section に書かれていた pseudocode は `_ => false` wildcard arm を含み Rule 11 (d-1) self-applied violation だった = iteration v6 minor の High 1 fix で Section #3 に完全 enumerate 版 として移行、本 Section #1 は overview pointer として簡略化 (third-party adversarial re-review (4th round) High 1 fix)。

#### 2. fn main synthesis dispatch (in-scope cells のみ enumerate、Rule 9 (a) 1-to-1 mapping compliant)

iteration v3 で Option β cohesive batch 採用 = Axis C1 全 cells が In Scope migration、dispatch tree の各 leaf は in-scope matrix cells と **1-to-1 対応** (Rule 9 (a) Spec→Impl Dispatch Arm Mapping)。**iteration v4 (third-party adversarial re-review fix)** で:

- 旧 4-tuple match (`is_executable_mode`, `user_main_kind`, `is_async_required`, `has_lit_top_level_const`) を **3-tuple match** (`is_executable_mode`, `user_main_kind`, `has_top_level_await`) に simplify。理由:
  - Critical 1 fix: 旧 dispatch tree は library mode + FnAsync user main の arm で `is_async_required=false` を pattern として claim していたが、`is_async_required = (FnAsync || has_top_level_await)` 定義より cells #5/#25 (FnAsync user main) は `is_async_required=true`、結果 `unreachable!()` panic に fall-through する logical bug が存在 (= dispatch tree axis-tuple ↔ definition mismatch)。
  - Medium 1 fix: 旧 4-tuple match の `has_lit_top_level_const` 次元は A6 (mixed) cells で複数 arm に partition される (cells #71/#72 等が 2 arms に double-claim)、Rule 9 (a) 1-to-1 違反。
  - 構造的 fix: `has_lit_top_level_const` は **per-item runtime decision** (= top-level item iteration 中、各 Decl::Var with Lit init のみ Item::Const として top-level emit、それ以外は MainStmt として fn main capture) に移行。dispatch tree は cell-level dispatch のみを扱う = `(is_exec, kind, has_top_await)` 3-tuple で 80 cells が 1-to-1 mapping。
  - High 1 fix: 旧 dispatch tree の comment cells listing で同 cell # を複数 arms に list していた (= cells #7/#27 を `(false, None, false, _)` arm の comment にも `(false, NonFn, false, _)` arm の comment にも記載) を排除、各 cell は 1 arm only に list。

```rust
// Dispatch dimension (3-tuple、collision detection は dispatch 前に identifier-level で先に reject、
// has_lit_top_level_const は per-item runtime decision に移行 = dispatch dimension 不在):
//   - is_executable_mode: bool (= top-level execution stmt 存在 = A1/A3/A6)
//   - user_main_kind: UserMain { None, FnSync, FnAsync, NonFn, Collision }
//   - has_top_level_await: bool (= Axis C1 partition、Stmt::Expr(Expr::Await) or Decl::Var with Expr::Await init 存在)
//
// Per-item runtime decision (dispatch tree leaf 内で実施、cell-level dispatch とは orthogonal):
//   - 各 top-level item (ModuleItem) に対し:
//     - Decl::Var with Lit init only (= A2 partition 形態) → 既存 path で top-level Item::Const emit (library 互換)
//     - Decl::Var with side-effect / await init → MainStmt::Let / MainStmt::LetAwait に capture
//     - Stmt::Expr / Stmt::Expr(Expr::Await) → MainStmt::Expr / MainStmt::ExprAwait に capture
//     - Stmt::Empty → silent skip
//     - Stmt::Debugger → Tier 2 honest reclassify
//     - Stmt::If/For/While/Try/etc. (control-flow) → Tier 2 honest preserve (本 PRD scope 外)
//
// Collision detection precedence (third-party review High #5 fix): UserMain::Collision arm は
// any (A, C) と互換 = identifier-level reservation invariant (INV-5) で先に reject、
// A-axis dispatch (control-flow / Empty / Debugger) は collision 検出後に reach されない。
// 即ち本 dispatch tree は collision arm を最上位に置く。
//
// Async dispatch trigger (INV-3 (a) integration): is_async_required は本 dispatch tree 内で
// derive される (= user_main is FnAsync || has_top_level_await)、各 leaf 内で is_async_required を
// boolean variable として算出して fn main synthesis (sync vs #[tokio::main] async) を分岐。

match (is_executable_mode, user_main_kind, has_top_level_await) {
    // ===== Collision (B4) は最優先 reject (INV-5、A-axis 先行 invariant) =====
    (_, UserMain::Collision, _) => Tier 2 honest error reclassify
        "`__ts_main` is reserved for transpiler-internal use; user must rename"
        // 全 reachable B4 cells:
        // [matrix # 9 (A0/C0), 19 (A1/C0), 20 (A1/C1), 29 (A2/C0), 39 (A3/C0), 40 (A3/C1),
        //  79 (A6/C0), 80 (A6/C1); A4/A5a/A5b + B4 cells (= matrix # 49/59/69) は本 collision arm が
        //  先行 reject = orthogonality merged (cells 49/59/69 は本 arm + cell 9 wording 共通)]

    // ===== Library mode (no executable trigger; has_top_level_await=false 構造的、A0/A2 本 mode) =====
    (false, UserMain::None, false) => library mode (declarations only emit、no fn main)
        // [matrix # 1 (A0/B0/C0), 21 (A2/B0/C0、Lit init は per-item runtime で top-level Item::Const として emit)]
    (false, UserMain::Fn { is_async: false }, false) => user sync main = fn main directly emit
        // (auto-append convention、TS strict vs Rust convention divergence note in Oracle Observations)
        // [matrix # 3 (A0/B1/C0), 23 (A2/B1/C0、Lit init は per-item runtime で top-level Item::Const + fn main 同居)]
    (false, UserMain::Fn { is_async: true }, false) => user async main = #[tokio::main] async fn main directly emit
        // [matrix # 5 (A0/B2/C0), 25 (A2/B2/C0、Lit init は per-item runtime で top-level Item::Const + tokio::main 同居)]
    (false, UserMain::NonFn, false) => library mode + non-fn preserved
        // [matrix # 7 (A0/B3/C0), 27 (A2/B3/C0、Lit init は per-item runtime で top-level Item::Const + non-fn preserved)]

    // ===== Executable mode + no top-await (sync dispatch unless FnAsync triggers) =====
    (true, UserMain::None, false) => synthesize sync fn main from top-level execution stmts
        // [matrix # 11 (A1/B0/C0), 31 (A3/B0/C0), 71 (A6/B0/C0、A6 cells の Lit init は per-item runtime で top-level const、side-effect init / Stmt::Expr は fn main body capture)]
    (true, UserMain::Fn { is_async: false }, false) => rename user main → __ts_main + sync fn main synthesis + main() substitute
        // [matrix # 13 (A1/B1/C0), 33 (A3/B1/C0), 73 (A6/B1/C0)]
    (true, UserMain::Fn { is_async: true }, false) => rename user async main → __ts_main + #[tokio::main] async fn main synthesis + __ts_main().await call site
        // (FnAsync trigger fires async dispatch even with no top-await)
        // [matrix # 15 (A1/B2/C0), 35 (A3/B2/C0), 75 (A6/B2/C0)]
    (true, UserMain::NonFn, false) => synthesize sync fn main + user non-fn symbol preserved
        // [matrix # 17 (A1/B3/C0), 37 (A3/B3/C0), 77 (A6/B3/C0)]

    // ===== Executable mode + top-await (always async dispatch via Trigger 2) =====
    (true, UserMain::None, true) => synthesize #[tokio::main] async fn main with top-await capture
        // [matrix # 12 (A1/B0/C1), 32 (A3/B0/C1), 72 (A6/B0/C1)]
    (true, UserMain::Fn { is_async: false }, true) => rename user sync main → __ts_main +
        #[tokio::main] async fn main synthesis (sync __ts_main() call non-await wrapping inside async fn main)
        // (cell 14 edge case INV-3 (c): sync user main + top-await cohabitation)
        // [matrix # 14 (A1/B1/C1), 34 (A3/B1/C1), 74 (A6/B1/C1)]
    (true, UserMain::Fn { is_async: true }, true) => rename user async main → __ts_main +
        #[tokio::main] async fn main synthesis + __ts_main().await call site
        // (Trigger 1 + Trigger 2 combined)
        // [matrix # 16 (A1/B2/C1), 36 (A3/B2/C1), 76 (A6/B2/C1)]
    (true, UserMain::NonFn, true) => synthesize #[tokio::main] async fn main + user non-fn symbol preserved
        // [matrix # 18 (A1/B3/C1), 38 (A3/B3/C1), 78 (A6/B3/C1)]

    // ===== 構造的 unreachable arm (exhaustivity for Rule 11 (d-1) compliance) =====
    // (false, _, true): library mode で has_top_level_await=true は構造的不可能
    //   = library mode (is_executable_mode = false) は execution stmt 不在 = Stmt::Expr / Decl::Var
    //     with await init partition 不在 = has_top_level_await trigger 構造的不可能
    //   = `tests/swc_parser_top_level_await_test.rs` 4 tests で AST shape level 構造的 mutual exclusion
    //     を empirical lock-in 済 (Axis A0/A2 + C1 = 25 NA cells)
    //   ⇒ matrix で reachable なし、unreachable!() macro で defensive lock-in
    (false, _, true) => unreachable!("Library mode + has_top_level_await=true is structurally impossible \
        (library mode has no execution stmt = no Stmt::Expr/Decl::Var with await partition; \
        empirically locked-in by tests/swc_parser_top_level_await_test.rs)"),
}
```

**Rule 9 (a) 1-to-1 mapping verification (iteration v4)**: 各 in-scope matrix cell が dispatch tree の **exactly 1 arm** に list されることを以下で structural verify:

| matrix cell # | dispatch arm |
|---|---|
| 1, 21 | (false, None, false) |
| 3, 23 | (false, FnSync, false) |
| 5, 25 | (false, FnAsync, false) |
| 7, 27 | (false, NonFn, false) |
| 9, 19, 20, 29, 39, 40, 79, 80 (+ A4/A5a/A5b-merged 49, 59, 69) | (_, Collision, _) |
| 11, 31, 71 | (true, None, false) |
| 12, 32, 72 | (true, None, true) |
| 13, 33, 73 | (true, FnSync, false) |
| 14, 34, 74 | (true, FnSync, true) |
| 15, 35, 75 | (true, FnAsync, false) |
| 16, 36, 76 | (true, FnAsync, true) |
| 17, 37, 77 | (true, NonFn, false) |
| 18, 38, 78 | (true, NonFn, true) |

各 cell は dispatch tree の 1 arm only に list、Rule 9 (a) compliant。Implementation Stage T3 で本 mapping table を unit test **`test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`** (= third-party adversarial re-review (3rd round) High 5 fix で test fn name を明示) として lock-in:

```rust
// Implementation Stage T3 で実装する unit test の概要:
// 各 in-scope matrix cell について以下を assert:
//   1. cell の (Axis A variant, Axis B variant, Axis C variant) を fixture-derive
//   2. helper で derive される (is_executable_mode, user_main_kind, has_top_level_await) 3-tuple を計算
//   3. dispatch tree の match で expected arm が選択されることを assert
//   4. expected arm の matrix # 列挙に本 cell が含まれることを cross-check
// 本 test が pass = Rule 9 (a) 1-to-1 mapping invariant が runtime で structural lock-in
#[test]
fn test_dispatch_arm_one_to_one_mapping_per_in_scope_cell() {
    let test_cases = [
        // (cell #, axis_a, axis_b, axis_c, expected_arm_id)
        (1, AxisA::A0, AxisB::B0, AxisC::C0, DispatchArm::LibraryNoneSync),
        (3, AxisA::A0, AxisB::B1, AxisC::C0, DispatchArm::LibraryFnSyncDirect),
        (5, AxisA::A0, AxisB::B2, AxisC::C0, DispatchArm::LibraryFnAsyncDirect),
        // ... full 80-cell matrix coverage (40 reachable + 25 NA + 15 orthogonality merged)
    ];
    for (cell, a, b, c, expected_arm) in test_cases {
        let tuple = derive_dispatch_tuple(a, b, c);
        let actual_arm = dispatch_tree_match(tuple);
        assert_eq!(actual_arm, expected_arm, "cell #{cell}: expected {:?}, got {:?}", expected_arm, actual_arm);
    }
}
```

本 unit test は dispatch tree axis-tuple ↔ definition mismatch (= iteration v4 Critical 1 root cause) を **structural detect する regression lock-in** = future iteration で同種 bug が混入した場合 unit test fail で発覚保証。

各 leaf の matrix # 列挙は **本 PRD scope の reachable cells と 1-to-1 対応**。Out of Scope cells (= Axis A4/A5a/A5b cells、Axis C1 NA cells、regression lock-in cells) は本 dispatch tree の leaf に出現せず、別 path で handle:

- **A4 (control-flow at top-level、matrix # 41/43/45/47 + B4-merged 49)**: dispatch tree から除外、`transform_module_item` の `_` arm refactor 後 Stmt::If/For/etc. variant を explicit enumerate して `UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; lift to a named function or use I-203 future expansion", span)` で reject (B4 collision case は本 dispatch tree の最優先 collision arm で先に reject、High #5 fix)。
- **A5a (Stmt::Empty、matrix # 51/53/55/57 + B4-merged 59)**: silent skip + 他 sub-component dispatch、orthogonality merge with cell 51 representative (B-axis variants は cell 1/3/5/7 dispatch と orthogonal compose、B4 collision は最優先 collision arm で reject)。
- **A5b (Stmt::Debugger、matrix # 61/63/65/67 + B4-merged 69)**: Tier 2 honest reclassify "`debugger` statement has no Rust equivalent"、orthogonality merge with cell 61 representative (B4 collision は最優先 collision arm で reject)。
- **NA cells (Axis A0/A2/A4/A5a/A5b + C1 = 25 cells)**: SWC parser empirical lock-in (`tests/swc_parser_top_level_await_test.rs` 4 tests passing) で structural mutual exclusion、本 dispatch tree の input である AST shape 自体が parse stage で別 partition に classified。

**A4 (control-flow at top-level) cells (matrix # 41/43/45/47/49)**: dispatch tree から除外 (= 本 PRD では Tier 2 honest preserve、`transform_module_item` の `_` arm で `UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; lift to a named function or use I-203 future expansion", span)` 経由 reject、source orthogonality merge with cell 41 representative)。

**A5a (Stmt::Empty) cells (matrix # 51/53/55/57)**: dispatch tree から除外 (= silent skip + 他 sub-component dispatch、orthogonality merge with cell 51 representative + B-axis dispatch from cells 3/5/7)。

**A5b (Stmt::Debugger) cells (matrix # 61/63/65/67)**: dispatch tree から除外 (= Tier 2 honest reclassify "`debugger` statement has no Rust equivalent"、orthogonality merge with cell 61 representative)。

#### 3. Top-level execution stmt capture + per-item runtime decision (third-party adversarial re-review (3rd round) Critical New + Medium 1/2 fix で完成)

`transform_module` の loop で per-item dispatch を実施。本 section は v4 で `has_lit_top_level_const` を dispatch tree 4-tuple 次元から削除し per-item runtime decision に移行した structural design の **完全 spec** を hard-code する (= 3rd adversarial review Critical New "Per-item runtime decision spec is incomplete" fix)。

**predicate 仕様 (`is_executable_mode` 判定 + Decl::Var path classification)**:

```rust
/// Module body の top-level item 全体を scan し、A1/A3 形態の execution stmt 存在
/// を判定。本 predicate の return 値が dispatch tree の `is_executable_mode` 次元値。
///
/// **Rule 11 (d-1) compliance (iteration v6 fix)**: Stmt 全 variants を explicit enumerate
/// (`_ => ` arm 不在)、新 SWC Stmt variant 追加時 compile error で全 dispatch fix 強制。
/// 本 PRD Goal #5 の self-applied invariant (= `transform_module_item` の `_` arm refactor
/// と同 standard for `is_executable_mode` predicate)。
fn is_executable_mode(module: &Module) -> bool {
    module.body.iter().any(|item| match item {
        ModuleItem::Stmt(stmt) => match stmt {
            // === A1 partition (Stmt::Expr): execution trigger ===
            Stmt::Expr(_) => true,

            // === A3 partition (Decl::Var with side-effect/await init): runtime check ===
            Stmt::Decl(Decl::Var(var)) => has_side_effect_init(var),
            // A2 partition (Decl::Var with Lit init only) は has_side_effect_init = false で除外

            // === Declarations partition (no execution stmt、type system only) ===
            // Decl::Fn / Decl::Class / Decl::TsInterface / Decl::TsTypeAlias / Decl::TsEnum /
            // Decl::TsModule / Decl::Using: is_executable_mode に contribute しない
            Stmt::Decl(
                Decl::Fn(_) | Decl::Class(_) | Decl::TsInterface(_) | Decl::TsTypeAlias(_)
                | Decl::TsEnum(_) | Decl::TsModule(_) | Decl::Using(_),
            ) => false,

            // === A5a (Stmt::Empty): silent skip target ===
            Stmt::Empty(_) => false,

            // === A5b (Stmt::Debugger): Tier 2 honest reclassify by transform_module_item ===
            // (本 dispatch tree より先行 reject = 本 predicate でも is_executable_mode に
            // contribute しない、A5b cells は dispatch tree leaf に到達しない、Medium 2 fix)
            Stmt::Debugger(_) => false,

            // === A4 partition (control-flow stmts): Tier 2 honest reject by transform_module_item ===
            // 全 control-flow Stmt variants を explicit enumerate per Rule 11 (d-1):
            // (`transform_module_item` の `_` arm refactor 後 explicit enumerate で
            //  `UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; ...", span)`
            //  経由 Tier 2 honest reject、本 predicate でも is_executable_mode に contribute しない、
            //  本 dispatch tree leaf に到達しない、Medium 2 fix)
            Stmt::Block(_) | Stmt::If(_) | Stmt::Switch(_) | Stmt::Throw(_)
            | Stmt::Try(_) | Stmt::While(_) | Stmt::DoWhile(_) | Stmt::For(_)
            | Stmt::ForIn(_) | Stmt::ForOf(_) | Stmt::Labeled(_)
            | Stmt::Continue(_) | Stmt::Break(_) | Stmt::Return(_) | Stmt::With(_) => false,
        },

        // === Module-level declarations (Axis E E1 partition): orthogonal to executable_mode ===
        // ModuleDecl::Import / ExportDecl / ExportNamed / ExportDefaultDecl / ExportDefaultExpr /
        // ExportAll / TsImportEquals / TsExportAssignment / TsNamespaceExport:
        // `pub` modifier preserve は executable_mode dispatch と orthogonal (Axis E orthogonality
        // merge declaration 整合)、is_executable_mode に contribute しない
        ModuleItem::ModuleDecl(_) => false,
        // (注: ModuleItem::ModuleDecl の inner ModuleDecl variant は Rule 11 (d-6) Architectural
        //  concern relevance により本 PRD scope 外、`_` arm 許容。本 PRD は ModuleItem level
        //  dispatch focus、ModuleDecl level の variant exhaustivity は別 PRD I-203 scope)
    })
}

/// Decl::Var の init 形態を分類。本 predicate は `has_lit_top_level_const` per-item runtime
/// decision の core (= v4 で dispatch tree dimension から削除した次元の per-item version)。
///
/// **Init kind classification 仕様** (各 init expression を以下の partition に分類):
/// - `InitKind::Lit`: `Expr::Lit(_)` のみ、または `Expr::Unary(UnaryOp::Minus, Lit(Number/BigInt))`
///   等の compile-time constant expressible form (Rust `const` 適合)。具体的に:
///     - `Expr::Lit(Lit::Num/Lit::Bool/Lit::Str/Lit::Null/Lit::BigInt/Lit::Regex)`
///     - `Expr::Unary { op: UnaryOp::Minus, arg: Expr::Lit(Lit::Num/Lit::BigInt) }`
///     - 上記以外の literal expression は本 PRD scope 外 (= 別 PRD I-016 で Tier 1 化候補)
/// - `InitKind::AwaitInit`: `Expr::Await(_)` (= Axis C1 partition の Decl::Var with await init)
/// - `InitKind::SideEffect`: 上記以外の全 expression (= Call / Ident / New / etc.、Axis A3 partition)
///   None init (`let x;`) は本 PRD scope では発生しない (TS const requires init)
fn classify_init_kind(var: &VarDecl) -> InitKind {
    let first_decl = var.decls.first().expect("VarDecl must have at least 1 declarator");
    match first_decl.init.as_deref() {
        Some(Expr::Lit(_)) => InitKind::Lit,
        Some(Expr::Unary(unary)) if matches!(unary.op, UnaryOp::Minus)
            && matches!(*unary.arg, Expr::Lit(Lit::Num(_) | Lit::BigInt(_))) => InitKind::Lit,
        Some(Expr::Await(_)) => InitKind::AwaitInit,
        Some(_) => InitKind::SideEffect,  // Call / Ident / New / etc.
        None => unreachable!("TS Decl::Var requires init (let/const without init = parse error in strict mode)"),
    }
}

/// `has_side_effect_init` predicate (= `is_executable_mode` の Axis A3 trigger):
/// Lit でも AwaitInit でもない init = 一般 expression evaluation (Axis A3 partition の core 定義)。
/// 本 predicate は AwaitInit を含めて `true` を return する (= top-await Decl::Var も executable
/// trigger、Axis C1 cells が dispatch tree の `is_executable_mode = true` arm に到達することを保証)。
fn has_side_effect_init(var: &VarDecl) -> bool {
    matches!(classify_init_kind(var), InitKind::SideEffect | InitKind::AwaitInit)
}

/// Decl::Var path classification (Library mode vs Executable mode、third-party review H-6 fix)
fn classify_decl_var_path(var: &VarDecl, is_executable_mode: bool) -> DeclVarPath {
    let init_kind = classify_init_kind(var);
    
    match (is_executable_mode, init_kind) {
        // Library mode: 既存 path 維持 (= convert_var_decl_module_level、I-016 owner)
        (false, _) => DeclVarPath::LibraryMode,
        
        // Executable mode + Lit init: top-level const として library 互換 emit
        // (= cell 21/23/25/27 等の A2 partition、A6 mixed のうち Lit init 部分も同 path)
        (true, InitKind::Lit) => DeclVarPath::ToplevelConst,
        
        // Executable mode + side-effect init / await init: fn main body capture path
        (true, InitKind::SideEffect) | (true, InitKind::AwaitInit) => DeclVarPath::FnMainBodyCapture,
    }
}
```

**Per-item iteration spec (transform_module loop、source-order preservation invariant)**:

`transform_module` は module body の `ModuleItem` array を **source-order preserving** で iterate、各 item を以下の dispatch leaf に routing:

| ModuleItem kind | Library mode dispatch | Executable mode dispatch |
|---|---|---|
| `Stmt::Expr(expr)` | (unreachable per `is_executable_mode` definition) | `MainStmt::Expr(convert_expr(expr))` (Expr::Await 含む top-await preserve、`#[tokio::main]` 内で `expr.await;` emission) を `main_stmts` に push (source order) |
| `Stmt::Decl(Decl::Var)` with Lit init | 既存 `convert_var_decl_module_level` path = top-level `Item::Const` emit | `DeclVarPath::ToplevelConst` = 同上 top-level `Item::Const` emit (= source order の中で Lit init Decl::Var のみ抜き出して Rust top-level に hoist、INV-1 source-order 保証は Lit init が runtime side effect 不在のため affected しない) |
| `Stmt::Decl(Decl::Var)` with side-effect init | (unreachable per `is_executable_mode` definition: side-effect init は executable trigger) | `DeclVarPath::FnMainBodyCapture` = `MainStmt::Let { name, init }` を `main_stmts` に push (source order) |
| `Stmt::Decl(Decl::Var)` with await init | (unreachable per AST mutual exclusion: await init は executable trigger + has_top_level_await=true、library mode 構造的不可能) | `DeclVarPath::FnMainBodyCapture` = `MainStmt::LetAwait { name, init }` を `main_stmts` に push、Rust 側 `let v = init.await;` emission (`#[tokio::main] async fn main` 内) |
| `Stmt::Empty` | silent skip (no emission) | silent skip (no emission、本 PRD scope で no-op に統一) |
| `Stmt::Debugger` | `transform_module_item` で先行 Tier 2 reject = `UnsupportedSyntaxError::new("debugger ...", span)` (本 dispatch tree leaf に到達しない、Medium 2 fix) | 同上 |
| `Stmt::If/For/ForIn/ForOf/While/DoWhile/Try/Switch/Throw/Labeled/Block` (control-flow) | `transform_module_item` で先行 Tier 2 reject = `UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; lift to a named function or use I-203 future expansion", span)` (本 dispatch tree leaf に到達しない、Medium 2 fix) | 同上 |
| `Stmt::Decl(Decl::Fn/Class/TsInterface/TsTypeAlias/TsEnum/TsModule)` | 既存 path 維持 (declarations as Rust items) | 同上 (declarations は executable_mode dispatch と orthogonal、source-order preserving in declarations partition) |
| `ModuleDecl::Import/Export*` | 既存 path 維持 (Axis E E1 partition、`pub` modifier preserve) | 同上 (executable mode でも Module export presence は orthogonal、INV-5 整合 = `__ts_main` rename target は private fixed) |

**INV-1 source-order preservation invariant の per-item runtime 適用詳細**:

A6 (mixed) cell の例: TS source `console.log("a"); const X = 1; const c = compute(); console.log("b", X, c);`
- iteration 1: `console.log("a");` → `MainStmt::Expr` を `main_stmts[0]` に push
- iteration 2: `const X = 1;` → Lit init = top-level `Item::Const` emit (Rust 側 `const X: f64 = 1.0;`、main_stmts には push しない、Rust top-level に hoist)
- iteration 3: `const c = compute();` → side-effect init = `MainStmt::Let` を `main_stmts[1]` に push
- iteration 4: `console.log("b", X, c);` → `MainStmt::Expr` を `main_stmts[2]` に push

Rust emission:
```rust
const X: f64 = 1.0;  // hoist to top-level (Lit init partition)
fn main() {
    println!("a");
    let c = compute();
    println!("b {} {}", X, c);
}
```

**INV-1 verification**: Lit init Decl::Var を top-level に hoist しても、Lit init が **runtime side effect 不在** (= compile-time constant expression、no I/O / no mutation) のため、TS module-load semantic と Rust execution semantic は byte-exact equivalent。Lit init 自体の "evaluation order" は spec 上意味を持たない (= compile-time)、TS では module load 時に value を bind するが Rust では const として bind される、両者 stdout に impact なし。

**Stmt::Empty / Decl::Fn 等の declarations partition は source-order に対して silent な item** (= runtime stdout 影響なし)、本 iteration spec では top-level に preserve するか Rust items として emit、INV-1 は満たされる。

**Stmt::Debugger / control-flow は per-item Tier 2 reject の precedence (Medium 2 fix)**: `transform_module` の per-item iteration loop は `transform_module_item(item)` を呼び出す。本 helper 内で control-flow / Debugger variants を explicit enumerate (Rule 11 (d-1) compliance)、該当 variant に対して即時 `Result::Err(UnsupportedSyntaxError::new(...))` return = 本 dispatch tree leaf 到達前に reject。即ち該当 variants が module body に含まれている場合、A6 cells でも本 PRD は **module 全体を Tier 2 honest error として reject** する (= partial silent drop ではなく structural reject、A4 + A6 mixed は Tier 2 reject に absorption)。

**A5a × B compositional invariant probe (Medium 3 fix)**: cell 51 (A5a + B0) representative fixture が cells 53/55/57/59 (A5a + B1/B2/B3/B4) に orthogonality merge する claim を Implementation Stage T3 で probe verify。`test_axis_a5a_compositional_orthogonality_with_b_axis` (新規 unit test) で:
- A5a (Stmt::Empty silent skip) + B1 (sync user main directly emit) = cell 53 expected output
- A5a + B2 (async user main directly emit) = cell 55 expected output
- A5a + B3 (non-fn preserved) = cell 57 expected output
- A5a + B4 (collision) = cell 59 expected output (= INV-5 collision priority arm が dispatch tree で先行 reject)

各 expected output が representative fixture cell-27a + B-axis dispatch leaf 出力の orthogonal composition と一致することを assert。

#### 4. User main rename + main() substitution

User function `main` (B1/B2、function decl / arrow / fn expr) detection 後:
- declaration を `Item::Fn { name: "__ts_main", ... }` に rename emit
- `transform_module` の expression conversion path で `Expr::Call { callee: Ident("main"), args }` を `Expr::Call { callee: Ident("__ts_main"), args }` に substitute (= 全 user-side `main()` call site が __ts_main() を call)

#### 5. Async dispatch synthesis

`is_async_required` true の場合、fn main 自体を `#[tokio::main]` async fn main として emit。Sync user main (B1) を async fn main 内から call する case (= **cell 14 = A1+B1+C1**、third-party adversarial re-review (3rd round) High 4 fix で旧 numbering "cell 15" の stale reference を新 matrix numbering "cell 14" に訂正; cells 34/74 = A3/A6+B1+C1 も同 sub-case = INV-3 (c) edge sub-case) は user main = sync `__ts_main` のまま、async fn main から非 await の sync call で invoke。

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

**`__ts_` namespace reservation 拡張対象** (I-154 source、empirical file path 2026-05-01 confirm):

- `src/transformer/expressions/mod.rs:57-98`: 既存 `TS_OLD_BINDING` / `TS_NEW_BINDING` / `TS_RECV_BINDING` constants 定義 + I-154 namespace reservation rationale doc comment。本 PRD で **`TS_MAIN_RENAME: &str = "__ts_main"`** constant 追加 + doc comment で B2 architectural concern (= user main rename target identifier) を記録
- `src/transformer/statements/mod.rs:39-48`: 既存 `check_ts_internal_label_namespace(label: &ast::Ident) -> Result<()>` validator (= label-level `__ts_` 衝突検出、Tier 2 honest error reject)。本 PRD で **`check_ts_internal_fn_name_namespace(fn_name: &str, span: Span) -> Result<()>`** 新規 validator 追加 (= function name-level `__ts_main` 衝突検出、symmetric structural enforcement で cells 5/13 実装)
- `src/transformer/main_synthesis.rs` (新規): user main rename + collision detection invocation site
- (validation invocation sites): `transform_module` / `transform_module_collecting` で user-defined function `main` detection 時に `check_ts_internal_fn_name_namespace` を invoke、collision なら Tier 2 honest error。さらに全 user identifier validation path (= 既存 user-side `__ts_X` 衝突検出 logic 該当箇所、本 PRD で empirical 拡張 audit) で `__ts_main` reserved を含めて validate

### Semantic Safety Analysis

**Required**: 本 PRD は型 fallback 導入を含まない (= `__ts_main` rename は identifier-level rename で型 system 関与なし、fn main synthesis は IR レベルの structural emission)。型 resolution 変更なし。

**判定**: Not applicable — no type fallback changes。

ただし silent semantic change の risk audit は別軸で実施 (= INV-1 によって TS / Rust execution order の byte-exact match を verify、INV-2 によって user `main` symbol substitution の completeness を verify)。これは型 fallback ではなく **execution semantic preservation** で本 PRD architectural concern の primary objective。

### TypeResolver impact (third-party review R-3 fix、INV-6 cross-reference)

本 PRD の architectural changes は TypeResolver layer (= `src/pipeline/type_resolver/` モジュール) の type resolution flow を **affecting しない**。具体的に:

- **Identifier rename (`main` → `__ts_main`)**: TypeResolver pipeline phase の **後段 (post-resolution AST transform)** で実施。TypeResolver は `Module` AST を input として user-defined `main` を function symbol として正しく resolve、本 PRD の rename は TypeResolver output 後の AST に identifier rewrite を後付け。TypeResolver phase 内では `main` は user identifier のまま (table key も user-source-text 基準で保持)。
- **`fn main` synthesis**: 新規 IR Item を Transformer phase で生成、TypeResolver は触らない。Synthesis 内の expression は user-source-derived な already-resolved type info を transitively reuse、新規 type resolution は不要。
- **Decl::Var dual-path dispatch**: executable mode / library mode の判定は **module body composition (top-level item kinds)** から派生、type information は不要。TypeResolver は input AST の form-level shape に基づき type resolution を実施、本 PRD の dispatch は TypeResolver 出力後の post-resolution AST を category-dispatch するため orthogonal。

INV-6 で structural verify (= `cargo test --lib pipeline::type_resolver::` 全 pass + Implementation stage T2 着手前に empirical probe で TypeResolver field を touch しない code review)。

### `__ts_main` user-code collision audit (third-party review R-4 fix、INV-5 cross-reference)

INV-5 で `__ts_main` reservation rule の structural enforcement を保証するが、**既存 codebase / Hono / 既存 e2e test に user-defined `__ts_main` identifier が存在しないか** の empirical audit が必要 (= reachable なら本 PRD で Tier 2 reject すべき新 errors が surface、Hono bench Tier-transition compliance 観点)。

**Audit method (Spec stage で実施、TS-7 task)**:

1. `grep -rn '__ts_main' /home/kyohei/ts_to_rs/src/ /home/kyohei/ts_to_rs/tests/ /home/kyohei/ts_to_rs/tools/` で internal codebase を full scan
2. `grep -rn '__ts_main' /tmp/hono*` で Hono benchmark target を full scan (= Hono 内 user identifier として `__ts_main` 出現 0 件 verify)
3. 既存 e2e fixture (`tests/e2e/scripts/`) 内 user-defined `__ts_main` は本 PRD の test fixtures (cells 9/19/20) を除き 0 件 verify

**Expected**: 0 hits (= Hono / Bench target / 一般 user code で `__ts_main` 識別子 use case は事前 reservation rule (I-154 既存 `__ts_old`, `__ts_new`, `__ts_recv`) と同 conventional reservation pattern)。1 hit でも detect なら本 PRD で migration 対応 task を別 起票 + 本 PRD scope 拡張検討。

## Spec Stage Tasks (Rule 5 (5-2) 適用、Stage 1 artifacts 完成 task)

### TS-0: Cartesian product matrix completeness (iteration v3 で 80 cells full Cartesian に拡張)

- **Work**: Problem Space Cartesian product matrix を 80 cells (= Axis A 8 × Axis B 5 × Axis C 2、Axis E orthogonality merge declaration) に完全 enumerate、全 cell に判定 (✓/✗/NA/regression lock-in/Tier 2 reclassify/orthogonality merged) 付与、abbreviation pattern 排除、orthogonality merge cells には source cell # 明示 (Rule 1 (1-4))
- **Completion criteria**: matrix table 内 `...` / range grouping / placeholder 不在、全 80 cell 独立 row、orthogonality merge declarations が Rule 1 (1-4-a/b/c) compliant、`audit-prd-rule10-compliance.py backlog/I-224-top-level-fn-main-mechanism.md` PASS
- **Status**: iteration v3 で 80 cells matrix 完成、Cell 27 split (A5a / A5b)、Cell 31 INV-2 sub-case integration、Axis E orthogonality merge declaration 含む全 third-party review C-1/C-4/H-7/M-2 fix を内包

### TS-1: Oracle observation log embed (iteration v3 で in-scope ✗ cells 拡張)

- **Work**: 各 ✗ / 要調査 cell について TS fixture 作成、`scripts/observe-tsc.sh` (`--esm --no-auto-main` for Spec stage fidelity) 実行、PRD doc `## Oracle Observations` section に embed
- **Completion criteria**: 全 in-scope ✗ cells (representative + orthogonality-merged source cells) について 4 項目 (TS fixture path / tsc output / matrix cell # link / ideal output rationale) 記載、`audit-prd-rule10-compliance.py` で section 不在 audit fail 排除
- **Status**: iteration v3 で in-scope ✗ cells のうち structural orthogonality merge 適用済 cells を除く representative cells (matrix # 9/11/12/13/14/15/16/17/18/19/20/31/33/35/37/51/61/71/73/76 の 20 cells) で empirical record 完了。NEW cells (matrix # 32/34/36/38/40/41/72/74/75/77/78/79/80) は Implementation Stage TS-3 で empirical record + green-ify と並行

### TS-2: SWC Parser Empirical Lock-in tests for Axis A vs C1 mutual exclusion (iteration v3 で本 PRD scope に integrate、third-party review C-2 fix)

- **Work**: `tests/swc_parser_top_level_await_test.rs` 新規 file 作成、`crate::parser::parse_typescript()` 直接呼びで NA cells (Axis A0/A2/A4/A5a/A5b + C1 = 25 cells) の AST 構造的 mutual exclusion を empirical lock-in。具体的 4 tests:
  - `test_top_level_bare_await_parses_as_stmt_expr_await_axis_a1`: `await x;` → A1 partition
  - `test_top_level_var_decl_with_await_init_parses_as_decl_var_axis_a3`: `const x = await y;` → A3 partition
  - `test_pure_axis_a0_source_contains_no_await_expression`: pure A0 source に Expr::Await 不在
  - `test_axis_c1_implies_a1_or_a3_partition_synthesis`: C1 forms 全て A1 / A3 collapse
- **Completion criteria**: 4 tests passing (`cargo test --test swc_parser_top_level_await_test` PASS)、PRD doc `## SWC Parser Empirical Lock-ins` section に test fn list 記載
- **Status**: iteration v3 で完成、4 tests passing (2026-05-01)、Rule 3 (3-2) hard violation を本 PRD scope 内 fix 完了

### TS-3: E2E fixture creation (red 状態 lock-in)

- **Work**: 各 in-scope ✗ cell に対応 `tests/e2e/scripts/i-224/cell-NN-*.ts` fixture 作成、`scripts/record-cell-oracle.sh` で expected output 記録 (red 状態 = ts_to_rs 出力と expected 不一致)。iteration v3 で Option β cohesive batch により Axis C1 cells (cells 14-18/30) の fixture 復元 + ESM mode oracle empirical record 完了。NEW cells (matrix # 32/34/36/38/40/41/72/74/75/77/78/79/80) は Implementation stage で coding と並行作成
- **Completion criteria**: 全 in-scope ✗ cells 用 fixture が `cargo test --test e2e_test` で red 状態 (= Implementation stage T1-T9 完了で green 化予定)

### TS-4: Impact Area audit findings record

- **Work**: `python3 scripts/audit-ast-variant-coverage.py --files src/transformer/mod.rs src/transformer/functions/arrow_fns.rs --verbose` 実行、結果を PRD doc `## Impact Area Audit Findings` section に完成、各 violation の決定 (本 PRD scope or I-203 defer) 記録、Empirical file path verify (impact area path strings の "or 該当" 等 uncertain expression を empirical confirm し PRD doc update)
- **Completion criteria**: 全 violations 列挙 + 決定記載、Empirical file path verify 完了 (= PRD doc 内 path strings が empirical 確認済 file/line/function に correspond)
- **Status**: iteration v2 完成、iteration v3 では追加更新なし

### TS-5: Test harness ESM upgrade design + spec stage trial (iteration v3 新規、Option β cohesive batch infra)

- **Work**: 
  - `scripts/observe-tsc.sh --esm` flag 設計 + iteration v3 で trial implementation (= temp dir に `package.json {"type":"module"}` 配置して tsx の ESM mode 起動)
  - `scripts/observe-tsc.sh --no-auto-main` flag 追加 (Spec stage oracle observation fidelity)
  - Implementation stage T7 で permanent integration (= CI 化、`scripts/observe-tsc.sh` の default 動作 review)
  - PRD doc `## Design > Test harness ESM mode` sub-section に design 記載 (= ESM mode trigger condition / fallback / regression risk)
- **Completion criteria**: trial implementation 動作確認 (cells 14-18/30 oracle empirical record 取得済、4 tests passing)、Implementation stage T7 task spec 確定
- **Status**: iteration v3 で trial implementation 完了 (`scripts/observe-tsc.sh` line 32-72 + line 138-145 改修済 2026-05-01)

### TS-6: Top-level await Tier 1 synthesis spec (iteration v3 新規、Option β cohesive batch transpiler-side spec)

- **Work**: 
  - Top-level await capture into `#[tokio::main] async fn main()` body の synthesis logic spec (= INV-3 wording revise で Trigger 2 = C1 を sync/async dispatch 条件に追加)
  - `Decl::Var with Expr::Await init` の `let v = init.await;` への変換 spec (= cells 32/34/36/38)
  - `Stmt::Expr(Expr::Await)` の `expr.await;` への変換 spec (= cells 12/14/16/18 等)
  - Sync user main + top-await mixed case の non-await call wrapping spec (= cell 14、INV-3 (c) verification)
- **Completion criteria**: PRD doc `## Design > 2. fn main synthesis dispatch` 内に C1 trigger leaf 全 enumerate、Implementation stage T8 task spec 確定
- **Status**: iteration v3 で完了 (Design section dispatch tree が Axis C1 in-scope leaves を全 enumerate、INV-3 wording revise 済)

### TS-7: `__ts_main` user-code collision audit + `pub fn init` external API audit (iteration v3 新規、third-party review R-2 + R-4 fix)

- **Work**: 
  - **R-4 audit**: `grep -rn '__ts_main' src/ tests/ tools/ /tmp/hono*` で codebase + Hono の user-defined `__ts_main` identifier 0 hits を empirical verify (= INV-5 reachability prerequisite)
  - **R-2 audit**: `grep -rn '\\binit\\s*(' src/ tests/ tools/` で internal codebase の `init()` call site enumerate + `grep -rn '\\binit\\s*(' /tmp/hono*` で Hono codebase enumerate + `tests/e2e_test.rs` runner の `init()` invocation logic 検出 (= INV-7 verification 用 baseline)
  - **Audit script `scripts/audit-no-pub-fn-init.sh`** 新規作成 (M-3 fix、本 PRD では作成 task を T4 から TS-7 spec 段階に整合)、Implementation stage T6a で CI integrate
- **Completion criteria**: 
  - audit 結果を PRD doc 末尾 `## Pre-Implementation Audit Findings` section に embed
  - 0 hits なら本 PRD scope で migration task 不要、>0 hits なら本 PRD scope 拡張または別 PRD 起票検討 (user 判断)
  - `scripts/audit-no-pub-fn-init.sh` script 完成 (本 task 内、内容は `grep -P '\\bpub\\s+fn\\s+init\\b' tests/e2e/rust-runner/ tools/extract-types/output/` 等の codebase + generated output area scan)
- **Status**: iteration v3 で execute 予定 (本 spec stage の最終 step、audit 結果 embed 後 self-review v3 完了)

## Implementation Stage Tasks

(TDD 順: RED → GREEN → REFACTOR、Spec stage 完了 + user 承認後着手。iteration v3 Option β cohesive batch で T7/T8/T9 追加 + T6 split per third-party review H-8。Tier-transition compliance = broken-fix PRD)

### T1: `__ts_` namespace reservation extension + collision detection

- **Work**: I-154 の `__ts_` reserved list に `__ts_main` 追加 (= `src/transformer/expressions/mod.rs:57-98` に `TS_MAIN_RENAME: &str = "__ts_main"` constant 追加)、user identifier validation で `__ts_main` を reject (= `src/transformer/statements/mod.rs:39-48` 参照の既存 `check_ts_internal_label_namespace` validator と symmetric な `check_ts_internal_fn_name_namespace` 新規追加)、matrix # 9/19/20 用 `UnsupportedSyntaxError::new("`__ts_main` is reserved for transpiler-internal use; user must rename", span)` emission path 追加
- **Completion criteria**: I-154 namespace test 拡張で `__ts_main` reserved verify、matrix # 9/19/20 fixture が Tier 2 honest error reject 出力 + collision-merged cells 29/39/40/49/59/69/79/80 で同 dispatch path 共通 invariant 確認 (third-party adversarial re-review (3rd round) High 3 fix で cell 40 を本 list に追加)
- **Depends on**: TS-1〜TS-7

### T2: `MainStmt` IR + `UserMainKind` enum + `collect_top_level_executions` helper

- **Work**: 新 `MainStmt` enum (variants: Expr (sync) / ExprAwait (top-await Stmt::Expr) / Let / LetAwait (top-await Decl::Var) / Debugger reclassify error)、`UserMainKind` enum (None / FnSync / FnAsync / NonFn / Collision)、`collect_top_level_executions(module: &Module) -> (Vec<MainStmt>, UserMainKind, IsAsyncRequired)` shared helper を新規 module `src/transformer/main_synthesis.rs` に実装。Decl::Var dual-path classifier `classify_decl_var_path(var: &VarDecl, is_executable_mode: bool) -> DeclVarPath` (= LibraryMode / ToplevelConst / FnMainBodyCapture) も同 module 内
- **Completion criteria**: helper unit test (= 80 cell input variation の representative cells × expected (MainStmt vec, UserMainKind, is_async_required) tuple、orthogonality-merged cells は representative dispatch verify)、INV-6 verify (= TypeResolver layer touch なし)、INV-3 sync/async dispatch トリガー条件の boundary value test (= Trigger 1 (B2) only / Trigger 2 (C1) only / Trigger 1+2 combined / no trigger)
- **Depends on**: T1

### T3: fn main synthesis + user main rename + main() substitution + Axis B/E orthogonality probe

- **Work**: `Transformer::synthesize_fn_main(main_stmts: Vec<MainStmt>, user_main: UserMainKind, is_async: bool) -> Vec<Item>` 実装、user main rename (B1a/B1b/B1c forms 全 → `__ts_main` 変名、Axis B B1 orthogonality merge legitimacy lock-in)、convert_expr の Call arm に `Ident("main")` → `Ident("__ts_main")` substitute logic 追加 (Transformer state field `user_main_substitution: bool`、async case では `__ts_main().await` への substitute)。Axis E orthogonality probe `test_axis_e_export_preserve_symmetric` も追加 (= representative cells 11/13/31 から E1 form を probe で `pub` modifier preserve verify)
- **Completion criteria**: 
  - representative in-scope cells (matrix # 11-20, 31-40, 71-80 のうち non-orthogonality-merged) の dispatch logic を unit test で verify (cell-by-cell の expected IR token-level assert)
  - `test_axis_b_b1a_b_c_rename_dispatch_symmetric` (B1 3 forms 全 → `__ts_main` rename + main() substitute symmetric) 追加
  - `test_axis_e_export_preserve_symmetric` (E1 form で `pub` modifier preserve) 追加
  - Multi-call boundary value test (= cell-31 fixture probe で全 call sites substituted) (INV-2 verification)
- **Depends on**: T2

### T4: `transform_module` / `transform_module_collecting` refactor + `pub fn init` 廃止

- **Work**: `transform_module` / `transform_module_collecting` の logic を T2 helper + T3 synthesis 経由に refactor、`init_stmts` → `main_stmts` rename、`build_init_fn` 削除、`build_main_fn` 新規追加。`transform_module_item` の `_ => Err` を expand (ModuleItem 全 variant explicit enumerate、Rule 11 d-1 compliance)、A4 (control-flow) cells で wording 改善 (`UnsupportedSyntaxError::new("ControlFlow at top-level requires fn main wrapping; lift to a named function or use I-203 future expansion", span)`)、A5b (Debugger) cells で wording 改善 (`UnsupportedSyntaxError::new("`debugger` statement has no Rust equivalent (= compile-time `panic!()` or `std::dbg!()` を user 自身で選択)", span)`)
- **Completion criteria**: cargo test 全 pass (`pub fn init` 言及の test は新 form に migrate)、`audit-ast-variant-coverage.py --files src/transformer/mod.rs` で `_` arm violation 0 件 (本 PRD scope file)、CI script `scripts/audit-no-pub-fn-init.sh` で codebase 0 hits
- **Depends on**: T3

### T5: E2E fixture green-ify + NEW fixtures creation + I-205 cell-09 unblock

- **Work**: 
  - TS-3 で red 状態だった既存 fixture (i-224 配下) を green 化
  - NEW fixtures (cells 32/34/36/38/40/41/72/74/75/77/78/79/80) を `tests/e2e/scripts/i-224/cell-NN-*.ts` で作成 + `scripts/record-cell-oracle.sh --esm --no-auto-main` (or appropriate flags) で expected output 記録
  - I-205 cell-09 (static-only、本 PRD のみ依存) を green 化、`#[ignore]` 解除
  - Tier-transition compliance verify (= existing Tier 2 errors transition Tier 1 = improvement、no new compile errors)
- **Completion criteria**: `cargo test --test e2e_test` 全 pass (本 PRD scope cells)、Hono bench Tier-transition compliance ("Improvement" or "Preservation" 結果、新 compile errors 0 件)、cell-09 の `#[ignore]` 解除
- **Depends on**: T4

### T6a: I-154 namespace doc + audit script CI integration (B2 scope、third-party review H-8 fix で T6 split)

- **Work**: 
  - I-154 namespace doc に `__ts_main` 追記 + reservation rationale (= 本 PRD source) 記載
  - `scripts/audit-no-pub-fn-init.sh` (TS-7 で新規作成) を CI workflow `.github/workflows/ci.yml` に integrate
- **Completion criteria**: I-154 doc update PR、CI step 追加 PR、`scripts/audit-no-pub-fn-init.sh` が CI で 0 hits invariant lock-in
- **Depends on**: T5
- **Note**: 旧 T6 に含まれていた `audit-prd-rule10-compliance.py` reinforce task は本 task から除外、framework rule integration は **別 PRD I-D scope** へ migrate (= R-1 + R-5 と統合、Rule 1/12 framework 改善 candidate)

### T7: Test harness ESM upgrade permanent integration (Option β cohesive batch infra)

- **Work**: 
  - TS-5 trial implementation を CI 化 (= `scripts/observe-tsc.sh --esm --no-auto-main` を CI workflow から正式 invoke)
  - `tests/e2e/rust-runner/Cargo.toml` に tokio runtime 依存追加 (= `tokio = { version = "1", features = ["macros", "rt-multi-thread"] }` 等、`#[tokio::main]` macro 用)
  - `tests/e2e_test.rs` runner template を ESM-mode に拡張 (= top-await を含む Rust binary を build / cargo run で execute、tokio runtime context で正しく実行)
- **Completion criteria**: 
  - `tests/e2e/rust-runner/` で cells 12/14/16/18/20/32/34/36/38/40/72/74/76/78/80 (Axis C1 in-scope cells) の Rust 出力が `#[tokio::main] async fn main()` で wrap、cargo run 成功 + tsc stdout と byte-exact match
  - CI で `--esm` mode が default for top-await fixtures (cells 14-18/30 + NEW Axis C1 cells)
- **Depends on**: T6a

### T8: Top-level await synthesis logic implementation (Option β cohesive batch transpiler)

- **Work**: 
  - INV-3 wording revise を実装 (= sync/async dispatch trigger を `is_user_main_async || has_top_level_await` に拡張)
  - `Stmt::Expr(Expr::Await)` capture into `MainStmt::ExprAwait` IR variant、Rust 側 `expr.await;` emission
  - `Decl::Var with Expr::Await init` capture into `MainStmt::LetAwait` IR variant、Rust 側 `let v = init.await;` emission
  - Sync user main + top-await mixed case の non-await call wrapping (= cell 14 で sync `__ts_main()` を async fn から非 await call で invoke、INV-3 (c) edge case verification)
  - cells 12/14/16/18/20 + 32-40 + 72-80 (Axis C1 in-scope cells) の dispatch logic 完成
- **Completion criteria**: 
  - Axis C1 in-scope cells の unit test pass (T2 helper unit test 拡張)
  - INV-3 (c) 4 sub-cases (Trigger 1 only / Trigger 2 only / Trigger 1+2 combined / no trigger) full coverage
- **Depends on**: T7

### T9: Axis C1 cells e2e fixture green-ify (Option β cohesive batch verification)

- **Work**: 
  - 既存 fixture cells 14-18/30 (旧 numbering、新 matrix # 12/14/16/18/20/76) の e2e green-ify
  - NEW fixtures (matrix # 32/34/36/38/40/72/74/78/80) の e2e green-ify
  - Tier-transition compliance: 全 Axis C1 cells が pre-PRD broken (compile fail in cjs context) → post-PRD Tier 1 (compile-pass + tsc runtime stdout 一致 in ESM mode)
- **Completion criteria**: 
  - `cargo test --test e2e_test` で全 Axis C1 in-scope fixtures green
  - Hono bench Tier-transition compliance verify (Hono 内 top-await 使用 reachability TBD、empirical scan で 0 件確認 or improvement)
- **Depends on**: T8

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

### Iteration v2 (2026-05-01、TS-1〜TS-4 完了後 self-applied verify)

**主要進捗 (本 iteration で resolve)**:

1. **TS-3 + TS-1 batch 完了**: 20 fixtures (i-224 配下) 作成、`scripts/record-cell-oracle.sh` で oracle observation 全 record。`## Oracle Observations` section に in-scope 14 cells (5/9/10/11/12/13/21/22/23/24/27b/28/29/31) の empirical record (stdout / stderr / exit_code) embed 完成。
2. **Axis C scope narrowing + I-226 起票** (Critical Spec gap discovery during TS-1): `scripts/observe-tsc.sh` の tsx + cjs combination が top-level await を runtime reject (`Top-level await is currently not supported with the "cjs" output format`)、cells 14-18/30 (Axis C1 ✗ cells) + cells 6/7/8 (Axis C1 NA cells) を **本 PRD scope から Out of Scope = I-226 (test harness ESM support + top-level await Tier 1 化 cohesive batch)** に narrow。1 PRD = 1 architectural concern 厳格適用 = test harness 改修 (test infra concern) と fn main mechanism (transpiler concern) を 分離。
3. **TS-2 → I-226 defer**: NA cells 6/7/8 SWC parser empirical lock-in test 作成は I-226 spec stage で Axis C 全 cells と cohesive batch で実施。本 PRD では spec-traceable NA reason のみ record。
4. **TS-4 完了**: `## Impact Area Audit Findings` section の violations table 完成、各 violation の本 PRD scope or I-203 / I-016 defer 決定 spec-traceable record。Empirical file path verify 完了 = `src/transformer/expressions/mod.rs:57-98` (`__ts_` namespace constants source)、`src/transformer/statements/mod.rs:39-48` (`check_ts_internal_label_namespace` 既存 validator、B2 で symmetric `check_ts_internal_fn_name_namespace` 追加) 等 empirical file/line/function correspond 確認済。
5. **新 PRD I-226 TODO 起票**: `TODO` の Tier 1 ゲートイシュー sub-section に I-226 entry 追加、defer scope (= 本 PRD cells 14-18/30 + 6/7/8) を spec-traceable record。
6. **In-scope cells 縮減 + 整合**: matrix 31 cells のうち in-scope = 14 ✗ cells (5/9/10/11/12/13/21/22/23/24/27b/28/29/31) + 6 regression lock-in cells (1/2/3/4/19/20) = **20 cells (本 PRD architectural concern boundary)**、Axis C1 全 11 cells (6/7/8/14/15/16/17/18/30) は Out of Scope = I-226。

**13-rule self-applied re-verify (iteration v2)**:

| Rule | Sub-rule check | iteration v1 verdict | iteration v2 verdict | Notes |
|---|---|---|---|---|
| 1 | (1-1) 全 cell ideal output | ✓ | ✓ | 31 cells 全 enumerate (in-scope + Out of Scope = I-226 含む) |
| 1 | (1-2) abbreviation pattern 不在 | ✓ | ✓ | 維持 |
| 1 | (1-3) audit script PASS | (TS-4 で実施) | ✓ | `audit-prd-rule10-compliance.py` PASS confirmed (2026-05-01 iteration v2) |
| 1 | (1-4) Orthogonality merge legitimacy | ✓ | ✓ | Axis B B1=function-decl/const-arrow/const-fn-expr orthogonality merge は dispatch-equivalent (本 PRD では 3 forms 全 sync user main として treat、TS-1 / Implementation Stage で empirical verify) |
| 2 | (2-1) Oracle grounding cross-reference | ✓ partial | ✓ full | 全 in-scope ✗ cells (14 件) で oracle grounding embed |
| 2 | (2-2) `## Oracle Observations` section embed | partial | ✓ | 14 cells 全 record embed (empirical 2026-05-01) |
| 2 | (2-3) audit script verify | (TS-4 で実施) | ✓ | audit script PASS |
| 3 | (3-1) NA spec-traceable | ✓ | ✓ | NA cells 6/7/8 spec-traceable reason 維持、I-226 で empirical SWC parser lock-in 実施予定 (Axis C cohesive batch) |
| 3 | (3-2) SWC parser empirical observation | partial | ✓ deferred to I-226 | 本 PRD scope で Axis C 全 cells を I-226 へ defer (= test harness ESM upgrade と cohesive batch) のため、NA cells 6/7/8 SWC parser empirical も I-226 scope = 1 PRD = 1 architectural concern 厳格適用 |
| 3 | (3-3) SWC accept → Tier 2 reclassify | ✓ | ✓ | I-226 spec stage で実施 |
| 4 | (4-1) reference doc 整合 | ✓ | ✓ | 維持 |
| 4 | (4-2) doc-first dependency order | N/A | N/A | 本 PRD は `doc/grammar/*` 改修なし |
| 4 | (4-3) audit verify | N/A | N/A | 上記 |
| 5 | (5-1) E2E fixture 準備 | partial | ✓ | TS-3 完了 (20 fixtures 作成、in-scope cells 全 cover、Out of Scope cells は I-226 へ migrate) |
| 5 | (5-2) `## Spec Stage Tasks` + `## Implementation Stage Tasks` 2-section split | ✓ | ✓ | 維持 |
| 5 | (5-3) Spec stage tasks に code 改修不在 | ✓ | ✓ | 維持 (TS-2 が I-226 defer に変更されたが code 改修は本 PRD で発生せず) |
| 5 | (5-4) audit verify | ✓ | ✓ | audit PASS、Spec Stage Tasks / Implementation Stage Tasks 2-section split section unrecognized fail 排除 |
| 6 | (6-1) Matrix Ideal output ↔ Design token-level 一致 | ✓ | ✓ | Design section #2 dispatch tree が in-scope cells (Axis C0 のみ) と corresponds、Axis C1 cells は Design tree から削除して整合 |
| 6 | (6-2) Scope 3-tier hard-code | ✓ | ✓ | In Scope / Out of Scope (I-226 defer 明示) / Tier 2 honest error reclassify 3 sub-section 維持 |
| 6 | (6-3) matrix Scope 列値 | ✓ | ✓ | Scope 列値が `本 PRD scope` / `regression lock-in` / `Tier 2 honest reclassify` / `Out of Scope = 別 PRD I-226 defer` 択一 |
| 6 | (6-4) Scope ↔ matrix cross-reference consistency | partial | ✓ | iteration v2 で Scope 3-tier section と matrix Scope 列が token-level で consistent (= cells 14-18/30 が両 section で I-226 defer と correspond) |
| 7 | Control-flow exit sub-case completeness | N/A | N/A | 維持 |
| 8 | (8-5) `## Invariants` 独立 section | ✓ | ✓ | INV-3 wording を v2 で update (= top-level await trigger を Out of Scope 削除、async dispatch trigger を Axis B B2 のみに narrow) |
| 9 | (a) Spec → Impl Dispatch Arm Mapping | partial | ✓ | Design section dispatch tree が in-scope dispatch leaves と 1-to-1 mapping (Axis C 削除済) |
| 9 | (b) Impl → Spec | N/A | N/A | Implementation stage で発動 |
| 9 | (c) Field-addition symmetric audit | N/A | N/A | 本 PRD は IR struct field 追加なし |
| 10 | Cross-axis matrix completeness | ✓ | ✓ | Axis A (top-level body) × Axis B (user main) を完全 enumerate、Axis C は Out of Scope だが matrix 内記載 維持 (= I-226 へ migrate 際の cohesive boundary 明示) |
| 11 | (d-1) `_ => ` 全廃 | partial | ✓ | `transform_module_item:449` `_ => Err` は T4 で全 ModuleItem variant explicit enumerate に refactor、`arrow_fns.rs:42` / `mod.rs:666` は I-016 / I-203 defer 維持 |
| 11 | (d-2) phase 別 mechanism | ✓ | ✓ | 維持 |
| 11 | (d-3) `ast-variants.md` single source of truth | ✓ | ✓ | 維持 |
| 11 | (d-4) audit script CI | (TS-4 で実施) | ✓ | audit-prd-rule10-compliance.py PASS confirmed |
| 11 | (d-5) Pre-draft audit | ✓ | ✓ | `## Impact Area Audit Findings` section 完成 (TS-4) |
| 11 | (d-6) Architectural concern relevance | ✓ | ✓ | 維持 (= `mod.rs:449` 本 PRD scope、`arrow_fns.rs:42` / `mod.rs:666` orthogonality declared = I-016 / I-203 defer) |
| 12 | Rule 10/11 Mandatory + structural | ✓ | ✓ | `## Rule 10 Application` section + audit script PASS |
| 13 | (13-1) skill workflow Step 4.5 | ✓ | ✓ | 本 iteration log 自身 |
| 13 | (13-2) `## Spec Review Iteration Log` record | ✓ | ✓ | iteration v1 + v2 record |
| 13 | (13-3) Critical findings 全 fix | partial | ✓ | iteration v2 で全 High findings (TS-1〜TS-4 + Axis C scope narrowing + I-226 起票) resolve、Critical = 0 状態維持 |
| 13 | (13-4) audit verify | (TS-4 で実施) | ✓ | audit PASS |
| 13 | (13-5) Self-applied integration | N/A initial | partial | iteration v2 で発見の framework 改善 candidate (= `scripts/observe-tsc.sh` の test harness ESM upgrade rule への structural integration、本 PRD I-226 起票 + I-D framework rule integration へ反映候補) を本 PRD close 時 collect |

**Iteration v2 findings count (self-claim)**: Critical = 0、High = 0、Medium = 0、Low = 0 (全 Spec Stage Tasks resolve、13-rule self-applied verify 全項目 ✓)。

**Iteration v2 Spec stage 完了判定 (self-claim、retracted by iteration v3)**: ~~Spec stage approved~~ → 第三者 `/check_job` review (skill invoke、2026-05-01) で **真の Critical = 4 + High = 8 + Medium = 4 + Review insights = 5 (= 計 21 件 actions)** が発見、Spec stage approval 不適切と判定 → **iteration v3 必要**。詳細は `report/I-224-spec-stage-v3-review-handoff.md` 参照。

**Framework 改善 candidate (本 PRD close 時 I-D へ integrate 候補)**:

- (改善 v2-1) `scripts/observe-tsc.sh` の test harness 制約 (= tsx + cjs での top-level await reject) を spec stage で empirical 検出する rule 追加候補。**iteration v3 で resolve** (= `--esm --no-auto-main` flag 追加、Spec stage で empirical record 完了)。
- (改善 v2-2) Multi-PRD spec stage interleaving = "前 PRD spec stage 中に発見した別 PRD scope" の即時起票 mechanism 整備。**iteration v3 で部分 resolve** (= I-226 起票を撤回、Option β cohesive batch で本 PRD scope 拡張)、framework rule level の structural 整備は I-D scope candidate (= R-5)。

### Iteration v3 (2026-05-01、third-party `/check_job` review 21 actions fix + Option β cohesive batch + 13-rule self-applied re-verify)

**経緯**:
- iteration v2 self-review が Critical/High = 0 と self-claim、user "Spec stage approved" 報告
- 第三者 `/check_job` review (skill invoke、2026-05-01) で 16 findings + 5 review insights = 計 21 件 actions 発見
- ユーザー判断 (= H-2 Option α/β/γ): Option β (B2 + I-226 cohesive batch 化、test harness ESM upgrade を本 PRD scope に integrate) 採用 (2026-05-01)
- iteration v3 で 21 件 actions 全 resolve (= 4 Critical + 8 High + 4 Medium fix + 5 Review insights action item integration)

**主要進捗 (iteration v3 で resolve)**:

1. **C-1 fix (Cartesian product 完全 enumerate)**: matrix を 31 cells (旧) → 80 cells (Cartesian 完全) に拡張、各 cell 独立 row、orthogonality merge cells には source cell # 明示、abbreviation pattern 不在 (Rule 1 (1-2/1-4) compliant)
2. **C-2 fix (SWC parser empirical lock-in)**: `tests/swc_parser_top_level_await_test.rs` 新規 file 作成、4 tests passing で Axis A vs C1 mutual exclusion (cells 2-10 NA + cells 22/24/26/28/30 NA + cells 42-50 NA + cells 52-60 NA + cells 62-70 NA = 25 NA cells) を structural lock-in (Rule 3 (3-2) hard violation を本 PRD scope 内 fix)
3. **C-3 fix (cell-05 fixture fidelity)**: cell-05 fixture から user-side `__ts_main();` call site を削除、A0 spec (declarations only、no execution) に整合、`scripts/observe-tsc.sh --no-auto-main` flag で fidelity mode 導入、oracle empirical re-record で stdout=(empty) lock-in (Rule 5 (5-1) + Rule 6 (6-3) violation を本 PRD で fix)
4. **C-4 fix (Cell 27 split + Axis A5a/A5b)**: matrix # 51 (A5a Stmt::Empty silent skip) + matrix # 61 (A5b Stmt::Debugger Tier 2 reclassify) に分離、Axis A 定義に sub-axis A5a / A5b 明示、cell-27a fixture 新規作成 (Rule 1 (1-2) violation を fix)
5. **H-1 fix (Axis B B1 orthogonality merge structural verify)**: `## Problem Space > Axis B B1 Orthogonality Verification` sub-section 追加、3 forms (function decl / const arrow / const fn expr) の rename target が同一 `__ts_main` namespace に collapse することを spec、Implementation stage T3 で `test_axis_b_b1a_b_c_rename_dispatch_symmetric` で structural lock-in (Rule 1 (1-4) compliant)
6. **H-2 fix (Option β cohesive batch)**: cells 14-18/30 + 6/7/8 を Out of Scope → In Scope migration、I-226 PRD 起票撤回、TODO + plan.md chain から I-226 references 削除予定、PRD scope 拡張 (TS-5/TS-6 + T7/T8/T9 新規 task 追加)
7. **H-3 fix (cells 21-24 Scope wording)**: 旧 "+ I-016 prerequisite chain" 表現を Design dispatch tree と整合する形に修正、cells 31-40 (新 numbering) Scope 列を "本 PRD scope (executable mode で fn main body capture path、library mode の I-016 path とは別 dispatch)" に明確化
8. **H-4 fix (Design dispatch tree narrow)**: Design #2 dispatch tree から旧 Out of Scope cells を削除、in-scope cells のみ enumerate、各 leaf に matrix # 新 numbering 反映、A4/A5a/A5b cells は dispatch tree から除外し orthogonality merge representative + dispatch annotation
9. **H-5 fix (INV-2 verification cells in-scope)**: INV-2 (c) Verification method を in-scope cells のみに narrow (旧 cells 15/16 を含む文を update、Option β cohesive batch で in-scope migration したため再 narrow 不要)
10. **H-6 fix (Decl::Var dual-path dispatch design)**: Design #3 に "Decl::Var dual-path dispatch decision tree" 追加、Library mode vs Executable mode 判定 + Lit / SideEffect / AwaitInit init kind classifier `classify_decl_var_path` 追加、I-016 silent skip 条件と整合
11. **H-7 fix (Cell 31 INV-2 sub-case integration)**: 旧 cell-31 (A1 + B1 + multi-call) を独立 cell から削除、Axis A1 sub-axis 化せず INV-2 (c) Verification method 内の "Multi-call boundary value sub-case" として cell-31 fixture が cell #13 boundary value test として locked-in する形で integration (Fix B 採用、Axis 過剰膨張 avoid)
12. **H-8 fix (T6 split into T6a + I-D candidate)**: T6 から `audit-prd-rule10-compliance.py` reinforce task を削除、I-D scope (= R-1 + R-5 framework rule integration) へ migrate、T6a は B2 scope (= I-154 doc + audit-no-pub-fn-init.sh CI integrate) のみ
13. **M-1 fix (NA cells 6/7/8 wording precision)**: NA reason wording を "AST shape 上 mutually exclusive" に precision up、25 NA cells (Axis A0/A2/A4/A5a/A5b + C1) を 1 unified reasoning で structural lock-in
14. **M-2 fix (Axis E "Module export presence" 追加)**: Axis E (E0 absent / E1 present) を入力次元として明示、orthogonality merge declaration で matrix sub-axis 化せず Implementation stage T3 で `test_axis_e_export_preserve_symmetric` で structural verify
15. **M-3 fix (audit-no-pub-fn-init.sh 作成 task 明示)**: `scripts/audit-no-pub-fn-init.sh` 新規作成 task を TS-7 (Spec stage audit task) に明示、T6a で CI integrate
16. **M-4 fix (Test Plan E2E test fn naming pattern 統一)**: `test_e2e_cell_i224_<NN>_<semantic_name>` naming pattern + regression lock-in cells (matrix # 1/3/5/7/21/23/51/61) も同 pattern で entry 必須記載
17. **R-1 action (audit-prd-rule10-compliance.py matrix cell completeness 検出 mechanism)**: I-D scope (= framework rule integration、`verify_cartesian_product_completeness` function 追加) candidate として記録、本 PRD close 時 I-D PRD 起票
18. **R-2 action (`pub fn init` 廃止 impact audit)**: TS-7 task に integrate (本 PRD spec stage 内、INV-7 verification method として codebase + Hono grep)
19. **R-3 action (TypeResolver impact assessment)**: Design section に "TypeResolver impact" sub-section 追加、INV-6 invariant lock-in
20. **R-4 action (`__ts_main` 既存 user code 衝突 audit)**: TS-7 task に integrate (codebase + Hono grep `__ts_main` 0 hits 確認)
21. **R-5 action (Multiple PRD spec stage interleaving rule framework 化)**: I-D scope candidate として記録 (= R-1 と統合、framework rule level integration)

**13-rule self-applied re-verify (iteration v3)**:

| Rule | Sub-rule check | iteration v2 verdict | iteration v3 verdict | Notes |
|---|---|---|---|---|
| 1 | (1-1) 全 cell ideal output | ✓ | ✓ | 80 cells 全 enumerate、空欄/TBD 不在 |
| 1 | (1-2) abbreviation pattern 不在 | ✓ (false) | ✓ (true) | iteration v3 で full Cartesian 80 cells、abbreviation/ellipsis/range grouping/placeholder 不在、各 cell 独立 row。**第三者 review C-1 fix** |
| 1 | (1-3) audit script PASS | ✓ (claim) | ✓ | iteration v3 で `audit-prd-rule10-compliance.py backlog/I-224-top-level-fn-main-mechanism.md` PASS 確認 (2026-05-01)、TS-7 + adversarial review iteration 完了後 stable PASS |
| 1 | (1-4) Orthogonality merge legitimacy | ✓ (partial) | ✓ | Axis B B1 orthogonality merge structural verify sub-section 追加 (third-party review H-1 fix)、Axis E orthogonality merge declaration 追加 (M-2 fix)、orthogonality merge cells に source cell # 明示 + Spec-stage structural consistency (T3 unit test で probe) + symmetry probe |
| 2 | (2-1) Oracle grounding cross-reference | ✓ | ✓ | in-scope ✗ cells (representative cells 20 cells、orthogonality-merged cells を除く) で oracle grounding embed |
| 2 | (2-2) `## Oracle Observations` section embed | ✓ | ✓ | in-scope ✗ cells 全 record embed (新 cells 14-18/30 ESM mode で 2026-05-01 record、cell-05 fidelity 修正 record、auto-append convention note 追加) |
| 2 | (2-3) audit script verify | ✓ | ✓ | audit-prd-rule10-compliance.py PASS 確認 (2026-05-01)、`## Oracle Observations` section 不在 fail 排除 |
| 3 | (3-1) NA spec-traceable | ✓ | ✓ | 25 NA cells を Axis A vs C1 mutual exclusion で 1 unified reasoning に統一、各 cell の "AST 構造的 mutual exclusion" reason は spec-traceable (M-1 precision-up) |
| 3 | (3-2) SWC parser empirical observation | partial → defer to I-226 (false claim) | ✓ | **iteration v3 で完成** (`tests/swc_parser_top_level_await_test.rs` 新規 file、4 tests passing 2026-05-01)、第三者 review C-2 fix |
| 3 | (3-3) SWC accept → Tier 2 reclassify | ✓ | ✓ | SWC parser が `await x;` を A1 partition として accept、A0 + C1 partition は空集合という structural reasoning で NA、本 PRD scope 内 `unreachable!()` macro 不要 (= reachable な C1 cells を全 in-scope migration) |
| 4 | (4-1) reference doc 整合 | ✓ | ✓ | `doc/grammar/ast-variants.md` の ModuleItem / Stmt / Decl / Expr Tier 1/2 と整合 |
| 4 | (4-2) doc-first dependency order | N/A | N/A | I-154 namespace doc は T6a で update、code change と同 PRD 内で sync (本 PRD は doc 改修前提の code 改修 sequence ではない) |
| 4 | (4-3) audit verify | N/A | N/A | 上記 |
| 5 | (5-1) E2E fixture 準備 | ✓ | ✓ | TS-3 完了 (in-scope cells 用 fixture が red 状態 lock-in 済、cell-05 fidelity 修正 + cell-27a 新規 + cells 14-18/30 復元、NEW cells 13 件は Implementation stage TS-3 で並行作成) |
| 5 | (5-2) `## Spec Stage Tasks` + `## Implementation Stage Tasks` 2-section split | ✓ | ✓ | 維持 + iteration v3 で TS-5/TS-6/TS-7 + T6a/T7/T8/T9 拡張 |
| 5 | (5-3) Spec stage tasks に code 改修不在 | ✓ | ✓ | TS-2 (SWC parser test 作成) は本 PRD scope の test code、`src/` 配下の Rust source 改修は不在。TS-5 (script 改修) は test infra (`scripts/`) 配下で `src/` 配下不在 |
| 5 | (5-4) audit verify | ✓ | ✓ | iteration v3 で TS-7 task 実施完了 (`## Pre-Implementation Audit Findings` section embed)、`audit-prd-rule10-compliance.py` PASS 確認 (2026-05-01)、Spec Stage Tasks / Implementation Stage Tasks 2-section split section unrecognized fail 排除。third-party adversarial re-review Medium 2 fix で stale `(TS-7 後 verify)` label を ✓ に update |
| 6 | (6-1) Matrix Ideal output ↔ Design token-level 一致 | ✓ | ✓ | iteration v3 で Design dispatch tree narrow + matrix # 新 numbering 反映 (third-party review H-3/H-4 fix)、Decl::Var dual-path dispatch decision tree 追加 (H-6 fix)、各 cell ideal output ↔ Design dispatch leaf が token-level corresponds |
| 6 | (6-2) Scope 3-tier hard-code | ✓ | ✓ | In Scope (Option β 拡張、cells 14-18/30 + 6/7/8 + NEW cells migration) / Out of Scope (regression lock-in cells + I-016 / I-221 / I-180 / I-203 别 PRD) / Tier 2 honest error reclassify (collision + Debugger + control-flow) 3 sub-section |
| 6 | (6-3) matrix Scope 列値 | ✓ | ✓ | `本 PRD scope` / `regression lock-in` / `本 PRD scope (orthogonality merged)` / `本 PRD scope (cohesive batch)` / `Tier 2 honest reclassify (本 PRD)` / `NA` 択一 |
| 6 | (6-4) Scope ↔ matrix cross-reference consistency | ✓ | ✓ | iteration v3 で Scope 3-tier section の cells list が matrix Scope 列と token-level corresponds |
| 7 | Control-flow exit sub-case completeness | N/A | N/A | 本 PRD は control-flow body / branch shape concern を含まない (top-level statement dispatch focus、user main body の control-flow は別 architectural concern) |
| 8 | (8-5) `## Invariants` 独立 section | ✓ | ✓ | INV-1〜INV-7 独立 section、各 4 項目 (a)(b)(c)(d) 記載。INV-3 wording revise (Axis C1 in-scope 反映、third-party review H-5 fix)、INV-6 (TypeResolver unaffected) + INV-7 (`pub fn init` audit) iteration v3 新規追加 (R-3 + R-2 fix) |
| 9 | (a) Spec → Impl Dispatch Arm Mapping | ✓ | ✓ | Design section dispatch tree が in-scope dispatch leaves と 1-to-1 mapping (Axis C1 + Decl::Var dual-path 追加で leaves 拡張、各 leaf に matrix # annotation) |
| 9 | (b) Impl → Spec | N/A | N/A | Implementation stage で発動 |
| 9 | (c) Field-addition symmetric audit | N/A | N/A | 本 PRD は MainStmt / UserMainKind enum 新規追加だが既存 IR struct field 追加なし、symmetric audit 不要 |
| 10 | Cross-axis matrix completeness | ✓ (claim) | ✓ | Axis A (8) × Axis B (5) × Axis C (2) Cartesian 80 cells、Axis E orthogonality merge declaration、9 default check axis のうち relevant axes (= trigger / operand type / body shape / outer context / AST dispatch hierarchy) を Rule 10 Application section で enumerate (M-2 で Axis E 追加) |
| 11 | (d-1) `_ => ` 全廃 | ✓ | ✓ | `transform_module_item:449` `_ => Err` は T4 で全 ModuleItem variant explicit enumerate refactor、`arrow_fns.rs:42` / `mod.rs:666` は I-016 / I-203 defer 維持 |
| 11 | (d-2) phase 別 mechanism | ✓ | ✓ | 維持 |
| 11 | (d-3) `ast-variants.md` single source of truth | ✓ | ✓ | 維持 |
| 11 | (d-4) audit script CI | ✓ | partial (Implementation stage で CI integrate) | iteration v3 で `audit-ast-variant-coverage.py` を ad-hoc run、`audit-prd-rule10-compliance.py` PASS 確認 (Spec stage で full PASS verify、Rule 11 (d-1) refactor は T4 = Implementation stage で実施 + T6a で CI integrate のため partial verdict、Spec stage 完了 block ではない) |
| 11 | (d-5) Pre-draft audit | ✓ | ✓ | `## Impact Area Audit Findings` section 完成 (iteration v2 維持) |
| 11 | (d-6) Architectural concern relevance | ✓ | ✓ | 維持 |
| 12 | Rule 10/11 Mandatory + structural | ✓ | ✓ | `## Rule 10 Application` section + audit script PASS verify (TS-7 audit empirical 完了済 2026-05-01、Critical=0 claim と consistent) |
| 13 | (13-1) skill workflow Step 4.5 | ✓ | ✓ | 本 iteration log 自身 |
| 13 | (13-2) `## Spec Review Iteration Log` record | ✓ | ✓ | iteration v1 + v2 + v3 record (history preserved) |
| 13 | (13-3) Critical findings 全 fix | ✓ (false) | ✓ | iteration v3 で旧 21 件 actions 全 resolve + iteration v3 third-party adversarial review で発見の追加 11 件 actions (Critical = 3 + High = 5 + Medium = 3) 全 resolve = duplicate 31-cell matrix 削除 + Design dispatch tree rewrite (4-tuple match + collision arm 最優先 + unreachable lock-in) + self-review verdict consistency fix + cell # 27 misclassification fix + Axis E `pub` modifier preservation rule 追加 + INV-3 B1+C1 cells exhaustive 列挙 + fixture count 訂正 + Out of Scope wording 訂正 |
| 13 | (13-4) audit verify | ✓ | ✓ | iteration v3 で audit-prd-rule10-compliance.py PASS 確認 (2026-05-01) + adversarial review iteration 後 stable PASS、structural compliance lock-in |
| 13 | (13-5) Self-applied integration | partial | partial | iteration v2 で発見の framework 改善 (= R-1 + R-5、`scripts/observe-tsc.sh` ESM upgrade rule、Multi-PRD spec stage interleaving rule) を本 PRD close 時 I-D scope candidate として integrate (iteration v3 で部分 resolve = `--esm --no-auto-main` flag 実装、framework rule level integration は I-D scope)。adversarial review iteration で発見の framework gap (= duplicate matrix detection mechanism、dispatch tree pseudocode validation) も I-D scope candidate (= Review insight #1 + #2 + #3) |

**Iteration v3 findings count (adversarial review iteration 完了後)**:
- 旧 21 件 actions (initial third-party `/check_job` review 由来): Critical 4 + High 8 + Medium 4 + Review insights 5 = 全 resolve ✓
- 新 11 件 actions (iteration v3 third-party adversarial agent review 由来): Critical 3 + High 5 + Medium 3 = 全 resolve ✓ + Review insights 4 (= I-D scope candidate に integrate)
- **Total**: Critical = 0、High = 0、Medium = 0、Review insights = 0 active (9 件全 I-D scope candidate integration 完了)

**Iteration v3 Spec stage 完了判定 (adversarial review iteration 後、self-claim、retracted by iteration v4)**: ~~Spec stage approved~~ → 第三者 adversarial agent re-review (2nd round) で **新 5 件 actions** (Critical 1 + High 2 + Medium 2 + Review insights 4) 発見、iteration v3 self-claim が再度 false-positive と判定。詳細は `### Iteration v4` entry 参照。**真の Spec stage approval は iteration v4 完了判定 section 参照**。

### Iteration v4 (2026-05-01、third-party adversarial agent re-review 2nd round で発見の新 5 件 actions fix)

**経緯**:
- iteration v3 で 32 件 actions 全 resolve + 13-rule self-applied verify 全項目 ✓ + audit-prd-rule10-compliance.py PASS 確認 → "Spec stage approved" claim
- 第三者 adversarial review (2nd round、independent agent invoke 経由) で **新 5 件 actions** (Critical 1 + High 2 + Medium 2 + Review insights 4) を empirical 発見、iteration v3 self-claim が再度 false-positive と判定
- iteration v4 で 5 件 actions 全 resolve

**主要進捗 (iteration v4 で resolve)**:

1. **Critical 1 fix (dispatch tree axis-tuple ↔ definition mismatch)**: 旧 4-tuple match (`is_executable_mode`, `user_main_kind`, `is_async_required`, `has_lit_top_level_const`) を **3-tuple match** (`is_executable_mode`, `user_main_kind`, `has_top_level_await`) に simplify、`has_lit_top_level_const` を per-item runtime decision に移行。理由: 旧 dispatch tree の library mode + FnAsync arms (lines 720, 722) が `is_async_required=false` を pattern として claim していたが、`is_async_required = (FnAsync || has_top_level_await)` 定義より cells #5/#25 (FnAsync user main) は `is_async_required=true` ⇒ 旧 dispatch tree の `(false, _, true, _)` unreachable!() arm が cells #5/#25 を catch して runtime panic。is_async_required を dispatch dimension から除外することで axis-tuple ↔ definition の structural consistency restoration。
2. **High 1 fix (cells #7/#27 double-listed in dispatch tree comments)**: 旧 dispatch tree の `(false, None, false, _)` arm comment (line 713) が cells #7 と #27 を含めていたが、これらは B3=NonFn cells で別 arm (`(false, NonFn, false, _)`) が 1-to-1 担当。3-tuple match rewrite に併せて comment cells listing を 1 arm only に整合化。
3. **High 2 fix (INV-3 (c) Trigger 1 cells exhaustive 列挙)**: INV-3 (c) Trigger 1 (B2) only verification cells を旧 `15/35/75` (executable mode のみ) → 新 `5/15/25/35/75` に拡張、library mode + FnAsync cells (5, 25) を明示。これにより iteration v4 Critical 1 の dispatch bug が INV-3 verification test で structural detect されることを保証。
4. **Medium 1 fix (A6 cells double-claimed in dispatch tree)**: 旧 4-tuple match で cells #71/#72 等 A6 cells が `(true, X, _, false)` と `(true, X, _, true)` の 2 arms に double-claim されていた (= `has_lit_top_level_const` axis 上で A6 cells の partition 不在のため)。3-tuple match rewrite で各 A6 cell が 1 arm only に list、Lit init は per-item runtime decision で top-level Item::Const として emit。Rule 9 (a) 1-to-1 mapping compliant。
5. **Medium 2 fix (Rule 5 (5-4) stale verdict label)**: Spec Review Iteration Log v3 table の Rule 5 (5-4) verdict が `(TS-7 後 verify)` のまま残っていた (= TS-7 audit empirical 完了済の事実と inconsistency、Critical=0/High=0 claim と矛盾)。本 verdict を ✓ に flip + audit empirical 完了の rationale を annotation。
6. **Review insight 4 fix (cell #25 "NEW" wording 整合化)**: In Scope section line 547 で cell #25 が "NEW" と表記されていたが orthogonality merged であり新 fixture 不要、wording を "newly migrated to In Scope" + "Fixture 不要 (orthogonality merged)" に整合化、Implementation Stage TS-3 NEW fixture list との consistency restoration。

**Rule 9 (a) 1-to-1 mapping verification table (iteration v4 新規追加)**: dispatch tree 各 leaf と matrix cells の 1-to-1 correspondence を Design section 内 mapping table で structural lock-in、Implementation Stage T3 unit test で empirical verify (= 各 cell の `(is_exec, kind, has_top_await)` 3-tuple を helper で derive、dispatch arm match の expectation assert)。

**Iteration v4 findings count**: Critical = 0 (resolved 1 件)、High = 0 (resolved 2 件)、Medium = 0 (resolved 2 件)、Review insights = 0 active (4 件、うち 1 件 = "NEW" wording integration を v4 で resolve、3 件 = framework gap candidates を I-D scope に追加 integrate = adversarial review が dispatch tree axis-tuple consistency check を auto verify する mechanism、Critical=0 claim ↔ stale verdict label inconsistency を audit script で auto detect する mechanism、A6 mixed cells の lit-axis sub-partition spec rule)。

**Iteration v4 Spec stage 完了判定 (self-claim、retracted by iteration v5)**: ~~Spec stage approved~~ → 第三者 adversarial agent re-review (3rd round) で **新 13 件 actions** (Critical 3 + High 5 + Medium 3 + Review insights 2、加えて compromise audit が "妥協なし" 不達と判定 = NEW fixtures 13 件 deferred が Rule 5 (5-1) 違反、per-item runtime spec incomplete) 発見、iteration v4 self-claim が 4 度目の false-positive と判定。詳細は `### Iteration v5` entry 参照。**真の Spec stage approval は iteration v5 完了判定 section 参照**。

### Iteration v5 (2026-05-01、third-party adversarial agent re-review 3rd round で発見の新 13 件 actions + Compromise audit fix)

**経緯**:
- iteration v4 で 37 件 actions resolve + 13-rule self-applied verify 全項目 ✓ + audit-prd-rule10-compliance.py PASS 確認 → "Spec stage approved" claim
- 第三者 adversarial review (3rd round、independent agent invoke 経由) で **新 13 件 actions** (Critical 3 + High 5 + Medium 3 + Review insights 2) + **Compromise audit "妥協なし" 不達** 発見、iteration v4 self-claim が 4 度目の false-positive と判定
- iteration v5 で 13 件 actions 全 resolve + Compromise = NEW fixtures 13 件 全作成 (Rule 5 (5-1) 厳格 compliance) + per-item runtime spec 完成

**主要進捗 (iteration v5 で resolve)**:

1. **Critical 1 fix (Cell 49 matrix entry ↔ dispatch tree contradiction)**: Matrix 内 cell 49 description を "control-flow priority + collision fallback" wording を "INV-5 collision priority + cell 9 collision dispatch orthogonality merge" に rewrite、dispatch tree (collision arm 最優先) と整合化。
2. **Critical 2 fix (Cells 41/43/45/47/49 triple-classification → Tier 2 honest error reclassify single-tier 化)**: 旧 In Scope / Out of Scope / Tier 2 reclassify 3 sections に同 cells が重複 listed されていた Rule 6 (6-2) violation を、Tier 2 honest reclassify section only に narrow (= 本 PRD で modify する Tier 2 wording 改善対象、Tier 1 化は別 PRD I-203)。In Scope / Out of Scope sections は Tier 2 reclassify section への内部 reference に置換。
3. **Critical 3 fix (INV-3 (c) sub-case lists 全 rebuild)**: dispatch tree 12 reachable arms から exhaustively derive で 4 sub-case lists (Trigger 1 / Trigger 2 / Trigger 1+2 / Sync) を rebuild、不足 cells 32/38/55/72/77/78 を追加。
4. **Critical New fix (Per-item runtime decision spec 完成)**: v4 で `has_lit_top_level_const` を per-item runtime に移行したが具体 rule 未記述だった gap を解消、Design section #3 に `is_executable_mode` predicate + `classify_init_kind` (Lit / SideEffect / AwaitInit partition) + `has_side_effect_init` predicate (= AwaitInit を含めて true return) + `classify_decl_var_path` の完全 spec を hard-code、A6 mixed cell の per-item iteration 詳細例 + INV-1 source-order preservation invariant verify rationale embed。
5. **High 1 fix (Cells 25/29 wording split)**: 旧条約 wording "orthogonality merged with cells 5/9 + 21" が cell 25 (5+21 merge) と cell 29 (9+21 merge) を condense した混乱を、各 cell の actual merge sources を separate bullet で明確化。
6. **High 2 fix (Cell 27 を regression lock-in lists に追加)**: matrix Scope 列 = "regression lock-in" の cell 27 を Out of Scope regression lock-in bullet + Test Plan E2E regression lock-in entries に追加。
7. **High 3 fix (Cell 40 を 4 sections に追加)**: dispatch tree comment では含まれていた cell 40 (A3+B4+C1) を Tier 2 honest error reclassify section + INV-5 verification list + T1 completion criteria + In Scope collision-merged list の 4 sections に追加、cross-reference consistency restoration。
8. **High 4 fix (Section #5 stale "cell 15" → "cell 14")**: 旧 numbering "cell 15" reference を新 matrix numbering "cell 14 = A1+B1+C1" に訂正、INV-3 (c) edge sub-case description と整合化。
9. **High 5 fix (Rule 9 (a) mapping unit test name 明示)**: 旧 spec で "Implementation Stage T3 で本 mapping table を unit test として lock-in" のみで test fn name 不在だった点を、`test_dispatch_arm_one_to_one_mapping_per_in_scope_cell` を Test Plan unit tests bullet + Design section dispatch tree mapping table 直後に明示、test code 概要 (test_cases array + assert per cell) も hard-code。
10. **Medium 1 fix (`has_side_effect_init` predicate behavior 明示化)**: per-item runtime decision spec の core predicate を Design section #3 内で AwaitInit を含めて true return する仕様として明示 (= `(false, _, true)` unreachable arm の structural reachability verify の foundation)。
11. **Medium 2 fix (A5b dispatch flow precedence 明示化)**: `is_executable_mode` predicate 内に Stmt::Debugger 不在の理由を annotation (= `transform_module_item` の Tier 2 reject が本 dispatch tree leaf より先行 fire)、Design section #3 per-item iteration spec 内にも同 precedence note を embed。
12. **Medium 3 fix (A5a × B compositional invariant probe test 仕様追加)**: cell 51 representative + B-axis dispatch leaves の orthogonal composition を `test_axis_a5a_compositional_orthogonality_with_b_axis` (新規 test fn name) として Test Plan unit tests bullet に明示、cells 53/55/57/59 の expected output が cell 51 + B0/B1/B2/B3 dispatch + cell 59 = B4 collision priority arm 先行 reject を assert する spec 確定。
13. **Compromise fix (Rule 5 (5-1) 厳格 compliance)**: NEW cells 13 件 (cell 32/34/36/38/40/41/72/74/75/77/78/79/80) に対して fixtures 全 作成 + ESM mode (Axis C1) / default mode (Axis C0) で `--no-auto-main` flag 適用の oracle empirical record 完了、`## Oracle Observations` section に 4-field embed (TS fixture path / tsc output / matrix cell # link / ideal output rationale)。これにより in-scope ✗ cells のうち representative + NEW = 33 cells 全 fixture が red 状態 lock-in 達成 (= 全 ✗ cells のうち structural orthogonality merge 適用済 cells = 残 ~7 cells のみ representative dispatch test で cover、Spec stage 完了時点で empirical fixture 不在の cells は厳密に orthogonality merge cells のみ)。

**13-rule self-applied re-verify (iteration v5)**:

| Rule | Sub-rule check | iteration v4 verdict | iteration v5 verdict | Notes |
|---|---|---|---|---|
| 1 | (1-2) abbreviation pattern 不在 | ✓ | ✓ | 80 cells 全 enumerate 維持 |
| 1 | (1-4) Orthogonality merge legitimacy | ✓ | ✓ | iteration v5 で cells 25/29 wording split + cell 27 regression lock-in list 追加 + cells 41/43/45/47 single-tier 化 |
| 2 | (2-2) `## Oracle Observations` section embed | ✓ partial | ✓ full | iteration v5 で NEW 13 cells 全 empirical record embed = full coverage |
| 3 | (3-1/3-2/3-3) | ✓ | ✓ | 4 SWC parser tests passing 維持 |
| 4 | grammar consistency | ✓ | ✓ | 維持 |
| 5 | (5-1) E2E fixture 準備 | partial (NEW 13 cells deferred) | **✓ full** | iteration v5 で 13 NEW fixtures 全作成 + oracle empirical record 完了 = third-party adversarial re-review (3rd round) Compromise audit fix |
| 5 | (5-2) 2-section split | ✓ | ✓ | 維持 |
| 5 | (5-3) Spec stage src/ 不在 | ✓ | ✓ | 維持 |
| 5 | (5-4) audit verify | ✓ | ✓ | iteration v5 PRD doc post-update audit-prd-rule10-compliance.py PASS 確認 |
| 6 | (6-1) Matrix Ideal ↔ Design token-level | ✓ (claim) | ✓ | iteration v5 Critical 1 fix (cell 49 整合) で matrix-design 矛盾解消 |
| 6 | (6-2) Scope 3-tier hard-code | ✓ (claim) | ✓ | iteration v5 Critical 2 fix で cells 41/43/45/47/49 triple-classification 解消、3-tier mutual exclusivity 達成 |
| 6 | (6-3) matrix Scope 列値 | ✓ | ✓ | 維持 |
| 6 | (6-4) Scope ↔ matrix cross-reference | ✓ (claim) | ✓ | iteration v5 で cell 27 / cell 40 / cells 25/29 wording の cross-reference consistency 復元 |
| 7 | Control-flow exit | N/A | N/A | 維持 |
| 8 | (8-5) `## Invariants` 独立 section | ✓ | ✓ | INV-1〜INV-7 維持 + iteration v5 INV-3 (c) 4 sub-case lists exhaustively rebuild |
| 9 | (a) Spec → Impl Dispatch Arm Mapping | ✓ (claim) | ✓ | iteration v5 で `test_dispatch_arm_one_to_one_mapping_per_in_scope_cell` test fn 明示 + dispatch tree mapping table が 1-to-1 mapping 完成 |
| 9 | (c) Field-addition symmetric audit | N/A | N/A | 維持 |
| 10 | Cross-axis matrix completeness | ✓ | ✓ | 80 cells full Cartesian + Axis E orthogonality merge declaration 維持 |
| 11 | (d-1/d-5/d-6) | ✓ partial | ✓ | iteration v5 Medium 2 fix で `transform_module_item` `_` arm refactor 後 explicit enumerate spec を Design section #3 で hard-code (Tier 2 precedence + control-flow / Debugger reject 経路) |
| 12 | Rule 10/11 Mandatory + structural | ✓ | ✓ | 維持 |
| 13 | (13-2/13-3) | ✓ partial | ✓ | iteration v5 で iteration v4 self-claim retract + iteration v5 entry に 13 件 actions resolve record + 全 sub-rule verdict ↔ Critical=0 claim consistency 達成 |
| 13 | (13-5) Self-applied integration | partial | partial | iteration v5 で発見の framework gap candidates (= cross-reference consistency check mechanism / spec-table-driven generator candidate) を I-D scope に追加 integrate |

**Iteration v5 findings count (3rd adversarial review iteration 完了後)**:
- 旧 21 件 actions (initial third-party `/check_job` review): resolved ✓
- iteration v3 11 件 actions (1st adversarial agent re-review): resolved ✓
- iteration v4 5 件 actions (2nd adversarial agent re-review): resolved ✓
- iteration v5 13 件 actions + 1 件 Compromise (3rd adversarial agent re-review): **resolved ✓**
- **Total**: 50 件 actions 全 resolve、Critical = 0、High = 0、Medium = 0、Review insights = 2 active (= I-D scope に追加 integrate 候補)

**Iteration v5 Spec stage 完了判定 (self-claim、retracted by iteration v6 minor)**: ~~Spec stage approved~~ → 第三者 adversarial agent re-review (4th round) で **新 2 件 actions** (High 1: `is_executable_mode` predicate `_ => false` arm が Rule 11 (d-1) self-applied violation + Medium 1: INV-3 (c) Sync list 内 library mode `fn main directly emit` cells 3/23 missing) 発見、iteration v5 self-claim が 5 度目の false-positive と判定 (ただし genuine convergence signal: Critical=0 が 5 iterations で初めて達成)。詳細は `### Iteration v6 minor` entry 参照。**真の Spec stage approval は iteration v6 minor 完了判定 section 参照**。

### Iteration v6 minor (2026-05-01、4th adversarial agent re-review で発見の 2 件 actions fix)

**経緯**:
- iteration v5 で 50 件 actions 全 resolve + 13-rule self-applied verify ✓ + audit-prd-rule10-compliance.py PASS + Compromise audit "妥協なし" 達成 → "真の Spec stage approved" claim
- 第三者 adversarial review (4th round、independent agent invoke 経由) で **High 1 + Medium 1 = 2 件 actions** + Review insights 3 発見、5 度目の false-positive 判定 (ただし Critical=0 達成 = genuine convergence signal、findings count 21→11→5→13→**2** で convergence pattern 確立)
- iteration v6 minor で 2 件 actions resolve (~30 min PRD doc edit work、structural defect なし、wording/code precision のみ)

**主要進捗 (iteration v6 minor で resolve)**:

1. **High 1 fix (`is_executable_mode` predicate Rule 11 (d-1) self-applied compliance)**: 旧 spec で `is_executable_mode` predicate 内に `_ => false` wildcard arm が残っていた (= Stmt::If/For/While/Try/Switch/Throw/Labeled/Block/Continue/Break/Return/With 等 control-flow + non-control-flow Stmt variants を catch-all)。本 PRD Goal #5 が "transform_module_item の `_` arm を ModuleItem 全 variant explicit enumerate に refactor、新 variant 追加時 compile error で全 dispatch fix 強制" を要求していたが、新規 introduce する `is_executable_mode` predicate 自身が同 invariant を violate していた self-applied compliance gap。**Fix**: Stmt 全 variants を explicit enumerate (Decl variants = Decl::Fn/Class/TsInterface/TsTypeAlias/TsEnum/TsModule/Using + Stmt::Empty + Stmt::Debugger + control-flow 15 variants Block/If/Switch/Throw/Try/While/DoWhile/For/ForIn/ForOf/Labeled/Continue/Break/Return/With) で `_ => ` arm 排除、Rule 11 (d-1) self-applied compliance 達成。新 SWC Stmt variant 追加時 compile error で `is_executable_mode` 全 dispatch fix 強制 = future-proof structural enforcement。
2. **Medium 1 fix (INV-3 (c) Sync list exhaustivity gap)**: 旧 INV-3 (c) Sync list "cells 11/13/17/31/33/37/71/73/77" が "本 PRD scope の Axis B0/B1/B3 + C0 全 cells" と claim していたが、library mode `fn main directly emit` cells 3 (A0+B1+C0) と 23 (A2+B1+C0) を omit していた exhaustivity gap。**Fix**: Sync list を "cells 3/11/13/17/23/31/33/37/71/73/77" に拡張 (= Axis B1/B3/B0 + C0 + has_top_level_await=false かつ `fn main` emit する全 in-scope cells) + library mode no-fn-main cells (cells 1/7/21/27 = library mode + B0/B3 user_main_kind = `fn main` emit しない) を INV-3 application scope 外として明示 (= 別 invariant "no fn main emission in library mode" で structural lock-in、Implementation Stage T2 helper unit test で assertion)。

**13-rule self-applied re-verify (iteration v6 minor)**:

iteration v5 から table 全項目 ✓ 維持。本 v6 minor の 2 件 fixes は以下 sub-rules を更に reinforce:

- **Rule 8 (8-5)** INV-3 (c) verification cells exhaustive 列挙 = sync list に library mode `fn main directly emit` cells 追加で完全 coverage
- **Rule 11 (d-1)** `_` arm 全廃 = `is_executable_mode` predicate self-applied compliance 達成 (本 PRD spec stage で introduce した predicate も ModuleItem dispatch path と同 standard で explicit enumerate)

**Iteration v6 minor findings count (4th adversarial review iteration 完了後)**:
- 旧 50 件 actions (initial /check_job + iteration v3 + v4 + v5): resolved ✓
- iteration v6 minor 2 件 actions (4th adversarial agent re-review): **resolved ✓**
- **Total: 52 件 actions 全 resolve、Critical = 0、High = 0、Medium = 0、Review insights = 3 active (= I-D scope に追加 integrate 候補)**

**Iteration v6 minor Spec stage 完了判定 (self-claim、refined by iteration v7 stub creation for true convention compliance)**: ~~Spec stage approved~~ → /check_problem session で I-205 v1.6 convention "Spec stage で helper test contracts (test fn / assertion / probe) を author + stub `#[test] #[ignore]` files 作成" の I-224 未履行 gap 発見、iteration v7 で stub files 作成して **Rule 9 (a) helper test contracts NEW + Rule 8 (8-c) audit symmetry** の structural compliance 達成。詳細は `### Iteration v7 (stub files convention compliance)` entry 参照。

### Iteration v7 (2026-05-01、stub test files 作成 = Spec stage convention 厳格 compliance)

**経緯**:
- iteration v6 minor で Critical=0 + High=0 + Medium=0 達成 + audit-prd-rule10-compliance.py PASS + 52 件 actions 全 resolve = "真の Spec stage approval" claim
- /check_problem session で I-205 v1.6 convention 履行状況 audit、I-205 は `tests/i205_helper_test.rs` (139 行) + `tests/i205_invariants_test.rs` (223 行) で stub files 履行済、I-224 は **未履行** = Spec stage convention compliance gap 発見
- ideal-implementation-primacy "妥協なし" mandate 観点で deferral 不可、iteration v7 で stub files 作成

**主要進捗 (iteration v7 で resolve)**:

1. **`tests/i224_helper_test.rs` 新規作成** (137 行、4 stubs `#[test] #[ignore]`):
   - `test_dispatch_arm_one_to_one_mapping_per_in_scope_cell` (Rule 9 (a) 80-cell ↔ dispatch arm 1-to-1 mapping、iteration v4 Critical 1 = axis-tuple ↔ definition mismatch の structural regression lock-in)
   - `test_axis_b_b1a_b_c_rename_dispatch_symmetric` (Axis B B1 3 forms 共通 dispatch、Rule 1 (1-4-c))
   - `test_axis_e_export_preserve_symmetric` (Axis E E1 `pub` modifier preservation rule + `__ts_main` private invariant)
   - `test_axis_a5a_compositional_orthogonality_with_b_axis` (cells 51/53/55/57/59 orthogonal composition probe)

2. **`tests/i224_invariants_test.rs` 新規作成** (228 行、7 stubs `#[test] #[ignore]`):
   - INV-1 (source-order preservation、T5/T9 fill in)
   - INV-2 (user main rename + cell-31 multi-call boundary value、T3 fill in)
   - INV-3 (sync/async dispatch 4 sub-cases、T2/T3 fill in)
   - INV-4 (`pub fn init` 廃止、T4/T5 fill in)
   - INV-5 (namespace + collision priority、T1 fill in)
   - INV-6 (TypeResolver layer separation、T2 fill in)
   - INV-7 (external API audit、T5 fill in)

3. **PRD doc Test Plan section update**: 旧 partial spec を helper test + invariants test の物理 file path + stub list で hard-code、Implementation Stage T1〜T9 fill-in 経路明確化

4. **Quality gate**: `cargo test --test i224_helper_test` = 0 passed / 4 ignored ✓ + `cargo test --test i224_invariants_test` = 0 passed / 7 ignored ✓ (= stub state lock-in、Implementation stage で `#[ignore]` 解除 + green-ify 待機)

**Iteration v7 findings count**: Critical = 0、High = 0、Medium = 0、Review insights = 0 (= stub creation = convention compliance closure task、新規 defect 発見なし)。

**Iteration v7 Spec stage 完了判定 (convention compliance + genuine convergence)**: ✓ **真の Spec stage true closure achieved** = 52 件 actions 全 resolve (iteration v3〜v6 minor 累積) + 11 stub files 作成 (Rule 9 (a) helper test contracts NEW + Rule 8 (8-c) audit symmetry の structural enforcement) + audit-prd-rule10-compliance.py PASS 維持 + 13-rule self-applied verify 全項目 ✓ + Compromise audit "妥協なし" 達成 + I-205 v1.6 convention 厳格 compliance。user 承認 → Implementation stage T1-T9 移行可能。

**Total stub coverage (iteration v7 closure)**:

| Test category | File | Stub count | Implementation Stage fill-in target |
|---|---|---|---|
| Helper test contracts | `tests/i224_helper_test.rs` | 4 | T2/T3 |
| Invariants verification | `tests/i224_invariants_test.rs` | 7 | T1/T2/T3/T4/T5/T9 |
| SWC parser empirical (already passing) | `tests/swc_parser_top_level_await_test.rs` | 4 active | (Spec stage で完成済) |
| **Total** | | **15 tests** | |

**Convergence pattern (iteration v2 → v7、final)**:

| Iteration | Findings | Critical | Notes |
|---|---|---|---|
| v2 (initial /check_job review) | 21 | 4 | baseline |
| v3 (1st adversarial re-review) | 11 | 3 | -47% |
| v4 (2nd adversarial re-review) | 5 | 1 | -55% |
| v5 (3rd adversarial re-review) | 13 | 3 | regression by deep cross-reference review |
| v6 minor (4th adversarial re-review) | 2 | 0 | -84%、Critical=0 初達成 |
| **v7 (stub convention compliance、/check_problem 由来)** | **0** | **0** | **stub creation = compliance closure、新規 defect 不在** |

最終: **Critical=0 + High=0 + Medium=0 + Review insights=0 active**、5 rounds adversarial review iteration + 1 convention compliance closure round で genuine convergence + structural compliance 達成。



**Convergence signal (iteration v2 → v3 → v4 → v5 → v6 minor)**:

| Iteration | Findings count | Critical | Convergence trend |
|---|---|---|---|
| v2 (initial /check_job review) | 21 | 4 | baseline |
| v3 (1st adversarial re-review) | 11 | 3 | 47% reduction |
| v4 (2nd adversarial re-review) | 5 | 1 | 55% reduction |
| v5 (3rd adversarial re-review) | 13 | 3 | regression by deep cross-reference review |
| **v6 minor (4th adversarial re-review)** | **2** | **0** | **84% reduction、Critical=0 初達成** |

**Pattern analysis**: v5 で 13 件 finding は v4 までの "structural defects" focus から "cross-reference / wording precision" focus への shift で発生 (= dense matrix 80 cells × 6 cross-reference contexts の manual-tracking density limit に到達)。v6 minor で Critical=0 + High=1 + Medium=1 で genuine convergence、4th adversarial review が "no compromise found in fundamental spec design、wording-only residual gaps" と評価。

**Framework 改善 candidate (iteration v6 minor で発見、I-D PRD scope に追加 integrate)**:

- (改善 v6-1) `audit-prd-rule10-compliance.py` の **`_` arm self-applied compliance check** mechanism 追加 — PRD doc 内 introduce される predicate / dispatch fn の Rust pseudocode に対しても Rule 11 (d-1) `_` arm prohibition を auto verify (= `is_executable_mode` 等の predicate も `transform_module_item` と同 standard for compile-time exhaustivity)。本 PRD self-applied check で同 type の self-violation を future PRD でも防止。
- (改善 v6-2) `spec-stage-adversarial-checklist.md` Rule 8 wording 強化 — invariant verification cell lists の exhaustive coverage を "本 PRD scope の Axis X 全 cells" claim と Cartesian product cells の cross-reference で auto verify、library mode vs executable mode 両 partition の coverage gap を syntactic detect (= INV-3 (c) Sync list で v6 minor で発見した "library mode `fn main directly emit` cells 漏れ" pattern を future PRD で防止)。

**Framework 改善 candidate (iteration v5 で発見、I-D PRD scope に追加 integrate)**:

**Framework 改善 candidate (iteration v5 で発見、I-D PRD scope に追加 integrate)**:

- (改善 v5-1) `audit-prd-rule10-compliance.py` の **cross-reference consistency check** mechanism 追加 — 80-cell matrix と各 cross-reference context (In Scope / Out of Scope / Tier 2 reclassify / INV-N verification lists / dispatch tree comments / Test Plan / T1-T9 completion criteria) の **cell # appearance consistency** を auto verify、cells 27/40 等の "missing in N+ sections" pattern を syntactic detect。3rd adversarial review Review insight 1 source。
- (改善 v5-2) `spec-stage-adversarial-checklist.md` Rule 6 wording 強化 — "matrix-driven PRD で 80+ cells × 6+ cross-reference contexts の dense matrix が manual-tracking density limit を超える場合、spec-table-driven generator (= matrix を single source-of-truth として、他 sections を機械的 derive) を使用必須" を recommendation 追加。3rd adversarial review Review insight 2 source。
- (改善 v3-1〜v3-3 + v4-1〜v4-3 と統合): I-D scope に 8 件の framework 改善 candidates を統合 integrate (= R-1 / R-5 / 改善 v3-4/5/6 / 改善 v4-1/2/3 / 改善 v5-1/2)。本 PRD I-224 close 後 I-D PRD 起票で cohesive batch 解決。

**Framework 改善 candidate (iteration v4 で発見、I-D PRD scope に追加 integrate)**:

- (改善 v4-1) `audit-prd-rule10-compliance.py` の **dispatch tree axis-tuple consistency check** mechanism 追加 — 各 in-scope matrix cell の axis values から `(is_exec, kind, has_top_await)` 3-tuple を derive + dispatch tree pseudocode の各 arm の pattern と match、cells fall-through to unreachable!() を syntactic detect (= iteration v4 Critical 1 source)。
- (改善 v4-2) `audit-prd-rule10-compliance.py` の **Critical=0 claim ↔ stale verdict label inconsistency** check mechanism 追加 — Spec Review Iteration Log table 内 sub-rule rows に `(TS-X 後 verify)` 等の stale label が残存している場合、findings count = 0 claim と inconsistency を flag (= iteration v4 Medium 2 source、改善 v3-6 の structural enforcement)。
- (改善 v4-3) `spec-stage-adversarial-checklist.md` Rule 9 (a) wording 強化 — "Spec→Impl Dispatch Arm Mapping table を独立 sub-section として hard-code (= 各 in-scope matrix cell ↔ dispatch tree leaf の 1-to-1 correspondence table)、audit script で本 table の completeness + 1-to-1 invariant を auto verify" (= iteration v4 Medium 1 + High 1 source、A6 cells double-claim 等の dispatch tree 構造的 bug を spec stage で前倒し検出)。

**Framework 改善 candidate (iteration v3 で発見、本 PRD close 時 I-D PRD 起票候補)**:

- (改善 v3-1) `audit-prd-rule10-compliance.py` の `verify_cartesian_product_completeness` function 追加 (= Axis 定義から expected cells 数を計算 + matrix table の cell # と diff、implicit cell omission 検出 mechanism、R-1 source = iteration v2 で C-1 漏れを audit が検出できなかった issue)
- (改善 v3-2) `spec-first-prd.md` に "Spec stage 中の Spec gap 由来 PRD 起票" 手順追加 (R-5 source = iteration v2 で I-226 起票したが framework rule level の formal procedure 不在)
- (改善 v3-3) `spec-stage-adversarial-checklist.md` Rule 5 (5-1) に "fixture 自体の tsx runtime empirical observation で fixture content 正当性 verify" を追加 (改善 v2-1 source、iteration v3 で部分 resolve 済 = `--esm --no-auto-main` flag、framework rule level 整備は I-D scope)
- (改善 v3-4 NEW、third-party adversarial review iteration 由来) `audit-prd-rule10-compliance.py` の **duplicate top-level matrix detection** mechanism 追加 (= adversarial review Critical #1 source = audit script は abbreviation pattern 検出のみで、複数 matrix table 共存 (= iteration 移行時の旧 matrix 残存) を検出しない issue)
- (改善 v3-5 NEW、third-party adversarial review iteration 由来) `audit-prd-rule10-compliance.py` の **dispatch tree pseudocode syntactic validation** mechanism 追加 (= adversarial review Critical #2 source = PRD Design section 内 Rust pseudocode の `match` arm exhaustivity / 重複 patterns / cell # 1-to-1 correspondence を syntactic validate、`rustfmt --check` 互換 rule 化候補)
- (改善 v3-6 NEW、third-party adversarial review iteration 由来) `spec-stage-adversarial-checklist.md` Rule 13 (13-3) wording 強化 = "any (TS-X 後 verify) 等の pending verdict が sub-rule 表に存在する場合、findings count は ≥1 として扱う、pending を全 resolve した後 Critical=0 / High=0 / Medium=0 を claim する order を明示" (= adversarial review Critical #3 source = pending verdict と 0 findings claim の inconsistency、iteration v2 false positive pattern repetition prevention)

## Test Plan

### Unit tests (Implementation stage T2-T9 で追加)

- **`src/transformer/main_synthesis.rs::tests`** (新規):
  - `collect_top_level_executions` の 80 cell matrix の representative cells × expected (MainStmt vec, UserMainKind, is_async_required) tuple (orthogonality-merged cells は representative dispatch verify)
  - `synthesize_fn_main` の cell-by-cell IR token-level expected assert
  - User main rename + `main()` substitution の boundary value (multiple call sites = cell-31 fixture probe)
  - `__ts_main` collision detection (matrix # 9/19/20 + collision-merged cells)
  - Sync / async dispatch trigger condition (INV-3 (c) 4 sub-cases: Trigger 1 only / Trigger 2 only / Trigger 1+2 combined / no trigger)
  - Decl::Var dual-path classifier (`classify_decl_var_path`) の Lit / SideEffect / AwaitInit partition + Library / Executable mode partition
  - **`test_axis_b_b1a_b_c_rename_dispatch_symmetric`**: Axis B B1 3 forms (function decl / const arrow / const fn expr) → `__ts_main` rename + main() substitute symmetric (Rule 1 (1-4-c) compliance)
  - **`test_axis_e_export_preserve_symmetric`**: Axis E E1 form で `pub` modifier preserve + dispatch logic invariant (Rule 1 (1-4-c) compliance)
  - **`test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`**: 全 in-scope matrix cell ↔ dispatch tree arm の 1-to-1 mapping を structural lock-in (= each cell の (Axis A/B/C) → (is_exec, kind, has_top_await) 3-tuple → expected dispatch arm を assert、Rule 9 (a) compliance)。**iteration v4 Critical 1 (axis-tuple ↔ definition mismatch) の regression lock-in test**、third-party adversarial re-review (3rd round) High 5 fix で test fn name を明示
  - **`test_axis_a5a_compositional_orthogonality_with_b_axis`**: A5a (Stmt::Empty silent skip) と B axis dispatch leaves の orthogonal composition probe (= cell 51 representative + B0/B1/B2/B3 dispatch outputs ↔ cells 53/55/57 expected outputs match、cell 59 = B4 collision priority arm が先行 reject confirm)。third-party adversarial re-review (3rd round) Medium 3 fix で本 test fn name を明示

### Integration tests (T2-T9 で追加)

- **`tests/i_224_namespace_test.rs`** (新規): I-154 namespace reservation 拡張で `__ts_main` reserved verify、user identifier validation で reject path lock-in
- **`tests/i_224_decl_var_dual_path_test.rs`** (新規): Decl::Var Library mode vs Executable mode dispatch boundary value test (= INV (Decl::Var dispatch decision tree) verification)

### Helper test contracts (Spec stage iteration v7 で `#[test] #[ignore]` stub 作成済、Implementation Stage T2/T3 で fill in)

- **`tests/i224_helper_test.rs`** (新規 2026-05-01、137 行、4 stubs `#[ignore]`): Rule 9 (a) helper test contracts NEW per spec-stage-adversarial-checklist v1.6 self-applied integration:
  - `test_dispatch_arm_one_to_one_mapping_per_in_scope_cell` (Rule 9 (a) 80-cell ↔ dispatch arm 1-to-1 mapping、iteration v4 Critical 1 axis-tuple ↔ definition mismatch の structural regression lock-in)
  - `test_axis_b_b1a_b_c_rename_dispatch_symmetric` (Axis B B1 3 forms 共通 `__ts_main` rename + main() substitute、Rule 1 (1-4-c) compliance)
  - `test_axis_e_export_preserve_symmetric` (Axis E E1 `pub` modifier preservation rule、`__ts_main` rename target は private = INV-5 整合)
  - `test_axis_a5a_compositional_orthogonality_with_b_axis` (cells 51/53/55/57/59 orthogonal composition probe)

### Invariants verification tests (Spec stage iteration v7 で `#[test] #[ignore]` stub 作成済、Implementation Stage T1〜T9 で fill in)

- **`tests/i224_invariants_test.rs`** (新規 2026-05-01、228 行、7 stubs `#[ignore]`): Rule 8 (8-c) helper test contracts NEW per spec-stage-adversarial-checklist v1.6:
  - `test_invariant_1_ts_rust_execution_order_byte_exact` (INV-1 source-order preservation、T5/T9 fill in)
  - `test_invariant_2_user_main_symbol_preservation_with_multi_call_subcase` (INV-2 user main rename + cell-31 multi-call boundary value、T3 fill in)
  - `test_invariant_3_sync_async_dispatch_consistency_4_subcases` (INV-3 sync/async dispatch 4 sub-cases、T2/T3 fill in)
  - `test_invariant_4_no_pub_fn_init_in_codebase_post_t4` (INV-4 `pub fn init` mechanism 廃止、T4/T5 fill in)
  - `test_invariant_5_ts_main_namespace_reservation_with_collision_priority` (INV-5 namespace + collision priority、T1 fill in)
  - `test_invariant_6_type_resolver_layer_unaffected` (INV-6 layer separation、T2 fill in)
  - `test_invariant_7_pub_fn_init_external_api_audit_post_t4` (INV-7 external API audit、T5 fill in)

### E2E tests (TS-3 で red 状態 lock-in、T5 + T9 で green 化)

- **`tests/e2e/scripts/i-224/cell-NN-*.{ts,expected}`** (既存 27 fixtures、含 cells 14-18/30 が iteration v3 で ESM-mode oracle re-recorded、cell-27a/cell-05 fidelity 修正 + NEW ~13 fixtures pending Implementation stage TS-3): per-cell E2E fixture (third-party review Medium #1 fix で empirical fixture count 訂正)
- **`tests/e2e_test.rs`**: per-cell test fn entries (`run_cell_e2e_test("i-224", "cell-NN-*")`)
  - Test fn naming pattern 統一 (M-4 fix): `test_e2e_cell_i224_<NN>_<semantic_name>` (e.g., `test_e2e_cell_i224_09_ts_main_collision_no_exec`、`test_e2e_cell_i224_14_top_await_no_main`)
  - Regression lock-in cells (matrix # 1/3/5/7/21/23/27 + 51 silent skip + 61 Tier 2 reclassify regression) も同 naming pattern で entry 必須
- **I-205 cell-09 e2e fixture**: 本 PRD T5 で `#[ignore]` 解除

### SWC parser empirical tests (TS-2 で作成、iteration v3 完成)

- **`tests/swc_parser_top_level_await_test.rs`** (新規 2026-05-01、4 tests passing):
  - `test_top_level_bare_await_parses_as_stmt_expr_await_axis_a1`: A1 partition lock-in
  - `test_top_level_var_decl_with_await_init_parses_as_decl_var_axis_a3`: A3 partition lock-in
  - `test_pure_axis_a0_source_contains_no_await_expression`: A0 partition lock-in
  - `test_axis_c1_implies_a1_or_a3_partition_synthesis`: C1 forms (4 variations) → A1/A3 collapse synthesis lock-in

### Snapshot tests

なし (本 PRD は IR-level emission concern、snapshot test は不要)

## Completion Criteria

**Matrix completeness requirement (最上位完了条件)**: Problem Space matrix の全 80 cells (Cartesian product 完全 enumerate、Axis A 8 × Axis B 5 × Axis C 2 = 80、Axis E orthogonality merge declaration) に対するテストが存在し、各 cell の実出力が ideal 仕様と一致 (✓ cells = regression lock-in、✗ cells = green 化、Tier 2 reclassify cells = honest error 出力 verify、NA cells = SWC parser empirical で structural lock-in、orthogonality-merged cells = representative dispatch test で覆う)。1 cell でも未カバー、または「多分 OK」で済ませた cell があれば PRD は未完成。

**Quality gates**:

- `cargo test` 全 pass (lib + integration + e2e + compile_test)
- `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
- `cargo fmt --all --check` 0 diffs
- `./scripts/check-file-lines.sh` OK (全 file < 1000 lines)
- `python3 scripts/audit-ast-variant-coverage.py --files src/transformer/mod.rs --verbose` で `_` arm violation 0 件 (本 PRD scope file)
- `python3 scripts/audit-prd-rule10-compliance.py backlog/I-224-top-level-fn-main-mechanism.md` PASS
- `scripts/audit-no-pub-fn-init.sh` (TS-7 で新規作成、T6a で CI integrate) で codebase 0 hits
- `cargo test --test swc_parser_top_level_await_test` 4 tests passing (Rule 3 (3-2) lock-in)

**Tier-transition compliance (broken-fix PRD として、`prd-completion.md` 適用)**:

- Pre-PRD state: existing Tier 2/Tier 1 broken
  - Axis C0 cells: matrix # 11/13/15/17/19/31/33/35/37/71/73 が E0601 compile fail or silent semantic change
  - Axis C1 cells: matrix # 12/14/16/18/20/32/34/36/38/40/72/74/76/78/80 が cjs context で compile fail (top-await rejected)
  - Tier 2 cells: matrix # 9/19/20/29/39/49/59/61/69/79/80 (collision + Debugger + control-flow) が generic `_ => Err(UnsupportedSyntaxError)` で wording precision 不足
- Post-PRD state: Tier 1 (compile-pass + tsc runtime stdout 一致) for cells in 本 PRD scope (Axis C0 + Axis C1 cohesive batch)
- Hono bench result classification:
  - **Improvement** (allowed): existing related errors transition Tier-2 → Tier-1 (clean files count 増加 / errors count 減少 = expected for Tier 2 broken-fix)
  - **Preservation** (allowed): existing related errors unchanged (Hono が top-level executable form を主要使用していない場合の正常な観測結果)
  - **New compile errors** (prohibited): 本 PRD 修正範囲外の features に対して新たな compile error 導入は **regression** = 完了 block

**Impact estimates verified by tracing actual code paths**: 本 PRD scope cells のうち少なくとも 4 representative instances (Axis C0 sync = matrix # 11、Axis C0 + user main = matrix # 13、Axis C0 + side-effect init = matrix # 31、Axis C1 with top-await + async user main = matrix # 16) で TS source → 生成 Rust → cargo run stdout → tsc / tsx stdout の全 chain を empirical trace、本 PRD fix が specific failure point を解消することを verify。

## Pre-Implementation Audit Findings (TS-7 で empirical record、third-party review R-2 + R-4 fix)

iteration v3 で TS-7 task として codebase + Hono + e2e fixture の empirical scan を実施 (2026-05-01)。本 audit は INV-5 (`__ts_main` collision detection reachability) + INV-7 (`pub fn init` external API audit) prerequisite verification を構造的に保証する。

### R-4 audit: `__ts_main` user-defined identifier collision

**Method**: `grep -rn '__ts_main' src/ tests/ tools/`

**Findings (2026-05-01 record)**:

- **Production source (`src/`)**: 0 hits. 本 PRD synthesized `__ts_main` 識別子 + 既存 `__ts_old` / `__ts_new` / `__ts_recv` 等の I-154 namespace constant のみ、user-defined ではない。
- **Test infrastructure (`tests/` 配下、本 PRD 自身の test fixtures を除く)**: 0 hits. 既存 e2e fixture / unit test / integration test に user-defined `__ts_main` identifier 不在。
- **Tools (`tools/`)**: 0 hits.
- **本 PRD 自身の test fixtures (`tests/e2e/scripts/i-224/cell-{05,13,18,31}-*.{ts,expected}`)**: 25+ hits. 全て本 PRD で意図的に作成した collision detection / multi-call substitution test fixture (matrix # 9/19/20 + cell-31 INV-2 sub-case)、本 audit の対象外。

**Hono codebase (`/tmp/hono*`)**: Hono benchmark target は本 audit 時点で未 fetch state (= `./scripts/hono-bench.sh` 実行で auto-clone される)、Implementation stage T5 で Hono bench Tier-transition compliance verify 時に同 grep で 0 hits empirical 確認 (= INV-5 Tier-transition prerequisite)。

**判定**: ✓ **0 reachable user-defined `__ts_main` collision in scoped paths** (本 PRD 自身の test fixture を除く)。INV-5 reachability prerequisite 満たす = `__ts_main` reservation extension は existing user code に breaking change を引き起こさない。Hono codebase verify は T5 で実施。

### R-2 audit: `pub fn init` external API breaking change reachability

**Method**: `grep -rn '\bpub fn init\b' src/ tests/ tools/` (definition site enumerate) + `grep -rn '\binit\s*(' tests/e2e/rust-runner/` (call site enumerate)

**Findings (2026-05-01 record)**:

| Hit type | Location | Path classification | Action |
|----------|----------|-----------|--------|
| **Definition site (production)** | `src/transformer/mod.rs:701` | doc comment ('/// into a \`pub fn init()\` ...') describing `build_init_fn` helper | T4 で `build_init_fn` 削除と同時に doc comment も削除 |
| **Definition site reference (test)** | `src/transformer/tests/module_items.rs:181` | test comment ('// Top-level expression like \`console.log("init")\` → pub fn init() { ... }') describing pre-PRD behavior | T4 で test を新 form (= fn main synthesis) に migrate、comment update |
| **Generated snapshot artefacts** | `tests/e2e/scripts/i-205/cell-{21,38,39}-*.rs` | 3 件 generated Rust files containing `pub fn init() { ... }` from pre-PRD output | T5 で e2e re-run = generated snapshot が `fn main` synthesis に regenerated、advisory hits 自動 clear |
| **Call site (production)** | `tests/e2e/rust-runner/`, `src/`, `tools/` | 0 hits | INV-7 reachability prerequisite ✓ = breaking change reachable surface 不在 |
| **PRD-related comment references** | `tests/e2e_test.rs:2287`, `tests/e2e/scripts/i-224/cell-{09,10,11}-*.ts` | 4 件 (本 PRD background reference + I-205 T12 ignore message) | T5 で I-205 T12 ignore message update + 本 PRD scope 全 fixture green 化で comment は historical reference として keep |

**判定**: ✓ **`pub fn init` 廃止は external API breaking change なし** (= call site 0 件)。本 PRD T4 で safe に廃止可能、Implementation Stage 移行 block する reachable surface area 不在。

### `scripts/audit-no-pub-fn-init.sh` script (M-3 fix、TS-7 で新規作成)

iteration v3 で `scripts/audit-no-pub-fn-init.sh` 新規作成 (2026-05-01)。動作:

- **Enforced paths** (= violation 検出で exit 1): `src/`, `tools/`, `tests/e2e/rust-runner/`
- **Advisory paths** (= advisory print のみ、exit 0 影響なし): `tests/e2e/scripts/` (= generated snapshot artefacts、e2e re-run で自動 clear)
- **Pattern**: `\bpub\s+fn\s+init\b` (Rust source files only、`*.rs` filter)
- **Pre-T4 expected behavior**: exit=1 with 2 src/ hits + 3 advisory hits (= 本 audit 時点 record state)
- **Post-T4 expected behavior**: exit=0 (= INV-4 lock-in、T4 で `build_init_fn` helper 削除 + test comment migrate + T5 e2e re-run で advisory hits 自動 clear)
- **CI integration target**: T6a で `.github/workflows/ci.yml` に integrate、PR merge gate

## References

- 関連 PRD: I-205 (T14 prerequisite block 由来、本 PRD direct beneficiary)、I-225 (B3 class field type inference、I-205 T14 sister prerequisite)、I-162 (constructor synthesis、cells 31-40 init expression conversion prerequisite)、I-016 (module-level const Call/Ident init、library mode counterpart、本 PRD と orthogonal scope)、I-221 (top-level Module-level statement TailExpr noise、本 PRD と隣接 area の独立 sub-defect)、I-180 (E2E harness async-main multi-execution、test infra defect、本 PRD T7 ESM upgrade と隣接だが orthogonal architectural concern)、I-154 (`__ts_` namespace reservation rule、本 PRD で `__ts_main` 拡張)、I-203 (codebase-wide AST exhaustiveness compliance、本 PRD scope 外 `_` arm violations の defer 先 + A4 control-flow Tier 1 化候補)
- **撤回 PRD**: ~~I-226 (test harness ESM support + top-level await Tier 1)~~ → iteration v3 で **本 PRD I-224 Option β cohesive batch に統合** (third-party review H-2 fix、user 承認 2026-05-01)、I-226 entry を TODO + plan.md chain から削除
- 関連 rule: `.claude/rules/spec-first-prd.md` / `.claude/rules/spec-stage-adversarial-checklist.md` (13-rule) / `.claude/rules/check-job-review-layers.md` (4-layer review) / `.claude/rules/post-implementation-defect-classification.md` (5-category) / `.claude/rules/problem-space-analysis.md` / `.claude/rules/ideal-implementation-primacy.md` / `.claude/rules/conversion-correctness-priority.md` / `.claude/rules/prd-completion.md` / `.claude/rules/type-fallback-safety.md` (本 PRD は N/A) / `.claude/rules/testing.md` / `.claude/rules/design-integrity.md` / `.claude/rules/pipeline-integrity.md` / `.claude/rules/incremental-commit.md` / `.claude/rules/pre-commit-doc-sync.md`
- 関連 doc: `doc/grammar/ast-variants.md` (ModuleItem / Stmt / Decl / Expr Tier 1/2 reference)
- 関連 handoff: `report/I-224-spec-stage-v3-review-handoff.md` (iteration v3 開始時点の議論経緯 + 16 findings + 5 review insights + Option α/β/γ 設計判断、Option β 採用後 archive 候補)
- discovery date: 2026-05-01 (PRD I-205 T14 着手判定調査由来)
- iteration history: v1 (initial draft 2026-05-01) → v2 (TS-1〜TS-4 完了 + Axis C scope narrowing + I-226 起票 2026-05-01) → **v3 (Option β cohesive batch、third-party review 21 actions fix、I-226 撤回 2026-05-01)**
- user 承認: 2026-05-01 (案 β Phase 1-A 採用 + Discovery Q1-Q4 全推奨採択)
