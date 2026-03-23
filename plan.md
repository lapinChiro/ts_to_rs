# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`

## 引継ぎ事項

### P8 の作業状態（作業中）

**Phase A〜D の大部分が完了。** 残りは Phase 3〜4（型解決統一）、D5（reg パラメータ統合）、Phase E（最終検証）。

#### 完了済みフェーズ（詳細は git history 参照）

- Phase A: 統一パイプライン本実装
- Phase B: lib.rs API 整理
- Phase C: main.rs 統一
- リファクタリング: DRY 修正、unwrap() → エラーハンドリング
- Phase D 完了分: D0a, D0b, D1, D6, D7, D-TR-1
- Phase 1: TypeResolver expected_types 完全化
- Phase 2: ExprContext 完全削除
- Phase 2.5: Expected Type 伝搬の一本化（全完了条件達成）
  - A: TypeResolver ギャップ埋め（5 パターン）
  - B: テストヘルパー整備（`TctxFixture::from_source`）
  - C: unit test の TypeResolver 経由移行（34 テスト）
  - D: プロダクションコードの手動伝搬削除（19 箇所）
  - 設計レビュー修正: Option wrapping 一本化、`_tctx` 削除、`set_expected_types_in_nested_calls` 文書化
  - 設計レビュー是正: calls.rs を `convert_expr` に戻し、`convert_expr_with_expected` を private に戻す
  - バグ修正: ネストされた三項演算子の二重ラップ（`ast_produces_option` を再帰的に拡張）

#### 次に着手すべき作業 — Phase 3（型解決統一）

Phase 2.5 完了により:
- Transformer は expected type を `tctx.type_resolution.expected_type(span)` からのみ読む
- `convert_expr_with_expected` は private（Option unwrap 再帰専用）
- プロダクションコードからの呼び出しなし

**Phase 3: Heuristic 削除**（`tasks.type-resolution-unification.md` タスク 3-1〜3-7）:
- 3-1: `resolve_expr_type` 呼び出し箇所の置換（約 30 箇所）
- 3-2: `resolve_expr_type` 関連関数の削除
- 3-3: `resolve_method_return_type` の置換検討
- 3-4: heuristic fallback テストの削除・書き換え
- 3-5: `resolve_expr` 副作用分離 → `set_expected_types_in_nested_calls` 廃止
- 3-6: `type_env` パラメータの部分的除去
- 3-7: `ast_produces_option` 削除（TypeResolver Cond/OptChain expr_type 強化）

**Phase 3 の後**: Phase 4（TypeEnv 簡素化）→ D5（reg パラメータ統合）→ Phase E（最終検証）

詳細: `tasks.type-resolution-unification.md`, `tasks.md`

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212 は P8 で**解消済み**。残存エラー: `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 保留中

（なし）
