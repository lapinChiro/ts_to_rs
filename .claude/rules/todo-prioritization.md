# TODO Prioritization Criteria

## When to Apply

When prioritizing TODO or backlog items, reordering plan.md batches, or selecting next work.

## Core Principle

> "If we don't do this now, will the cost per unit of future development increase?"

Always address root causes, not surface symptoms. Prioritize based on the nature of the root cause (design flaw, reliability risk, etc.), not the visible symptom (compile error, unsupported syntax, etc.).

**Primary goal**: 本ルールは [`ideal-implementation-primacy.md`](ideal-implementation-primacy.md) に subordinate (最上位原則 / patch vs structural / metric positioning は同 file 参照)。

## Decision Flow (Overview)

```
┌────────────────────────────────────────────────────────────┐
│ Step 0: Uncertainty Check                                  │
│   未解決の調査債務 (INV) があるか?                          │
│   ├─ Yes → 調査を実施して債務を解消 (L1-L4 判定より先)       │
│   └─ No  → Step 1 へ                                       │
└────────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────────┐
│ Step 1: Root Cause Clustering                              │
│   症状ではなく根本原因でグルーピング                         │
└────────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────────┐
│ Step 2: Priority Level (L1 > L2 > L3 > L4)                 │
│   L1: Reliability Foundation (silent semantic change 等)    │
│   L2: Design Foundation (基盤欠陥、繰り返し発生)             │
│   L3: Expanding Technical Debt (時間で拡大)                 │
│   L4: Localized Problem (局所的)                            │
└────────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────────┐
│ Step 3: Ordering Within Same Level                         │
│   Leverage > Expansion Rate > Fix Cost                     │
└────────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────────┐
│ 修正適用時の原則                                             │
│   Structural Fix > Interim Patch                           │
│   (ideal-implementation-primacy.md 参照)                    │
└────────────────────────────────────────────────────────────┘
```

## Related Rules

| Rule | Relation |
|------|----------|
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 最上位原則 (本ルールが subordinate)。理想実装優先、patch vs structural の base |
| [todo-entry-standards.md](todo-entry-standards.md) | TODO 項目の記載フォーマット (通常 + INV)。Step 0 (Investigation Debt) の format spec |
| [conversion-correctness-priority.md](conversion-correctness-priority.md) | L1 判定の基準 (silent semantic change = Tier 1 の分類) |
| [conversion-feasibility.md](conversion-feasibility.md) | 「難しい」を理由にした降格の禁止 |
| [problem-space-analysis.md](problem-space-analysis.md) | Step 1 root cause clustering の理論的根拠 (matrix で問題空間 enumerate) |

## Step 0: Uncertainty-Driven Investigation Check (前置ステップ)

**優先度を決める前に、未解決の不確定要素を棚卸しする。**

計画に影響する「未調査の前提」や「assumption ベースで書かれた根拠」を列挙し、
それらの解消を routine な L1-L4 作業より先に実施する。未調査のまま先の計画を
確定すると、後から根本的な設計覆しが発生する risk がある。

### 調査債務 (Investigation Debt) の扱い

- 調査債務は TODO に `[INV-N]` 形式で一級市民として記録する (詳細は `todo-entry-standards.md`)
- 調査債務は「影響範囲の絞り込みが十分に出来るレベル」まで潰してから次段階に進む
- 「現状動いているから調査は後回し」は禁止。動いている時点では銀行残高が不明なだけで、負債は累積している

### 判定基準

次のいずれかに該当する計画は、実施前に Investigation Debt を解消する:

1. **Root cause 未特定**: 修正対象の call site を grep で全列挙できない
2. **Assumption 依存**: 「〜と思われる」「おそらく〜」で根拠が書かれている
3. **影響範囲未計測**: 修正によって触る可能性があるファイル/関数を列挙できない
4. **類似 defect の網羅未確認**: 同じ root cause の別経路がないことを probe/trace で確認していない

### 例外

L1 (Reliability Foundation) で「進行中の silent semantic change が他作業を
汚染し続けている」ケースのみ、調査債務を残したまま interim patch を適用して
良い。ただし `ideal-implementation-primacy.md` の interim patch 条件を全て満たすこと。

## Step 1: Root Cause Clustering

Do not prioritize individual issues by symptom. First group them by root cause.

- Identify issues sharing the same function, module, or design flaw
- Determine priority at the cluster level and address as a batch
- Standalone issues are treated as clusters of size 1

## Step 2: Priority Level Assignment

Classify each cluster into one of 4 levels. Priority order: L1 > L2 > L3 > L4.

### L1: Reliability Foundation

**If left unaddressed, all other development output becomes untrustworthy.**

Criteria (any of):
- **Tier 1 (Silent semantic change)**: Code compiles but behaves differently from TypeScript. Tests may not detect it. (Tier 分類は [`conversion-correctness-priority.md`](conversion-correctness-priority.md) 参照)
- **Test infrastructure compromise**: Tests pass but quality is not guaranteed (e.g., E2E stdout comparison polluted by Tier 1 bugs)

### L2: Design Foundation

**No immediate breakage, but the same class of problem recurs with every future development.**

Criteria (any of):
- **Responsibility violation / DRY violation**: Each feature addition propagates the same anti-pattern
- **Foundational logic deficiency**: Root cause blocking multiple downstream issues (e.g., narrowing infrastructure deficiency → 6+ issues blocked)
- **Lack of structural equivalence**: Name-by-occurrence instead of name-by-structure, correctness not guaranteed by design

### L3: Expanding Technical Debt

**Fix cost increases over time, but not as fundamental as L1/L2.**

Criteria (any of):
- **Blocker for other issues**: Resolving this unblocks downstream issues
- **Expanding fix scope**: Each new code addition increases affected locations
- **Gate issue**: A prerequisite for feature extensions

### L4: Localized Problem

**No impact on other development, fix cost does not change over time.**

Criteria:
- Impact limited to a specific syntax or pattern
- Not a prerequisite for any other issue
- Explicitly skipped or error-reported (unsupported syntax, etc.)

## Step 3: Ordering Within the Same Level

Within the same level, determine order by:

1. **Leverage**: Resolving this simplifies/eliminates N other issues → higher N goes first
2. **Expansion rate**: Fix cost increases proportionally with delay → faster expansion goes first
3. **Fix cost**: If the above are equal, smaller cost goes first (eliminate risk sooner)

## Prohibited

- Prioritizing based solely on surface symptoms (compile error / unsupported syntax)
- Ordering individual issues without root cause clustering
- Demoting L1/L2 issues to L3/L4 based on fix cost
- Deferring L2 issues because "effort is large"
- **Step 0 (Uncertainty Check) をスキップして L1-L4 判定に進むこと**
- **調査債務を放置したまま PRD 起票・commit plan 確定に進むこと**
- **ベンチ数値を optimization target にすること** (`ideal-implementation-primacy.md` 違反)
