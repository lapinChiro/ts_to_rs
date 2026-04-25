今回の開発作業と実装を、第三者の視点で徹底的にレビューしてください。
妥協した実装はありませんか？理由に関わらず、実装および設計、もしくはそれらに付随する確認作業で妥協は絶対に許容しません。
最も理想的でクリーンな実装にすることだけを考えてください。
開発工数や変更規模は判断基準になりません。最も理想的でクリーンな実装だけが正解です。

また、必要十分で高品質な自動テストが実装されていることも確認してください。

**Variant note**: 本 command は **matrix-driven PRD の structural form** (4-layer review)。Tier 1 silent semantic change のみを対象とした軽量 review は [/semantic_review](semantic_review.md) を、session 内未対応問題の light な振り返りは [/check_problem](check_problem.md) を、conversion code 全体の periodic full audit は [/correctness-audit](../skills/correctness-audit/SKILL.md) skill を使用。

## Action

1. 対象 PRD が matrix-driven か判定 (本 command 内 Stage Dispatch 参照)
2. matrix-driven なら Spec / Implementation stage を判別 (PRD の状態 + backlog/<id>.md の Task 進捗)
3. Spec stage なら 10-rule checklist verification、Implementation stage なら 4-layer framework 全実施
4. Non-matrix-driven の場合は Layer 1 + Layer 4 を必須実施 (Layer 2-3 optional)
5. 発見 defect を 5-category (Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight) に trace ベースで分類
6. Output Format に従い structural markdown で report

## Stage Dispatch (matrix-driven PRD)

対象 PRD が matrix-driven (`.claude/rules/spec-first-prd.md` 適用対象) の場合、
PRD の現在の stage に応じて review framework を切り替えてください。

### Spec Stage (Implementation 未着手)

`.claude/rules/spec-stage-adversarial-checklist.md` の **10-rule checklist を全項目 verification** してください:

1. Matrix completeness
2. Oracle grounding
3. NA justification
4. Grammar consistency
5. E2E readiness
6. Matrix/Design integrity (token-level 一致 + side-by-side diff)
7. Control-flow exit sub-case completeness (4 sub-case 必須 enumerate)
8. Cross-cutting invariant enumeration (4 必須項目 a-d)
9. Dispatch-arm sub-case alignment (Spec→Impl + Impl→Spec 双方向)
10. Cross-axis matrix completeness (3 step procedure + 8 default axis)

1 つでも未達の項目があれば明確に指摘し、Implementation stage への移行を block してください。
**実装コードは review 対象外** (存在しないため)。

### Implementation Stage (Spec approved 後)

`.claude/rules/check-job-review-layers.md` の **4-layer framework を初回 invocation で全実施** してください:

- **Layer 1 (Mechanical)**: 静的解析、TODO/FIXME/unwrap 残存、test name 形式、clippy/fmt 違反等を verify
- **Layer 2 (Empirical)**: probe / fixture validation で silent semantic change を捕捉、Dual verdict (TS/Rust) check
- **Layer 3 (Structural cross-axis)**: 解決軸と直交する dimension からの cross-check、Spec gap detection
- **Layer 4 (Adversarial trade-off)**: pre/post matrix で trade-off を批判的評価、patch vs structural fix 分類

`/check_job deep` / `/check_job deep deep` modifier は **廃止**。4 layer は初回 default で全実施されるため、深度制御は不要。

発見された defect は `.claude/rules/post-implementation-defect-classification.md` の **5 category** (Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight) に **trace に基づき** 分類してください (主観判断は禁止)。**Spec gap = framework 失敗 signal** であり、framework 自体の改善検討対象です。

## Non-Matrix-Driven PRD

非 matrix-driven PRD (infra, refactor, bug fix) の場合、**Layer 1 (Mechanical) + Layer 4 (Adversarial trade-off)** を必須実施。Layer 2-3 (Empirical / Structural cross-axis) は対象 PRD に matrix が存在しないため optional。

## Output Format

`.claude/rules/check-job-review-layers.md` の "Output Format (全 layer 統合)" section に従い、Layer 1-4 の findings + Defect Classification Summary + Action Items を構造化された markdown で report してください。

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [check-job-review-layers.md](../rules/check-job-review-layers.md) | 4-layer framework spec (本 command の core) |
| Rule | [spec-stage-adversarial-checklist.md](../rules/spec-stage-adversarial-checklist.md) | Spec stage 10-rule checklist (Spec stage dispatch 時) |
| Rule | [post-implementation-defect-classification.md](../rules/post-implementation-defect-classification.md) | 5-category defect 分類 (Implementation stage dispatch 時) |
| Rule | [spec-first-prd.md](../rules/spec-first-prd.md) | matrix-driven PRD lifecycle (本 command の trigger 判定) |
| Rule | [conversion-correctness-priority.md](../rules/conversion-correctness-priority.md) | Layer 2 で発見の Tier 1 silent semantic change の base 分類 |
| Rule | [ideal-implementation-primacy.md](../rules/ideal-implementation-primacy.md) | Layer 4 patch / structural fix 区分の base |
| Skill | [correctness-audit](../skills/correctness-audit/SKILL.md) | full conversion audit (本 command より broad、periodic) |
| Command | [/semantic_review](semantic_review.md) | Tier 1 silent semantic change 専用 (本 command Layer 2 と部分重複) |
| Command | [/check_problem](check_problem.md) | light review variant (本 command の subset) |
