# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`

## 引継ぎ事項

### P8 の作業状態（作業中）

**Phase A〜D + D5 + D-2 + D-2-2 が全完了。** D-2-2-2（追加メソッド化）が進行中。残り: D-2-2-2 → Phase E（最終検証）。

#### 完了済みフェーズ（詳細は git history 参照）

- Phase A〜C: 統一パイプライン本実装、lib.rs API 整理、main.rs 統一
- リファクタリング + Phase D 完了分: D0a, D0b, D1, D6, D7, D-TR-1
- Phase 1〜2.5: TypeResolver expected_types 完全化 → ExprContext 完全削除 → Expected Type 伝搬の一本化
- Phase 3: Heuristic 削除（全 7 サブタスク完了）
- Phase 4: TypeEnv 簡素化（narrowing 除去、update() 削除）
- D5: `reg: &TypeRegistry` パラメータ削除（99 関数、13 ファイル、~350 呼び出し箇所を修正。全関数で `tctx.type_registry` に統一）
- D-2: Transformer struct 導入（106 関数メソッド化、ラッパー全削除、current_file_dir パラメータ除去、メソッドリネーム完了）。全完了条件達成
- D-2-2: 監査指摘 5 課題全修正（4 free function メソッド化、NarrowingGuard リファクタリング、フィールド private 化、`let reg` 全除去、entry point 簡素化）

#### 次に着手すべき作業 — D-2-2-2（追加メソッド化）

D-2-2 完了後のレビューで検出。`get_expr_type` と `resolve_field_type` が free function のまま残存し、25+ 箇所で `self.tctx` を手動で渡している。Transformer メソッド化で引数を省略し、設計一貫性を向上させる。

**D-2-2-2 の後**: Phase E（最終検証）

#### D-2 の設計判断（後続への引継ぎ）

**サブ Transformer パターン**: `convert_fn_decl` は `local_synthetic` 分離パターンを使用。成功時のみ `self.synthetic.merge(local_synthetic)`。サブ Transformer は独自のローカル TypeEnv を move で所有する。

**TypeEnv 所有化**: `type_env` フィールドは `TypeEnv`（所有）。ファクトリメソッド `for_module()` で内部作成。過渡的パターン（take+restore / clone）は全ラッパー削除で完全解消済み。

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212 は P8 で**解消済み**。残存エラー: `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 保留中

（なし）
