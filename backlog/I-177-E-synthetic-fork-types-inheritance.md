# I-177-E: TypeResolver synthetic fork inheritance gap (variants lost on dedup hit)

**Plan η Step 1.5 (PRD 1.5、I-177-B prerequisite)** — TypeResolver-Synthetic registry integration cohesion fix。Plan η Step 2 (I-177-B) の empirical defect resolution の prerequisite。

## Background

`SyntheticTypeRegistry::fork_dedup_state` (`src/pipeline/synthetic_registry/mod.rs:462`) は dedup state (`union_dedup` / `struct_dedup` / `intersection_enum_dedup` / counters) を継承しつつ `types: BTreeMap::new()` で空 fork する設計。

```rust
pub fn fork_dedup_state(&self) -> Self {
    Self {
        types: BTreeMap::new(),                          // ← empty
        union_dedup: self.union_dedup.clone(),           // ← inherited
        struct_dedup: self.struct_dedup.clone(),
        intersection_enum_dedup: self.intersection_enum_dedup.clone(),
        struct_counter: self.struct_counter,
        synthetic_counter: self.synthetic_counter,
        type_param_scope: Vec::new(),
    }
}
```

意図 (doc コメントから):
> "This prevents duplicate generation when a second pass (e.g., TypeResolver) processes the same file that already had synthetic types generated in a first pass (e.g., TypeCollector)."

production pipeline (`src/pipeline/mod.rs:96-113`) で TypeResolver の synthetic として fork が使われる:

```rust
let mut file_resolver_synthetic = synthetic.fork_dedup_state();  // builtins の dedup を継承、types は空
let mut resolver = type_resolver::TypeResolver::new(&shared_registry, &mut file_resolver_synthetic);
resolver.resolve_file(file)
```

**問題**: builtin types loader (`external_types::load_builtin_types()`) が既に `synthetic.types` に多数の union 型を pre-register 済 (e.g., `union:F64,String → F64OrString` を含む 22+ entries、最終的に `union_dedup_len=63` まで蓄積)。fork の `types` が empty で起動するため、TypeResolver が `string|number` を再 register しようとすると:

1. `register_union([F64, String])` → `union:F64,String` signature が `union_dedup` に存在 (継承)
2. **dedup hit → existing name "F64OrString" を return、`types` には何も追加されない**
3. fork の `types` には F64OrString の `Item::Enum { variants: [{name:"F64",data:Some(F64)}, {name:"String",data:Some(String)}] }` が **存在しない**
4. `synthetic_enum_variants("F64OrString")` (`narrow_context.rs:22-29`) は `self.synthetic.get(enum_name)` で fork の `types` を query → **None を return**
5. `compute_complement_type` (`narrowing_analyzer/guards.rs:553`) が None を返す → `else_branch_complement` narrow event と `post-if EarlyReturnComplement` narrow event が **両方 push されない**

### Empirical defect chain (I-177-B PRD 起票時の investigation で判明)

```
function h(x: string | number): string | number {
    if (typeof x === "string") return 0;
    else { console.log("ne"); }
    return x;
}
```

**Pre-fix production trace**:
```
DBG register_union: existing dedup hit for sig="union:F64,String" → F64OrString (types_len=0, union_dedup_len=63)
DBG compute_complement_type: var=x positive=String var_type=Known(Named{F64OrString})
  → return None (enum F64OrString not in synthetic registry)
[narrow_events_len=1: only Primary(TypeofGuard) String at [82,91)]
[Missing: Primary(TypeofGuard) F64 at [101,123) (else complement)]
[Missing: EarlyReturnComplement(TypeofGuard) F64 at [91,139) (post-if)]
```

`return x` の post-if scope に narrow event がないため、collect_expr_leaf_types が declared `F64OrString` を返し、wrap_leaf step 5 で hard error 終了:
```
Error: cannot determine return variant at byte 135..136 for union F64OrString
```

**Pre-existing latent bug の affected scope** (推定):

- 全 typeof / instanceof / OptChain narrow guard with synthetic union member type (`string | number` / `Foo | Bar` / `Promise<X> | Y` 等)
- 全 if-stmt narrow with builtin-pre-registered union signature
- I-177-B (本 PRD で resolve) + I-177-A (typeof if-block elision related cases) + I-177-C (symmetric XOR) の empirical scenarios の **prerequisite**

