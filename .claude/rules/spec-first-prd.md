# Spec-First PRD Workflow (SDCDF)

## When to Apply

Matrix-driven PRD (変換機能の PRD で問題空間マトリクスを持つもの) を作成・実装する際に適用する。
Non-matrix PRD (infra, refactor, bug fix) には適用しない。

判定基準: PRD が `problem-space-analysis.md` の入力次元 (AST shape / TS type / emission context)
を持つなら matrix-driven。

本ルールは I-SDCDF PRD の Root Cause 1-4 分析から derive された (plan.prd.md v4、
git history 参照)。

## Core Principle

> **Implementation に先立ち、specification を外部 oracle と grammar-derived matrix で
> grounding する。実装は spec 準拠のみを目的とし、ad-hoc な spec 拡張を禁止する。**

## PRD Lifecycle: 2 Stage

Matrix-driven PRD は以下の 2 stage を順に通過する。stage をまたいだ逆戻りは許可するが、
逆戻り時は前 stage の artifact を更新してから再開する。

### Stage 1: Specification (実装着手前)

**目的**: 問題空間を完全に enumerate し、全セルの ideal 出力を外部根拠で確定する。

**必須 artifact**:

1. **Grammar-derived matrix**
   - 入力次元は `doc/grammar/ast-variants.md`, `doc/grammar/rust-type-variants.md`,
     `doc/grammar/emission-contexts.md` の reference doc を参照して enumerate する。
   - 「思いつく variant」ではなく、reference doc の variant リストから「この機能に
     関与するか否か」を全 variant について判定する。
   - NA (Not Applicable) のセルは理由を記載する。理由は TS syntax error / SWC が
     reject / Rust type system の構造的制約等の **spec-traceable** な根拠のみ。
     「稀」「見たことがない」「多分使われない」は NA 理由として不可。

2. **tsc observation (外部 oracle)**
   - matrix の各セル (少なくとも ✗ および 要調査 のセル) に対して、TS fixture を
     作成し `scripts/observe-tsc.sh` で tsc / tsx の挙動を観測する。
   - 観測結果 (stdout, stderr, exit_code) を PRD に記録する。
   - Ideal Rust 出力は「tsc observation の runtime stdout を Rust でも再現する」
     を原則とする。tsc 挙動と ideal 出力が乖離する場合は乖離理由を明記する。

3. **Per-cell E2E fixture (red 状態)**
   - `tests/e2e/scripts/<prd-id>/<cell-id>.ts` に cell 単位の TS fixture を作成する
     (Phase 2 完了後)。
   - `scripts/record-cell-oracle.sh` で expected output を記録する (Phase 2 完了後)。
   - Implementation stage 着手時点で全 fixture が red (= ts_to_rs 変換結果が
     expected output と不一致) であることを確認する。
   - **目的 (単なる test の準備ではない)**: E2E fixture 作成は **Rust emission の
     empirical probe** を兼ねる。tsc observation は TS 側のみ ground するため、
     matrix cell が「observed ✓ preserved」と記されていても Rust emission が
     broken なケースが頻発する。T1 E2E probe で初めて `observation ✓ / Rust ✗`
     の乖離が検出され、cell を「✗ 本 PRD scope / 別 PRD scope / pre-existing
     defect」に empirical 再分類する責任を持つ (下記 **Dual verdict (TS / Rust)**
     参照)。
   - **Transitional (Phase 2 未完了時)**: per-cell layout が未整備の間は、既存 E2E
     framework (`tests/e2e/scripts/<name>.ts` + `tests/e2e_test.rs`) の形式で
     cell fixture を作成してよい。ただし cell 単位で分離すること (1 fixture に
     複数 cell を混在させない)。Phase 2 完了後に新 layout へ移行する。

4. **Ideal output specification**
   - 全セルに Ideal Rust 出力を記載する。空欄 / T.B.D. / 「後で決める」は不可。
   - Ideal 出力は tsc observation に grounding されている (上記 2)。

