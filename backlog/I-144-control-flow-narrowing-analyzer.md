# I-144: Control-flow narrowing analyzer (CFG-based type narrowing infrastructure)

**Status**: Implementation stage 進行中 — **T0-T5 + T6-1 + T6-2 + I-169 T6-2 follow-up + T6-3 完了** (T6-3: 2026-04-21)、T6-4 着手可能 (phase 分割 plan は `plan.t6.md`)
**Matrix-driven**: ✅ Yes (Trigger × LHS type × Reset cause × Flow context × **Read context** × Emission)
**SDCDF 2-stage workflow 適用**: 必須
**起票日**: 2026-04-19
**Revise 履歴**:
- v1 (2026-04-19): 初稿、4 次元 (T × L × R × F × E) matrix、T0 observation 完了
- v2 (2026-04-19): レビューで E 次元が「使用状況 cluster」と「Rust AST pattern」の混同判明。**Read Context 次元 (RC)** を新設、E 次元を AST pattern に純化、T 次元拡張、JS coerce_default table を追加
- **v2.1 (2026-04-19)**: T2 `/check_job` Spec Stage adversarial review で 7 gap 発見。主要 2 件を解消:
  (D3) E4 `match exhaustive` と I-025 complex case の semantic 矛盾 → E5 を E5a/b に分割 (後に v2.2 で rollback)。
  (D4) Closure reassign の Rust emission が未 pin → **Phase 3b "Closure Reassign Emission Policy"** section を Design に追加 (Policy A: FnMut + `let mut` / Policy B: `Rc<RefCell<>>`、escape 検出で切替)。
  副次 4 件 (D1 L2 Union 表記 / D2 E 変数カウント 11→12 / D5 RC1 閉包宣言 / D6 L4×R2a 行追加) も同時修正
- **v2.2 (2026-04-19)**: v2.1 に対する再 adversarial review で 5 defect 発見。解消:
  (R1) E5a/b split は Rust emission 上 semantic 差なし (CFG dominator 分析で tail 収束) → **E5 単一 variant に rollback**、Sub-matrix 3 mapping を単一 E5 に統一。
  (R2) cell-i024 が `!x` on `Option<Union<string, number>>` を exercise するが PRD E10 は primitive truthy のみカバー → **E10 定義を primitive + composite `Option<Union<T, U>>` に拡張** (matches! guard 式で 各 variant 別 truthy を合成)。
  (R3) E 変数 count off-by-one → E5 rollback で 12 に再統一。
  (R4) Policy A NLL 前提が未記述 → Phase 3b に **borrow lifetime 要件 + explicit block scope fallback** 節を追加。
  (R5) F4 loop body / F6 try body / R4 `&&=` / R5 `??=` narrowed の regression ✓ fixture 欠落 — T1 completion criterion (9 ✗ + 3 ✓ 代表) 内だが coverage 強化案として T3 着手後に補充検討 (scope out、lock-in test はこれら cell には既存 snapshot)。
- **T3 実装完了 (2026-04-19)**: `src/pipeline/narrowing_analyzer/` 新設 (events.rs 360 + classifier.rs 908 + mod.rs 227 + 5 分割 test file 計 2253 行)。scope-aware classifier (VarDecl L-to-R / closure param / block decl shadow) + branch/sequential merge combinator + peel-aware wrapper + unreachable stmt prune + closure/fn/class/object-method descent (outer ident → `ClosureReassign`)。`??=` 各 site に `EmissionHint` (ShadowLet / GetOrInsertWith) を hint-only で算出。`/check_job` × 4 round (deep / deep deep × 3) + `/check_problem` で計 42 defect 発見 → 全解消。
- **T4 実装完了 (2026-04-19)**: `NarrowingEvent` struct を `NarrowEvent::{Narrow, Reset, ClosureCapture}` enum に migrate、`FileTypeResolution::narrow_events` rename、`NarrowEventRef` borrowed view + `as_narrow()` / `var_name()` accessor 追加、`PrimaryTrigger` + `NarrowTrigger` 2-layer 型で nested `EarlyReturnComplement` を構造的排除。全 consumer (`type_resolver/narrowing.rs`, `visitors.rs`, Transformer) を borrowed view 経由に統一。`block_always_exits` 削除 → `stmt_always_exits` (narrowing_patterns.rs) を single source of truth 化、共通 peel 関数 + 22 unit test 集約。
- **T5 実装完了 (2026-04-20)**: `type_resolver/narrowing.rs` (524 行) 削除、narrow guard 検出を `narrowing_analyzer/guards.rs` に集約、`NarrowTypeContext` trait で registry access を抽象化、trait boundary 専用 unit test 19 件を追加。
- **I-169 T6-2 follow-up 実装完了 (2026-04-20)**: T6-2 直後の `/check_job` 第三者 review で発見した 5 defect (P1 multi-fn scope conflict / P2 inner-fn local var leak / P3 param-as-candidate 未対応 / R-2 nested-fn shadow walker 欠陥 / D-2 file-size violation + D-3/D-4/D-5 test gap) を structural 解消。`NarrowEvent::ClosureCapture` に `enclosing_fn_body: Span` field 追加、`analyze_function(body, params)` / `collect_emission_hints(body, params)` signature 拡張 (5 callers: visit_fn_decl / visit_method_function / ctor / resolve_arrow_expr / resolve_fn_expr)、`is_var_closure_reassigned(name, position)` / Transformer accessor も position-aware 化、`narrowed_type` suppress を position check で filter、`try_generate_narrowing_match(guard, then, else, guard_position)` signature 拡張、`maybe_coerce_for_arith` / `maybe_coerce_for_string_concat` で `ast_expr.span().lo.0` 使用。`src/pipeline/narrowing_analyzer/closure_captures.rs` 新設 (~740 行、classifier.rs の T6-2 追加分 ~635 行を移動、over-collection 源の `collect_assignment_lhs_idents_in_*` を廃止、`collect_outer_candidates` / `collect_pat_idents` / `collect_top_level_decl_idents` / `remove_pat_idents` + candidate-limited + active shadow-tracking walker に再実装)。class prop init / private prop / auto accessor を `ArrowOrFnBody::Expr` 経由で closure boundary 扱い、matrix cell #14 GREEN。Test: `closure_capture_events` module を別 file (~400 行) に分離、15 test 追加 + position-aware accessor 2 test + structural snapshot 3 + E2E multi-fn isolation fixture。既存 4 ClosureCapture 構築サイトを新 field 追加で適応。Quality gate: lib 2831 pass (+19)、integration 122 / compile 3 / E2E 108 + 4 ignored、clippy 0 / fmt 0 diff、全 file < 1000 行 (classifier.rs 921 / closure_captures.rs 767 / closures.rs 602 / closure_capture_events.rs 469)、Hono bench 0 regression。multi-fn probe empirical verify (g() が match-shadow narrow 発火、f() のみ suppress + coerce)。I-165〜I-168 TODO は「I-169 fix 後の future enhancement」として継続。
- **T6-2 実装完了 (2026-04-20)**: `helpers/coerce_default.rs` 新設 (JS coerce table の (F64, RC1Arith) → `0.0` と (F64, RC6StringInterp) → `"null"` を T6-2 scope 限定実装、5 unit test)。`narrowing_analyzer/classifier.rs` に `collect_closure_capture_pairs` + 走査ヘルパ (`capture_walk_*` / `collect_assignment_lhs_idents_in_*`、~500 行) を追加し、function body を walking して outer ident を reassign する closure (arrow / fn expr / nested fn decl / class member / object method / static block / setter) を全列挙。既存 `classify_closure_body_for_outer_ident` 経由で param/local 由来 shadow を structural に除外。`AnalysisResult.closure_captures: Vec<NarrowEvent>` field を新設し、`analyze_function` が pair から `NarrowEvent::ClosureCapture` を生成 (outer_narrow は T6-2 scope では `RustType::Any` placeholder、Phase 3b emission policy で richer 解決可能)。`TypeResolver::collect_emission_hints` で closure_captures を `narrow_events` に merge、`FileTypeResolution::is_var_closure_reassigned(name)` accessor を追加 (Transformer 側にも 1:1 wrapper)。`narrowed_type` を closure-reassign 検出時 `None` 返す suppress に変更し、既存 `narrow_events` を介した narrow 抑制と Transformer の narrow guard 抑制を整合化。`try_generate_narrowing_match` の `complement_is_none && is_early_return && is_swap` arm に suppress 分岐を追加 (closure_reassigned なら match-shadow ではなく `if x.is_none() { exit }` 形式 emit)。`convert_bin_expr` に `maybe_coerce_for_arith` / `maybe_coerce_for_string_concat` private helper を追加し、Add/Sub/Mul/Div/Mod arithmetic で LHS/RHS が closure-reassigned な Option<T> Ident なら `unwrap_or(coerce_default(T, RC1))` で wrap、string concat (FormatMacro 経路) では `map(|v| v.to_string()).unwrap_or_else(|| "null".to_string())` で wrap。cell-c2b (`x + 1` → `x.unwrap_or(0.0) + 1.0`) / cell-c2c (`"v=" + x` → string coerce 経由 format!) E2E un-ignore (structural GREEN)。`narrowing_analyzer/tests/closures.rs` に `closure_capture_events` テストモジュール (8 test、arrow / fn expr / nested fn decl / class method / object method / shadowing / read-only / nested closures カバー) を追加。既存 `test_narrowed_type_mixed_variants_returns_only_narrow` を T6-2 semantic に沿って 2 test (interleaved Reset only / closure-reassign suppression) に分割。Hono bench non-regression empirical 確認 (clean 112/158 / errors 62 / compile 157/158 / 全数値変動なし)、lib 2812 pass (+15 from 2797)、integration 122 / compile 3 / E2E 107 + 4 ignored (cell-c2b / cell-c2c GREEN、残 4 cell は phase T6-3 〜 T6-5 別 ignore reason)、clippy 0 warn / fmt 0 diff。
- **T6-1 実装完了 (2026-04-20)**: Pipeline wiring + scanner 完全削除 + ??= EmissionHint dispatch (当初 T6 を T6-1〜T6-6 に phase 分割、`plan.t6.md` 参照)。
  **追加新設**: `FileTypeResolution.emission_hints: HashMap<u32, EmissionHint>` field + accessor `emission_hint(stmt_lo)`、`TypeResolver::collect_emission_hints` helper (独立 module `emission_hints.rs`)、5 entry point (fn decl / method / ctor / arrow BlockStmt / fn expr) から `analyze_function` を call、Transformer の `get_emission_hint` accessor + `build_option_get_or_insert_with` IR helper (always-lazy 形で `x.get_or_insert_with(\|\| d)` 固定、TS `??=` lazy semantics に合わせる)。
  **dispatch 書換**: `try_convert_nullish_assign_stmt` の Ident LHS + ShadowLet strategy arm を `Some(GetOrInsertWith) → E2a / _ → E1 shadow-let` の pattern match に変更。
  **scanner 完全削除**: `pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts` + `stmt_has_reset` + `expr_has_reset` + ヘルパ 4 種 (vardecl_init_has_reset / for_head_binds_ident / pat_binds_ident / prop_has_reset) + 8 call site、計 -440 行。T6 / T7 を統合して一括削除 (broken-window 防止)。
  **test 書換**: cell14_* 4 tests を共通 helper `assert_cell14_emits_get_or_insert_with` 経由の structural emission assertion に統一、`test_e2e_cell_i144` 集約 1 関数を per-cell 14 関数に分割 (baseline GREEN 5 + T6-1 GREEN 3 un-ignore / 残 6 phase 別 ignore)。cell-14 fixture の `String(v)` を template literal `${v}` に書換 (I-163 pre-existing defect 回避)。
  **Deep deep review 追加 fix**: `build_option_get_or_insert_with` unit test 4 件 + `emission_hint` accessor unit test 2 件 + 4 entry point regression lock-in (method / ctor / arrow BlockStmt / fn expr に対して `collect_emission_hints` 呼出し削除を guard)、doc drift 3 件修正 (FileTypeResolution.emission_hints field doc key 記述 / nullish_assign.rs module doc / switch.rs `convert_switch_case_body` doc)。pre-existing defect は TODO 起票 (I-163 `String()` callable / I-164 static block TypeResolver 未 visit)。
  **Quality gate**: lib 2797 pass (+10)、integration 122 / compile 3 / E2E 105 + 6 ignored、clippy 0 / fmt 0 diff、Hono bench 変動 0 (clean 112/158 / errors 62 / compile 157/158 不変)。

## Background

### 現状の narrowing 実装と限界

ts_to_rs の narrowing 実装は歴史的に **複数の独立した ad-hoc mechanism** として実装されており、
共通の control-flow graph (CFG) 基盤を持たない:

1. **`type_resolver/narrowing.rs`** (461 行): `typeof` / `instanceof` / null check の
   `if` condition ベース narrowing。`NarrowingEvent { scope_start, scope_end, var_name, narrowed_type }`
   を emit し、Transformer が scope 内の `get_type_for_var` 呼び出しで narrowed type を返す
2. **`nullish_assign.rs` の shadow-let** (I-142): `x ??= d;` stmt 文脈で `let x = x.unwrap_or(d)` を
   emit し scope-local narrow を実現。ただし TypeResolver scope には narrow event を登録しない
3. **`pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts`** (I-142 Step 3 D-1 interim): shadow-let 発動前に後続 stmts を scan し、`x = null;` 等 reset があれば `UnsupportedSyntaxError` surface
4. **`any_enum_analyzer`**: `any`-typed var への `typeof` narrow を any-narrowing enum で
   表現 (I-030 関連)
5. **DU switch narrowing**: discriminated union の switch case arm で tag-based match pattern を emit

### 引き起こされる構造的 defect

| Defect | 原因 | 本 PRD で吸収 |
|--------|------|--------------|
| I-024 `if (x)` complex truthy narrowing | 複雑 case の narrow event 生成欠落 | ✓ |
| I-025 Option return 暗黙 None の complex case | complement narrow 伝播欠落 | ✓ |
| I-142 Cell #14 narrowing-reset | reset 検出が interim scanner (false-positive あり、empirical C-1) | ✓ |
| I-142 Step 4 C-1 scanner false-positive | scanner が `x += 1` 等の narrow-preserving を reset と誤判定 | ✓ |
| I-142 Step 4 C-2 closure body reassign | `shadow-let` emission が TypeResolver scope と不整合、closure 内 `x = 1` が `x = Some(1.0)` で emit → rustc E0308 | ✓ |
| I-142 Step 4 C-3 / C-4 scanner test coverage | scanner 廃止により moot | ✓ |
| I-142 Step 4 D-1 call site DRY | scanner 廃止により moot | ✓ |

### Root cause の統一理解

**Rust は一つの変数に一つの型**。TS の flow-sensitive type narrowing を Rust に写像する際、
narrow 状態と reset 状態を追跡できる **control-flow graph-based analyzer** が必要。現状は:

- TypeResolver scope: 変数宣言時の型を保持、`NarrowingEvent` で scope 単位の override はあるが
  linear control flow (sequential assign) を追跡しない
- Transformer shadow-let: emission 時に scope-local narrow を実現するが、TypeResolver scope と
  同期せず silent compile error を招く (empirical 確認 `report/i142-step4-inv1-closure-compile.md`)

CFG analyzer を導入することで:
- narrow 状態と reset 状態を per-basic-block で追跡
- TypeResolver と Transformer が同一の narrow state を共有 (I-040 原則「TypeResolver scope は IR と整合」遵守)
- closure capture boundary を明示的に表現

---

## Problem Space

本 PRD は **matrix-driven**。6 次元構造 (T trigger × L LHS type × R reset cause × F flow context ×
**RC read context** × E emission) を採り、意味的 sub-matrix (5 種) の cell 単位で ideal 出力を
enumerate する。全次元 cartesian は ~O(10^5) で実施不可能なため sub-matrix に分割。

**v2 追加**: RC 次元 (Read Context) 新設 — narrow 変数がどの expression context で読まれるかで
emission が決まる。RC は F (flow) と orthogonal。

### 入力次元 (Dimensions)

以下 6 次元を `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md` の
reference doc と cross-check して列挙:

#### 次元 T (Narrowing trigger / 発火元)

narrow event を生成する AST pattern (observation 結果を反映、v2 で T9-T12 追加):

| ID | Trigger | AST shape | observation |
|----|---------|-----------|------------|
| T1 | `typeof x === "string"` | `BinExpr(EqEq, TypeOf x, Str)` | ✓ narrowing.rs:100+ |
| T2 | `x instanceof C` | `BinExpr(InstanceOf, x, C)` | ✓ narrowing.rs |
| T3a | `x == null` / `x != null` | `BinExpr(Eq/NotEq, x, Null)` | ✓ narrowing.rs |
| T3b | `x === undefined` | `BinExpr(EqEqEq, x, Ident("undefined"))` | ✓ observed (t3b-eq-undefined-*) |
| T3c | `x !== null` / `x !== undefined` | complement 生成 | ✓ observed (verify-complement-narrow) |
| T4a | `if (x)` truthy on `Option<T>` | → `if let Some(x) = x` | 一部 (I-024 complex case) |
| T4b | `if (x)` truthy on `Any` | any-enum 経由 | ✓ observed (t4b) / I-030 scope |
| T4c | `if (x)` truthy on `String` | 非空 narrow → `!x.is_empty()` | ✓ observed (t4c) |
| T4d | `if (x)` truthy on `Number` | 非 0 ∧ 非 NaN → `x != 0.0 && !x.is_nan()` | ✓ observed (t4d) — **NaN 追加要** |
| T4e | `if (x)` truthy on `Bool` | true narrow → 直接使用 | ✓ 既存 |
| T4f | `if (x)` truthy on `Array/Record/Map` | **常に truthy** (empty でも truthy) | ✓ observed (t4f, l17-stdcollection) |
| T5 | user-defined type guard `x is T` | fn return `x is T` | 未実装 (scope out) |
| T6 | `??=` narrow | `x ??= d` → x inner T 型 | 一部 (I-142 shadow-let) |
| T7 | OptChain `x?.prop !== undefined` | x non-null narrow | ✓ observed (t7, verify-t7) — **compound narrow 対応** |
| T8 | DU switch case | `switch(s.kind) { case "...": }` | ✓ (DU emission) |
| T9 | Negation `!(cond)` | `UnaryExpr(Not, cond)` | ✓ observed (verify-complement-narrow) |
| T10 | Compound `cond1 && cond2` / `cond1 \|\| cond2` | `BinExpr(LogicalAnd/Or)` | ✓ observed (verify-complement, compound-condition) |
| T11 | Early-exit narrow `if (x==null) throw;` | scope 後続で narrow | ✓ observed (verify-complement) — block_always_exits |
| T12 | Short-circuit `x && x.v` | x.v 側で x non-null | ✓ observed (compound-condition) |

#### 次元 L (LHS 型 at narrow entry / pre-narrow type)

narrow される変数の入口での型。`doc/grammar/rust-type-variants.md` 18 variant から抽出:

