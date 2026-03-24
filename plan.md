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

D-2-A〜E + F-0〜F-2 + F-3b 完了。残り: D-2-F（F-3〜F-8）→ G → H → I。

- **D-2-F**: F-2（statements/ ラッパー36個全削除 + ローカル変数抽出除去 + inline Transformer 除去 + 外部呼び出し元サブ Transformer 化 + テスト更新）完了。F-3b（8箇所のサブ Transformer 書き換え）も全完了。残り: F-3（functions/ ラッパー削除）→ F-4（classes/ ラッパー削除）→ F-5〜F-8（mod.rs ラッパー削除 + pipeline 更新 + 検証）
- **D-2-G**: `current_file_dir` パラメータ除去（`self.current_file_dir()` に統一）
- **D-2-H**: テスト更新（テストヘルパーが Transformer を構築してメソッド呼び出し）
- **D-2-I**: クリーンアップ + 最終検証（clippy, fmt, test, doc コメント）

詳細: `tasks.d2-transformer-struct.md`

**D-2 の後**: Phase E（最終検証）

#### D-2 の設計判断（後続への引継ぎ）

**D-2-D**: `convert_fn_decl` のサブ Transformer + `local_synthetic` 分離パターン
- `self.synthetic` は `register_any_enum` のみ、`local_synthetic` は他の全サブコール用。成功時のみ `self.synthetic.merge(local_synthetic)`。**F-2 で維持確認済み**（`functions/mod.rs` F-3b #1 のサブ Transformer 構築で `synthetic: &mut local_synthetic` を渡す形で維持）。
- `convert_ident_to_param` は mod.rs から classes.rs に移動済み（Transformer メソッド化）。mod.rs に残骸なし。

**D-2-E**: TypeEnv 共有の安全性検証
- `transform_decl` から `self.convert_fn_decl()` / `self.transform_class_with_inheritance()` / `self.convert_var_decl_arrow_fns()` を直接呼び出し（ラッパー経由ではない）。
- **検証済み**: これら 3 メソッドは `self.type_env` を一切使用していない。各メソッドが独自のローカル TypeEnv を作成するため、Transformer の TypeEnv を共有してもセマンティクスの変化はゼロ。
- `pipeline/mod.rs` はラッパー free function 経由で呼び出し。Transformer の内部構造に直接依存しない。F-5 でラッパー削除時に、ファクトリメソッド経由の呼び出しに移行する。

**D-2-F-0**: TypeEnv 所有化の設計判断
- `type_env` フィールドを `&'a mut TypeEnv`（参照）から `TypeEnv`（所有）に変更。ファクトリメソッド `for_module()` を導入。
- 過渡的パターン（take+restore / clone）は **F-1 と F-2 で expressions/ と statements/ の全ラッパーが削除され、解消済み**。残りは F-3〜F-5（functions/, classes/, mod.rs）。
- エントリポイントの `drop(t)` はスコープブロックに置き換え済み（`synthetic` の `&mut` 借用を解放するために必要）。

**D-2-F-3b（8 箇所のサブ Transformer 書き換え）**: **全 8 箇所が F-1 と F-2 で対応済み。** 詳細は `tasks.d2-transformer-struct.md` の F-3b テーブルを参照。

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212 は P8 で**解消済み**。残存エラー: `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 保留中

（なし）