unit test (synthetic_registry::new() を直接使う、builtin 不在) では再現不能、production pipeline 経由でのみ顕在化。これが I-177-B PRD 起票時に既存 narrow E2E test が GREEN だった理由 (test fixture の synthetic registry には fork pattern 使用なし)。

### 関連 PRD

- **I-177-B (Plan η Step 2)**: collect_expr_leaf_types query 順序 fix + leaf type resolution cohesion (5 unit test GREEN、empirical E2E は本 PRD prerequisite)
- **I-177-A / I-177-C / I-048**: 同様に narrow event 依存。本 PRD で structurally 解消されることで、後続 PRD の前提が確立する

## Problem Space

本 PRD は **non-matrix-driven** (architectural integrity fix、~20-50 LOC、light spec)。代替として **call-site / usage matrix** で完全 coverage を problem space とする。

### 入力次元 (Dimensions)

| 次元 | Variant 列挙 |
|------|------------|
| Synthetic type kind | UnionEnum / AnyEnum / InlineStruct / ImplBlock / Trait / External |
| Dedup hit state | (a) miss (新規 register) / (b) hit (既存 type、parent から fork 経由で types に存在しない) |
| Query path | (i) `synthetic.get(name)` (Transformer) / (ii) `synthetic_enum_variants(name)` (NarrowTypeContext) / (iii) `register_union/struct/intersection_enum` (TypeCollector / TypeResolver) |
| Pipeline phase | builtin loading / TypeCollector / TypeResolver / Transformer / OutputWriter |

### 組合せマトリクス

| # | Synthetic kind | Dedup state | Query path | Pre-fix 出力 | Post-fix 出力 (ideal) | 判定 |
|---|----------------|------------|-----------|------------|----------------------|------|
| 1 | UnionEnum | miss | (iii) register_union | 新規 entry を types に追加 | 同じ | ✓ no change |
| 2 | UnionEnum | hit | (iii) register_union | dedup hit、types 不変 | dedup hit、types 不変 | ✓ no change |
| 3 | UnionEnum | hit (parent から継承) | (i) synthetic.get(name) | **None (BUG: types に entry なし)** | Some(def) (fork.types に parent から clone 済) | ✗ **本 PRD 修正対象** |
| 4 | UnionEnum | hit (parent から継承) | (ii) synthetic_enum_variants | **None (BUG)** | Some(variants) | ✗ **本 PRD 修正対象** |
| 5 | AnyEnum | hit | (i)/(ii) | None (any-enum は **per-file `file_any_synthetic`** で生成、global `synthetic` に merge されないため fork.types に含まれず) | 同 (NA: any-enum は本 fix の query path に乗らない) | **NA** (post-`/check_job` review 2026-04-26 で確定: pipeline/mod.rs:81-93 で `file_any_synthetic` は `register_extra_enums` で TypeRegistry に登録される独立 path、TypeResolver fork からは query 不能。any-typed var の typeof narrowing は `any_enum_analyzer::analyze_any_enums` が直接 narrow event を生成し `compute_complement_type` を経由しない。本 fix の effect 対象外) |
| 6 | InlineStruct | hit | (i) | None (BUG、struct も同一 root cause) | Some(def) | ✗ **本 PRD 修正対象** (cross-cutting) |
| 7 | ImplBlock | hit | (i) | None (BUG) | Some(def) | ✗ **本 PRD 修正対象** (cross-cutting) |
| 8 | External | hit | (i) | None (BUG) | Some(def) | ✗ **本 PRD 修正対象** (cross-cutting) |
| 9 | Trait | hit | (i) | None (BUG) | Some(def) | ✗ **本 PRD 修正対象** (cross-cutting) |
| 10 | (any) | miss → register_union → fork merge back → parent | (i) on parent | Some(def) | Some(def) | ✓ no change (round-trip ok) |

