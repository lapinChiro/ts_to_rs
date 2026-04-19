# INV-Step4-1: closure body reassign Rust compile 実測結果

**Base commit**: `38dba52` (uncommitted: TODO/plan.md 修正, tests/compile-check/src/lib.rs artifact)
**調査日**: 2026-04-19
**関連**: `doc/handoff/I-142-step4-followup.md` C-2

## 調査目的

`cell14_closure_body_reassign_does_not_surface_reset` が現行 emission の silent
compile error を lock-in している疑い (C-2) の empirical 確認。

## 再現 TS

```ts
function closureOk(x: number | null): number {
    x ??= 0;
    const reassign = () => { x = 1; };
    reassign();
    return x;
}
```

## ts_to_rs 変換結果 (`--no-builtin-types`)

```rust
fn closureOk(x: Option<f64>) -> f64 {
    let mut x = x;
    let mut x = x.unwrap_or(0.0);
    let mut reassign = || {
        x = Some(1.0);
    };
    reassign();
    x
}
```

## `cargo check` 実測診断

```
error[E0308]: mismatched types
 --> src/main.rs:5:13
  |
3 |     let mut x = x.unwrap_or(0.0);
  |                 ---------------- expected due to this value
4 |     let mut reassign = || {
5 |         x = Some(1.0);
  |             ^^^^^^^^^ expected `f64`, found `Option<{float}>`
  |
  = note: expected type `f64`
             found enum `Option<{float}>`
```

## 判定: Tier 分類

| 基準 | 結果 |
|------|------|
| Tier 1 (silent semantic change) | ✗ 該当せず。rustc が E0308 で検知 |
| Tier 2 (compile error) | ✓ **該当**。rustc エラー有り |
| Tier 3 (unsupported syntax) | ✗ 該当せず。変換自体は実行される |

**結論**: C-2 は **L1 silent ではなく、L3 Tier 2 compile error**。`doc/handoff/
I-142-step4-followup.md` の「silent compile error lock-in の疑い」は確認されたが、
Tier 分類は `conversion-correctness-priority.md` に照らすと **L3** (rustc detects)。

## 根本原因分析

生成コードの問題: 内側 closure で `x = 1` を `x = Some(1.0)` に wrap emission。

**Root cause**: `convert_assign_expr` の RHS conversion (`self.convert_expr(&assign.right)`) が
expected type を `Option<f64>` として wrap を適用している。これは TypeResolver の変数
scope 上、x が `Option<f64>` として登録されており、**shadow-let が emission-level の
rewrite** であり **TypeResolver のスコープには反映されていない** ため。

I-040 で確立した「TypeResolver scope は IR と整合しなければならない」原則の **shadow-let
に対する未対応**。I-142 の shadow-let は statement-level emission trick として導入
されたが、TypeResolver には narrowing event を登録していない。

```
TS 入力:  x: number | null → ??= 0 → closure { x = 1 } → return x
          ↓
IR 生成:  x: Option<f64>  → shadow-let: f64  → closure { x = Some(1.0) } ← 不整合
          ↓
Rust:     f64 var を Option<f64> で代入 → E0308
```

## 修正方針 3 パターン分析

### Option A: I-144 CFG Narrowing で structural 解決 (ideal)

**実装**: CFG analyzer が `??=` 後の narrow event を登録し、**かつ** closure capture
境界での narrow reset を検出。Closure が outer `x` を mutate する場合:
- shadow-let を諦めて `let mut x: Option<f64>` + `x.get_or_insert_with(|| 0.0)` path
- Closure 内 `x = 1` は Option<f64> 文脈で正しく `x = Some(1.0)` 出力
- Return は `x.unwrap_or_default()` 相当が必要

これは I-144 の問題空間に本質的に含まれる。**structural fix**。

### Option B: Closure-capture-aware scanner 拡張 (interim)

**実装**: 現行の `has_narrowing_reset_in_stmts` に「後続 stmts 内の closure body が
outer ident を capture + mutate」検出を追加。該当したら `UnsupportedSyntaxError`
surface。

