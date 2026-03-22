# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P6: Transformer の移行** — `backlog/p6-transformer-migration.md`（作業中）
2. **P7: Generator の純粋化 + OutputWriter** — `backlog/p7-generator-output.md`（P6 の後）
3. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`（P6-P7 の後。P4 未達成の AnyTypeAnalyzer 統合・I-212 解消を含む）

## 引継ぎ事項

### P6 の作業状態（作業中）

**完了済み:**
- Step 1: テスト設計（RED）— 11 テスト作成、4 つが RED で意図通り
- Step 2: TransformContext 構造体定義（`src/transformer/context.rs`）
- Step 6: Generator セマンティック判断の移動（2/3 完了）
  - regex import スキャン: Generator → Transformer（`inject_regex_import_if_needed`）✓
  - match `.as_str()` 注入: Generator → Transformer（switch 変換時に discriminant を wrap）✓
  - enum 分類: **未着手**（PRD 完了条件に含まれるが、Generator の `has_data_variants` / `is_numeric_enum` は IR の情報に基づくレンダリング戦略の選択であり、TS セマンティクスの判断かどうか要検討。Phase C または Phase D で対応）
- Phase A: プロダクションコードのシグネチャ変更
  - 14 ファイル、105 関数に `tctx: &TransformContext<'_>` パラメータ追加
  - `cargo check --lib` 0 エラー、`cargo clippy --lib` 0 エラー
  - `clippy.toml`: `too-many-arguments-threshold = 10`
- Phase B: テストコード修正（462 箇所）+ TctxFixture リファクタリング
  - 7 テストファイルの全関数呼び出しに `tctx` パラメータ追加
  - 各テストモジュールに `TctxFixture` 構造体を定義し、4 行のボイラープレートを 2 行に集約（DRY）
  - `cargo test --lib` 1078 GREEN、clippy 0、fmt 通過
  - E2E（60件）、integration（69件）、compile（3件）、doc（2件）全 GREEN

- Phase C: FileTypeResolution lookup の実装（完了）
  - C1+C2: `resolve_expr_type` 先頭に FileTypeResolution.expr_types lookup 追加（呼び出し側変更不要）
  - C3: `convert_expr` の expected 型決定で FileTypeResolution.expected_type(span) を優先
  - C4: `resolve_expr_type_heuristic` の Ident ケースで narrowed_type() を TypeEnv.get() より優先
  - C5: Generator の enum 分類は IR レンダリング戦略選択であり移動不要と判定
- Phase D: 最終検証（完了）
  - clippy 0、fmt 通過、Hono ベンチマーク変化なし（53.2%）

**次に着手すべき作業:**

P6 PRD の完了条件を照合し、全条件が達成されているか確認する。未達成の条件があれば対応する。

**詳細な実装計画**: `tasks.md` に記載。`/large-scale-refactor` スキルに従い、フェーズごとにコミットする。

### 振り返りレポート

`report/p6-phase-a-retrospective.md` — Phase A での 3 回のリセットの原因分析と教訓。以下のルール・スキルを新規作成:
- `.claude/rules/incremental-commit.md` — フェーズ完了時のコミット義務
- `.claude/rules/bulk-edit-safety.md` — 一括置換の安全手順
- `.claude/skills/large-scale-refactor/` — 大規模リファクタリングの計画・実行手順

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212（同一 union 型の enum 重複定義）

## 上記完了後の作業

- **現在の実装の徹底的なレビュー**: どのような実装が理想的であるか、どのような観点でレビューしなければいけないかを言語化し、詳細な観点リストを作成したうえで、レビューを行う。作成した観点リスト、理想的な実装の説明文書はルールやスキルなど、適切な方法で永続化し、今後の実装が理想的にあり続けるようにする。
- **モジュール参照の解決システム検討**: import/exportマップを作成し、それを解析することで参照を再構築する。このようにした方が、確実にuseすることができるのではないか。つまり、import/exportは逐次変換するのではなく、先に解析し、それに基づいてRust側でモジュール間の関係性をマッピングし構築しなおす、という手法の可能性と合理的な方法かの検討を行う
- **数1000行になっているファイルの見直し**: ファイル行数が多くなりすぎると、作業効率が急激に低下する。DRYであり直交性が保たれていることを担保したうえでリファクタリングし、全てのファイルの行数を1000行以下になるようにする。800行を超えるが分割が合理的ではない理由がある場合は相談する。対象はプロダクションコードとテストコードのすべてとする

## 保留中

（なし）
