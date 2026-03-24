# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`

## 引継ぎ事項

### P8 の作業状態（作業中）

**Phase A〜D + D5 が全完了。** D-2（Transformer struct 導入）が進行中。残りは D-2 の続き → Phase E（最終検証）。

#### 完了済みフェーズ（詳細は git history 参照）

- Phase A〜C: 統一パイプライン本実装、lib.rs API 整理、main.rs 統一
- リファクタリング + Phase D 完了分: D0a, D0b, D1, D6, D7, D-TR-1
- Phase 1〜2.5: TypeResolver expected_types 完全化 → ExprContext 完全削除 → Expected Type 伝搬の一本化
- Phase 3: Heuristic 削除（全 7 サブタスク完了）
- Phase 4: TypeEnv 簡素化（narrowing 除去、update() 削除）
- D5: `reg: &TypeRegistry` パラメータ削除（99 関数、13 ファイル、~350 呼び出し箇所を修正。全関数で `tctx.type_registry` に統一）

#### 次に着手すべき作業 — D-2（Transformer struct 導入）の続き

D-2-A〜D 完了。残り: D-2-E → F → G → H → I。

- **D-2-E**: mod.rs の 7 関数メソッド化 + entry point 3 関数を Transformer 構築 + メソッド呼び出しに変更 + `pipeline/mod.rs` の呼び出し更新
- **D-2-F**: 全モジュールのラッパー free function 削除（expressions/statements/functions/classes/mod.rs）
- **D-2-G**: `current_file_dir` パラメータ除去（`self.current_file_dir()` に統一）
- **D-2-H**: テスト更新（テストヘルパーが Transformer を構築してメソッド呼び出し）
- **D-2-I**: クリーンアップ + 最終検証（clippy, fmt, test, doc コメント）

詳細: `tasks.d2-transformer-struct.md`

**D-2 の後**: Phase E（最終検証）

#### D-2-D の設計判断（後続への引継ぎ）

- `convert_fn_decl` はサブ Transformer + `local_synthetic` で per-function synthetic 分離を維持。元コードのエラー時分離セマンティクスを保存。`self.synthetic` は `register_any_enum` のみ、`local_synthetic` は他の全サブコール用。成功時のみ `self.synthetic.merge(local_synthetic)`。D-2-F でラッパー削除時、この分離パターンはそのまま維持すること。
- `convert_ident_to_param` は mod.rs から classes.rs に移動済み（Transformer メソッド化）。mod.rs に残骸なし。

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212 は P8 で**解消済み**。残存エラー: `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 保留中

（なし）
