# I-144 T1: per-cell E2E fixture red-state confirmation

**Date**: 2026-04-19
**PRD**: [`backlog/I-144-control-flow-narrowing-analyzer.md`](../backlog/I-144-control-flow-narrowing-analyzer.md) Task T1
**SDCDF stage**: Spec stage — Spec-Stage Adversarial Review Checklist #5 (E2E readiness) を green 化

## 目的

I-144 PRD Sub-matrix 1-5 の各 cell に対して per-cell E2E fixture を作成し、
現行 `cargo build --release` + TS runtime (tsx) で以下を empirical に確認する:

1. **✗ cell** — 現行 emission が red 状態 (transpile fail / cargo run fail / stdout mismatch)
2. **✓ regression cell** — 現行 emission が既に green (T3-T6 migration 後も green 維持させる lock-in)

## Fixture inventory

配置: `tests/e2e/scripts/i144/`、oracle (`*.expected`) は `scripts/record-cell-oracle.sh`
で tsx runtime stdout から記録。

| Fixture | Matrix cell | 期待 ideal emission (post-I-144) |
|---------|-------------|----------------------------------|
| `cell-14-narrowing-reset-structural.ts` | I-142 Cell #14 (R1b linear reset) | E2a `x.get_or_insert_with(\|\| 0.0)` — Option 維持 |
| `cell-c1-compound-arith-preserves-narrow.ts` | Sub-matrix 2 L1×R2a | scanner 廃止、R2a で narrow 維持 |
| `cell-c2a-nullish-assign-closure-capture.ts` | Sub-matrix 5 RC3×L1 stale | E2a `x.get_or_insert_with(\|\| 0.0)` |
| `cell-c2b-closure-reassign-arith-read.ts` | Sub-matrix 5 RC1×L1 stale | E2b `x.unwrap_or(0.0) + 1.0` |
| `cell-c2c-closure-reassign-string-concat.ts` | Sub-matrix 5 RC6×L1 stale | E2b with String default "null" |
| `cell-i024-truthy-option-complex.ts` | Sub-matrix 1 T4a×L1 complex | `if let Some(x) = x` + 複合 typeof/negation narrow |
| `cell-i025-option-return-implicit-none-complex.ts` | E4 match-exhaustive (multi exit) | 各 non-returning branch に implicit `None` 注入 |
| `cell-t4d-truthy-number-nan.ts` | Sub-matrix 1 T4d×L5 | E10 predicate `x != 0.0 && !x.is_nan()` |
| `cell-t7-optchain-compound-narrow.ts` | Sub-matrix 1 T7×L1 compound | OptChain 比較で `x` non-null narrow |
| `cell-regression-closure-no-reassign-keeps-e1.ts` | ✓ negative lock-in | E1 shadow-let (reassign なし closure は E2 に降格しない) |
| `cell-regression-null-check-narrow.ts` | ✓ T3a×L1 | `if let Some(x) = x` |
| `cell-regression-rc-narrow-read-contexts.ts` | ✓ RC1-RC8 alive survey | 既存 narrow emission (各 RC) |
| `cell-regression-f4-loop-body-narrow-preserves.ts` | ✓ F4 (narrow + loop body、no reassign) | `if let Some(x) = x { for i in 0..3 { out += x; } }` |
| `cell-regression-r5-nullish-on-narrowed-is-noop.ts` | ✓ R5 (narrow alive で `??=` 無効化) | E9 passthrough (narrow 維持) |

**union-coercion-dependent な regression** (typeof union / instanceof) は
snapshot test (`tests/fixtures/type-narrowing.input.ts` /
`narrowing-truthy-instanceof.input.ts`) で既に lock-in 済のため、E2E fixture
は作成しない: 現行の call-site literal → synthetic union 変換 gap (I-050 scope)
により runtime 比較できない。narrow emission 自体の回帰は snapshot で検出可能。

**T1 追加 probing で発見した pre-existing defect** (2026-04-19、observed ✓ → Rust
emission RED の差分):

- **I-161** `&&=` / `||=` 基本 emission 欠陥: R4 compound logical assign を
  `x = x && y` で素朴 emit → `&&` が f64 に非適用 (E0308)。cell-regression-r4-*
  fixture 試行で surfaced、regression lock-in 不能と判明して fixture 削除。
  TODO I-161 として別 PRD 化。
- **I-149 scope** F6 try/catch narrow + reassign: `throw` を関数 signature 無視で
  `return Err(...)` emit + catch body 欠落 + narrow reassign 型不整合。
  cell-regression-f6-* fixture 試行で surfaced、PRD Sub-matrix 4 F6 行を
  「I-149 scope」に明記、fixture 削除。

