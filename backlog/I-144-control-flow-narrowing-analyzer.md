# I-144: Control-flow narrowing analyzer (CFG-based type narrowing infrastructure)

**Status**: Implementation stage 進行中 — **T0-T4 完了** (2026-04-19)、T5 着手可能
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
| E10 | Type-specific truthy predicate | **Primitive**: `!x.is_empty()` (String) / `x != 0.0 && !x.is_nan()` (F64) / `x` (Bool). **Composite `Option<Union<T, U>>`**: `matches!(x, Some(v) if <inner predicate per variant>)` — Option `None` は false、Some(inner) は inner variant 別 truthy。例: `!x` on `Option<String \| number>` → `!matches!(&x, Some(F64OrString::String(s)) if !s.is_empty()) && !matches!(&x, Some(F64OrString::F64(n)) if *n != 0.0 && !n.is_nan())` | T4c/T4d/T4e primitive + T4a composite (I-024) truthy narrow 述語 |

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
| T4a if truthy Option | **✗ I-024 complex case** | - | - | - | - | - | - | - |
| T4b if truthy Any | - | - | ✓ any-enum (I-030 scope) | - | - | - | - | - |
| T4c if truthy String | - | - | - | ✓ predicate `!x.is_empty()` | - | - | - | - |
| T4d if truthy Number | - | - | - | - | **Enhance predicate** (`!= 0.0 && !is_nan()`) | - | - | - |
| T4e if truthy Bool | - | - | - | - | - | ✓ 既存 | - | - |
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
| `src/pipeline/type_resolver/narrowing.rs` | typeof/instanceof detection | CFG analyzer sub-routine に移行 |
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

#### T5: 既存 `narrowing.rs` を CFG analyzer 経由に移行 (Phase 2)

- **Work**:
  - `type_resolver/narrowing.rs::detect_narrowing_guard` を `NarrowingAnalyzer` の
    sub-routine に refactor
  - typeof/instanceof/null check detection を CFG analyzer 内部に集約
  - 既存 scope-based NarrowingEvent 生成を `NarrowEvent::Narrow` 経由に統合
- **Completion criteria**:
  - 既存 narrowing unit test 全 pass (DRY による regression 0)
  - typeof/instanceof fixture / E2E 全 pass
- **Depends on**: T4

#### T6: Interim scanner 短絡 + Transformer emission 連動 (Phase 3、v2 で T6/T7 合流)

**v2 で T6 と T7 を合流**: 旧 T6 (emission 変更) 実装中は旧 scanner が UnsupportedSyntaxError を
surface 続け新 emission が test で green 化しない。scanner 短絡を先行実施。

- **Work (Step 6-1: scanner 短絡)**:
  - `pre_check_narrowing_reset` / `has_narrowing_reset_in_stmts` の call site を CFG analyzer 出力の
    `EmissionHint` 参照に置換 (関数本体は残置、本 commit で無効化のみ)
  - 6 call site (`statements/mod.rs:200`, `switch.rs:100`, `classes/members.rs:205/334/382`,
    `expressions/functions.rs:129/291/310`) を `EmissionHint` 参照 or `false` 固定に変更
- **Work (Step 6-2: emission 連動)**:
  - `transformer/statements/nullish_assign.rs::try_convert_nullish_assign_stmt` を
    `EmissionHint` + `RcContext` 参照に書換:
    - `(ShadowLet, RC1)` → E1 shadow-let
    - `(E2aGetOrInsert, RC3 stmt)` → E2a `x.get_or_insert_with(|| d)`
    - `(E2bUnwrapOr, RC1 stale)` → E2b `x.unwrap_or(coerce_default(T))`
    - `(IfLetSome, RC1/RC7 alive)` → E3 `if let Some(x) = x`
  - `convert_assign_expr::NullishAssign` expression-context arm も同様
  - **JS coerce_default helper** 実装: `src/transformer/helpers/coerce_default.rs`
    (T 型 → Rust literal expr, e.g. `F64 → 0.0, String → "null"`)
  - C-2a/b/c 各 empirical 再現 TS を green 化 verify
