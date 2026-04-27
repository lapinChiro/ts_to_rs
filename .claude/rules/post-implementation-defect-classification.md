# Post-Implementation Defect Classification

## When to Apply

Matrix-driven PRD の Implementation stage 完了後 (`/check_job` review 中)、
各 layer で発見された defect を分類する際に適用する。`check-job-review-layers.md` の
全 4 layer (Mechanical / Empirical / Structural cross-axis / Adversarial trade-off) が
本 file の category 分類を共有する。

## Core Principle

> **Defect の分類は trace に基づく (主観判断ではない)。各 category の trace 方法を
> 適用し、客観的な evidence をもって分類する。特に Spec gap は framework 失敗 signal
> として framework 自体の改善契機とする。**

## 5 Category

| Category | 定義 | trace 方法 |
|----------|------|-----------|
| **Grammar gap** | reference doc (`doc/grammar/`) に entry がない variant が関与する defect | reference doc (`ast-variants.md` / `rust-type-variants.md` / `emission-contexts.md`) に該当 entry がないことを確認 |
| **Oracle gap** | tsc observation が不十分 (未観測 or 観測不足) で ideal output が誤確定された defect | 該当 cell の observation log (`scripts/observe-tsc.sh` 出力) の有無 / 充足度を確認 |
| **Spec gap** | reference doc + oracle から derivable だったが matrix に漏れていた defect (= **framework 失敗 signal**) | reference doc に entry がある && observation も十分なのに enumerate されていないことを確認 |
| **Implementation gap** | spec 通りでない実装による defect | spec の ideal output と実装出力の diff を確認 |
| **Review insight** | spec も実装も正当、reviewer の新たな気づき (上記いずれにも分類不可) | 上記 4 category の trace 全てが該当しないことを確認 |

## Trace の優先順序

複数 category に該当しうる defect は、以下の順序で確認する (上位優先):

1. **Grammar gap**: reference doc の entry 有無を最初に check (gap があれば doc 更新が prerequisite)
2. **Oracle gap**: observation log の充足度を check (不足があれば observation 追加が必要)
3. **Spec gap**: enumerate の完全性 check (上記 1, 2 が ✓ で enumerate 漏れの場合)
4. **Implementation gap**: spec と実装の diff を check (spec は正しいが実装が乖離)
5. **Review insight**: 上記いずれにも該当しない真の新発見

## 成功条件

PRD 完了時の defect classification 結果が以下を満たす:

- **Spec gap = 0**: framework が機能していれば derivable な defect は spec stage で
  捕捉される。1 件でも Spec gap があれば `spec-stage-adversarial-checklist.md` の
  rule 適用が不十分だった証拠 → framework 改善契機
- **Implementation gap = 0**: spec と実装の乖離が残らない状態
- **Grammar gap / Oracle gap > 0 の場合**: PRD 完了前に reference doc / observation を
  追加し、再度 spec stage に戻る (`spec-first-prd.md` 「Spec への逆戻り」)
- **Review insight > 0**: framework が捕捉できない領域の発見、後続 PRD で取り扱う候補
  として TODO 起票

## Spec gap 発見時の対応

Spec gap (= framework 失敗 signal) を発見した場合、以下を実施する:

1. **Defect 自体の fix**: Spec stage に戻り matrix を更新、ideal output を確定、
   該当 cell の implementation を追加
2. **Framework 改善検討**: なぜ matrix 構築時に enumerate されなかったかを分析:
   - axis enumeration が不足? → `spec-stage-adversarial-checklist.md` Rule 10
     (Cross-axis matrix completeness) の 8 default check axis に該当 axis 追加検討
   - cross-cutting invariant 未認識? → Rule 8 (Cross-cutting invariant enumeration)
     の候補 invariant カテゴリ追加検討
   - dispatch arm enumerate 不足? → Rule 9 (Dispatch-arm sub-case alignment)
     の verification 手順強化検討
3. **TODO 起票**: framework 改善が必要と判断した場合、`.claude/rules/` 改修 PRD として
   TODO 起票

## Output Format

`/check_job` review 結果の Defect Classification Summary section で以下を report:

```markdown
### Defect Classification Summary

| Category | Count | Action |
|----------|-------|--------|
| Grammar gap | 0 | (無し) |
| Oracle gap | 0 | (無し) |
| Spec gap | 1 | **framework 失敗 signal** — Rule 10 の axis 追加検討 |
| Implementation gap | 2 | spec 通りに修正 |
| Review insight | 1 | TODO [I-NNN] 起票候補 |

### Spec gap detail (framework 失敗 signal)
- Defect: <description>
- Trace: reference doc に entry あり (`ast-variants.md:X`) && observation 十分
  (`tests/e2e/scripts/<prd>/<cell>.ts`) && matrix に enumerate されず
- Framework 改善検討: Rule 10 の default axis に "outer emission context = match arm body"
  追加 → TODO 起票候補
```

## Prohibited

- defect 分類を **主観判断** で行うこと (trace 方法を skip すること)
- Spec gap 発見時に framework 改善検討を行わず、defect 単体の fix のみで完了とすること
- Grammar gap / Oracle gap を Implementation gap として誤分類すること (root cause を
  reference doc / observation 側に move しなければならない defect を実装側で fix
  しようとして失敗する pattern)
- Review insight を「TODO 起票なしで close」すること (framework が捕捉できない領域は
  後続 PRD で取り扱う必要がある)

## Related Rules

| Rule | Relation |
|------|----------|
| [check-job-review-layers.md](check-job-review-layers.md) | 各 layer で発見された defect の分類で本 file を参照 |
| [spec-stage-adversarial-checklist.md](spec-stage-adversarial-checklist.md) | Spec gap 発見時の framework 改善検討対象 (Rule 6-12) |
| [spec-first-prd.md](spec-first-prd.md) | Spec gap 発見時に「Spec への逆戻り」手順を発動 |
| [problem-space-analysis.md](problem-space-analysis.md) | Spec gap 発見時に matrix 構築 methodology の改善検討 |
| [conversion-correctness-priority.md](conversion-correctness-priority.md) | Implementation gap で発見された silent semantic change の Tier 1 分類 |

## Versioning

- **v1.0** (2026-04-25): `spec-first-prd.md` line 154-167 の "Post-Implementation Review:
  Defect Classification" を本 file に分離。Trace 優先順序、Spec gap 発見時の framework
  改善検討手順を強化追加。