**問題**: 現在 convertible なコードが unsupported になる (conversion quality 降格)。
I-144 が landed したら撤去必須の **interim patch**。

### Option C: Closure capture 時の emission path 切替 (部分 structural)

**実装**: Closure 内 outer ??= ident mutation を検出したら shadow-let を使わず直接
`let mut x: Option<f64>` + `get_or_insert_with` 経路に emit。TypeResolver には
narrow event を登録せず、x は終始 Option<f64> のまま。

**問題**: narrowed arithmetic (`x + 1` for numeric narrow) が使えない。TS の
`x: number` narrow を失う。Option B より変換品質は高いが Option A には劣る。

## 判断: 最適な修正パス

`ideal-implementation-primacy.md` に従い **Option A (I-144 structural)** を採用。
Option B / C は interim patch であり、Option A に置換される前提で導入することは
無駄作業。I-144 PRD の問題空間マトリクスに以下 cell を含める:

- **Cell "closure capture + ??=":** `x ??= d; const f = () => { x = v; };` —
  closure body 内で outer ??= ident mutate。emission は shadow-let を諦めて
  Option 保持 path。narrow reset の意味論は TS の CFG boundary (closure 非降下) を
  尊重しつつ、Rust lexical scope 上では mutable capture として安全に表現。

## 影響: I-142 Step 4 follow-up 全体の priority 再評価

| Item | 旧 priority (plan.md 記載) | 新 priority (empirical base) |
|------|-------------------------|--------------------------|
| C-2 | 🔴 L1 (silent compile error lock-in risk) | 🟡 **L3 Tier 2 compile error**。I-144 で structural 解消 |
| C-1 | 🔴 L3 (false-positive surface) | 🟡 L3 Tier 3 (unsupported surface、変換品質降格)。I-144 で scanner 置換により解消 |
| C-3 | 🔴 L3 (scanner branch test 欠落) | 🟡 **L4 moot**。I-144 で scanner 廃止、cell matrix E2E に吸収 |
| C-4 | 🔴 L3 (non-reset case test 欠落) | 🟡 **L4 moot**。同上 |
| C-5〜C-7 | 🟡 L3-L4 | 変更なし (I-144 独立) |
| C-8 | 🟡 L4 (.clone INTERIM doc) | 変更なし |
| C-9 | 🟡 L3 (bench regression INV) | 変更なし (INV-Step4-2 実施必要) |
| D-1 | 🟢 L4 (DRY helper) | 🟡 **L4 moot**。I-144 で scanner 廃止により call site 消滅 |

**結論**: I-142 Step 4 follow-up の過半は **I-144 で structural 吸収**。残る独立
item は C-5 / C-6 / C-7 / C-8 / C-9 のみ。従って **plan.md priority 1 は I-142
Step 4 follow-up ではなく I-144 (または先行する I-153 L1 silent) に移すべき**。

## 次アクション

1. plan.md priority table を再編成: I-153 (L1 確認済 silent) → I-144 (structural) →
   その他
2. I-142 Step 4 follow-up のうち I-144 で moot となる項目 (C-1, C-2, C-3, C-4, D-1) は
   **I-144 PRD に merge**。独立 item (C-5/C-6/C-7/C-8/C-9) のみ別 sub-PRD 化。
3. INV-Step4-2 (`utils/concurrent.ts:12` bench regression bisection) は git 操作が
   必要なため user 依頼事項として記録 (Claude 制約で直接実行不可)。

## 参考ファイル

- `src/transformer/statements/nullish_assign.rs:129` (`pre_check_narrowing_reset`)
- `src/transformer/expressions/assignments.rs:58-193` (`convert_assign_expr` NullishAssign arm)
- `src/pipeline/type_resolution.rs:42-56` (`NarrowingEvent` 既存 infra)
- `src/pipeline/type_resolver/narrowing.rs` (`typeof`/`instanceof` 既存実装)
- `doc/handoff/I-142-step4-followup.md` (C-2 予測 root cause)