- **Completion criteria**:
  - I-144 E2E fixture (matrix ✗ cell) 全 green
  - C-2a/b/c の empirical 再現 TS が E0308 を出さず正常 compile
  - JS coerce_default が table 通り適用されていることを unit test で verify
  - I-142 Cell #14 lock-in test を structural assertion に書換 (E2a 採用確認)
- **Depends on**: T5

#### T7: Interim scanner 完全削除 (Phase 4)

- **Work**:
  - `pre_check_narrowing_reset` 関数削除 (`nullish_assign.rs:129`)
  - `has_narrowing_reset_in_stmts` + `expr_has_reset` + `stmt_has_reset` 削除
  - 関連 unit test (`cell14_narrowing_reset_*`) を structural emission 期待に書換
  - Dead code / unused import cleanup
- **Completion criteria**:
  - Scanner 関数完全削除、grep で残存 0 確認
  - 全 test 全 pass
  - clippy 0 warn
- **Depends on**: T6

#### T8: 吸収対象 defect の regression lock-in

- **Work**:
  - I-024 (truthy narrowing complex case) の test 追加
  - I-025 (Option return implicit None complex case) の test 追加
  - I-142 Step 4 C-1 / C-2 の regression test (compound arith narrow 維持 / closure reassign
    emission) 追加
  - `functions` compile_test fixture (I-319 以外の narrow 関連) 検証
- **Completion criteria**:
  - I-024 / I-025 / I-142 Step 4 C-1 / C-2 対応 test 全 pass
  - `functions` fixture compile 成功条件が narrow 関連に限定されれば unskip 可能性 verify
- **Depends on**: T7

#### T9: Quality gate + Hono bench verification

- **Work**:
  - `cargo test --lib / --test '*'` 全 pass
  - `cargo clippy` 0 warn / `cargo fmt` 0 diff
  - `./scripts/hono-bench.sh` 実測、errors 62 非増加 verify
  - Bench 変動があれば category 別分析 (narrow 精度向上で追加 error/compile 改善あるか)
- **Completion criteria**: 全 quality gate pass、bench 非後退
- **Depends on**: T8

#### T10: `/check_job` Implementation Stage review

- **Work**:
  - Implementation stage 用 `/check_job` で第三者 review
  - Defect 分類 (Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight)
  - Spec gap = 0 および Implementation gap = 0 を目指す
  - 発見 defect を fix または別 TODO 化
- **Completion criteria**: review で Spec gap = 0 + Implementation gap = 0
- **Depends on**: T9

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
7. ✅ Matrix ✗ cell (C-1, C-2a, C-2b, C-2c, I-024, I-025, I-142 Cell #14, T4d, T7) の E2E 全 green
8. ✅ Matrix ✓ cell (既存 narrowing 動作) regression 0
9. ✅ `cargo test` (lib/integration/compile/E2E) 全 pass
10. ✅ `cargo clippy` 0 warn / `cargo fmt` 0 diff
11. ✅ Hono bench non-regression (errors 62 維持以上、改善があれば category 別分析)
12. ✅ 吸収対象 (I-024/I-025/I-142 Cell #14/C-1/C-2a-c/C-3/C-4/D-1) 解消確認、TODO entry 削除
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
- `src/pipeline/type_resolver/narrowing.rs` (既存 narrowing 実装)
- `src/transformer/statements/nullish_assign.rs:129` (廃止対象 interim scanner)
- `.claude/rules/spec-first-prd.md` (SDCDF 2-stage workflow)
- `.claude/rules/problem-space-analysis.md` (matrix enumerate 必須ルール)
- `.claude/rules/pipeline-integrity.md` (pipeline 境界保持)
- `.claude/rules/type-fallback-safety.md` (Semantic Safety Analysis 手順)
