# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`

## 引継ぎ事項

### P8 の作業状態（作業中）

**完了済み（Phase A-D の大部分）:**
- Phase A: 統一パイプライン本実装（`transpile_pipeline` 全 Pass 接続、`TranspileOutput` 拡張、`transpile_single` API、`find_common_root`）
- Phase B: lib.rs API 整理（公開 API を `transpile` / `transpile_collecting` の 2 関数に整理。旧 API 5 関数削除。重大バグ 2 件修正: expected_type 優先順位、Promise unwrap）
- Phase C: main.rs 統一（`TranspileInput` + `transpile_pipeline` + `OutputWriter` 直接使用。Hono ベンチ: ディレクトリコンパイル 91.8%→98.7%）
- リファクタリング: DRY 修正（`byte_pos_to_line_col` / `resolve_unsupported` / `run_rustfmt` を lib.rs に集約）、`unwrap()` → エラーハンドリング、`find_common_root` テスト 6 件追加、compile_test を `transpile_pipeline` ベースに書き換え
- Phase D 完了分:
  - D0a: AnyTypeAnalyzer 統合（`generate_any_enum` → `build_any_enum_variants` + `register_any_enum`）
  - D0b: I-212 解消確認（統一パイプライン dedup で解消済み）
  - D7: `to_pascal_case` 重複解消（`any_narrowing.rs` に集約）
  - SyntheticTypeRegistry の `into_items()` / `all_items()` に名前順ソートを追加（出力の決定性保証）
  - D-TR-1: TypeResolver カバレッジギャップ調査完了（`report/d-tr1-type-resolver-coverage-gaps.md`）
  - D1: import 解決に ModuleGraph lookup + fallback を適用
  - D6: `FileOutput` に `source` フィールドを追加し `files.clone()` を解消
  - Phase 1: TypeResolver expected_types 完全化（`propagate_expected` メソッド追加）
  - Phase 2: `ExprContext` 完全削除（パラメータ除去、テスト29件修正、struct削除、clippy 0警告）
    - expected type 伝搬を `convert_expr_with_expected` 経由に統一（calls.rs, binary.rs, assignments.rs, data_literals.rs, member_access.rs, functions.rs, statements/mod.rs, classes.rs）
    - E2E テスト: 60/60 全 GREEN（旧コード 50/60 から改善）

**次に着手すべき作業 — Phase 2.5: Expected Type 伝搬の一本化:**

`tasks.expected-type-unification.md` に詳細設計を記載。Phase 2 で追加した Transformer の手動伝搬（`convert_expr_with_expected` 経由 19 箇所）と TypeResolver の `propagate_expected` の二重性を解消する。Phase 3 の前に実施。

調査レポート: `report/expected-type-dual-propagation.md`

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212 は P8 で**解消済み**。残存エラー: `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 上記完了後の作業

- **数1000行になっているファイルの見直し**: ファイル行数が多くなりすぎると、作業効率が急激に低下する。DRYであり直交性が保たれていることを担保したうえでリファクタリングし、全てのファイルの行数を1000行以下になるようにする。800行を超えるが分割が合理的ではない理由がある場合は相談する。対象はプロダクションコードとテストコードのすべてとする
- **現在の実装の徹底的なレビュー**: どのような実装が理想的であるか、どのような観点でレビューしなければいけないかを言語化し、詳細な観点リストを作成したうえで、レビューを行う。作成した観点リスト、理想的な実装の説明文書はルールやスキルなど、適切な方法で永続化し、今後の実装が理想的にあり続けるようにする。
- **モジュール参照の解決システム検討**: import/exportマップを作成し、それを解析することで参照を再構築する。このようにした方が、確実にuseすることができるのではないか。つまり、import/exportは逐次変換するのではなく、先に解析し、それに基づいてRust側でモジュール間の関係性をマッピングし構築しなおす、という手法の可能性と合理的な方法かの検討を行う
- **Rules/Skills/CLAUDE.mdの言語**: これらは英語で記載した方が効率的なのではないか。レスポンスだけ日本語にして、読み込まれるファイルはすべて英語の方が良いのではないか

## 保留中

（なし）