**Cross-cutting invariant**: fork の任意 query が「dedup state が hit する synthetic type の variants/fields にアクセスできる」必要がある。本 PRD は UnionEnum (cell #3, #4) を core として全 SyntheticTypeKind に均等適用する fix を選択する。

### 直交軸 (cross-axis)

| 軸 | 検討 | 結論 |
|----|------|------|
| Pre-fix で empirical reproduce 可能な pattern | typeof / instanceof / OptChain guard with synthetic union member type | **本 PRD scope 内** |
| 既存テストで cover される pattern | unit test (`synthetic_registry::new()`) は fork なし; integration test (`tests/`) は CLI pipeline 経由なので fork 影響 | 既存 integration test が pass している事実 = 本 bug が runtime regress していなかった (narrow event の loss が compile error にならず silent に「狭まらない」だけ → silent type widening) |
| Memory cost of clone | builtin synthetic は ~63 entries、各 file fork で clone → per-file overhead ~10-50 KB (Item enum 構造体 size 依存) | **acceptable** (Hono 規模で全 file 合計でも MB order を超えない) |
| 並行 mutation safety | fork 間で types を share しない (各 file 独立) → mutate concurrent risk なし | safe |

### Cross-cutting Invariants

| ID | Invariant | Verification |
|----|-----------|--------------|
| INV-CE-1 | **Fork query consistency**: `synthetic.fork_dedup_state()` の戻り値で `synthetic.get(name)` を query した場合、parent で `get(name)` が `Some(def)` なら fork でも `Some(def)` を返す (post-fix) | Unit test on `fork_dedup_state` で parent register → fork query → Some assertion |
| INV-CE-2 | **Dedup-types consistency**: fork の `union_dedup` / `struct_dedup` / `intersection_enum_dedup` の entry に対応する type が必ず `types` 内に存在する (parent 経由 inherited も含む) | Property-based: 任意の parent で register、fork、dedup と types entry の同値性確認 |
| INV-CE-3 | **Round-trip preservation**: parent → fork → register more → merge back → parent の round-trip で、parent の元 types が unchanged で残る (overwrite-with-same-value も含む) | Integration test on the merge cycle |

## Goal

`SyntheticTypeRegistry::fork_dedup_state` を **`types` も clone する** ように修正し、TypeResolver fork が builtin / parent-inherited synthetic types を query 可能にする。これにより:

1. **Empirical defect chain 解消**: typeof / instanceof / OptChain guard with synthetic union member の post-narrow scope が正しく narrow event を持つようになる。`compute_complement_type` が成功し、`else_branch_complement` と `post-if EarlyReturnComplement` の両 narrow event が push される。
2. **I-177-B prerequisite 確立**: I-177-B の T4 E2E fixture (declaration form + callable interface form) が GREEN になる。Plan η Step 2 完了の前提が満たされる。
3. **後続 PRD の cohesion 確立**: I-177-A / I-177-C / I-048 で narrow event 依存の機能 fix が正しく実装可能になる。
4. **回帰ゼロ**: 既存 lib test 3131 / integration 122 / E2E 155 / compile 3 を全 pass。

## Scope

### In Scope

- `SyntheticTypeRegistry::fork_dedup_state` (`src/pipeline/synthetic_registry/mod.rs:462`) の `types: BTreeMap::new()` を `types: clone_types(&self.types)` に変更
  - `Item` (in `SyntheticTypeDef.item`) は既に `derive(Clone)`、`SyntheticTypeDef` には `derive(Clone)` を追加 (現状は `derive(Debug)` のみ)
  - clone は per-entry deep clone (Item は recursive 構造、Vec/Box が clone される)
- 既存 test `test_fork_dedup_state_includes_intersection_enum` (`synthetic_registry/tests.rs:487`) の assertion `forked.types.len() == 0` を更新 (新仕様で `forked.types.len() == 1`)
- 新規 unit test: `fork_dedup_state` が parent の types を継承し、`synthetic.get(name)` が fork で Some を返すこと
- 新規 unit test: `compute_complement_type` が builtin pre-registered union の variants を fork 経由で query 可能であること (narrowing_analyzer 側 integration verify)
- I-177-B との integration: I-177-B の `collect_leaves_typeof_narrow_post_if_return` test (callable interface form、現在 RED) が GREEN に転じること
- E2E lock-in: `tests/e2e/scripts/i177-e-synthetic-fork-narrow-cohesion.ts` (typeof + post-if return パターン、declaration form + callable interface form)

### Out of Scope

- I-177-B の T4 / T5 (本 PRD 完了後に I-177-B を resume)
- I-177-A / I-177-C / I-048
- `fork_dedup_state` の rename (`fork_dedup_state` 名を残す。content semantics 拡張のみ)
- Memory optimization (lazy clone / Cow / Rc) — 現状の eager clone で memory overhead 許容内 (per-file ~10-50 KB)
- 全 SyntheticTypeKind に対する fork query の網羅 unit test (cell #5/#6/#7/#8/#9 は core fix が cross-cutting に effect、unit test cover は core (cell #3, #4) で十分。E2E + integration test で cross-cutting verify)

## Design

### Technical Approach

**Step 1 — `SyntheticTypeDef` に Clone derive 追加**

```rust
// src/pipeline/synthetic_registry/mod.rs
#[derive(Debug, Clone)]
pub struct SyntheticTypeDef {
    pub name: String,
    pub kind: SyntheticTypeKind,
    pub item: Item,
}
```

`Item` (ir::Item) は既に `derive(Clone, PartialEq, Debug)` 済 (`src/ir/item.rs`)。`SyntheticTypeKind` は単純 enum、Clone 追加可能。

```rust
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SyntheticTypeKind { ... }
```

**Step 2 — `fork_dedup_state` の types 継承を有効化**

```rust
// src/pipeline/synthetic_registry/mod.rs:462
pub fn fork_dedup_state(&self) -> Self {
    Self {
        types: self.types.clone(),    // ← 変更: BTreeMap::new() → self.types.clone()
        union_dedup: self.union_dedup.clone(),
        struct_dedup: self.struct_dedup.clone(),
        intersection_enum_dedup: self.intersection_enum_dedup.clone(),
        struct_counter: self.struct_counter,
        synthetic_counter: self.synthetic_counter,
        type_param_scope: Vec::new(),
    }
}
```

doc comment も更新:
```rust
/// Creates a new registry that inherits state from `self`.
///
/// All persistent state (types, dedup signatures, counters) is cloned;
/// `type_param_scope` is reset because it is per-pass mutable state.
/// This means the fork can query any type registered in `self`
/// (e.g., builtin synthetic union types) via `get(name)` or
/// `synthetic_enum_variants(name)`. Subsequent `register_union` /
/// `register_struct` calls add new entries to the fork without affecting
/// `self`.
///
/// **Round-trip note**: when the fork is later merged back into `self`
/// via `merge`, the fork's clones of `self`'s types overwrite themselves
/// idempotently (same name → same content), so no data is lost or
/// duplicated. Only entries newly registered in the fork are net additions.
```

**Step 3 — 既存 test 更新**

```rust
// src/pipeline/synthetic_registry/tests.rs:498-506
let forked = reg.fork_dedup_state();
// Fork inherits both dedup state and types (post-I-177-E).
assert_eq!(
    forked.types.len(),
    1,
    "forked registry should inherit parent's types"
);
```

assertion message も update。

### Design Integrity Review

per `.claude/rules/design-integrity.md`:

- **Higher-level consistency**: `fork_dedup_state` は SyntheticTypeRegistry 内 1 method、外部 API。本 PRD は内部 semantics の正常化 (現状の "forked has no types" は dedup hit 時に query が壊れる buggy semantics、post-fix の "forked inherits all state" は self-consistent な correct semantics)。同 module 内の他 method (`merge`, `register_union`, etc.) と整合性向上。
- **DRY**: 本 fix は 1 箇所の semantics 修正、knowledge duplication なし。fork = 完全継承 (type_param_scope 除く) という invariant が単一 source of truth に。
- **Orthogonality**: fork_dedup_state の責務が「dedup state inheritance」から「persistent state inheritance (= types + dedup + counters)」に拡張されるが、本来の意図 ("prevents duplicate generation when second pass processes ...") は dedup と types の両方が必要 (dedup だけでは query が壊れる)。post-fix で意図と挙動が整合。
- **Coupling**: 変更は SyntheticTypeRegistry 内に閉じる。pipeline mod / TypeResolver / Transformer の API 不変 (fork の戻り値型は同じ、shape も同じ、内容のみ変化)。Coupling 増加なし。

**Broken windows 検出**:
- 既存 test `test_fork_dedup_state_includes_intersection_enum` の assertion が誤った invariant ("forked has no types") を lock-in していた。これは本 bug を test レベルで「正しい」と認める形 (= broken window)。本 PRD で正しい invariant に update。

**Verified, no design issues remaining post-fix.**

### Impact Area

- **変更**: `src/pipeline/synthetic_registry/mod.rs`
  - `SyntheticTypeDef` に `Clone` derive 追加 (1 行)
  - `SyntheticTypeKind` に `Clone` derive 追加 (1 行)
  - `fork_dedup_state` body の `types: BTreeMap::new()` → `types: self.types.clone()` (1 行) + doc comment update
- **変更**: `src/pipeline/synthetic_registry/tests.rs`
  - `test_fork_dedup_state_includes_intersection_enum` の assertion update (~3 行)
  - 新規 test `test_fork_dedup_state_inherits_types_for_query` 追加 (~25 行)
- **変更**: `tests/e2e/scripts/i177-e-synthetic-fork-narrow-cohesion.ts` (新規 fixture)
- **変更**: `tests/e2e_test.rs` (新規 entry 追加)

LOC 推定: 新規 ~50 LOC、変更 ~10 LOC = 正味 +60 LOC (うち test ~50 LOC)。production code change は 3 行 + doc。

### Semantic Safety Analysis

per `.claude/rules/type-fallback-safety.md`:

本 PRD は型 fallback 導入なし。**fork semantics の修正のみ** (現状壊れている query path を正常化)。3-step analysis:

1. **Identify all usage sites**: `fork_dedup_state` の戻り値で `get(name)` / `synthetic_enum_variants(name)` を query する全 production path:
   - `narrow_context.rs::synthetic_enum_variants` (TypeResolver narrow guard)
   - `Transformer` 内の `synthetic.get(name)` 全 call site (type lookup)
2. **Classify each usage site**:
   - **Pre-fix existing query for parent-inherited type**: dedup hit but types empty → `get(name) == None` → narrow event NOT pushed (silent type widening) または Transformer が "unknown type" として degraded handling
   - **Post-fix existing query for parent-inherited type**: types contains clone → `get(name) == Some(def)` → narrow event pushed correctly、Transformer が正しい type info を取得
   - **Pre-fix register new type**: dedup miss → register adds to types → `get(name) == Some(def)` → no behavioral change
   - **Post-fix register new type**: same as pre-fix → no behavioral change
3. **Verdict**: **Safe** — pre-fix で query が None を返していた箇所が post-fix で Some を返すようになり、**より正確な型情報が伝播**する。silent semantic regression は発生不可能 (None → Some(correct_type) は narrow event を新規 push するだけで、既存 narrow event は変更しない)。silent type widening の解消は **改善方向の semantic 変化** (TS narrowing semantics により忠実に)。

**E2E regression risk**: 既存 E2E test 155 件は pre-fix で何らかの narrow path を通過していた。post-fix で追加 narrow event が発生する場合、IR emission が変わる可能性。**緩和策**: T5 で Hono benchmark + 全 E2E test 実行、回帰検出。pre-fix で silently 動作していた cell が post-fix で改善 (clean) するのが期待値、worsening cell があれば調査。

## Task List

TDD 順序: T1 (RED test) → T2 (GREEN fork_dedup_state fix) → T3 (既存 test update) → T4 (E2E lock-in) → T5 (verification)。

### T1: RED — fork_dedup_state types inheritance test 追加

- **Work**:
  - `src/pipeline/synthetic_registry/tests.rs` に新規 test 追加:
    - `test_fork_dedup_state_inherits_types_for_query`: parent で `register_union` → fork → fork で `get(name)` が Some を返すこと、`synthetic_enum_variants(name)` 相当の query が成功すること
    - `test_fork_dedup_state_round_trip_preserves_parent_types`: parent → fork → fork で更に register → merge back → parent の元 types が unchanged であること (overwrite idempotent)
- **Completion criteria**: 上記 2 test 追加、**全て RED** (現状 fork.types は empty なので)。`cargo test test_fork_dedup_state_inherits_types_for_query` 失敗確認
- **Depends on**: なし
- **Prerequisites**: なし

### T2: GREEN — `SyntheticTypeRegistry` fork semantics 修正

- **Work**:
  - `SyntheticTypeDef` に `Clone` derive 追加 (`#[derive(Debug, Clone)]`)
  - `SyntheticTypeKind` に `Clone` derive 追加 (`#[derive(Debug, PartialEq, Eq, Clone)]`)
  - `fork_dedup_state` の body 修正: `types: BTreeMap::new()` → `types: self.types.clone()`
  - doc comment update (Design section の Step 2 参照)
- **Completion criteria**:
  - T1 で追加した 2 test が GREEN
  - `cargo test --lib pipeline::synthetic_registry::tests` 全 pass
  - `cargo build` 警告 0
- **Depends on**: T1
- **Prerequisites**: なし

### T3: 既存 test update + I-177-B regression 確認

- **Work**:
  - `src/pipeline/synthetic_registry/tests.rs:498-506` の `test_fork_dedup_state_includes_intersection_enum` の assertion を新仕様に update:
    ```rust
    assert_eq!(forked.types.len(), 1, "forked registry inherits parent's types (I-177-E)");
    ```
  - I-177-B PRD で追加した `collect_leaves_typeof_narrow_post_if_return` test (callable interface form) が **本 PRD のみで GREEN に転じる** ことを確認 (I-177-B の query 順序 fix と組み合わせて empirical defect 解消の証跡)
  - 既存 lib test 3131 + integration 122 + compile 3 が全 pass
- **Completion criteria**:
  - `cargo test --lib` 全 pass (lib + integration)
  - `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
  - `cargo fmt --all --check` 0 diff
- **Depends on**: T2
- **Prerequisites**: なし

### T4: E2E lock-in — empirical scenario fixture (declaration + callable interface forms)

- **Work**:
  - `tests/e2e/scripts/i177-e-synthetic-fork-narrow-cohesion.ts` を作成 (declaration form + callable interface form):
    ```ts
    function h(x: string | number): string | number {
        if (typeof x === "string") return 0;
        else { console.log("ne"); }
        return x;
    }
    interface I { (x: string | number): string | number }
    const i: I = (x): string | number => {
        if (typeof x === "string") return 0;
        else { console.log("ne"); }
        return x;
    };
    console.log(h(42));
    console.log(h("a"));
    console.log(i(42));
    console.log(i("a"));
    ```
    expected stdout: `ne\n42\nne\n0\nne\n42\nne\n0\n` (tsx 出力 + Rust 一致)
  - `tests/e2e_test.rs` に該当 entry 追加
  - 生成 Rust の variant wrap 確認 (`return F64OrString::F64(x)` または trailing tail `F64OrString::F64(x)` パターン、bare `x` 不在)
- **Completion criteria**:
  - `cargo test --test e2e_test i177_e_synthetic_fork_narrow_cohesion` GREEN
  - tsc / tsx の runtime stdout と Rust 実行結果が byte-exact 一致
- **Depends on**: T3
- **Prerequisites**: なし

### T5: 回帰 verification + Hono benchmark

- **Work**:
  - `cargo test` (lib + integration + compile + e2e) 全 pass を確認
  - `./scripts/hono-bench.sh` 実行、pre/post で `clean files` / `error instances` の diff を測定
  - **期待**: 回帰 0 (既存 silent type widening が正常化されるので、potentially clean files +N の improvement あり、worsening 0 が必須)
- **Completion criteria**:
  - `cargo test` 0 fail
  - Hono bench で worsening 0 (clean files +N possible、improvement のみ)
  - non-deterministic variance ±1/±2 範囲内なら GREEN
- **Depends on**: T4
- **Prerequisites**: なし

## Test Plan

### Unit tests (新規 2 件)

1. `pipeline::synthetic_registry::tests::test_fork_dedup_state_inherits_types_for_query`
2. `pipeline::synthetic_registry::tests::test_fork_dedup_state_round_trip_preserves_parent_types`

### Test coverage gap analysis (Step 3b)

`.claude/rules/testing.md` の C1 branch coverage / equivalence partition technique 適用:

| Gap | Missing pattern | Technique | Severity |
|-----|----------------|-----------|----------|
| G1 | fork_dedup_state types inheritance (本 PRD core defect) | C1 (parent register → fork query branch) | High |
| G2 | round-trip preservation invariant | Decision table (register before vs. after fork × merge timing) | Medium |
| G3 | I-177-B callable interface E2E (cross-PRD integration、prerequisite を本 PRD で解消) | Integration test cross-PRD | High |

全 gap を T1 (G1, G2) + T3 (G3) + T4 (G3 E2E) でcover。

### E2E (新規 1 fixture)

- `tests/e2e/scripts/i177-e-synthetic-fork-narrow-cohesion.ts` (declaration form + callable interface form、各々 number / string 入力で typeof narrow path を完全 cover)

### Regression protection (既存 test)

- 既存 lib test 3131 / integration test 122 / compile test 3 / E2E test 155 が全 pass
- Hono benchmark で `clean files` / `error instances` 回帰 0 (improvement のみ)
- `test_fork_dedup_state_includes_intersection_enum` 既存 test の assertion update に伴う semantic shift の justification を doc コメントで明示

## Completion Criteria

`.claude/rules/prd-completion.md` 準拠:

- [ ] T1〜T5 全 task の Completion criteria 達成
- [ ] Problem Space matrix の全 cell (#1〜#10) に対し post-fix 出力が ideal 仕様と一致
- [ ] 全 cell に lock-in test が存在 (cell #1, #2, #10 = 既存 test cover、cell #3, #4 = T1 新規 test cover、cell #5-9 = T1 cross-cutting fix で自動的に正常化、E2E で間接 verify)
- [ ] `cargo test` 全 pass (lib + integration + compile + e2e + 新規 2 unit + 1 E2E)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 warning
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `./scripts/check-file-lines.sh` 0 violation
- [ ] Hono benchmark で `clean files` / `error instances` 回帰 0 (improvement のみ)
- [ ] **I-177-B との integration**: I-177-B の `collect_leaves_typeof_narrow_post_if_return` test (現在 RED) が本 PRD 完了で GREEN に転じることを T3 で確認
- [ ] `/check_job` で **Layer 1 (Mechanical)** + **Layer 4 (Adversarial trade-off)** を実施 (本 PRD は non-matrix-driven のため Layer 2-3 は optional、ただし fork semantics 変更は cross-cutting effect ありなので Layer 2 軽量実施推奨)
- [ ] CLAUDE.md / plan.md / TODO 更新 (PRD close 時、I-177-E 該当 entry 削除 + 直近完了作業 section 追記 + Plan η の chain で本 PRD を Step 1.5 として正規化)

### Impact estimate verification (3 instance trace)

本 PRD の core defect verification として、3 instance を trace:

1. **Instance 1**: `/tmp/i177b-fn.ts` (declaration form、ts_to_rs hard error → post-fix 解消、I-177-B 修正と組み合わせて empirical 確認)
2. **Instance 2**: `/tmp/i177b-repro.ts` (callable interface form、silent broken tail → post-fix structurally correct)
3. **Instance 3**: Hono codebase 内で typeof narrow + post-narrow Ident return パターン、本 PRD 修正後 silent type widening が解消される cell の sample (該当パターンが Hono に存在しない場合は別 synthetic fixture で代替)

## Spec Review

本 PRD は **non-matrix-driven** のため `spec-stage-adversarial-checklist.md` 10-rule の全項目検証は不要。代替として以下の 5-point check を実施 (PRD 起票時 self-check):

- [x] **Defect empirical reproduction**: dbg trace で `register_union: existing dedup hit ... types_len=0, union_dedup_len=63` を 2026-04-26 に確認、`compute_complement_type → return None` まで chain 確認済
- [x] **Root cause completeness**: fork_dedup_state の body 内 `types: BTreeMap::new()` が直接的根本原因、追加 contributor なし
- [x] **Fix scope minimality**: production code change は 3 行 + doc。test update は existing 1 件 + 新規 2 件。LOC 最小限
- [x] **Cross-cutting effect verification**: 本 fix は SyntheticTypeKind 全種類 (UnionEnum / AnyEnum / InlineStruct / ImplBlock / Trait / External) に等しく effect、core fix が cross-cutting に正常化
- [x] **I-177-B integration**: 本 PRD 完了時に I-177-B の RED test が GREEN に転じる事を T3 で確認

---

## 参考 (関連ファイル)

- `src/pipeline/synthetic_registry/mod.rs:462` (fork_dedup_state — 修正対象)
- `src/pipeline/synthetic_registry/tests.rs:487` (test_fork_dedup_state_includes_intersection_enum — assertion update 対象)
- `src/pipeline/narrowing_analyzer/guards.rs:553` (compute_complement_type — root cause manifestation site)
- `src/pipeline/type_resolver/narrow_context.rs:22` (synthetic_enum_variants — 直接 affected query path)
- `src/pipeline/mod.rs:104` (production fork_dedup_state caller)
- `tests/e2e/scripts/` (E2E fixture 配置先)
- `backlog/I-177-B-collect-expr-leaf-types-cohesion.md` (I-177-B PRD、本 PRD prerequisite)
- `plan.md` Plan η Step 1.5 (本 PRD)
