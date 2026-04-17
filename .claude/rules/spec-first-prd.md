# Spec-First PRD Workflow (SDCDF Alpha)

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

**Spec への逆戻り (Implementation → Spec)**:

実装中に以下を発見した場合、**必ず Spec stage に戻る**:

1. Spec に記載のないセルが必要と判明した。
2. Spec の ideal 出力が tsc 挙動と矛盾していると判明した。
3. Spec が曖昧で、2 通り以上の実装が可能。

逆戻り手順:
1. 発見内容を PRD の「Spec Revision Log」section に記録する。
2. 必要に応じて tsc observation を追加実施する。
3. Matrix を更新し、ideal 出力を確定する。
4. Per-cell E2E fixture を更新する。
5. 上記完了後に Implementation stage を再開する。

## Spec-Stage Adversarial Review Checklist

Spec stage 完了時、以下の 5 項目を全て [x] にする。1 つでも未達なら
Implementation stage に移行不可。

```markdown
## Spec-Stage Review Checklist

- [ ] **Matrix completeness**: 全セルに ideal output が記載されている
      (空欄 / TBD なし)
- [ ] **Oracle grounding**: ✗ / 要調査 セルの ideal output が tsc observation
      log と cross-reference されている
- [ ] **NA justification**: NA セルの理由が spec-traceable (syntax error,
      grammar constraint 等) であり、「稀」「多分」等の曖昧理由がない
- [ ] **Grammar consistency**: matrix に reference doc に未記載の variant が
      存在しない (存在すれば reference doc を先に更新)
- [ ] **E2E readiness**: 各セルに対応する E2E fixture が (red 状態で) 準備
      されている
```

## Post-Implementation Review: Defect Classification

Implementation stage 完了後の `/check_job` review で発見された defect は
以下の 5 category に分類する。分類は **trace** に基づく (主観判断ではない):

| Category | 定義 | trace 方法 |
|----------|------|-----------|
| Grammar gap | reference doc に entry がない variant が関与 | doc に該当 entry がないことを確認 |
| Oracle gap | tsc observation が不十分 (未観測 or 観測不足) | 該当 cell の observation log 有無を確認 |
| Spec gap | reference doc + oracle から derivable だったが matrix に漏れ | doc に entry があり、observation も十分なのに enumerate されていない |
| Implementation gap | spec 通りでない実装 | spec の ideal output と実装の diff |
| Review insight | spec も実装も正当、reviewer の新たな気づき | 上記いずれにも分類不可 |

**成功条件**: Spec gap = 0 かつ Implementation gap = 0。

## `/check_job` Stage Dispatch

`/check_job` 実行時、PRD の状態に応じて review 内容を切り替える:

- **Spec stage** (Implementation 未着手):
  - Spec-Stage Adversarial Review Checklist の 5 項目を検証。
  - Matrix の各セルに対して「この ideal output は正しいか」を adversarial に検証。
  - Reference doc との cross-check。
  - **実装コードは review 対象外** (存在しないため)。

- **Implementation stage** (Spec approved 後):
  - 従来の check_job review (実装の理想性、テスト品質)。
  - 追加: 各セルの実装出力が spec の ideal output と一致するかを検証。
  - Post-implementation defect classification (上記 5 category)。

## Reference Docs の使い方

| Doc | 用途 |
|-----|------|
| `doc/grammar/ast-variants.md` | AST shape 次元の variant 列挙時に全件チェック |
| `doc/grammar/rust-type-variants.md` | TS type 次元の variant 列挙時に全件チェック |
| `doc/grammar/emission-contexts.md` | Outer context 次元の列挙時に全件チェック |

各 doc の Version snapshot (SWC version, observation date) を確認し、PRD 作成時点の
コードベースと整合していることを担保する。stale な場合は doc を先に更新する。

## 関連ルール

| ルール | 関係 |
|--------|------|
| `problem-space-analysis.md` | 本ルールの前提。matrix 構築の詳細手順 |
| `ideal-implementation-primacy.md` | 最上位原則。本ルールは subordinate |
| `prd-completion.md` | 完了条件に matrix 全セルカバーを含める |
| `prd-design-review.md` | 設計段階の review は本ルールの Spec stage 内で実施 |
| `type-fallback-safety.md` | 型 fallback 導入時の安全性分析 |

## Prohibited

- Matrix-driven PRD で本ルールを適用しないこと。
- Spec stage の artifact が不完全な状態で Implementation を開始すること。
- tsc observation なしに ideal output を「宣言」すること。
- Implementation 中に発見した spec gap を spec に戻らず ad-hoc に fix すること。
- Post-implementation review で defect を trace なしに分類すること。
- 「Spec stage は overhead が大きいのでスキップ」と判断すること。

## Pilot Lessons Learned (I-050-a)

Phase 3 Pilot (I-050-a: primitive Lit → Value coercion) で得た知見:

1. **TypeResolver の expr_type と IR 型の乖離**: `input as string` は TypeResolver 上
   `String` だが IR 上は `serde_json::Value` のまま。Ident に対する coercion は
   TypeResolver の精度向上 (IR 型との整合) が前提であり、Lit のみに限定すべき。
2. **Any-narrowing enum との交差**: `typeof` guard 付き `any` 変数は narrowing enum
   に置換されるため、`RustType::Any` マッチの coercion 条件に該当しない。Narrowing
   enum の variant constructor wrap は I-030 (別 PRD) の scope。
3. **expected_override と TypeResolver expected_type の分離**: TypeResolver の
   expected_type() は false-positive `Any` を propagate するケースがある (closure body,
   NC branch)。coercion は expected_override (明示呼び出し元) のみで発動させるのが安全。

## Versioning

本ルールは SDCDF **Beta** (Phase 4 Rollout 昇格: 2026-04-17)。
Phase 3 Pilot で **Spec gap = 0** を達成し、正式 rule として昇格。
今後の matrix-driven PRD に必須適用する。
