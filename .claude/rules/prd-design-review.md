# PRD Design Review

## When to Apply

When creating a PRD (backlog/ document) with a design section. The review is performed as a self-check before finalizing the PRD.

## Constraints

PRD の設計セクションを書いた後、以下の 3 観点で第三者目線のレビューを行い、問題があれば修正してから PRD を確定する。

### 1. 凝集度 (Cohesion)

各モジュール・関数・構造体が**単一の明確な責務**を持っているか。

- 設計で追加・変更する各単位（関数、struct、モジュール）の責務を一文で説明できるか
- 「A かつ B を行う」のように複数の責務が混在していないか
- 既存コードに新しい責務を追加する場合、その責務は既存の責務と**同じ抽象レベル**か

### 2. 責務の分離 (Separation of Concerns)

異なる関心事が異なるモジュール/関数に分離されているか。

- **走査と判定の分離**: データ構造を走査するロジックと、各要素に対する判定ロジックは分離されているか（例: 式ツリーの再帰走査 vs mutation の判定条件）
- **収集とアクションの分離**: 情報の収集と、収集結果に基づくアクション（コード生成等）は分離されているか
- **知識の所在**: ある判定に必要な知識（メタデータ、設定）はその判定を行う場所に閉じているか、不必要に伝播していないか

### 3. DRY (Don't Repeat Yourself)

**知識**の重複がないか（コードの見た目の重複ではなく、同じルール・判定・変換が複数箇所に存在しないか）。

- 同じ判定条件（例: ハードコードリスト）が複数ファイルに存在しないか
- 同じ走査パターン（例: 式ツリーの再帰）が複数の実装を持っていないか
- 重複を解消する際、共有することでモジュール間の**結合度**が不必要に上がらないか（DRY と結合度のトレードオフ）

### レビュー手順

1. 設計セクションの各変更について、上記 3 観点のチェックを行う
2. 問題が見つかった場合、PRD の設計を修正する
3. 修正後、再度チェックを行い問題がないことを確認する

## Prohibited

- 凝集度・責務分離・DRY のレビューを行わずに PRD を確定する
- 「実装時に整理する」として設計段階の問題を先送りする（設計の問題は設計段階で解決する）
- DRY 解消のために凝集度を犠牲にする（トレードオフがある場合は明示的に記述する）

## Related Rules

| Rule | Relation |
|------|----------|
| [design-integrity.md](design-integrity.md) | 設計判断の higher-level consistency check (本ルールと相補) |
| [problem-space-analysis.md](problem-space-analysis.md) | 問題空間 enumerate を経て確定した設計を本ルールで review |
| [spec-first-prd.md](spec-first-prd.md) | matrix-driven PRD の Spec stage で本ルールを適用 |
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 設計判断の唯一基準 (本ルールが subordinate) |
