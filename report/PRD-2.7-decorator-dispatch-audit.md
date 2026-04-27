# PRD 2.7 — Decorator Dispatch Audit Report

**Date**: 2026-04-27
**Audit scope**: ts_to_rs の TypeResolver / Transformer / narrowing_analyzer / その他 src/ 配下全 module における SWC AST `decorators` field の handle 状況
**Audit purpose**: PRD 2.7 T1.5 (Q4 確定 = `_` arm 全面禁止 + ast-variants.md single source of truth) の前提として、Decorator framework が ts_to_rs で silent drop 状態にあることを構造的に記録し、`doc/grammar/ast-variants.md` に Decorator entry を新規追加する根拠を確立する。

---

## Audit method

```bash
grep -rn "decorator\|Decorator\|decorators" /home/kyohei/ts_to_rs/src/
```

実施日時: 2026-04-27、ts_to_rs working tree (commit `d423751`).

## Result

**0 件** (matched lines: 0)

`src/` 以下に `decorator` / `Decorator` / `decorators` を含む source line は **一切存在しない**。

これは以下を構造的に意味する:

- SWC AST `Class { decorators: Vec<Decorator>, ... }` field (class-on-top decorator) を **読まない**
- SWC AST `ClassMethod { function: Box<Function>, ... }` の `Function::decorators` field を **読まない**
- SWC AST `ClassProp { decorators: Vec<Decorator>, ... }` field を **読まない**
- SWC AST `PrivateMethod` / `PrivateProp` の `decorators` field を **読まない**
- SWC AST `TsParamProp { decorators: Vec<Decorator>, ... }` field (constructor parameter property decorator) を **読まない**
- SWC AST `AutoAccessor { decorators: Vec<Decorator>, ... }` field を **読まない**
- SWC AST `Param { decorators: Vec<Decorator>, ... }` field (function parameter decorator、legacy) を **読まない**

(SWC AST `Decorator` struct 自体の field 定義は `swc_ecma_ast` v21 source 参照、本 audit では handle 状況のみ確認)

## Implication (Conversion correctness 軸)

- TS 側で `@dec` が記述された class / method / property / parameter / accessor は、ts_to_rs の Rust 出力では **decorator semantic を一切 emit しない**
- → **Tier 1 silent semantic change** (= conversion-correctness-priority.md Tier 1)
  - TS では decorator hook (init / get / set / addInitializer) が runtime に発火し、class shape / method behavior / property storage を変容
  - Rust 出力では当該 hook の effect が **完全 drop**、生成 struct / impl が decorator-pre 状態を反映
- 該当 TS code を Rust runtime で実行しても compile error は発生しない (= silent)、ただし behavior は TS と乖離 (= semantic change)

これは ideal-implementation-primacy.md の最上位原則 (理想的 transpiler の獲得) に照らして **未達状態** として認識される。

## Cross-check: AutoAccessor との関係

- AutoAccessor は TS 5.0+ stable syntax (`accessor x: T = init`) で **decorator なしでも valid** (= bare AutoAccessor)
- ただし AutoAccessor は decorator framework の primary use case の一つ (`@dec accessor x` で decorator hook が getter/setter pair に作用)
- 本 PRD 2.7 では AutoAccessor は **Tier 2 honest error reported via UnsupportedSyntaxError** として明示化 (cell 7)
- AutoAccessor 完全 Tier 1 化 (decorator なし subset) は **I-201-A** (L3、user 承認 2026-04-27)
- Decorator framework 完全変換 (AutoAccessor 含む全 application) は **I-201-B** (L1 silent semantic change、user 承認 2026-04-27、PRD 7 として post-PRD 6 next-priority)

## Scope decision (本 PRD 2.7)

本 PRD 2.7 architectural concern = "framework Rule 改修 (Rule 10 + Rule 4) + 拡張による coverage gap detection 完成 + structural enforcement"。Decorator framework 全実装は **architectural concern が異なる** ため対象外 (1 PRD = 1 architectural concern 原則)。

本 PRD 2.7 で実施するのは以下のみ:

1. **`doc/grammar/ast-variants.md` に Decorator entry 新規追加** (Tier 2 Unsupported、I-201-B で Tier 1 化予定 言及)
   - 目的: silent drop 状態を **structural 認識化** (= reference doc に entry が存在することで、後続 PRD で coverage gap detection の対象となる)
   - 本 audit で確認した 0 件 silent drop 状態を doc 側で ground truth 化
2. **Tier 2 honest error mechanism は本 PRD では新規追加しない**
   - decorator field を読む code path が現状存在しないため、`UnsupportedSyntaxError` 経由 honest error report は I-201-B で初めて意味を持つ (= decorator handle 経路を新設する PRD で同時に honest error も導入)
   - 本 PRD 2.7 で decorator-detect path を追加すると 1 PRD = 1 architectural concern 原則を violate

## I-201-B (PRD 7) への引継ぎ事項

I-201-B 着手時に以下を確立する必要がある (本 audit からの handoff):

1. **decorator hook semantic の Rust 等価表現**:
   - TC39 Stage 3 decorators の hook signature: `(value, context: { kind, name, addInitializer, ... }) => replacement`
   - Rust 等価候補: proc macro 経由 generation / trait dispatch / runtime registry / phantom-typed AST transform
   - hook semantic の **runtime ↔ compile-time 区分** が Rust 表現の決定要因 (= Rust は compile-time が default、TS decorator は runtime initialization が default)
2. **decorator type 別 handling**:
   - class decorator (class shape transform)
   - method decorator (function dispatch wrap)
   - property decorator (storage layout transform)
   - parameter decorator (legacy のみ、TS 5.0 stage 3 spec では削除)
   - accessor decorator (= AutoAccessor decorator、I-201-A foundation を leverage)
3. **AutoAccessor (I-201-A) との順序**:
   - I-201-A (decorator なし subset) → I-201-B (decorator framework + AutoAccessor decorator 統合)
   - I-201-A の `accessor x: T = init` → `struct field + getter/setter pair` strategy が I-201-B の `@dec accessor x` の foundation

## References

- Q1 (b) confirm: 2026-04-27 user (AutoAccessor を Tier 2 honest error 化、Tier 1 化は別 PRD I-201-A/B)
- Audit 知見 (2026-04-27): plan.md line 78 (`grep "Decorator\|decorator" src/` 結果空の事実)
- I-201-A / I-201-B PRD 起票根拠: plan.md line 192-198
- ts_to_rs working tree commit: `d423751` (PRD 2.7 Spec stage 完了 commit)
- 本 audit 結果は `doc/grammar/ast-variants.md` の新 Decorator entry (cell 25 / T11 完了時) で ground truth として参照される
