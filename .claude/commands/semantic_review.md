今回の開発範囲における変換の意味論的正確性を、サイレント意味変更の観点で徹底的にレビューしてください。

TypeScript から Rust へのトランスパイルにおいて、ブラックボックス的に見て挙動が変わることは絶対に許容できません。
特に、Rust コンパイラが検出できないサイレントな意味変更（Tier 1）は最も危険です。

**Variant note**: 本 command は **Tier 1 silent semantic change 専用 light review**。matrix-driven PRD の structural review (4-layer) は [/check_job](check_job.md) Layer 2 (Empirical) で同等の verification を含む。conversion code 全体の periodic full audit は [/correctness-audit](../skills/correctness-audit/SKILL.md) skill を使用。

## Action

### Step 1: 変更箇所の特定

git diff で今回の変更内容を全て洗い出し、変換ロジックに影響する変更を特定してください。

### Step 2: 各変更の意味論的影響分析

変更ごとに以下を分析してください：

- **Before**: 変更前の変換結果（エラー/具体的な型/値）
- **After**: 変更後の変換結果
- **TypeScript のセマンティクス**: 元の TypeScript コードの正確な意味
- **生成される Rust コードの挙動**: 変換後の Rust コードが実行時にどう振る舞うか
- **乖離の有無**: TypeScript と Rust で挙動が異なるか

### Step 3: 型フォールバックの安全性検証

型フォールバック（Any, wider union, HashMap 等）が導入されている場合、`.claude/rules/type-fallback-safety.md` の 3 ステップ分析を実施：

1. フォールバック型の全使用サイトを特定
2. 各サイトで「コンパイルエラー（Safe）」か「サイレント挙動変更（UNSAFE）」かを分類
3. UNSAFE パターンがあれば報告し、修正方針を提案

### Step 4: エッジケースの検証

以下の観点で潜在的なサイレント意味変更を探索：

- `serde_json::Value` が具体型の位置に配置された場合、trait impl（Display, PartialEq 等）経由で暗黙的に型制約を満たしてしまわないか
- wider union 型が pattern match で unreachable arm を生み、将来の変更で到達可能になるリスクはないか
- over-approximation された型がジェネリクスの型引数として伝播し、下流でサイレントに異なる振る舞いを引き起こさないか
- 以前は変換エラーだったパターンが Any に変わることで、周囲の正しく変換されたコードに副次的な影響を与えないか

### Step 5: 結果の報告

以下の形式で報告してください：

| 変更 | 影響分類 | 根拠 |
|------|---------|------|
| 変更の概要 | Safe/UNSAFE/要検討 | 分析根拠 |

UNSAFE が見つかった場合は、以下を報告してください（即座の修正は行わない）：
- 具体的な原因（どのコードパスでサイレント意味変更が発生するか）
- 影響範囲（どの TS パターンが影響を受けるか、Hono ベンチマークでの該当件数）
- 修正方針の候補（複数案あれば列挙）

即座にアドホックな修正を行うのではなく、詳細な原因と影響範囲の分析を経て PRD 化して対応します。

要検討が見つかった場合は、追加調査を行い Safe/UNSAFE を確定してください。

発見した問題は TODO に記載してください（`.claude/rules/todo-entry-standards.md` に従い、コード箇所・解決方向性を含めること）。
UNSAFE 問題は Tier 1（サイレント意味変更）として最優先で記載してください。

## Related Rules / Skills / Commands

| Type | Reference | Relation |
|------|-----------|----------|
| Rule | [conversion-correctness-priority.md](../rules/conversion-correctness-priority.md) | Tier 1/2/3 分類の base (本 command が Tier 1 専用 review) |
| Rule | [type-fallback-safety.md](../rules/type-fallback-safety.md) | 3-step safety analysis (Step 3 で適用) |
| Rule | [todo-entry-standards.md](../rules/todo-entry-standards.md) | UNSAFE 発見時の Tier 1 起票 format |
| Rule | [ideal-implementation-primacy.md](../rules/ideal-implementation-primacy.md) | サイレント意味変更禁止の最上位原則 |
| Skill | [correctness-audit](../skills/correctness-audit/SKILL.md) | conversion correctness audit (本 command の上位、periodic full audit) |
| Command | [/check_job](check_job.md) | matrix-driven 4-layer review (Layer 2 Empirical で本 command 同等の verify) |
