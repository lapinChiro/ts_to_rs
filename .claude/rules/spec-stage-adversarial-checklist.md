# Spec-Stage Adversarial Review Checklist

## When to Apply

Matrix-driven PRD (`spec-first-prd.md` 適用対象) の **Spec stage 完了時**、Implementation stage への移行可否を判定する自己 review として全項目を verification する。1 つでも未達の項目があれば Implementation stage への移行は不可。

## Core Principle

> **Spec stage 完了の合否を「artifact が揃った」ではなく「全 10 項目の adversarial check が
> pass した」で判定する。各 rule は empirical defect chain から derive されたもので、
> rule を逐次 verification することで同 root cause の defect 再発を構造的に防ぐ。**

## Checklist (10 rule)

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
- [ ] **Matrix/Design integrity**: PRD の Problem Space matrix「Ideal output」列の
      emission shape と、PRD の Design section に記述された emission strategy
      (helper signature, emit pattern, target Rust syntax) が **token-level に一致** する。
      乖離が 1 例でも存在する場合は、(a) どちらが正規 spec かを明記し、
      (b) 非正規側を正規側に updating commit してから checklist を満たしたとみなす。
      Verification: matrix の各 cell について Design section から該当 emission rule を
      引用し、両者を side-by-side で diff する。
      (Lesson: I-161 SG-2 — matrix が `pred(x)` を ideal とし Design が `match` block を
      ideal としていた spec 内乖離を初回 review で漏らした事例。)
- [ ] **Control-flow exit sub-case completeness**: Matrix cell の dimension に
      "body shape" / "branch shape" が含まれる場合、各 branch の **exit-or-fallthrough
      状態** を独立次元として enumerate する。最低 4 sub-case
      (then_exits × else_exits = T/T, T/F, F/T, F/F) を明示 cell として PRD に記載し、
      各 sub-case に対応する E2E fixture と ideal output を spec する。
      "any × any" / "either-exits" / "regardless of else" 等の集約表現は禁止
      (集約は post-implementation の audit で defect を hide する)。
      Verification: matrix table で body / else dimension の cell を抽出し、各 cell の
      row が 4 行に展開されていることを目視確認。
      (Lesson: I-171 T5 SG-T5-DEEP1 — C-5 cell が "any body × any else" で lump され、
      (then_exits=T, else_exits=F) sub-case の post-if narrow materialization 漏れが
      4 度目の iteration で発覚。)
- [ ] **Cross-cutting invariant enumeration**: 機能仕様の中に「matrix cell に展開
      できない / 全 cell で同時に成立する必要がある」transversal property が存在しないか
      自問し、存在する場合は PRD に独立 section として `## Invariants` を設けて列挙する。
      各 invariant について以下 4 項目を必須記述:
      - (a) **Property statement**: 1 文で書けるレベルの不変条件
            (例: 「TypeResolver の expr_type と IR の Type が同一 span に対し一致」)
      - (b) **Justification**: なぜこの invariant が必要か
            (この invariant 違反でどんな defect class が発生するか)
      - (c) **Verification method**: 実装後に invariant 成立を verify する具体手順
            (probe / test / static analysis のどれを使うか)
      - (d) **Failure detectability**: invariant 違反が compile error / runtime error /
            silent semantic change のどれで顕在化するか
      候補 invariant カテゴリ (探索 prompt として活用): TypeResolver-IR cohesion /
      並列 emission path symmetry / closure-reassign suppression cohesion /
      scope boundary preservation / mutability propagation。
      (Lesson: I-171 T5 SG-T5-DEEPDEEP1/2 — matrix cell 列挙では捕捉不能な「TypeResolver
      と IR の同期 invariant」「if-stmt の then/else 並列 emission path の symmetry
      invariant」が後付けで INV-1/2/3 として retroactive に enumerate された経緯。
      前置 enumerate していれば 2 度目 deep iteration の defect は initial review で
      発見可能だった。)
- [ ] **Dispatch-arm sub-case alignment**: Matrix の各 type-dimension は、実装側で
      **branch / dispatch / pattern-match を分けるあらゆる sub-classifier** と一対一の
      粒度で enumerate する。具体例: `Named` を単一 cell として記述する代わりに、実装が
      `is_synthetic_union` flag や `is_always_truthy` 判定で dispatch 分岐するなら
      `Named (synthetic union)` / `Named (always-truthy)` / `Named (other)` の 3 cell に
      分割。
      Verification (両方向の同期):
      - (a) **Spec → Impl**: PRD 確定後、実装着手前に「実装 file の dispatch / match を
            全 enumerate し、matrix cell と 1-to-1 対応するか」を check
      - (b) **Impl → Spec**: 実装中に新しい dispatch arm を追加する必要を発見した場合、
            Spec stage に戻って matrix cell を分割する (`spec-first-prd.md` の
            「Spec への逆戻り」手順を発動)
      実装の dispatch arms と PRD matrix cell が乖離する場合は **Spec gap signal**:
      PRD 起草時に問題空間を網羅していなかった証拠であり、現 PRD scope の rework または
      別 PRD 切り出しを判断する。
      (Lesson: I-171 T5 SG-T5-FRESH1 — `Option<Named other>` を PRD で 1 cell とした
      結果、実装側で synthetic union / 非 synthetic 2 dispatch arm の存在に気付かず、
      4 度目 iteration で `always-truthy 全型対応漏れ` として defect 発覚。)
