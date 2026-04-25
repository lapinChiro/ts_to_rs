# Design Integrity Check

## When to Apply

When writing a PRD's design section, evaluating refactoring, or making design decisions.

## Constraints

Before finalizing a design, verify from the following perspectives, including not just the change target but **one layer above** (callers, dependencies, sibling modules at the same level).

### Checklist

1. **Higher-level design consistency**: Is this change consistent with the interfaces of parent modules and other modules at the same abstraction level? Does it align with the overall conversion pipeline design (parser → transformer → generator)?
2. **DRY (knowledge duplication)**: Does the same knowledge (conversion rules, type mappings, business logic) exist in multiple places? However, allow duplication if shared code would increase inter-module coupling
3. **Orthogonality**: Does the change target focus on a single responsibility? Does it avoid side effects on unrelated modules?
4. **Coupling**: Are inter-module dependencies not increasing unnecessarily? If they are, is the dependency inherently necessary?

### Broken Window Detection and Response

When existing code issues are found during the check:

- Fixable within PRD scope → Include in tasks
- Outside PRD scope → Record in TODO (do not leave unaddressed)

"That's how the existing code works" does not justify a design decision (broken window ratification).

## Decision Criteria

設計判断の唯一の基準は理想実装。詳細は [`ideal-implementation-primacy.md`](ideal-implementation-primacy.md) 参照 (本ルールが subordinate)。Cost / effort / scope は判断基準として禁止。現実装が ideal と乖離している場合は本 PRD scope で fix するか TODO 起票を選択する。

## Prohibited

- Limiting impact analysis to only the target module
- Choosing a non-ideal design because "the effort is large" or "the impact scope is wide"
- Discovering existing broken windows but neither recording nor fixing them

## Related Rules

| Rule | Relation |
|------|----------|
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 最上位原則 (本ルールが subordinate)。Decision Criteria の base |
| [prd-design-review.md](prd-design-review.md) | PRD 設計セクション作成後の review (cohesion / 責務分離 / DRY)、本ルールと相補 |
| [problem-space-analysis.md](problem-space-analysis.md) | 設計対象の問題空間 enumerate methodology |
| [pipeline-integrity.md](pipeline-integrity.md) | Pipeline 整合性 (parser → transformer → generator) の制約 |
