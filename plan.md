# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`（P4 未達成の AnyTypeAnalyzer 統合・I-212 解消を含む）

## 引継ぎ事項

### P8 の作業状態（作業中）

**完了済み:**
- Phase A: 統一パイプライン本実装
  - `TranspileOutput` に `module_graph` + `synthetic_items` フィールド追加
  - `transpile_pipeline()` を全 Pass 接続（Parse → ModuleGraph → TypeCollection → TypeResolution → Transform + Generate）
  - `transpile_single()` 簡易 API 追加
  - 全テスト GREEN（1092 lib + 60 E2E + 69 integration + 3 compile + 2 doc）

**次に着手すべき作業 — Phase B: lib.rs API ラッパー化:**

新パイプラインは合成型をファイル出力に含めず `TranspileOutput.synthetic_items` で別途返す設計。旧 API（`transpile()` 等）は合成型をファイル出力に含めていた。Phase B でこの差分を吸収する必要がある。

具体的な方針: `transpile_pipeline()` の Pass 4-5 で per-file synthetic を items に prepend する。`TranspileOutput.synthetic_items` にも入れる（OutputWriter 用）。詳細は `tasks.md` Phase B セクションに記載。

**Phase B 以降の作業:**
- Phase C: main.rs 統一（単一ファイル + ディレクトリモードを統一パイプライン経由に）
- Phase D: 不要コード削除（ExprContext, TypeEnv narrowing, resolve_expr_type_heuristic, tctx+reg 二重パラメータ）
- Phase E: 最終検証（全テスト + Hono ベンチマーク）

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