- [ ] **Cross-axis matrix completeness**: PRD の matrix は **「PRD の解決軸 single
      dimension」ではなく「機能 emission を決定する全 input dimension の直積
      (Cartesian product)」** で構築する。
      Construction procedure (3 steps):
      1. **Axis enumeration**: 機能 emission を決定する独立 input dimension を全列挙
         する。最低限 check すべき axis 候補:
         (a) trigger condition (operator / syntax-form), (b) operand type variants,
         (c) guard variant (typeof / equality / instanceof / truthy),
         (d) body shape (block / expr / single-stmt / empty),
         (e) closure-reassign 有無, (f) early-return 有無,
         (g) outer emission context (return / assign target / call arg / branch arm),
         (h) control-flow exit (上記 Rule "Control-flow exit sub-case completeness"
         の 4 sub-case)。機能依存で必要な axis を追加するが、上記 8 候補は default
         check 対象とする。
      2. **Orthogonality verification**: 各 axis pair (A, B) について「A の variant 変化が
         B の variant 出力を変えるか」を自問し、yes なら独立 dimension として保持、
         no なら一つに統合 (NA cell を作る前にこの統合を実施)。
      3. **Cartesian product expansion**: 残った全 axis の直積を matrix table に展開し、
         各 cell に ideal output を spec する。NA cell には spec-traceable な reason を
         記載 (`spec-first-prd.md` Stage 1 artifact #1 参照)。

      直交軸の発見方法 (axis enumeration を補完する 3 prompt):
      - **(I) 逆問題視点**: 解決軸の対立軸を試案化。
        例: 解決軸=cohesion → 反問軸=trade-off /
        解決軸=symmetric-coverage → 反問軸=asymmetric-coverage /
        解決軸=preservation → 反問軸=erasure
      - **(II) 実装 dispatch trace**: 実装の dispatch / branch / pattern-match が消費する
        次元 (guard variant, type, operator, context) を全列挙し、各々を独立 axis として
        PRD に reflect
      - **(III) 影響伝搬 chain**: "X が変わると Y が変わるか" を再帰適用し、間接 dimension
        を抽出

      禁止事項: "実装した cell リスト" を matrix と称すること /
      "思いつく組合せのみ" / "代表的な軸のみ" / 「典型的でない組合せは省略」。
      (Lesson: I-161 T7 三度の `/check_job` iteration — 4 defect 全てが直積 enumeration
      不足に帰着。Defect 1 = trigger conditions × guard variants で Truthy 誤発火 /
      Defect 2 = emission paths × null-check direction で path 3 symmetric coverage 欠落 /
      Defect 3 = test source × structural properties で empty else false-positive /
      Defect 4 = emission form × body shape で Scenario A regression。
      解決軸内 coverage は意識していたが直交軸を見ていなかった。)
```

## Prohibited

- 1 つでも未達の checkbox がある状態で Implementation stage に移行すること
- checkbox を「[x] にした」だけで内容未検証で済ませること (各 rule の Verification 手順を
  実際に実施すること)
- Rule 6-10 の Lesson source を読まずに「過去の事例だから」と省略判断すること
  (lesson は rule 適用時の判断基準として load-bearing)
- 「典型的でない」「実装が複雑になる」を理由に matrix cell を省略すること
  (`problem-space-analysis.md` の最上位原則違反)

## Related Rules

| Rule | Relation |
|------|----------|
| [spec-first-prd.md](spec-first-prd.md) | 本 checklist を発動する PRD lifecycle workflow。Spec stage 完了 verification 手段として本 file を参照 |
| [problem-space-analysis.md](problem-space-analysis.md) | Rule 10 (Cross-axis matrix completeness) の理論的根拠。matrix construction の detailed methodology |
| [check-job-review-layers.md](check-job-review-layers.md) | 本 checklist の symmetric counterpart (Implementation stage 側 review framework)。Rule 10 ↔ Layer 3 (Structural cross-axis) は同 lesson の spec/review 両面 |
| [post-implementation-defect-classification.md](post-implementation-defect-classification.md) | Spec stage で漏れた defect が Implementation stage で発見された場合の category 分類 (Spec gap = 本 checklist 失敗の indicator) |
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 最上位原則。本 checklist は理想実装達成のための structural verification mechanism |

## Versioning

- **v1.0** (2026-04-25): 5-rule (Matrix completeness / Oracle grounding / NA justification / Grammar consistency / E2E readiness) を `spec-first-prd.md` から本 file に分離、Rule 6-10 を I-178 で追加。
  - Rule 6: Matrix/Design integrity (lesson: I-161 SG-2)
  - Rule 7: Control-flow exit sub-case completeness (lesson: I-171 T5 SG-T5-DEEP1)
  - Rule 8: Cross-cutting invariant enumeration (lesson: I-171 T5 SG-T5-DEEPDEEP1/2)
  - Rule 9: Dispatch-arm sub-case alignment (lesson: I-171 T5 SG-T5-FRESH1)
  - Rule 10: Cross-axis matrix completeness (lesson: I-161 T7 三度 `/check_job` iteration)