| ID | Pre-narrow type | narrow 可能性 |
|----|----------------|--------------|
| L1 | `Option<T>` | ✓ T に narrow (null check, truthy, `??=`, OptChain) |
| L2 | Union `T \| U` (= `Named { name, type_args }` の **synthetic enum subtype**、rust-type-variants.md §1 #12 / §6 参照) | ✓ 特定 variant に narrow (typeof, instanceof, DU switch) |
| L3 | `Any` (serde_json::Value) | ✓ 具象型に narrow (typeof → any-enum) |
| L4 | `String` | ✓ 非空 narrow (truthy) |
| L5 | `F64` | ✓ 非 0 narrow (truthy) |
| L6 | `Bool` | ✓ true narrow (truthy) |
| L7 | `Vec<T>` | ✗ TS truthy は配列空でも true、narrow しない |
| L8 | `Named { ... }` (user struct) | ✗ always non-null、narrow 不要 |
| L9 | `DynTrait(name)` | ✗ always non-null (trait object) |
| L10 | `Fn { ... }` | ✓ typeof "function" narrow |
| L11 | `TypeVar { name }` (generic) | 要調査 (narrow は concrete 型要) |
| L12 | `Tuple(...)` | ✗ fixed-length、narrow 不要 |
| L13 | `Result<T, E>` | ✓ Ok/Err narrow (but Result は ts_to_rs 内部 emission、user は書けない) |
| L14 | `Never` | ✗ 到達不能、narrow 不要 |
| L15 | `Unit` | ✗ narrow 不要 |
| L16 | `Primitive(kind)` | L5 同等 |
| L17 | `StdCollection` | 要調査 (HashMap 等の truthy は?) |
| L18 | `Ref(inner)` | ✗ non-null、narrow 不要 (inner が narrow 対象) |
| L19 | `QSelf` | ✗ 関与しない |

#### 次元 R (Reset cause / narrow を無効化する操作)

narrow 状態を無効化する操作:

| ID | Reset cause | narrow 影響 | 現行 scanner |
|----|-------------|-----------|--------------|
| R1a | 直接代入 `x = newValue` | ✓ reset (新型に書き換え) | ✓ 検出 |
| R1b | null 代入 `x = null` | ✓ reset (Option に戻す) | ✓ 検出 (shadow-let blocks) |
| R2a | 算術 compound `x += 1` / `x -= 1` / `x *= 2` etc. | ✗ narrow 維持 (numeric 演算は型変えない) | ✗ scanner false-positive (C-1) |
| R2b | bitwise compound `x &= 1` / etc. | ✗ narrow 維持 | ✗ scanner false-positive (C-1) |
| R3 | Update expr `x++` / `++x` / `x--` | ✗ narrow 維持 (numeric のみ) | ✗ scanner false-positive (C-1) |
| R4 | AndAssign `x &&= y` / OrAssign `x \|\|= y` | ✓ reset (RHS 型で narrow 再計算) | ? |
| R5 | NullishAssign `x ??= y` (既 narrow 状態で) | ✗ narrow 維持 (x が non-null なら no-op) | 要検証 |
| R6 | Pass-by-mutation `doSomething(x)` | TS: narrow 維持 / Rust: ownership 影響あり | 要調査 |
| R7 | Closure capture reassign `() => { x = 1; }` | TS: narrow 維持 (CFG 非降下) / Rust: shadow-let 内の closure 捕獲で不整合 (C-2) | 要 structural 対応 |
| R8 | Loop iteration boundary | ✓ narrow reset per iteration (保守的) | 要調査 |
| R9 | Function boundary (inner fn decl) | ✓ narrow lost at boundary | 要検証 |
| R10 | Method call on x `x.method()` | TS: narrow 維持 / Rust: 同上 | 要確認 |

#### 次元 E (Emission strategy / 生成する Rust AST pattern)

**v2 revise**: E 次元を **pure Rust AST pattern** に純化。使用状況 cluster (「C-2 解消」等) は
Sub-matrix 5 の RC × 状態マッピングで決定する。

| ID | Rust AST pattern | 構造 | 用途概要 |
|----|-----------------|------|---------|
| E1 | Shadow-let | `let x = x.unwrap();` or `let x = x.unwrap_or(d);` | narrow 有効 scope 内の inner T 参照 |
| E2a | `get_or_insert_with` (statement) | `x.get_or_insert_with(\|\| d);` | `??=` stmt、x 保持 Option |
| E2b | `unwrap_or(coerce_default)` (read-only) | `let v = x.unwrap_or(<coerce_default(T)>);` | narrow stale 後の T 読み取り (JS coerce 準拠) |
| E2c | Direct Option read | `x.as_ref().map(\|v\| ...)` 等 | narrow stale 後の Option 直接操作 |
| E3 | `if let Some(x) = x` | `if let Some(x) = x { ... }` | 単一 branch narrow、closure capture 対応 |
| E4 | `match` exhaustive on Option | `match x { Some(v) => ..., None => ... }` | Option<T> に literal match (両 arm binding を明示) |
| E5 | Implicit None at reachable fall-off | CFG reachability 分析で `None` 注入位置を決定 (single-exit は関数末尾に 1 回、multi-exit でも全 fall-off path は dominator=tail に収束するため基本的に tail 挿入で足りる。expression-match / switch 末尾等の例外は per-branch 挿入) | I-025 basic & complex (同一 emission 機構) |
| E6 | Any-enum variant match | `match x { AnyNarrow::String(s) => ... }` | Any-typed typeof narrow (I-030) |
| E7 | DU struct pattern | `Shape::Circle { radius, .. } => ...` | DU switch case (既存) |
| E8 | Union variant bind | `Union::String(s) => ...` | union typeof narrow (既存) |
| E9 | Passthrough (no emission change) | narrow 維持で型同一、binding 変更不要 | `let mut x = 0; x += 1;` |
| E10 | Type-specific truthy predicate | **Primitive** `if (x)` context: `!x.is_empty()` (String) / `x != 0.0 && !x.is_nan()` (F64) / `x` (Bool) / `x != 0` (integer primitive). Falsy (`if (!x)`) は De Morgan 反転。**Composite `Option<Union<T, U>>` + early-return context** (`if (!x) <exit>`): 実装採用形は **consolidated match** — `let x = match x { Some(Union::V(v)) if <v truthy> => Union::V(v), ..., _ => <exit> };` で truthy check + Option unwrap + 外側 narrow への Union 再構築を 1 match に集約。Non-primitive variant payload (Named / Vec / Tuple 等) は JS で常に truthy のため guard なしで `Some(Union::V(v)) => Union::V(v)` を emit。例 `!x` on `Option<string \| number>`: `let x = match x { Some(F64OrString::String(v)) if !v.is_empty() => F64OrString::String(v), Some(F64OrString::F64(v)) if v != 0.0 && !v.is_nan() => F64OrString::F64(v), _ => return "none".to_string() };` | T4c/T4d/T4e primitive + T4a composite (I-024) truthy narrow 述語 |

#### 次元 RC (Read Context / narrow 変数の使用 context) ← v2 新設

narrow された変数が **どの expression context で読まれるか**。`emission-contexts.md` の 51 context
から narrow 関与 subset を emission 要件で cluster 化:

| ID | Read Context | emission-contexts.md 対応 # | 必要な Rust emission |
|----|-------------|---------------------------|---------------------|
| RC1 | **Expect-T-value** (直接 inner T 読取) | #1/#2/#3/#6/#7/#9(arith)/#12/#13/#18-20/#25/#26/#27-31/#33-35/#39/#41/#42/#43/#45/#46/#48/#49/#50 (**= RC2-RC8 に含まれない全 T-expected context の閉包**) | narrow alive: 直接 T binding / stale: `.unwrap_or(coerce_default(T))` |
| RC2 | **Expect-Option<T>** (Option として読取) | #10 NC LHS / #11 NC RHS / #47 OptChain receiver | narrow alive: Option 保持 or `Some(wrap)` / stale: Option 直接 |
| RC3 | **Mutation target** (`??=`, `=` 等) | #6 (stmt 左辺) / #8 NullishAssign | stmt: E2a `get_or_insert_with` / `=`: Option reassign |
| RC4 | **Boolean / truthy read** | #14-17 / #24 | type-specific truthy (E10) or `.is_some()` |
| RC5 | **Match discriminant** | #22 switch discriminant | match on narrow T or Option |
| RC6 | **String interp / concat** | #38 template / #9 `+` with String | narrow alive: `.to_string()` / stale: `.map_or("null", \|v\| v.to_string())` |
| RC7 | **Callback body capture** | #32 callback body | F8 scope 可視性ルール適用 (outer narrow 可視性) |
| RC8 | **Expression stmt / paren passthrough** | #4 / #44 | inherit from outer、emission 無変更 |

**Key observation (empirical, rc-validation.ts)**: RC1-RC8 全ての context で narrow 動作を TS で確認済。
RC 次元は F (flow context) と orthogonal: 同じ narrow が F1 sequential + RC1 expect-T でも、
F8 closure + RC2 Option 保持でも使われ得る。

#### 次元 F (Flow context / control-flow 位置)

narrow 発生位置の control-flow 構造:

| ID | Flow context | narrow scope |
|----|--------------|-------------|
| F1 | Sequential (linear stmts in block) | 発火点以降 block 末尾まで |
| F2 | if then-body | 発火 condition の then-body scope |
| F3 | if else-body (complement narrow) | 発火 condition の else-body scope |
| F4 | while/for body | body scope (per iteration reset) |
| F5 | switch case arm body | 該当 arm body scope |
| F6 | try body | try block 内 |
| F7 | catch body | catch param 型は error (narrow 別系統) |
| F8 | Closure body (inner scope, outer var captured) | closure 内 scope、outer narrow の可視性問題 |
| F9 | Nested fn body | narrow invisible (新 scope) |
| F10 | Labeled block | label scope (I-158 後対応) |

### 組合せマトリクス (次元交差の enumerate)

完全 cartesian (T × L × R × E × F) は ~9600 cell で実施不可能。代わりに **意味のある部分集合**
を列挙する 4 sub-matrix で構成:

#### Sub-matrix 1: Trigger × LHS type (narrow event 生成の有効性)

| T / L | L1 Option | L2 Union | L3 Any | L4 String | L5 F64 | L6 Bool | L10 Fn | 他 |
|-------|----------|---------|--------|----------|--------|---------|--------|-----|
| T1 typeof | - | ✓ 既存 | ✓ any-enum | NA | NA | NA | ✓ | NA |
| T2 instanceof | - | ✓ 既存 | ✓ any-enum | NA | NA | NA | NA | Named: ✓ |
| T3a `x==null` | ✓ 既存 | ✓ (observed T3b cf.) | ✓ 既存 | NA | NA | NA | NA | NA |
| T3b `x===undefined` | ✓ observed (narrow→T) | ✓ observed (union variant narrow) | ✓ any-enum `is_undefined` | NA | NA | NA | NA | NA |
| T4a if truthy Option | ✓ T6-3 (cell-i024 consolidated match: `let x = match x { Some(Enum::V(v)) if <v truthy> => Enum::V(v), _ => <exit> }`) | - | - | - | - | - | - | - |
| T4b if truthy Any | - | - | ✓ any-enum (I-030 scope) | - | - | - | - | - |
| T4c if truthy String | - | - | - | ✓ T6-3 predicate `!x.is_empty()` (cell-regression-t4c) | - | - | - | - |
| T4d if truthy Number | - | - | - | - | ✓ T6-3 predicate `x != 0.0 && !x.is_nan()` (cell-t4d) | - | - | - |
| T4e if truthy Bool | - | - | - | - | - | ✓ T6-3 identity (cell-regression-t4e) | - | - |
| T4f if truthy Array | - | - | - | - | - | - | - | NA (TS 常に truthy — const-fold 別 PRD) |
| T6 `??=` | **✗ I-142 Cell #14** | - | **✗ I-142 Cell #5/9 (I-050 依存)** | - | - | - | - | - |
| T7 OptChain | **Enhance** (compound narrow via `x?.v !== undefined` → x non-null) | - | - | - | - | - | - | - |
| T8 DU switch | - | ✓ (tag field 経由) | - | - | - | - | - | - |

**凡例**: ✓ (既存または明確に動作) / - (該当外) / NA (意味をなさない) / ✗ (broken、本 PRD 対象) / Enhance (本 PRD で強化)

**本 PRD scope**: ✗ cell (I-024/I-025/I-142 関連) + Enhance cell の structural 解消。
observation 詳細は `report/i144-spec-observations.md` 参照。
✓ cell は regression lock-in test でカバー (現行挙動維持)。

#### Sub-matrix 2: LHS type × Reset cause (narrow 維持/リセット判定)

| L / R | R1a 代入 | R1b null代入 | R2 算術compound | R3 update expr | R4 `&&=`/`||=` | R5 `??=` | R6 mutate call | R7 closure reassign |
|-------|---------|-------------|----------------|----------------|---------------|---------|-----------------|--------------------|
| L1 Option→T | reset → Option に戻す | reset → None (Option 戻し) | NA (non-numeric) | NA | **TS ✓ preserved / Rust ✗ `&&=` 基本 emission 欠陥** (I-new: `x = x && 3.0` で && が f64 に非適用、別 PRD scope) | ✓ elide (no-op, observed + T1 empirical GREEN) | ✓ preserved (observed R6) | **✗ C-2 silent-compile (E2 経路要)** |
| L1 Option→T (T=F64) | reset | reset (None) | **✗ C-1 false-positive** | **✗ C-1 false-positive** | TS ✓ / Rust ✗ (同上) | ✓ elide | ✓ preserved | **✗ C-2** |
| L2 Union→T | reset | NA | 維持 (T 内 arith) | 維持 | ✓ preserved | ✓ elide (R5 observed) | ✓ preserved | closure reassign 稀 (要再観測) |
| L3 Any→T | reset | reset (Value::Null) | any-enum 再 widen (I-030) | any-enum 再 widen | any-enum 維持 | ✗ (I-050 依存) | any-enum 維持 | any-enum 維持 |
| L4 String (non-empty narrow) | reset | NA | **✓ preserved** (`s += "x"` は `String` 内 concat で narrow-preserving; runtime: 非空 narrow は `"" + "x"` 経路を除外済 = narrow 維持で安全) | NA | ✓ preserved | NA | ✓ preserved | closure reassign 稀 |

**本 PRD scope**: ✗ cell を structural 解消。C-1 (compound/update narrow 維持) + C-2 (closure
capture reassign shadow-let 不整合) が主要対象。R5 `??=` on narrowed は observation により
**predicate elide** が ideal (I-142 Cell #14 structural 解消の中核)。

#### Sub-matrix 3: Narrow state × Emission strategy (変数の narrow 状態と AST pattern 選択)

**v2 revise**: E 次元純化により、「使用状況」は RC 次元 (Sub-matrix 5) に移動。本 sub-matrix は
**narrow state (alive/stale/未発火) × 代表的 T-L 組合せ → 選択される E AST pattern** に限定。

| Narrow 発生元 T | LHS L | Narrow state | 選択 E AST pattern |
|----------------|-------|-------------|------------------|
| T4a Option truthy | L1 | alive (reset なし、closure なし) | E1 shadow-let (現行) |
| T4a Option truthy | L1 | alive (if-then 単一 branch) | E3 if-let Some |
| T4a Option truthy | L1 | stale (reset あり) | RC 依存 (Sub-matrix 5) |
| T4a Option truthy | L1 | stale (closure capture あり) | RC 依存 (Sub-matrix 5) |
| T6 `??=` narrow | L1 | stmt 文脈 | E2a `get_or_insert_with` |
| T1 typeof | L2 Union | alive | E8 variant binding (既存) |
| T1 typeof | L3 Any | alive | E6 any-enum variant (既存) |
| I-025 Option return implicit None | L1 | alive (tail None) | E5 implicit None (関数 tail に単一 fall-off、typical case) |
| I-025 complex (multi exit) | L1 | alive (multi branch) | E5 implicit None (multi branch fall-off の全 path は dominator=tail に収束 → tail 挿入 1 回で cover。expression-match 等の例外時のみ per-branch fallback) |
| T8 DU switch | L2 (synthetic enum) | alive (arm) | E7 struct pattern bind |

#### Sub-matrix 4: Flow context × narrow propagation (narrow state の scope と lifetime)

| F context | narrow propagation 挙動 | 現行 実装 | 本 PRD 対応 |
|-----------|----------------------|----------|------------|
| F1 Sequential | 発火点以降 block 末尾まで narrow | ✓ TypeResolver scope 連動 | 維持 |
| F2 if then-body | then scope 内 narrow | ✓ narrowing.rs の NarrowingEvent | 維持 (CFG 分析で置換可能) |
| F3 if else-body (complement) | else scope で complement 型 | ✓ (typeof の負 variant 算出) | 維持 |
| F4 Loop body | **reassign 有無で分岐**: reassign なし → narrow 維持 / reassign あり → loop head で widen (fixpoint, observed) | **T1 empirical**: reassign なし case は ✓ GREEN (`cell-regression-f4-loop-body-narrow-preserves.ts`)。reassign あり case は未検証 | **E1/E2 切替** (reassign あり → E2 `let mut Option`) |
| F5 Switch case arm | arm scope 内 narrow | ✓ DU emission / union variant | 維持 |
| F6 Try body | try 内 narrow、catch 到達で widen (observed) | **T1 empirical ✗** (Rust emission broken: `throw` が関数 signature 無視で `return Err(...)` emit、catch body 欠落、narrow + reassign 崩壊) | **I-149 scope** (try/catch emission の structural 刷新)。本 PRD では E2E fixture lock-in 不能 |
| F7 Catch body | catch param 独立 | ✓ catch_body emission | 関与なし |
| F8 Closure body | **outer narrow の可視性問題** | **✗ C-2 broken** | **本 PRD 核心 (E2 経路選択)** |
| F9 Nested fn body | narrow 不可視 (新 scope) | ✓ (scope lookup 境界) | 維持 |
| F10 Labeled block | I-158 完了後対応 | — | 別 PRD |

#### Sub-matrix 5: Read Context × Narrow State × LHS type → Emission (v2 新設)

**最重要 sub-matrix**: RC 次元導入の主目的は、narrow 変数が「どう読まれるか」で emission が
決まる構造を明示すること。C-2 (closure reassign) の正しい解は RC 毎に異なる。

| RC | L1 alive (narrow 有効) | L1 stale (reset/closure 後) | L3 Any (alive) |
|----|---------------------|--------------------------|---------------|
| RC1 Expect-T | E1 shadow-let (現行) | **E2b `.unwrap_or(coerce_default(T))`** | E6 any-enum variant |
| RC2 Expect-Option | E9 passthrough (Option 保持) | E9 (Option 直接) | E9 (Value 直接) |
| RC3 Mutation (`??=` stmt) | E9 (narrow state では no-op predicate elide) | **E2a `get_or_insert_with(\|\| d)`** | ✗ (I-050 scope) |
| RC4 Boolean | E9 (narrow alive で常に truthy) | **E10 `.is_some()` or type predicate** | E10 any-enum truthy |
| RC5 Match disc | E1 + match on T | match on Option | match on Value |
| RC6 String interp | E1 + `.to_string()` | **E2b with "null" default** | any-enum Display |
| RC7 Callback body | E3 `if let Some(x) = x { closure }` (capture narrow) | **E3 or E2c, narrow 不伝播** | any-enum capture |
| RC8 Passthrough | inherit outer | inherit outer | inherit outer |

**L1 stale (closure reassign) の emission**: RC によって異なる AST pattern:
- RC1 (arithmetic 等): `x.unwrap_or(coerce_default(T))` — JS coerce 準拠
- RC3 (mutation): `x.get_or_insert_with(|| d)` — stmt effect
- RC4 (boolean): `.is_some()` — runtime null → falsy
- RC6 (string concat): `.unwrap_or("null".to_string())` — JS `null + "s" = "nulls"`

これにより **C-2 "解消" の正確な定義**:
- C-2a (`??=` + closure capture): RC3 → E2a
- C-2b (closure reassign + arithmetic read): RC1 stale → E2b
- C-2c (closure reassign + string concat): RC6 stale → E2b (string default)
- C-2d (closure reassign + return): RC1 stale + return type 対応 → E2b or signature widen

### Matrix Completeness Audit

- [x] T (trigger) 17 pattern を enumerate (v2 で T3c/T9/T10/T11/T12 追加): typeof/instanceof/null/truthy(×6 LHS)/type-guard/??=/OptChain/DU switch/Negation/Compound/Early-throw/Short-circuit
- [x] L (LHS type) 18 RustType variant + subtype を列挙、narrow 可能性判定済
- [x] R (reset cause) 10 pattern を AssignOp / UpdateExpr / 他 mutation pattern から enumerate、property/element reset は **scope-out** 明示
- [x] E (emission) 12 pattern (v2 で E2 を E2a/b/c 分割、E10 追加; v2.2 で E10 を primitive + composite `Option<Union>` に拡張) を Rust AST pattern として純化
- [x] F (flow context) 10 pattern を statement kind + function/closure boundary から enumerate
- [x] **RC (read context) 8 pattern を `emission-contexts.md` の 51 context から narrow 関与 subset として enumerate** (v2 新設)
- [x] Sub-matrix 5 種でカバー (v2 で Sub-matrix 5 追加)、N-D cartesian は意味的部分集合のみ
- [x] 要調査 cell: T3b, T3c, T4b-f, T7, T9-T12, R4/R5/R6, F4/F6, Closure×Loop, L11/L17, RC1-RC8 → **tsc observation 完了** (`report/i144-spec-observations.md`)
- [ ] ✗ cell (C-1, C-2a/b/c/d, I-024 complex, I-142 Cell #14, I-025 complex): 本 PRD で structural 解消
- [ ] ✓ cell: regression lock-in test で担保 (既存動作維持)
- [ ] **JS coerce_default table** を Semantic Safety Analysis に明記 (C2 gap 解消)

### tsc observation 対象 cell (Discovery 要解消)

以下 cell は empirical tsc/tsx 観測で ideal 出力を確定する:

| Cell | 再現 TS (draft) | 確認事項 |
|------|----------------|---------|
| T4b truthy Any | `function f(x: any) { if (x) return x; }` | x が truthy 時の Rust narrow (any-enum path の有無) |
| T4c truthy String | `function f(x: string) { if (x) return x; }` | TS は非空 narrow、Rust `!x.is_empty()` で narrow 型変化するか |
| T4d truthy Number | `function f(x: number) { if (x) return x; }` | TS は非 0 narrow、Rust で narrow 型変化するか |
| T4f truthy Array | `function f(x: string[]) { if (x) return x; }` | Array は empty でも truthy、narrow 実質 no-op |
| T7 OptChain | `function f(x: { v: number } \| null) { return x?.v; }` | OptChain 内で x が non-null narrow されるか |
| R4 `&&=` / `\|\|=` | `let x: number \| null = 5; x ??= 10; x &&= 3;` | `&&=` は narrow リセットか維持か |
| R5 `??=` on narrowed | `let x: number \| null = 5; x = 10; x ??= 0;` | 既 narrow 状態での `??=` は no-op 維持か |
| R6 pass-by-mutation | `function f(x: number[]) { mutate(x); return x.length; }` | TS narrow 維持、Rust borrow/move 影響 |
| R7 closure reassign | empirical 確認済 (C-2 `report/i142-step4-inv1-closure-compile.md`) | TS narrow 維持、Rust E0308 |
| F4 Loop body narrow | `let x: number \| null = 5; for (;;) { x; if (cond) x = null; }` | Loop per-iteration narrow reset の要否 |

---

## Goal

本 PRD 完了時に以下を達成:

1. **`src/pipeline/narrowing_analyzer.rs`** (新規、~400-600 行) が CFG-based な narrowing
   分析を提供。関数本体を basic block に分解し、各 block で var × narrow state を計算
2. **`NarrowingEvent` variant 拡張**: 既存の scope-based narrow に加え、`Reset` / `ClosureCapture` /
   `CondBranch` variant を追加
3. **既存 narrowing.rs の機能を CFG analyzer に移行**: typeof/instanceof/null check を重複
   維持せず single source of truth に集約
4. **Transformer shadow-let の置換**: `nullish_assign.rs::try_convert_nullish_assign_stmt` の
   shadow-let emission が CFG analyzer の narrow state を参照し、reset がある scope で
   **E2 経路 (`let mut Option` + `get_or_insert_with`)** を選択、closure capture がある
   scope でも E2 経路を選択 (C-2 解消)
5. **Interim scanner 廃止**: `pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts` を
   削除、代わりに CFG analyzer の narrow-reset event で emission 選択
6. **Matrix 全 cell に対応する test**:
   - Unit test: CFG analyzer 単体 (各 event 生成 + state transition)
   - Integration test: emission 選択 (E1/E2/E3 の branch decision)
   - Per-cell E2E: `tests/e2e/scripts/i144/<cell-id>.ts` で runtime stdout 一致 verify
7. **Hono bench 非後退** (clean 112/158、errors 62 維持以上)
8. **Compile test**: `functions` fixture の I-319 以外の narrow 関連残存が解消 (部分)

### 吸収する既存 defect

本 PRD 完了で以下が自動解消:

- **I-024** `if (x)` complex truthy narrowing (Option<T> 多段 + typeof guard 交差)
- **I-025** Option return 暗黙 None の complex case (複数 exit path)
- **I-142 Cell #14** narrowing-reset (structural emission に昇格、interim surface 除去)
- **I-142 Step 4 C-1** scanner false-positive (compound/update narrow 維持判定)
- **I-142 Step 4 C-2** closure body reassign shadow-let 不整合 (E2 経路選択)
- **I-142 Step 4 C-3 / C-4** scanner test coverage (scanner 廃止により moot)
- **I-142 Step 4 D-1** scanner call site DRY (scanner 廃止により moot)

---

## Scope

### In Scope

- CFG analyzer 新規実装 (`pipeline/narrowing_analyzer.rs`)
- `NarrowingEvent` variant 拡張 (`Reset`, `ClosureCapture`, `CondBranch` 追加)
- 既存 `type_resolver/narrowing.rs` の CFG analyzer への統合 (typeof/instanceof/null check)
- Transformer shadow-let emission の CFG analyzer 連動化 (E1/E2 経路選択)
- Interim scanner (`pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts`) 廃止
- Per-cell E2E fixture: `tests/e2e/scripts/i144/<cell-id>.ts` (matrix cell 相当数)
- `cell14_narrowing_reset_emits_shadow_blocked_by_i144` lock-in test の structural 置換
- Matrix cell ✓ (既存動作維持) の regression lock-in

### Out of Scope

- **I-050 (Any coercion umbrella)**: `??=` on Any LHS (Cell #5/#9) は引き続き blocked。本 PRD
  では T4b truthy Any / T1 typeof Any の既存 any-enum 経路のみ統合
- **I-158 (non-loop labeled stmt)**: labeled block 内 narrow は I-158 emission 安定後
- **I-143 (`??` 演算子完全仕様)**: NC operator は別 PRD
- **T5 user-defined type guard** (`x is T`): TS 4.x の function return type assertion、独立機能、別 PRD
- **Control-flow based exhaustive match analysis**: `if/else if/else` の exhaustiveness を
  narrow 経由で検出 (advanced)、別 PRD 候補
- **Interprocedural narrowing**: 関数境界を越える narrow 伝播、別 PRD (複雑性大)

---

## Design

### Technical Approach

#### Phase 1: CFG analyzer 基盤 (`pipeline/narrowing_analyzer.rs`)

**新規モジュール構造**:

```rust
// src/pipeline/narrowing_analyzer.rs
pub struct NarrowingAnalyzer<'a> {
    registry: &'a TypeRegistry,
    // per-function state
    cfg: BasicBlockGraph,
    var_narrow_state: HashMap<VarId, BlockNarrowMap>,
}

/// per-basic-block narrow state
struct BlockNarrowMap {
    entry_state: HashMap<String, NarrowedType>,  // var -> type at block entry
    exit_state: HashMap<String, NarrowedType>,   // var -> type at block exit
    reset_events: Vec<ResetEvent>,               // reset within block
    narrow_events: Vec<NarrowEntryEvent>,        // narrow introduction within block
}

pub enum NarrowEvent {
    /// Variable narrowed to specific type in scope
    Narrow { var: String, scope: Span, narrowed_type: RustType, trigger: NarrowTrigger },
    /// Variable narrow invalidated (re-widened)
    Reset { var: String, position: u32, cause: ResetCause },
    /// Closure captures var which is narrowed in outer; emission must use E2 path
    ClosureCapture { var: String, closure_span: Span, outer_narrow: NarrowedType },
    /// Branch-specific narrow (then-body vs else-body complement)
    CondBranch { var: String, then_scope: Span, else_scope: Option<Span>, ... },
}

pub enum NarrowTrigger {
    TypeofGuard(String),     // "string", "number", ...
    InstanceofGuard(String), // class name
    NullCheck(NullKind),     // ==null / !=null / ===undefined
    Truthy,                  // if (x)
    NullishAssign,           // x ??= d
    OptChainInvariant,       // x?.y: x is non-null in .y
    DiscriminatedUnion(String), // switch(s.kind) case "..."
}

pub enum ResetCause {
    DirectAssign(RustType),       // x = value
    NullAssign,                   // x = null
    CompoundArith(BinOp),         // x += 1 (narrow 維持 = non-reset)
    CompoundLogical(BinOp),       // x ||= y (narrow 再計算 = reset)
    ClosureMutation(Span),        // captured var reassigned in closure
    LoopIteration,                // loop boundary reset
}
```

**API**:
```rust
impl<'a> NarrowingAnalyzer<'a> {
    pub fn analyze_function(&mut self, body: &ast::BlockStmt) -> AnalysisResult;
}

pub struct AnalysisResult {
    pub narrow_events: Vec<NarrowEvent>,
    pub per_block_state: HashMap<BlockId, BlockNarrowMap>,
    pub emission_hints: HashMap<Span, EmissionHint>,
}

pub enum EmissionHint {
    ShadowLet,              // E1
    LetMutOptionWithInsert, // E2 (reset または closure capture 検出)
    IfLetSome,              // E3
    MatchExhaustive,        // E4
    ImplicitNone,           // E5
    AnyNarrowEnum(String),  // E6
    VariantBinding(String), // E7/E8
    Passthrough,            // E9
}
```

#### Phase 2: 既存 narrowing.rs を CFG analyzer に統合

既存 `type_resolver/narrowing.rs::detect_narrowing_guard` は CFG analyzer から
呼ばれる sub-routine に変更。`NarrowingEvent` 直接生成を `NarrowEvent::Narrow` 経由に統合。

#### Phase 3: Transformer の CFG analyzer 連動

`try_convert_nullish_assign_stmt` を書換:

```rust
// Before (I-142 shadow-let):
NullishAssignStrategy::ShadowLet => {
    vec![Stmt::Let { name: "x", init: Some(unwrap_or(x, d)) }]
}

// After (CFG analyzer 連動):
match emission_hint {
    EmissionHint::ShadowLet => { /* 現行 shadow-let */ }
    EmissionHint::LetMutOptionWithInsert => {
        // reset または closure capture 検出時
        vec![Stmt::Expr(Expr::MethodCall {
            object: Box::new(Expr::Ident("x")),
            method: "get_or_insert_with",
            args: vec![Expr::Closure { body: d }]
        })]
    }
    ...
}
```

同様に `convert_assign_expr::NullishAssign` expression-context arm も CFG analyzer 連動。

#### Phase 3b: Closure Reassign Emission Policy (D4 解消)

Closure が外側 narrow 変数を reassign するケース (C-2a/b/c、Sub-matrix 5 L1 stale) の
Rust emission 手段を**明示 pin する**。選択肢を以下の decision tree で確定:

```
closure 内 `x = null` / `x = value` 検出
  ├── closure が宣言 scope 内で fully consumed (call されて return し、escape しない)
  │    → Policy A: `let mut x: Option<T>` + **FnMut closure** (Rust 標準 idiom)
  │       outer 側 `let mut x`、closure は `|| { x = None; }` (FnMut 自動推論)
  │       borrow checker が scope 内完結を要請 (`reset(); x;` の順序厳守)
  └── closure が escape する (return される / struct field に格納 / async spawn)
       → Policy B: `Rc<RefCell<Option<T>>>` wrapper
          outer `let x = Rc::new(RefCell::new(Some(5.0)));`
          closure `{ let x = x.clone(); move || { *x.borrow_mut() = None; } }`
          consumer `*x.borrow()` で read
```

**Default**: Policy A (FnMut)。C-2a/b/c の empirical 再現 TS は全て **scope 内完結**
のため Policy A で対応可能 (verify-closure-reassign-emission.ts / cl3b で確認済)。

**Policy A borrow lifetime 要件**: FnMut closure は capture 対象 (outer `Option<T>`) に対し
mutable borrow を closure 変数の lifetime 中保持する。closure 呼び出し後に outer x を read
する場合、**Rust NLL (Non-Lexical Lifetimes)** による borrow scope 短縮で両立可能。
NLL で解決できない複雑 case (closure が複数の異なる read/write と interleave 等) は
emitter 側で closure を **explicit block scope で wrap** する fallback を持つ:

```rust
// NLL 解決ケース: closure 最終 call 後に borrow 自動 release
let mut reset = || { x = None; };
reset();          // FnMut borrow ends at last use (NLL)
x.unwrap_or(-99)  // OK

// 複雑 case 用 explicit block:
{ let mut reset = || { x = None; }; reset(); }  // reset dropped before block exits
x.unwrap_or(-99)  // OK even without NLL
```

**Escape 検出アルゴリズム** (T3 の analyzer 内で実装):
1. Closure expr の usage を追跡: `let reset = () => ...;` → `reset()` call のみ → scope 内完結
2. 以下いずれかの検出で escape 判定 → Policy B:
   - closure 変数が `return` される
   - 親関数の callee に渡される (`setTimeout(reset, ...)` 等)
   - struct field / array element に代入される
   - async / promise context に渡される

**Fallback**: escape 検出が曖昧 (分析不能) な場合 Policy B に降格 (conservative)。

**C-2d (closure reassign + return signature 不整合)** は本 PRD scope out。
return type widening が必要で interprocedural 分析要となるため別 PRD。

**Matrix cell にポリシー注記**:
- C-2a (`??=` + closure reassign): RC3 × L1 stale → **E2a + Policy A** (default)
- C-2b (closure reassign + arith read): RC1 stale → **E2b + Policy A**
- C-2c (closure reassign + string concat): RC6 stale → **E2b + Policy A**
- C-2a/b/c escape variant (将来): Policy B 自動切替 (runtime regression 0)

#### Phase 4: 既存 interim 除去

- `pre_check_narrowing_reset` 削除 (`nullish_assign.rs:129`)
- `has_narrowing_reset_in_stmts` 削除 (`nullish_assign.rs:438`)
- 6 call site (statements/mod.rs / switch.rs / classes/members.rs / expressions/functions.rs)
  削除
- D-1 pattern (`iter_block_with_reset_check` 候補) 不要化

### Design Integrity Review

`.claude/rules/design-integrity.md` checklist:

- **Higher-level consistency**:
  - TypeResolver と Transformer の scope 整合 (I-040 原則) を CFG analyzer 経由で構造的保証
  - 既存 `NarrowingEvent` (scope-based override) を superset の `NarrowEvent` に拡張し、
    downstream consumer は hint 経由で emission 選択
- **DRY**:
  - typeof/instanceof/null check detection logic を CFG analyzer に集約、`narrowing.rs`
    との重複解消
  - shadow-let 判定条件 (reset + closure check) を CFG analyzer に集約、interim scanner 廃止
- **Orthogonality**:
  - CFG analyzer は narrow state 計算に単一責務、Transformer は emission hint 消費に集中
  - 既存 `any_enum_analyzer` / `du_analysis` との integration point を明確化 (要確認)
- **Coupling**:
  - 新規 pipeline/narrowing_analyzer.rs は TypeResolver pipeline に追加される phase
  - Transformer は `FileTypeResolution` 経由で immutable data 受領 (`pipeline-integrity.md` 準拠)
- **Broken windows**:
  - I-142 shadow-let 経路が TypeResolver scope と不整合 → CFG analyzer 連動で解消
  - Interim scanner (`pre_check_narrowing_reset`) → 廃止
  - I-024/I-025 complex case の個別 heuristic 修正 → CFG-based 統一解法に置換

**Verified**: design integrity OK、major broken windows 全て本 PRD で解消。

### Impact Area

| File | 役割 | 変更種別 |
|------|------|---------|
| `src/pipeline/narrowing_analyzer.rs` | CFG-based narrow analyzer (新規) | 新規 ~400-600 行 |
| `src/pipeline/type_resolution.rs` | `NarrowingEvent` 定義 | **破壊的変更**: struct → enum migration、全 consumer を一括更新。`narrowing_events` field は `narrow_events: Vec<NarrowEvent>` に rename |
| `src/pipeline/type_resolver/mod.rs` | TypeResolver 本体 | CFG analyzer 呼び出し追加 |
| ~~`src/pipeline/type_resolver/narrowing.rs`~~ | typeof/instanceof detection | **削除済** (T5、2026-04-20)、`narrowing_analyzer/guards.rs` に移管 |
| `src/pipeline/narrowing_analyzer/guards.rs` | typeof/instanceof/null/truthy + early-return complement | 新規 (T5、~430 行) |
| `src/pipeline/narrowing_analyzer/type_context.rs` | `NarrowTypeContext` trait | 新規 (T5、~70 行) |
| `src/pipeline/type_resolver/narrow_context.rs` | `NarrowTypeContext` for `TypeResolver` impl | 新規 (T5、~40 行) |
| `src/transformer/statements/nullish_assign.rs` | `??=` shadow-let emission | CFG analyzer 連動に書換、interim scanner 削除 |
| `src/transformer/statements/mod.rs` / `switch.rs` / `classes/members.rs` / `expressions/functions.rs` | scanner call site | 削除 (`pre_check_narrowing_reset` call) |
| `src/transformer/statements/tests/nullish_assign.rs` 等 | interim surface test | structural emission test に書換 |
| `tests/e2e/scripts/i144/*.ts` | per-cell E2E fixture (新規) | 推定 20-30 fixture |
| `tests/fixtures/nullish-coalescing.input.ts` / 他 | compile_test fixture | narrowing-reset ケース追加 |

### Semantic Safety Analysis

`.claude/rules/type-fallback-safety.md` 準拠。本 PRD は narrow 精度を上げる方向で既存 silent
semantic change を解消する側:

1. **Current silent**: shadow-let が TypeResolver scope と不整合 → closure 内 `x = 1` が
   `Some(1.0)` で emit → E0308 compile error (empirical 確認)。rustc が検知するため silent
   semantic change ではない (Tier 2)
2. **本 PRD 変更**: narrow state (alive/stale) × RC context で E AST pattern を選択し、
   stale 時に E2b (`unwrap_or(coerce_default)`) を適用 → narrow scope 保持したまま closure
   capture 対応、JS 実行時 semantic を保持
3. **Verdict**: Safe。本 PRD は既存 silent/compile issue を解消する側、新規 silent 導入は
   しない (coerce_default table 準拠)

#### JS coerce_default table (v2 追加、C2 gap 解消)

TS の narrow が closure reassign 等で stale 化したとき、runtime の null/undefined を Rust で
再現するための coerce_default。RC1/RC4/RC6 等で `.unwrap_or(coerce_default(T))` として適用。

**JS coercion 規則 (empirical: `tests/observations/i144/verify-null-coercion.ts`)**:

| RC | LHS type | null coerce | undefined coerce | 出典 |
|----|---------|-------------|------------------|------|
| RC1 arithmetic `+`/`-`/`*`/`/` | f64 | `0.0` | `f64::NAN` | `null + 1 = 1`, `undefined + 1 = NaN` |
| RC1 arithmetic | Primitive(int) | `0` (as cast) | N/A (TS は f64 のみ) | — |
| RC1 comparison `===` | T | 型別 sentinel (unreachable で equal false) | 同上 | `null === 5 → false` |
| RC4 truthy | f64 | `false` (null is falsy) | `false` | `if (null) → false` |
| RC4 truthy | String | `false` | `false` | — |
| RC4 truthy | Bool | `false` | `false` | — |
| RC6 String concat `+` | String | `"null"` | `"undefined"` | `null + "x" = "nullx"` |
| RC6 Template interp | String | `"null"` | `"undefined"` | \`${null}\` → "null" |
| RC1 return | T (function sig = T, unsound) | 型別 Option wrap or unreachable panic | 同上 | **別 PRD (signature widen)** |

**適用範囲の限定**:
- coerce_default は narrow **stale** 時のみ適用 (closure reassign 等で runtime null が到達し得る場合)
- narrow **alive** 時は shadow-let の直接 T binding で十分 (runtime null 到達しないため)
- RC1 return (signature 不整合) は **本 PRD scope out** — 返り値型変更が必要で interprocedural

**矛盾 check**: coerce_default を E2b 以外で誤用すると silent semantic change の risk:
- narrow alive 時に E2b を使うと無駄な branch 生成 → perf 影響のみ、semantic は正しい ✓
- narrow stale 時に E1 を使うと `x.unwrap()` → runtime panic → rustc 検知不可だが runtime で explicit panic (silent ではない) ✓

**Verdict**: coerce_default table を導入することで C2 gap 解消、新 silent semantic change の
導入を防止。

---

## Spec-Stage Adversarial Review Checklist (v2 再実施)

`.claude/rules/spec-first-prd.md` 5 項目を本 PRD v2 で再検証:

| # | Checklist item | Status | 根拠 |
|---|---------------|--------|------|
| 1 | **Matrix completeness**: 全 cell に ideal output 記載、空欄/TBD なし | ✅ | Sub-matrix 1-5 全 cell に判定/ideal 出力記載。C-2 は C-2a/b/c/d に分化、C-2d のみ scope out 理由明記 |
| 2 | **Oracle grounding**: ✗/要調査 cell の ideal output が tsc observation log と cross-ref | ✅ | `report/i144-spec-observations.md` + `tests/observations/i144/*.ts` (22 fixture) で全 ✗/要調査 cell を empirical grounding |
| 3 | **NA justification**: NA cell の理由が spec-traceable (syntax error, grammar constraint 等) | ✅ | T4f NA は「empty array/record も truthy」で TS grammar traceable。L8/L9/L12/L14/L15/L18/L19 NA は RustType 構造 traceable |
| 4 | **Grammar consistency**: matrix variant が `doc/grammar/*.md` reference doc に全て存在 | ✅ | T 次元 12 variant は ast-variants.md §5/§6 準拠、L 次元 18 variant は rust-type-variants.md §1 準拠、RC 次元 8 variant は emission-contexts.md 51 context から cluster 化 |
| 5 | **E2E readiness**: 各 ✗ cell の E2E fixture が (red 状態で) 準備 | ✅ T1 完了 (2026-04-19) | `tests/e2e/scripts/i144/` 14 fixture (9 RED ✗ + 5 GREEN ✓) + `test_e2e_cell_i144` (#[ignore])、red 状態 + pre-existing defect 3 件発見の confirm `report/i144-t1-red-state.md` |

**Outstanding**: なし (v2 で #1-#5 全 [✅])。

---

## Spec Revision Log

Implementation stage で発見された spec の曖昧性や変更を `.claude/rules/spec-first-prd.md`
"Implementation → Spec 逆戻り" 手順に従って記録する。

### T6-3 E10 composite emission: `matches! || matches!` → consolidated `match`

- **Date**: 2026-04-20
- **Stage**: Implementation (T6-3)
- **Discovery context**: cell-i024 (`if (!x) return "none"` on `string | number | null`)
  実装時に、`matches!(x, Some(Union::V1(v)) if ...) || matches!(x, Some(Union::V2(v)) if ...)`
  形式は predicate 式として正しい truthy 判定を出力するが、**narrow 状態を外側スコープに
  materialize できない** ことが判明。predicate ONLY 形式だと後続コードで x は依然
  `Option<Union>` のままで、`if (typeof x === "string") return "s:" + x;` の既存 typeof
  narrow path が `Option<Union>` を剥がさず `match x { Some(Enum::String(s)) => ..., ... }`
  を生成できない。
- **Decision**: E10 composite を **consolidated match** 形式に変更:
  ```rust
  let x = match x {
      Some(Enum::V1(v)) if <v1 truthy> => Enum::V1(v),
      Some(Enum::V2(v)) if <v2 truthy> => Enum::V2(v),
      _ => <exit>,
  };
  ```
  この 1 match で (a) composite truthy predicate、(b) Option unwrap、(c) 外側 `x` への
  Union 再 bind を同時に達成する。後続 typeof narrow は narrowed `x: Union` を素直に
  match 可能。
- **NA justification for predicate form**: `matches! || matches!` 形式は `if` 文の
  condition としては正しいが、narrow materialization が不可能。consolidated match は
  let-binding で narrow を外側スコープに伝播できる唯一の Rust 慣用 form。
- **Matrix impact**: Sub-matrix 1 T4a × L1 cell を `✓ T6-3 consolidated match` に更新。
  E10 行定義を consolidated match 記述に差し替え (primitive 単体の場合は従来の predicate
  wrap `if x != 0.0 && !x.is_nan()`、Option<Union> の場合は consolidated match)。
- **Non-primitive variant 拡張 (H-3 fix)**: `string | number | { name: string }` 等の
  mixed union で Named (object) variant は JS 常に truthy。consolidated match 内で
  `Some(Enum::ObjVariant(v)) => Enum::ObjVariant(v)` (guard なし) を emit する規約を
  追加。E10 行定義に明記。
- **Regression test**: cell-i024 (Primitive × 2 variant) の E2E + H-3 (Primitive +
  Named variant) の integration test (`test_try_generate_option_truthy_complement_match_h3_mixed_union_emits_guard_only_for_primitives`) で lock-in。

---

## Task List

TDD: RED → GREEN → REFACTOR 順。Phase 間は SDCDF spec stage / implementation stage 境界。

### Spec Stage (Implementation 未着手)

#### T0: Problem space matrix の refinement (Discovery) ✅ 完了 (2026-04-19)

- **Work** (完了):
  - Sub-matrix 1-4 の「要調査」cell に対し `scripts/observe-tsc.sh` で tsc observation
    を実施 → 15 fixture 作成 (`tests/observations/i144/*.ts`)
  - 結果を `report/i144-spec-observations.md` に記録 (commit 5490ed4)
  - 全 要調査 cell の判定を empirical 結果で確定 (Sub-matrix 1-2, 4 更新済)
- **v2 追加 work** (完了):
  - レビューで E 次元の conflate 判明 → RC 次元 enumerate
  - 追加 observation: rc-validation / l11-typevar / l17-stdcollection / compound-condition / verify-null-coercion / verify-complement-narrow / verify-t7-narrow-vs-value / verify-closure-reassign-emission (計 22 fixture)
  - Sub-matrix 5 新設、E 次元純化、T 次元拡張、JS coerce_default table 追加
- **Completion criteria** (完了):
  - [x] 要調査 cell = 0 件 (v2 で L11/L17/RC も解消)
  - [x] 全 cell に ideal 出力 + 判定記載
  - [x] T4d NaN predicate 強化、T7 compound narrow 強化、R5 predicate elide を scope に追加
  - [x] Matrix structure review (C1/C2/C3 gap 解消、Sub-matrix 5 新設)
- **Depends on**: — (完了)

#### T1: Per-cell E2E fixture 作成 (red state) ✅ 完了 (2026-04-19)

- **Work** (完了):
  - Matrix ✗ cell 9 種 (C-1 / C-2a / C-2b / C-2c / I-024 / I-025 / I-142 Cell #14 / T4d / T7) +
    ✓ regression lock-in 3 種 (null-check narrow / closure no-reassign keeps E1 / RC1-RC8 survey)
    を `tests/e2e/scripts/i144/cell-*.ts` に作成 (計 14 fixture、9 RED + 5 GREEN regression)
  - `scripts/record-cell-oracle.sh --all tests/e2e/scripts/i144/` で `*.expected` oracle 記録
  - Release binary で transpile + `tests/e2e/rust-runner` で cargo run を empirical 確認:
    9 ✗ cell = RED (2 TRANSPILE FAIL + 7 CARGO RUN FAIL) / 3 ✓ cell = GREEN
  - `tests/e2e_test.rs` に `test_e2e_cell_i144` 関数追加 (`#[ignore]` 付き、T6 で外す)
  - 詳細 report: [`report/i144-t1-red-state.md`](../report/i144-t1-red-state.md)
- **Scope note (v2 出荷 decision)**: typeof/instanceof **union-coercion** に依存する
  regression E2E は I-050 synthetic union coercion gap により runtime verify 不能と判明。
  narrow 自体の回帰は snapshot test (`tests/fixtures/type-narrowing.input.ts` /
  `narrowing-truthy-instanceof.input.ts`) で既に lock-in 済のため、E2E 重複追加せず
  snapshot に委譲。T1 report Fixture inventory section 参照
- **Completion criteria** (達成):
  - [x] Matrix ✗ cell 9 種の E2E fixture が red 状態で存在
  - [x] 代表 ✓ cell (narrow alive 系 3 種) が green で regression lock-in
  - [x] oracle (`*.expected`) が tsc runtime 準拠で記録
  - [x] test harness 登録 (`test_e2e_cell_i144` `#[ignore]`)
- **Depends on**: T0 ✅

#### T2: Spec-Stage Adversarial Review Checklist 完走 ✅ 完了 (2026-04-19)

- **Work** (完了):
  - `.claude/rules/spec-first-prd.md` の 5 項目 checklist を再検証: 全 [x]
  - `/check_job` Spec Stage adversarial review 実施 → 7 gap 発見 (D1-D7)、
    主要 2 件 (D3 E5a/b split / D4 Closure Reassign Policy) + 副次 4 件 (D1/D2/D5/D6) 解消、
    D7 (RC3 alive case は trivial E9 passthrough) は non-essential につき close
  - PRD v2.1 revise (Revise 履歴 + Sub-matrix 3 + E 次元 + Phase 3b 新設)
- **Completion criteria** (達成):
  - [x] Checklist 5 項目全 [x] (D1 doc cross-ref 明記、D4 Policy A/B 決定)
  - [x] Spec gap = 0 (D3/D4 は PRD v2.1 で解消、D1/D2/D5/D6 doc clarify で解消、D7 non-essential)
  - [x] Implementation stage 移行条件達成
- **Depends on**: T1 ✅

### Implementation Stage (Spec approved 後)

#### T3: `NarrowingAnalyzer` 基盤実装 (Phase 1) ✅ 完了 (2026-04-19)

- **Work** (完了):
  - `src/pipeline/narrowing_analyzer/` 新設 (events.rs 360 + classifier.rs 908 + mod.rs 227 行)
  - `NarrowEvent` / `ResetCause` / `NarrowTrigger` / `PrimaryTrigger` / `EmissionHint` /
    `RcContext` enum 定義 (`events.rs`、Sub-matrix 3/5 から derive、RC1-RC8 は
    `emission-contexts.md` と整合)
  - Scope-aware classifier (`classifier.rs`): VarDecl L-to-R shadow / closure param shadow /
    block-level decl shadow / branch merge (`merge_branches`、invalidating 優先 +
    preserving source order 決定) / sequential merge (`merge_sequential`、invalidating
    short-circuit) / peel-aware wrapper handling (Paren + 6 TS wrapper: TsAs /
    TsTypeAssertion / TsNonNull / TsConstAssertion / TsSatisfies / TsInstantiation) /
    unreachable stmt pruning (`stmt_always_exits` via `narrowing_patterns`) / closure /
    fn decl / class method / ctor / prop init / static block / object method / getter /
    setter descent (outer ident mutation → `ResetCause::ClosureReassign`)
  - `??=` 各 site に対し後続 sibling を classify し `EmissionHint` (`ShadowLet` /
    `GetOrInsertWith`) を hint-only 算出 (mod.rs `analyze_function` / `classify_nullish_assign`)
  - Unit test 5 file 分割 (cohesion 基軸): `types_and_combinators.rs` (301 行) +
    `hints_flat.rs` (450) + `hints_nested.rs` (546) + `scope_and_exprs.rs` (354) +
    `closures.rs` (602)、計 2253 行
- **Completion criteria** (達成):
  - [x] Module 実装完了、5 file に cohesion 基軸で分割 (全 file < 1000 行)
  - [x] Unit test 全 pass (2771 lib pass、+179 from baseline)
  - [x] 既存 pipeline test regression 0
  - [x] `/check_job` × 4 round (deep / deep deep × 3) + `/check_problem` で計 42 defect 解消
- **Depends on**: Spec approved (T0-T2 完了) ✅

#### T4: `NarrowingEvent` → `NarrowEvent` 拡張 (Phase 1b、breaking change) ✅ 完了 (2026-04-19)

- **Work** (完了):
  - `src/pipeline/type_resolution.rs` の `NarrowingEvent` struct を `NarrowEvent` enum に migrate
  - 既存 `FileTypeResolution::narrowing_events: Vec<NarrowingEvent>` を
    `narrow_events: Vec<NarrowEvent>` に rename + type change
  - Variant: `Narrow{ var_name, scope_start, scope_end, narrowed_type, trigger }` /
    `Reset{ var_name, position, cause }` / `ClosureCapture{ var_name, closure_span, outer_narrow }`
  - `NarrowEventRef` borrowed view + `as_narrow() -> Option<NarrowEventRef<'_>>` /
    `var_name() -> &str` accessor 追加 (legacy struct field assertion を natural に維持)
  - `PrimaryTrigger` + `NarrowTrigger` 2-layer 型: `NarrowTrigger::Primary(PrimaryTrigger)` /
    `NarrowTrigger::EarlyReturnComplement(PrimaryTrigger)` — nested `EarlyReturnComplement` を
    型レベルで構造排除。`primary()` / `is_early_return_complement()` accessor 提供
  - 全 consumer 更新: `type_resolver/narrowing.rs` (`detect_narrowing_guard` /
    `detect_early_return_narrowing` が `NarrowEvent::Narrow` を emit)、`visitors.rs` の
    `stmt_always_exits` import 更新、Transformer の narrow 取得 API を borrowed view 経由に統一
  - `block_always_exits` (type_resolver/narrowing.rs) 削除 → `stmt_always_exits`
    (narrowing_patterns.rs) を single source of truth 化、共通 peel 関数 +
    22 unit test (`narrowing_patterns::tests`) 集約
  - Test file 分割: `type_resolver/tests/narrowing/` に `legacy_events.rs` (629) +
    `trigger_completeness.rs` (372) の 2 file cohesion 分割、`narrow_views` helper で
    enum-variant destructuring を抽象化
- **Completion criteria** (達成):
  - [x] enum migration 完了、`NarrowingEvent` struct 残存 0 (grep 確認)
  - [x] 全 consumer call site 更新完了
  - [x] 既存 narrowing 機能 (typeof/instanceof/null check/early-return complement) regression 0
  - [x] `block_always_exits` / `stmt_always_exits` DRY 違反解消 (`/check_problem` で発見)
  - [x] narrowing 関連 rustdoc で 0 warning (intra-doc link 修正後)
- **Depends on**: T3 ✅

#### T5: 既存 `narrowing.rs` を CFG analyzer 経由に移行 (Phase 2) ✅ 完了 (2026-04-20)

- **Work** (完了):
  - `type_resolver/narrowing.rs` (524 行) を削除、narrow guard 検出を
    `src/pipeline/narrowing_analyzer/guards.rs` (430 行) に移植
  - `NarrowTypeContext` trait を新設 (`narrowing_analyzer/type_context.rs`、4 method:
    `lookup_var` / `synthetic_enum_variants` / `register_sub_union` / `push_narrow_event`)
    で registry access + event push を抽象化
  - `TypeResolver` が `NarrowTypeContext` 実装 (`type_resolver/narrow_context.rs`、
    scope stack + synthetic registry + result.narrow_events への薄い adapter)
  - Visitor は `crate::pipeline::narrowing_analyzer::{detect_narrowing_guard,
    detect_early_return_narrowing}` free fn を直接呼出し (`type_resolver/visitors.rs:694`)
  - 移植した detection logic: `detect_narrowing_guard` / `detect_early_return_narrowing` /
    `extract_typeof_narrowing` / `extract_null_check_narrowing` / `compute_complement_type` /
    `resolve_typeof_narrowed_type_from_var` / `classify_null_check` /
    `typeof_to_variant_name` / `variant_matches_typeof`
  - Trait boundary 専用 unit test 19 件を `narrowing_analyzer/tests/guards.rs` に追加
    (MockNarrowTypeContext 経由で registry-less に検証、EC: typeof 3+ variant 時の
    sub-union register、2-variant 時の bare type、typeof `!==` 反転 dispatch、
    typeof "object" synthetic enum variant lookup、null check complement 抑止、
    `x === null` alt branch narrow、NullCheckKind decision table 6 variant、
    truthy non-option no-op、compound `&&` 双方 recurse、unresolved var silent skip、
    early-return null/bang-truthy/typeof complement/instanceof complement/empty range)
  - **Dead code 除去 (T3/T4 残置)**: `NarrowingAnalyzer` struct / `new()` / `Default` impl /
    `var_types` field / `with_var_types()` / `var_type()` / `AnalysisResult.events` field を
    削除、`??=` 分析を `narrowing_analyzer::analyze_function` + private free fn
    (`analyze_stmt_list` / `recurse_into_nested_stmts` / `classify_nullish_assign`) に統一
    (guards.rs の free fn style と整合、YAGNI 準拠)
  - **Hono bench non-regression empirical verify** (2026-04-20): clean 112/158 → 112/158
    (0)、errors 62 → 62 (0)、compile 157/158 → 157/158 (0)。T5 pure refactor として
    意味論的 drift ゼロを確認
  - `narrowing_analyzer.rs` に `guards` + `type_context` module 登録、
    `detect_narrowing_guard` / `detect_early_return_narrowing` / `NarrowTypeContext` を
    pub re-export
  - test/narrowing_analyzer/tests.rs の sub-module list に `guards` を追加
- **Completion criteria** (達成):
  - [x] 既存 narrowing unit test 全 pass (regression 0; `type_resolver/tests/narrowing/`
        legacy_events + trigger_completeness は無変更で 2771 → 2787 lib pass)
  - [x] typeof / instanceof / null check / truthy / early-return complement の全
        fixture / E2E 全 pass
  - [x] `narrowing_analyzer` が narrow 検出の single source of truth に (narrowing.rs
        削除により二重実装解消)
  - [x] T3/T4 残置 dead code (`NarrowingAnalyzer` struct / `var_types` / `AnalysisResult.events`)
        を除去して free fn 統一 (YAGNI + guards.rs の style と整合)
  - [x] clippy 0 warn / fmt 0 diff / cargo test 全 pass (lib 2787 / integration 122 /
        compile 3 / E2E 97 + 14 i144 fixture `#[ignore]`)
  - [x] Hono bench non-regression empirical verified (clean 112/158 / errors 62 / compile
        157/158、全て T5 前後変動なし)
- **Depends on**: T4 ✅

#### T6-1: Pipeline wiring + scanner 完全削除 + ??= EmissionHint dispatch ✅ 完了 (2026-04-20)

**Scope 決定** (2026-04-20): 当初 T6 を Step 6-1 (scanner 短絡) + Step 6-2 (emission 連動) の
2 step 構成で定義したが、9 cell ✗ 全 green 化は完了条件として scope が大きいため、
`plan.t6.md` に従って T6-1 〜 T6-6 の 6 sub-phase に分割。T6-1 は pipeline foundation +
ShadowLet/GetOrInsertWith dispatch のみ (Cell #14 / C-1 / C-2a の 3 cell GREEN 化)。T7 は
T6-1 に畳み込み (scanner 関数 + call site を同時削除、broken-window 防止)。

- **Work (完了)**:
  - `FileTypeResolution.emission_hints: HashMap<u32, EmissionHint>` field + accessor
    `emission_hint(stmt_lo)` 新設 (`src/pipeline/type_resolution.rs`)
  - `TypeResolver::collect_emission_hints(body: &BlockStmt)` helper を独立 module
    `src/pipeline/type_resolver/emission_hints.rs` に新設、`narrowing_analyzer::analyze_function`
    を call して `self.result.emission_hints.extend(...)`
  - 5 entry point から呼び出し: `visit_fn_decl` (visitors.rs:126) / `visit_method_function`
    (visitors.rs:524) / `visit_class_body` の `Constructor` arm (visitors.rs:469) /
    `resolve_arrow_expr` の `BlockStmt` 分岐 (fn_exprs.rs:198) / `resolve_fn_expr`
    (fn_exprs.rs:252)
  - Transformer に `get_emission_hint(&self, stmt_lo: u32) -> Option<EmissionHint>`
    accessor (`src/transformer/expressions/type_resolution.rs`) + `build_option_get_or_insert_with`
    IR helper (`src/transformer/mod.rs`) を新設。後者は always-lazy 形で
    `x.get_or_insert_with(|| d)` を emit (TS `??=` lazy semantics 遵守、Copy lit も closure wrap)
  - `try_convert_nullish_assign_stmt` を EmissionHint dispatch に書換 (Ident LHS +
    ShadowLet strategy arm 内で `match self.get_emission_hint(ident.id.span.lo.0)`、
    `Some(GetOrInsertWith) → E2a` / `_ → E1 shadow-let`)
  - **Interim scanner 完全削除** (T7 統合): `pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts`
    + `stmt_has_reset` + `expr_has_reset` + helpers 4 種 (vardecl_init_has_reset /
    for_head_binds_ident / pat_binds_ident / prop_has_reset) + 8 call site
    (statements/mod.rs / switch.rs / classes/members.rs × 3 / expressions/functions.rs × 3)
    を削除、計 -440 行
  - cell14_* 4 tests を structural emission assertion に書換 (共通 helper
    `assert_cell14_emits_get_or_insert_with` 経由で linear null / inner-if / for-of body /
    closure body reassign 全 cell を `.get_or_insert_with(|| 0.0)` emission + error 無しで assert)
  - `test_e2e_cell_i144` 集約 1 関数を per-cell 14 関数に分割、baseline GREEN 5 + T6-1
    GREEN 3 = 8 cell un-ignore、残 6 cell は phase 別 `#[ignore = "I-144 T6-N: ..."]` reason で明示
  - cell-14 fixture の `String(v)` 呼出しを template literal `${v}` に書換 (I-163
    pre-existing transpiler defect 回避、cell-i025 と同 pattern)
  - Doc 更新: `narrowing_analyzer.rs` module doc (T6 retirement 記述)、`nullish_assign.rs`
    module doc (EmissionHint dual dispatch 表記)、`transformer/mod.rs` の
    `transform_module_collecting_with_context` doc、`switch.rs` の `convert_switch_case_body` doc
- **Work (Deep deep review で追加実施、2026-04-20)**:
  - `build_option_get_or_insert_with` unit test 4 件 (Copy lit / String lit / FnCall /
    target preservation) を `src/transformer/tests/mod.rs` に追加、lazy-closure invariant を lock-in
  - `FileTypeResolution::emission_hint` accessor unit test 2 件 (Some / None 分岐) を
    `src/pipeline/type_resolution.rs::tests` に追加
  - 4 entry point regression lock-in: `collect_emission_hints_wired_into_class_method_body` /
    `_constructor_body` / `_arrow_block_body` / `_fn_expr_body` を
    `src/transformer/expressions/tests/nullish_assign.rs` に追加、5 entry point のうち
    非 fn-decl 4 entry が `collect_emission_hints` 呼出し削除で silent regression しないことを guard
  - Doc drift 修正: `FileTypeResolution.emission_hints` field doc の key 説明を
    `ident.id.span.lo.0` (LHS ident span low) に訂正 / `nullish_assign.rs` module doc を
    dual dispatch 表記に更新 / `switch.rs` `convert_switch_case_body` doc を T6-1 後形式に更新
  - Pre-existing defect を TODO 起票: I-163 (`String()` callable が user-struct 化する defect、
    cell-14 fixture で再現) / I-164 (static block body が TypeResolver で未 visit、regression
    なしだが将来対応候補)
- **Completion criteria** (達成):
  - [x] Cell #14 / C-1 / C-2a の E2E GREEN (`test_e2e_cell_i144_14_narrowing_reset_structural` /
        `_c1_compound_arith_preserves_narrow` / `_c2a_nullish_assign_closure_capture` 全 pass)
  - [x] Scanner 関数完全削除、grep で残存 0 確認 (transformer/ 以下に `pre_check_narrowing_reset` /
        `has_narrowing_reset_in_stmts` 0 hits)
  - [x] cell14_* 4 tests を structural assertion に置換 (E2a `.get_or_insert_with(|| 0.0)`
        emission + "narrowing-reset" error 無しを assert)
  - [x] lib 2797 pass (+10 from 2787 baseline) / integration 122 / compile 3 / E2E 105 + 6 ignored
  - [x] clippy 0 warn / fmt 0 diff / Hono bench 非後退 (clean 112/158 / errors 62 / compile
        157/158、T6-1 前後変動なし)
  - [x] `/check_job deep` + `/check_job deep deep` + `/check_problem` review で発見した defect
        (test coverage / doc drift / pre-existing defect) を全解消 (pre-existing は TODO 起票)
- **Depends on**: T5 ✅

#### T6-2: `coerce_default` helper + E2b stale read emission (C-2b / C-2c GREEN 化)

- **Work**:
  - `src/transformer/helpers/` module 新設 (`mod.rs` + `coerce_default.rs`)
  - `coerce_default(inner_ty: &RustType, rc: RcContext) -> Expr` 実装 — JS coerce_default
    table (RC1 arithmetic F64 → `0.0` / RC6 StringInterp → `"null".to_string()` を初期 scope)
  - 分類器拡張 or TypeResolver scope 切断で closure-reassign 後の narrow stale 状態を
    Transformer に伝達 (probe-1/2/3 で設計選択確定: `NarrowEvent::ClosureCapture` emit vs
    narrow event scope_end 調整)
  - Transformer 側で RC1 arithmetic / RC6 string concat の read site に `x.unwrap_or(coerce_default(T))`
    を emit、既存 expected_type coerce 経路と合流
  - cell-c2b / cell-c2c E2E un-ignore
- **Completion criteria**:
  - cell-c2b / cell-c2c E2E GREEN
  - `coerce_default` unit test が (RC1 F64, RC6 String) 網羅 (YAGNI: T6-2 scope に必要な RC のみ)
  - T6-1 cell regression 0
  - clippy 0 / fmt 0 / Hono bench 非後退
- **Depends on**: T6-1 ✅

#### T6-3: Truthy predicate E10 (primitive NaN + composite `Option<Union<T,U>>`) — ✅ 完了 (2026-04-20)

- **Work (実装採用形)**:
  - `src/transformer/helpers/truthy.rs` 新設 — primitive truthy/falsy predicate を中央化
    (F64 → `x != 0.0 && !x.is_nan()`, String → `!x.is_empty()`, Bool → identity,
    integer primitive → `x != 0`)。falsy は De Morgan 反転
  - `statements/helpers.rs::generate_truthiness_condition` / `generate_falsy_condition`
    を helper への thin delegate に migrate (DRY 化)
  - `convert_if_stmt` fallback path に `try_generate_primitive_truthy_condition` 追加 —
    bare `if (x)` / `if (!x)` on F64/String/Bool/Primitive(int) を predicate wrap
    (`unwrap_parens` で `if ((x))` / `if (!(x))` も同経路)
  - `convert_if_stmt` に `try_generate_option_truthy_complement_match` 追加 —
    `if (!x) <exit>` on `Option<T>` を consolidated match emission
    (`let x = match x { <truthy arms> => ..., _ => <exit> }`)。
    inner type 別に分岐: Primitive → `Some(v) if <v truthy> => v`, Named (synthetic
    Union) → per-variant arm (primitive variant は truthy guard, non-primitive は
    guard なし)。inner binding は arm-local `__ts_union_inner`
  - `expressions/literals.rs` に `wrap_in_synthetic_union_variant` 追加 —
    expected type が synthetic union の Named 型のとき primitive literal を variant
    constructor で自動 wrap (`f("hi")` on `Option<F64OrString>` →
    `f(Some(F64OrString::String("hi".to_string())))`)。TypeRegistry +
    `variant_name_for_type` convention で reverse lookup
  - `return_wrap::wrap_leaf` priority 0 guard 追加 —
    expr が同一 enum への UserEnumVariantCtor call なら再 wrap せず返す
    (`convert_lit` の pre-wrap と return_wrap の衝突による `Enum::V(Enum::V(inner))`
    double-wrap を structural 防止)
  - `ir_body_always_exits` に `Stmt::Match` 全 arm exit 判定 + 空 Match 防御を追加
    (T6-4/T6-5 で match ベース emission が導入される際の silent bug 予防、/check_job
    R2-I1)
  - E2E un-ignore: cell-t4d-truthy-number-nan / cell-i024-truthy-option-complex +
    regression cell-regression-t4c-truthy-primitive-string /
    cell-regression-t4e-truthy-primitive-bool
- **Completion criteria (すべて達成)**:
  - [x] cell-t4d / cell-i024 E2E GREEN
  - [x] Truthy predicate unit test が全 RustType variant を網羅 (supported primitive
        は per-variant 値 assert、unsupported は None 返しを exhaustive 一括テスト)
  - [x] wrap_in_synthetic_union_variant 全分岐 (expected=None / 非 Named / type_args
        非空 / registry miss / 非 enum / string literal enum / discriminated enum /
        variant miss / F64/String/Bool hit) を unit test 網羅
  - [x] return_wrap priority 0 guard の regression test (same-enum absorb +
        different-enum fall-through) 追加
  - [x] `ir_body_always_exits` の全 stmt shape (Return/Break/Continue/If/IfLet/Match/
        空) unit test 6 case 追加
  - [x] H-3 mixed-union non-primitive variant emission の integration test lock-in
        (`test_try_generate_option_truthy_complement_match_h3_mixed_union_emits_guard_only_for_primitives`)。E2E lock-in は call-arg Union coercion gap (I-050-c track) の解消後
  - [x] 既 GREEN cell regression 0 (i144 cell 全 13 pass)
  - [x] clippy 0 / fmt 0 / Hono bench 非後退 (clean 112/158, errors 62)
  - [x] `/check_job` × 2 round + `/check_problem` で 15 defect 全 structural 対応
        (詳細: Spec Revision Log + R2 round 追加項目 + 周辺 defect を I-050-c 拡充 /
        I-171 新規起票で track、ad-hoc patch 0 件)
- **Depends on**: T6-2 ✅
- **発見された周辺 defect** (本 PRD scope 外、別 TODO/PRD で track):
  - **I-050-c (TODO)**: synthetic union coercion の非 literal expr (Ident / UnaryExpr /
    Call / Member 等) / return value / var-init / array-element 対応。T6-3 は literal
    (Num/Str/Bool) のみ完了、残を I-050 umbrella 内 sub-PRD 化。
  - **I-171 (TODO)**: `if (!x)` 汎用 truthy/falsy 変換 — non-Ident LHS (Member / OptChain
    / BinExpr / LogicalAnd / UnaryExpr / Lit)、non-exit body、else branch など 9 pattern
    の Tier 2 compile error。独立 matrix-driven PRD として起票 target。

#### T6-4: Compound OptChain narrow detection (T7 GREEN 化) ✅ 完了 (2026-04-21)

- **Work (完了)**:
  - `narrowing_patterns.rs` に `extract_optchain_base_ident` を DRY 共有ヘルパーとして新設
    (base ident 抽出: `x?.v` → `x`、deep chain / call / computed 対応)
  - `guards.rs` に `extract_optchain_null_check_narrowing` を新設 (OptChain 専用、bare-ident
    `extract_null_check_narrowing` とは分離)。`extract_non_nullish_side` + `unwrap_option_type`
    を共通 helper 化して DRY 解消
  - `detect_narrowing_guard` に OptChain パス追加 (bare-ident null check の `else if` として、
    `PrimaryTrigger::OptChainInvariant` で event 発行)
  - `detect_early_return_narrowing` に `is_eq` 分岐内で OptChain 対応追加
  - `transformer/expressions/patterns.rs::extract_narrowing_guard` に OptChain LHS 対応追加
    → `NarrowingGuard::NonNullish` 抽出で既存 `if let Some(x) = x` 生成パスを活用
  - unit test +22 (narrowing_patterns 6 / guards 11 / patterns 6)
  - cell-t7 E2E un-ignore → GREEN
- **Completion criteria** ✅:
  - cell-t7 E2E GREEN
  - 既 GREEN cell regression 0 (lib 2878 / integration 122 / compile 3 / E2E 113)
  - Hono bench 改善: clean 113/158 (+1), errors 60 (-2)
- **Depends on**: T6-3

#### T6-5: Multi-exit Option return implicit None emission (I-025 GREEN 化)

- **Work**:
  - Option return tail injection の現状実装箇所を probe-7 で特定
  - 複数 exit path (all-fall-off branch) の末尾に implicit `None` を inject
  - cell-i025 E2E un-ignore
- **Completion criteria**:
  - cell-i025 E2E GREEN
  - 既 GREEN cell regression 0
- **Depends on**: T6-4

#### T6-6: Quality gate + regression lock-in + `/check_job` Implementation stage review + PRD close

元 T8 / T9 / T10 を統合。

- **Work**:
  - 吸収対象 (I-024 / I-025 / I-142 Cell #14 / C-1 / C-2a-c / C-3 / C-4 / D-1) の
    snapshot / unit regression 追加
  - `functions` compile_test fixture (I-319 以外の narrow 関連) 検証、unskip 可能性確認
  - `cargo test` 全 pass / `cargo clippy --all-targets --all-features -- -D warnings` 0 warn /
    `cargo fmt --all --check` 0 diff
  - `./scripts/hono-bench.sh` 実測、errors 62 非増加 verify + category 別分析
  - `/check_job` Implementation Stage review で Spec gap = 0 + Implementation gap = 0
  - PRD close 処理: `backlog/I-144-...` を git history に archive、plan.md を完了済に移行、
    吸収対象 TODO entry 削除、設計判断 archive (`doc/handoff/design-decisions.md`) に
    CFG analyzer / NarrowTypeContext trait / EmissionHint dispatch / coerce_default table 等を追記
- **Completion criteria**: PRD 全 Completion Criteria (13 項目) 達成
- **Depends on**: T6-5

> **Note**: 元 T7 (scanner 完全削除) / T8 (regression lock-in) / T9 (quality gate) / T10
> (`/check_job` Implementation review) は T6 phase 分割時に再配分済:
> T7 → T6-1 に統合 (scanner 関数と call site を同時削除して broken-window 防止)、
> T8 / T9 / T10 → T6-6 に統合 (phase ごとに quality gate を完走するため最終 T6-6 で PRD
> 全体 Completion Criteria を verify する単一 task に集約)。

---

## Test Plan

### Unit tests (新規)

- **NarrowingAnalyzer** (`src/pipeline/narrowing_analyzer/tests/`):
  - CFG basic-block 分解 (if/else、loop、try、switch)
  - Per-block state 伝播 (entry → exit、loop fixpoint)
  - Reset event 検出 (R1a/R1b/R2/R3/R4/R5/R6/R7/R8 各 pattern)
  - Closure capture detection (R7)
  - Narrow trigger detection (T1-T12 各 pattern)
  - RC context 分類 (RC1-RC8)
  - Matrix ~70 unit test (Sub-matrix 1-5 全 cell 相当)

- **coerce_default helper** (`src/transformer/helpers/coerce_default.rs`):
  - Per RustType variant × per RC の coerce_default 出力 verify (JS coerce table 準拠)

### Integration / snapshot tests

- 既存 narrowing / nullish-assign integration test を CFG analyzer 経由で pass 維持
- Snapshot 更新 (shadow-let → E2a/b/c に emission 変化する cell): regression lock-in

### E2E tests (新規)

- **`tests/e2e/scripts/i144/`**: per-matrix-cell fixture
  - **C-1 系**: cell-c1-compound-arith-preserves-narrow.ts
  - **C-2a**: cell-c2a-nullish-assign-closure-capture.ts
  - **C-2b**: cell-c2b-closure-reassign-arith-read.ts
  - **C-2c**: cell-c2c-closure-reassign-string-concat.ts
  - **I-142 Cell #14**: cell-14-narrowing-reset-structural.ts
  - **I-024**: cell-i024-truthy-option-complex.ts
  - **I-025**: cell-i025-option-return-implicit-none-complex.ts
  - **T4d**: cell-t4d-truthy-number-nan.ts
  - **T7**: cell-t7-optchain-compound-narrow.ts
  - **Regression lock-in**:
    - cell-typeof-union-narrow.ts / cell-instanceof-narrow.ts / cell-null-check-narrow.ts
    - cell-closure-no-reassign-keeps-e1.ts (negative lock-in)
    - cell-rc1-rc8-narrow-read-contexts.ts (RC × 状態の既存挙動 lock-in)

### Matrix coverage audit

各 sub-matrix (1, 2, 3, 4, **5**) の全 cell が少なくとも 1 test (unit / integration / E2E) で
lock-in されていることを T10 で confirm。

---

## Completion Criteria

1. ✅ Spec Stage checklist 5 項目全 [x]、`/check_job` Spec Stage defect 0
2. ✅ `pipeline/narrowing_analyzer.rs` 実装完了、Unit test 全 pass
3. ✅ 既存 `type_resolver/narrowing.rs` が CFG analyzer 経由に統合、regression 0
4. ✅ Transformer emission が CFG analyzer + RC context 連動、E1/E2a/E2b/E2c/E3/E4/E5/E6/E7/E8/E9/E10 全 12 経路選択可能 (E10 は primitive + composite `Option<Union<T,U>>` 両対応)
5. ✅ `coerce_default` helper が JS coerce table 準拠で実装、unit test で全 RustType variant × RC verify
6. ✅ Interim scanner (`pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts`) 廃止、
   関連 call site 全削除
7. ⏳ Matrix ✗ cell (C-1, C-2a, C-2b, C-2c, I-024, ~~I-025~~, I-142 Cell #14, T4d, T7) の E2E 全 green — **T6-4 時点: 8/9 GREEN** (I-025 は T6-5 pending)
8. ✅ Matrix ✓ cell (既存 narrowing 動作) regression 0
9. ✅ `cargo test` (lib/integration/compile/E2E) 全 pass
10. ✅ `cargo clippy` 0 warn / `cargo fmt` 0 diff
11. ✅ Hono bench non-regression (errors 62 維持以上、改善があれば category 別分析) — T6-4: 60 errors (改善)
12. ⏳ 吸収対象 (I-024/I-025/I-142 Cell #14/C-1/C-2a-c/C-3/C-4/D-1) 解消確認、TODO entry 削除 — I-025 は T6-5 pending
13. ✅ `/check_job` Implementation Stage で Spec gap = 0 + Implementation gap = 0

**Matrix completeness requirement**: Sub-matrix 1, 2, 3, 4, **5** (v2 新設) の全 cell に対する test
(unit/integration/E2E のいずれか) が存在し、各 cell の実出力が ideal 仕様と一致。1 cell でも
未カバーなら未完成。

**Impact estimates (error count reduction) の empirical trace**:

本 PRD の Hono bench 直接改善見込みは限定的 (narrow は emission 精度改善で silent
semantic change を防止、compile error 数は大きく変わらない想定)。ただし `functions`
fixture の narrow 関連部分が解消されれば compile_test unskip 可能性あり。3 representative
instances を trace:

- Instance 1: `functions` fixture の I-024 基本 case → CFG analyzer で E3 (`if let Some`) 採用確認
- Instance 2: Hono 内 `x ??= d` pattern の closure capture (empirical 調査)
- Instance 3: Hono 内 complex `if (x)` narrow 後の compound arith (C-1 pattern)

Trace 結果を PRD 完了時に plan.md 記録。

---

## Rationale

**なぜ CFG analyzer を新設するか、既存 narrowing.rs を拡張しないか**:

- 既存 `narrowing.rs` は `if` condition ベースの scope-based narrow のみ。linear assign /
  loop iteration / closure capture 等の CFG-level 概念を持たない
- 拡張で対応すると責務が肥大化、DRY 違反 (shadow-let 判定 + typeof narrow + null check +
  reset scanner が散在)
- 新モジュールとして CFG analyzer を起こし、narrow event の single source of truth を
  確立する方が Design Integrity 高

**なぜ I-144 を phased に進めるか (Spec stage 先行)**:

- 問題空間 matrix が 4 次元で cell 数多、Spec stage で事前確定しないと実装が ad-hoc 化
- SDCDF Pilot (I-050-a、I-153) が Spec stage + Implementation stage 2-stage で Spec gap = 0
  を達成、本 PRD も同 framework 適用で品質担保
- Implementation stage は phased (T3-T10) で部分コミット可能、incremental-commit.md 遵守

**なぜ interim scanner を廃止するか**:

- Scanner (`has_narrowing_reset_in_stmts`) は false-positive (C-1 compound arith) を含む
- Closure capture (C-2) を検出できない
- CFG analyzer が同等以上の情報を構造的に提供、scanner は冗長かつ brittle

## 関連参照

- `plan.md`「次の作業」priority 1 + 「先行調査まとめ」section
- `doc/handoff/I-142-step4-followup.md` (C-1〜C-9 詳細、本 PRD で C-1/C-2/C-3/C-4/D-1 解消)
- `report/i142-step4-inv1-closure-compile.md` (C-2 empirical 確認)
- `src/pipeline/narrowing_analyzer/guards.rs` (narrow guard 検出、T5 で `type_resolver/narrowing.rs` から移管)
- `src/pipeline/narrowing_analyzer/type_context.rs` (`NarrowTypeContext` trait)
- `src/pipeline/type_resolver/narrow_context.rs` (`NarrowTypeContext` for `TypeResolver` impl)
- `src/transformer/statements/nullish_assign.rs:129` (廃止対象 interim scanner)
- `.claude/rules/spec-first-prd.md` (SDCDF 2-stage workflow)
- `.claude/rules/problem-space-analysis.md` (matrix enumerate 必須ルール)
- `.claude/rules/pipeline-integrity.md` (pipeline 境界保持)
- `.claude/rules/type-fallback-safety.md` (Semantic Safety Analysis 手順)
