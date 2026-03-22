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
  - 4 関数は意図的に対象外: `convert_ident_to_param`（pipeline 呼び出し）、`wrap_trait_for_position`（同）、`transform_module`（公開 API）、`transform_module_collecting`（同）
  - `transform_module` / `transform_module_collecting` は内部でデフォルト tctx 生成
  - `src/lib.rs` のエントリポイントも対応済み
  - `cargo check --lib` 0 エラー、`cargo clippy --lib` 0 エラー
  - `clippy.toml`: `too-many-arguments-threshold = 10`
  - **注意: `cargo test --lib` は 462 コンパイルエラーで全テスト実行不可**（Phase A はプロダクションコードのみ変更し、テストコードは未修正のため）
  - E2E テスト（60件）、integration テスト（69件）、compile テスト（2件）は全 GREEN（これらはプロダクションの公開 API を使うため影響なし）

**次に着手すべき作業 — Phase B: テストコード修正（最優先）:**

`src/` 内の `#[cfg(test)]` モジュールのコンパイルエラー 462 箇所を修正する。全て `tctx` パラメータの追加。Phase B が完了するまで `cargo test --lib` は実行不可。

**Phase B の進め方:**
`/large-scale-refactor` スキルに従い、コードに触る前にまず以下を行う:
1. 各テストファイルの実際の呼び出しパターンを調査（どの関数が呼ばれ、引数がどう渡されているか）
2. ファイルごとの具体的な修正手順を `tasks.md` に記載
3. 手順を見直してから実装着手
Phase A の教訓（`report/p6-phase-a-retrospective.md`）: パターンを把握せずにスクリプト実行→失敗→リセットを繰り返さないこと。

| ファイル | エラー数 | 内容 |
|----------|---------|------|
| `src/transformer/expressions/tests.rs` | 305 | `convert_expr` 等の呼び出しに tctx 追加 |
| `src/transformer/statements/tests.rs` | 73 | `convert_stmt` / `convert_stmt_list` 等に tctx 追加 |
| `src/transformer/functions/tests.rs` | 43 | `convert_fn_decl` 等に tctx 追加 |
| `src/transformer/classes.rs` 内テスト | 28 | `extract_class_info` 等に tctx 追加 |
| `src/transformer/tests.rs` | 6 | `extract_fn_return_type` / `extract_fn_param_types` 呼び出しに tctx 追加 |
| `src/transformer/expressions/type_resolution.rs` テスト | 6 | `resolve_expr_type` 等に tctx 追加 |
| `src/transformer/context.rs` テスト | 1 | `transform_module_with_path` 呼び出し修正 |

**テスト修正パターン:**
各テストモジュールに TctxFixture ヘルパーを定義し、デフォルト tctx を生成:
```rust
use crate::transformer::context::TransformContext;
use crate::pipeline::{ModuleGraph, type_resolution::FileTypeResolution};
let mg = ModuleGraph::empty();
let res = FileTypeResolution::empty();
let tctx = TransformContext::new(&mg, &reg, &res, std::path::Path::new("test.ts"));
```
各呼び出し箇所で `tctx` を `reg` の前に追加。multi-line 呼び出しでは `reg,` の前の行に `tctx,` を挿入。

**Phase B 完了後の作業:**

- Phase C: FileTypeResolution lookup の実装
  - C1: `resolve_expr_type_or_lookup` ヘルパー作成
  - C2: `resolve_expr_type` の 31 箇所を lookup に置換
  - C3: `ExprContext::expected` を `type_resolution.expected_type(span)` に置換
  - C4: TypeEnv narrowing を `type_resolution.narrowed_type()` に置換
- Phase D: 最終検証（clippy, fmt, Hono ベンチマーク）
- E2E テスト追加

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
- **数1000行になっているファイルの見直し**: ファイル行数が多くなりすぎると、作業効率が急激に低下する。DRYであり直交性が保たれていることを担保したうえでリファクタリングし、全てのファイルの行数を1000行以下になるようにする。800行を超えるが分割が合理的ではない理由がある場合は相談する。

## 保留中

（なし）