**禁止事項 (Spec stage)**:
- 上記 4 artifact のいずれかが不完全な状態で Implementation stage に移行すること。
- 「思いつく variant のみ」で matrix を構成すること (reference doc を使う)。
- tsc observation なしに ideal 出力を「宣言」すること。
- NA 理由に「稀」「頻度が低い」を使うこと。
- **tsc observation の ✓ を Rust emission の ✓ と同一視すること** (下記「Dual verdict」)。
- matrix cell に単なる「✓ preserved」等と記載し TS / Rust の検証状態を区別しないこと。

**Stage 1 完了 verification**: [`spec-stage-adversarial-checklist.md`](spec-stage-adversarial-checklist.md)
の 10 項目を全 verification する。1 つでも未達なら Implementation stage 移行不可。

### Dual verdict (TS / Rust) の明示 — observation ✓ ≠ Rust emission ✓

tsc observation (Stage 1 artifact #2) は **TS 側の runtime semantic のみ** を ground
する。Rust 側の emission が同 semantic を達成しているかは **T1 per-cell E2E fixture
作成時の empirical probe** (artifact #3) で初めて検証される。両者は独立な事実:

| TS observation | Rust emission | matrix 記載の ideal 形 |
|----------------|---------------|--------------------|
| ✓ preserved | ✓ GREEN | `✓ preserved` (TS ground + T1 empirical 確認済) |
| ✓ preserved | ✗ RED | **`TS ✓ / Rust ✗ (<defect-id or scope note>)`** — 別 PRD scope か本 PRD 対象か T2 で判定 |
| ✗ unreachable | — (NA) | `NA (<理由>)` |
| 要調査 | 未 probe | `要調査` — Discovery で解消必須 |

**Spec stage で Rust emission 側を「観測せず ✓」と書いてはならない**。matrix 生成時
には「observed (TS) ✓ / Rust 未 probe」が正しい状態。T1 E2E fixture 作成で Rust
側が確定し、そこで初めて verdict を固定する。

**実装ガイド**:
- Discovery (T0) 段階の matrix では「TS ✓ / Rust 要 T1 probe」形式で書く
- T1 probe で RED が判明した cell は **pre-existing defect** として新 TODO 起票または
  既存別 PRD (例: I-050 / I-149) に吸収 → 本 PRD regression fixture から削除
- T1 probe で GREEN が判明した cell のみ「TS ✓ / Rust ✓」regression fixture として lock-in

### Stage 2: Implementation (spec approved 後)

**目的**: Spec stage で確定した仕様を実装し、per-cell E2E fixture を green にする。

**許可される活動**:
- Spec で定義された各セルの ideal 出力を実装する。
- Per-cell E2E fixture を red → green にする。
- Unit test / integration test を追加する。
- 実装中に spec の曖昧性を発見した場合、**Spec stage に戻る** (下記参照)。

**禁止事項 (Implementation stage)**:
- Spec に定義されていないセルを実装すること (silent scope 拡張)。
- Spec の ideal 出力と異なる出力を「実装の都合で」採用すること。
- Ad-hoc に新しい edge case を発見し、spec を更新せずに実装に組み込むこと。

**Stage 2 完了 verification**: [`check-job-review-layers.md`](check-job-review-layers.md)
の 4 layer (Mechanical / Empirical / Structural cross-axis / Adversarial trade-off)
を `/check_job` 初回 invocation で全実施する。発見された defect は
[`post-implementation-defect-classification.md`](post-implementation-defect-classification.md)
の 5 category に分類する。

### Spec への逆戻り (Implementation → Spec)

実装中に以下を発見した場合、**必ず Spec stage に戻る**:

1. Spec に記載のないセルが必要と判明した。
2. Spec の ideal 出力が tsc 挙動と矛盾していると判明した。
3. Spec が曖昧で、2 通り以上の実装が可能。
4. `/check_job` review で **Spec gap** category の defect が発見された
   (`post-implementation-defect-classification.md` 参照)。

逆戻り手順:
1. 発見内容を PRD の「Spec Revision Log」section に記録する。
2. 必要に応じて tsc observation を追加実施する。
3. Matrix を更新し、ideal 出力を確定する。
4. Per-cell E2E fixture を更新する。
5. 上記完了後に Implementation stage を再開する。

## Reference Docs の使い方

| Doc | 用途 |
|-----|------|
| `doc/grammar/ast-variants.md` | AST shape 次元の variant 列挙時に全件チェック |
| `doc/grammar/rust-type-variants.md` | TS type 次元の variant 列挙時に全件チェック |
| `doc/grammar/emission-contexts.md` | Outer context 次元の列挙時に全件チェック |

各 doc の Version snapshot (SWC version, observation date) を確認し、PRD 作成時点の
コードベースと整合していることを担保する。stale な場合は doc を先に更新する。

## Prohibited

- Matrix-driven PRD で本ルールを適用しないこと。
- Spec stage の artifact が不完全な状態で Implementation を開始すること。
- tsc observation なしに ideal output を「宣言」すること。
- Implementation 中に発見した spec gap を spec に戻らず ad-hoc に fix すること。
- Post-implementation review で defect を trace なしに分類すること
  (`post-implementation-defect-classification.md` の trace 方法を遵守)。
- 「Spec stage は overhead が大きいのでスキップ」と判断すること。

## Related Rules

| Rule | Relation |
|------|----------|
| [problem-space-analysis.md](problem-space-analysis.md) | 本ルールの前提。matrix 構築の detailed methodology |
| [spec-stage-adversarial-checklist.md](spec-stage-adversarial-checklist.md) | Stage 1 完了 verification (10-rule checklist) |
| [check-job-review-layers.md](check-job-review-layers.md) | Stage 2 完了 verification (4-layer review framework) |
| [post-implementation-defect-classification.md](post-implementation-defect-classification.md) | Stage 2 review 結果の defect 5 category 分類 |
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 最上位原則。本ルールは subordinate |
| [prd-completion.md](prd-completion.md) | 完了条件に matrix 全セルカバーを含める |
| [prd-design-review.md](prd-design-review.md) | 設計段階の review は本ルールの Spec stage 内で実施 |
| [type-fallback-safety.md](type-fallback-safety.md) | 型 fallback 導入時の安全性分析 |

## Versioning

- **v1.0** (2026-04-25 SDCDF Rollout 1.0): I-178 で Spec-Stage Adversarial Review Checklist が
  5 → 10 rule に拡張、`spec-stage-adversarial-checklist.md` に分離。I-183 で `/check_job`
  Stage Dispatch を `check-job-review-layers.md` に分離、4-layer framework 化。
  Defect Classification 5 category を `post-implementation-defect-classification.md`
  に分離。本 file は Stage 1/2 lifecycle + Dual verdict + Spec への逆戻り に責務集中。
- **Beta** (2026-04-17 Phase 4 Rollout 昇格): SDCDF Pilot (I-050-a) で Spec gap = 0 を
  達成し正式 rule として昇格。
- **Lessons Learned (Pilot Phase)**:
  - **TypeResolver と IR 型の乖離**: `input as string` は TypeResolver 上 `String` だが IR
    上 `serde_json::Value` のまま。Ident coercion は TypeResolver 精度向上が前提、
    Lit のみに限定すべき (I-050-a Pilot)。
  - **Any-narrowing enum との交差**: `typeof` guard 付き `any` 変数は narrowing enum に
    置換、`RustType::Any` マッチの coercion 条件に該当しない (I-050-a Pilot)。
  - **expected_override と TypeResolver expected_type の分離**: closure body / NC branch で
    false-positive `Any` propagation あり、coercion は expected_override (明示呼び出し元)
    のみで発動が安全 (I-050-a Pilot)。
  - **Dual verdict (TS / Rust)**: I-144 T1 で R4 / F6 が TS observation ✓ だが Rust emission ✗
    (E0308 / try body 崩壊) を empirical 発見、framework に Dual verdict 条項を追加
    (2026-04-19)。
