# Ideal Implementation Primacy

本プロジェクトの最上位原則。他の全てのルールはこの原則に従属する。

## When to Apply

全ての作業 (PRD 起票、設計、実装、review、commit) の前提として常時適用される。
他 rule が本ルールと conflict した場合、本ルールが優先する。

## Top-Level Goal

**理論的に最も理想的な TypeScript → Rust トランスパイラを獲得すること。**

「あらゆる valid な TypeScript プログラムに対して、意味論的に等価な Rust を
生成できること」が究極の完成状態。実装コスト・変更規模・既存計画との整合性は
判断基準として二次的。

## Metric Positioning

Hono ベンチマーク (`clean files`, `compile rate`, `error instances`) や類似の
定量指標は **defect 発見のためのシグナル** であり、**最適化すべきターゲットでは
ない**。

| 場面 | 正しい姿勢 | 誤った姿勢 |
|---|---|---|
| ベンチ数値が改善 | 「意味論的に正しい解になっているか検証する」 | 「数値が上がったから OK」 |
| ベンチ数値が変わらない | 「silent semantic loss が潜んでいないか確認する」 | 「無害だから commit して良い」 |
| ベンチ数値が悪化 | 「原因を特定する (回避でも良い = root cause 理解が優先)」 | 「数値を戻すことを目的化する」 |
| 理想解がわかるが変更範囲が大きい | 「調査を尽くし、理想解の PRD を起票する」 | 「影響が小さい patch で済ませる」 |

## Patch vs Structural Fix

- **Patch**: 症状を抑えるが、根本の IR / 設計欠陥を変更しない修正
- **Structural fix**: 根本欠陥そのものに対処する修正

### Prohibited

- Structural fix が feasible なのに patch を **永続解** として commit すること
- Patch 適用後、その patch を「動いているから良い」として放置すること
- 「まずは patch で、structural fix は後で」という曖昧な先送り

### Interim Patch (暫定対応) として許容される条件

以下 **全て** を満たす場合のみ、patch を interim として許容する:

1. Structural fix への PRD または調査タスクが **同時に起票** されている
2. Patch 箇所のコメントに `// INTERIM: <structural fix task ID>` が記載されている
3. Patch が silent semantic change を **導入していない** ことが検証されている
4. `session-todos.md` (project root 直下) に削除基準 (when to remove) が記載されている

これらを満たさない場合、patch の commit は禁止。

### `session-todos.md` の定義

- **Path**: project root 直下 (`/home/kyohei/ts_to_rs/session-todos.md`)
- **Role**: 進行中 session で発生した interim patch の **削除基準** を集約する file。git tracked (commit 対象)、各 entry は patch ID + structural fix task ID + when-to-remove condition の triple
- **Lifecycle**: structural fix PRD 完了時に対応 entry を削除。session-todos.md が空になった時点で fully clean
- **Format example**:
  ```markdown
  ## INTERIM-NNN: <description>
  - **Patch location**: `src/foo.rs:42`
  - **Structural fix task**: I-XXX (link to TODO / backlog)
  - **When to remove**: `<condition, e.g., "I-XXX completed and pre/post empirical verified">`
  ```
- **本 file 不在時の扱い**: interim patch の commit は禁止 (条件 4 違反)。commit 前に file を作成する

## Prohibited

- ベンチ数値の改善を PRD completion criteria の主要指標にすること
- 「実装コストが高い」「影響範囲が広い」を理由に structural fix を patch に降格すること
- 調査不足のまま「現状動いている」判断で patch を確定すること
- Interim patch の removal criteria を記録せず次作業に移ること

## Related Rules

本ルールは最上位原則であり、以下の rule が subordinate:

| Rule | Relation |
|------|----------|
| [problem-space-analysis.md](problem-space-analysis.md) | 本ルールを実装レベルで具体化する最上位 PRD rule。理想実装は問題空間の網羅なしに達成不可能 |
| [conversion-correctness-priority.md](conversion-correctness-priority.md) | Tier 1 (silent semantic change) 定義と整合、本ルール origin |
| [conversion-feasibility.md](conversion-feasibility.md) | 「難しい」を理由にした優先度降格の禁止、本ルールの強化版 |
| [type-fallback-safety.md](type-fallback-safety.md) | 型 fallback 導入時の安全性分析、Tier 1 silent semantic 防止 methodology |
| [todo-prioritization.md](todo-prioritization.md) | 本ルールを前提として優先度を決定。Step 0 (Uncertainty Check) は本ルールに従属 |
| [prd-completion.md](prd-completion.md) | PRD 完了は理想解の達成を意味し、patch による基準ずらしを禁止 |
| [design-integrity.md](design-integrity.md) | 設計判断の唯一の基準は「最も理想的な実装」、本ルール由来 |
| [spec-first-prd.md](spec-first-prd.md) | matrix-driven PRD lifecycle の理論的根拠 (理想実装の structural 達成 framework) |
| [check-job-review-layers.md](check-job-review-layers.md) | Layer 4 (Adversarial trade-off) の patch / structural fix 区分 base |