## Red-state verification

各 fixture を `target/release/ts_to_rs` で変換し、`tests/e2e/rust-runner` で
`cargo run` した結果:

| Fixture | Status | Error / observation |
|---------|--------|---------------------|
| cell-14-narrowing-reset-structural | **TRANSPILE FAIL** | `UnsupportedSyntaxError("nullish-assign with narrowing-reset (I-144)")` |
| cell-c1-compound-arith-preserves-narrow | **TRANSPILE FAIL** | 同上 (scanner false-positive: compound `x += 1` が reset 扱い) |
| cell-c2a-nullish-assign-closure-capture | **CARGO RUN FAIL** | `error[E0308]` shadow-let `let x = x.unwrap_or(0.0)` の f64 に closure `x = None` 代入不可 |
| cell-c2b-closure-reassign-arith-read | **CARGO RUN FAIL** | `error[E0308]` 同系統 + `x + 1` 側の f64 binding |
| cell-c2c-closure-reassign-string-concat | **CARGO RUN FAIL** | `error[E0308]` 同系統 |
| cell-i024-truthy-option-complex | **CARGO RUN FAIL** | `error[E0600]: cannot apply unary operator !` — `!x` narrow が Option<Union> に対して未 desugar |
| cell-i025-option-return-implicit-none-complex | **CARGO RUN FAIL** | `error[E0317]: if may be missing an else clause` — 多 branch fall-off で implicit None 未注入 |
| cell-t4d-truthy-number-nan | **CARGO RUN FAIL** | `error[E0308]` — truthy predicate が `!x.is_nan()` 未加、`if x` が Bool expected に非対応 |
| cell-t7-optchain-compound-narrow | **CARGO RUN FAIL** | `error[E0609]: no field v on type Option<_TypeLit0>` — compound `x?.v !== undefined` narrow 未伝播 |
| cell-regression-closure-no-reassign-keeps-e1 | **GREEN** | 既存 E1 shadow-let 経路が正常動作 |
| cell-regression-null-check-narrow | **GREEN** | 既存 T3a narrow が正常動作 |
| cell-regression-rc-narrow-read-contexts | **GREEN** | 既存 RC1-RC8 narrow read が正常動作 |
| cell-regression-f4-loop-body-narrow-preserves | **GREEN** | F4 narrow + loop body (no reassign) 正常動作 |
| cell-regression-r5-nullish-on-narrowed-is-noop | **GREEN** | R5 narrow alive での `??=` 予期通り no-op |

Red: 9/9 ✗ cell、Green: 5/5 ✓ cell。**T1 completion criterion 達成**。

## Test harness 組み込み

`tests/e2e_test.rs` に `test_e2e_cell_i144` 関数を追加。14 fixture 中 9 が RED
(TRANSPILE FAIL / CARGO RUN FAIL) のため `#[ignore]` 付き:

```rust
#[test]
#[ignore = "I-144 T1 red state — unignore at T6 when emission is rewired"]
fn test_e2e_cell_i144() {
    run_cell_e2e_tests("i144");
}
```

T6 完了時点で `#[ignore]` を外し、全 12 cell が green 化することを completion
criterion (Spec-Stage Review Checklist #5) の empirical 証左とする。

## Spec-Stage Adversarial Review Checklist

| # | Checklist item | Status | 根拠 |
|---|----------------|--------|------|
| 1 | Matrix completeness | ✅ | 全 cell に ideal output 記載 (PRD Sub-matrix 1-5) |
| 2 | Oracle grounding | ✅ | `tests/observations/i144/*.ts` (26 fixture) + tsx 観測記録 |
| 3 | NA justification | ✅ | T4f / L8/L9/L12/L14/L15/L18/L19 の NA 理由が spec-traceable |
| 4 | Grammar consistency | ✅ | T / L / RC 全 variant が `doc/grammar/*.md` 準拠 |
| 5 | **E2E readiness** | ✅ (T1 で green 化) | 本 report の 14 fixture (9 RED ✗ + 5 GREEN ✓) + test harness 登録完了 |

**Outstanding**: T2 で `/check_job` Spec Stage review 実施、defect 0 を目指す。

## Next step

- **T2**: Spec-Stage Adversarial Review Checklist 5 項目の再確認 + `/check_job`
  Spec Stage 版 (実装コード review なし、matrix / ideal output / oracle
  grounding 中心) 実施
- T2 完了で Spec stage 承認、**T3 Implementation stage** 着手
