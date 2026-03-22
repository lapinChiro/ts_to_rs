# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`（P4 未達成の AnyTypeAnalyzer 統合・I-212 解消を含む）

## 引継ぎ事項

### P8 の作業状態（作業中）

**完了済み:**
- Phase A: 統一パイプライン本実装（`transpile_pipeline` 全 Pass 接続、`TranspileOutput` 拡張、`transpile_single` API）
- Phase B: lib.rs API ラッパー化（公開 API を `transpile` / `transpile_collecting` の 2 関数に整理。重大バグ 2 件修正: expected_type 優先順位、TypeResolver の Promise unwrap）
- Phase C: main.rs 統一（`TranspileInput` + `transpile_pipeline` + `OutputWriter` を直接使用。Hono ベンチ: ディレクトリコンパイル 91.8%→98.7%）
- リファクタリング: DRY 修正（`byte_pos_to_line_col` / `resolve_unsupported` / `run_rustfmt` を lib.rs に集約）、死んだ API 5 関数削除（`build_shared_registry` / `transpile_with_registry` 系 4 関数）、`unwrap()` → エラーハンドリング、`find_common_root` テスト追加、compile_test を `transpile_pipeline` ベースに書き換え

**次に着手すべき作業 — Phase D: 統合残課題 + 不要コード削除:**

PRD スコープ内で Phase A-C で未着手の 2 項目を先に対応:
- D0a: AnyTypeAnalyzer 統合（`generate_any_enum` を SyntheticTypeRegistry 経由に変更）
- D0b: I-212 解消（D0a 完了後、compile test `type-narrowing` のスキップ解除）

その後、不要コード削除:
- D1: `convert_relative_path_to_crate_path` の評価
- D2-D4: ExprContext / TypeEnv narrowing / resolve_expr_type_heuristic のフォールバック削除可否検証
- D5: tctx + reg 二重パラメータ統合（105 関数）
- D6: `files.clone()` 解消（FileOutput にソース文字列を含める or TranspileInput が参照を受け取る）

詳細は `tasks.md` の Phase D セクション参照。

**Phase D 以降:** Phase E（最終検証: 全テスト + Hono ベンチマーク + doc コメント）

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212（enum 重複定義）は P8 統一パイプラインで**解消済み**。残存エラーは `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 上記完了後の作業

- **現在の実装の徹底的なレビュー**: どのような実装が理想的であるか、どのような観点でレビューしなければいけないかを言語化し、詳細な観点リストを作成したうえで、レビューを行う。作成した観点リスト、理想的な実装の説明文書はルールやスキルなど、適切な方法で永続化し、今後の実装が理想的にあり続けるようにする。
- **モジュール参照の解決システム検討**: import/exportマップを作成し、それを解析することで参照を再構築する。このようにした方が、確実にuseすることができるのではないか。つまり、import/exportは逐次変換するのではなく、先に解析し、それに基づいてRust側でモジュール間の関係性をマッピングし構築しなおす、という手法の可能性と合理的な方法かの検討を行う
- **数1000行になっているファイルの見直し**: ファイル行数が多くなりすぎると、作業効率が急激に低下する。DRYであり直交性が保たれていることを担保したうえでリファクタリングし、全てのファイルの行数を1000行以下になるようにする。800行を超えるが分割が合理的ではない理由がある場合は相談する。対象はプロダクションコードとテストコードのすべてとする

## 保留中

（なし）
