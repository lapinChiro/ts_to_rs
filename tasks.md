# P8 統合 + 既存 API 置き換え — tasks.md

## 完了条件（PRD より）

1. 統一パイプライン `transpile_pipeline(TranspileInput) -> TranspileOutput` が全コンポーネントを接続して動作する
2. 既存 lib.rs 公開 API が統一パイプラインのラッパーになっている
3. 既存 main.rs のディレクトリ/単一ファイルモードが統一パイプライン呼び出しになっている
4. `transpile_single()` の簡易 API が提供されている
5. 不要コードが削除されている
6. 既存の全 E2E テスト・スナップショットテストが GREEN
7. cargo test 全 GREEN
8. clippy 0 警告
9. Hono ベンチマーク結果が改善している
10. pub な型・関数に doc コメントがある

## 現在の状況

**パイプライン:** 全 Pass 接続済み。`transpile_pipeline` 本実装。
**lib.rs:** `transpile()` / `transpile_collecting()` の 2 関数のみ。旧 API 削除済み。
**main.rs:** `TranspileInput` + `transpile_pipeline` + `OutputWriter` 直接使用。旧 API 依存なし。
**Transformer:** AnyTypeAnalyzer 統合済み。to_pascal_case 集約済み。SyntheticTypeRegistry ソート修正済み。

**残存する実装不足:**
- tctx + type_env + synthetic パラメータ貫通: 105 関数（D-2 で Transformer struct に統合予定）

## タスク一覧

### Phase A-C + リファクタリング + D0a/D0b/D7（全完了）

省略（git history 参照）

### Phase D: 残作業

**実施順序と依存関係:**

```
D1 (import 解決)            ─┐
D6 (files.clone 解消)       ─┼─→ 完了
                             │
D-TR〜D4 (型解決の統一)     ─┤
  Phase 1〜3: 完了           │
  Phase 4: TypeEnv 簡素化   │  完了
                             │
D5 (reg パラメータ削除)     ─┘─→ 完了
                             │
D-2 (Transformer struct)    ─┘─→ D5 完了後に実施
```

#### D1: import 解決の ModuleGraph 統合 ✅

- [x] **D1**: `transform_import` / `transform_export_named` / `export_all` に ModuleGraph lookup + fallback を適用
  - `resolve_import_path_with_fallback` ヘルパー追加。`ModuleGraph.resolve_import()` を優先し、失敗時は `convert_relative_path_to_crate_path` にフォールバック
  - re-export chain の解決に対応（テスト 3 件追加）

#### D6: files.clone() 解消 ✅

- [x] **D6**: `FileOutput` に `source: String` フィールドを追加し `main.rs` の `files.clone()` を解消

#### D-TR 〜 D4: 型解決の統一

**詳細計画: `tasks.type-resolution-unification.md`（Phase 1〜4）**

根本的な設計問題: TypeResolver（pre-pass）と runtime fallback（ExprContext / heuristic / TypeEnv narrowing）が同一機能を並行実装している。TypeResolver を完全化し、全 fallback を削除する。

- [x] **D-TR-1**: TypeResolver カバレッジギャップ調査（`report/d-tr1-type-resolver-coverage-gaps.md`）
  - heuristic 無効化: 50 テスト失敗（大半はテスト構造の問題。TypeResolver は heuristic のスーパーセット）
  - ExprContext 無効化: 47 テスト失敗（TypeResolver の expected_types が 3 パターンのみで不完全）
  - TypeEnv narrowing 無効化: 4 テスト失敗（TypeEnv 自体のユニットテストのみ。narrowing_events で完全カバー済み）
- [x] **Phase 1** (1-1〜1-12): TypeResolver expected_types 完全化 — `propagate_expected` で expected_type の再帰的伝搬を実装（テスト 11 件追加）
- [x] **Phase 2**: ExprContext 完全削除 — struct 削除、テスト 29 件修正、clippy 0 警告、E2E 60/60 GREEN
- [x] **Phase 2.5**: Expected Type 伝搬の一本化 — TypeResolver と Transformer の二重伝搬を解消（全完了条件達成済み）
  - [x] **Phase 2.5-A**: TypeResolver ギャップ埋め（5 パターン追加）+ `visit_var_decl` 再構成 + `resolve_arrow_expr`/`resolve_fn_expr` expected type 読み取り
  - [x] **Phase 2.5-B**: テストヘルパー整備（`TctxFixture::from_source`）
  - [x] **Phase 2.5-C**: unit test の TypeResolver 経由移行（50+ テスト書き換え）
  - [x] **Phase 2.5-D**: Transformer の手動伝搬削除（19 箇所）+ 設計レビュー修正
- [x] **Phase 3** (3-1〜3-7): Heuristic 削除 — 全完了。`resolve_expr_type` 関連関数削除、`set_expected_types_in_nested_calls` 廃止、`type_resolution.rs` の `type_env` 除去、`ast_produces_option` 削除（TypeResolver Cond/OptChain 強化）
- [x] **Phase 4** (4-1〜4-3): TypeEnv 簡素化 — narrowing 用 push_scope/pop_scope 削除、update() 削除、関連テスト 2 件削除

#### D5: tctx + reg 二重パラメータ統合

Phase 2（ExprContext 削除）で `ctx` パラメータが消えた後、シグネチャが安定した状態で実施する。Phase 2 より前に実施すると、シグネチャ変更が二度手間になる。

- [x] **D5**: 99 関数の `reg: &TypeRegistry` を削除し `tctx.type_registry` に統一（13 ファイル、~350 呼び出し箇所を修正）

#### D-2: Transformer struct 導入

**詳細計画: `tasks.d2-transformer-struct.md`**

`tctx`, `type_env`, `synthetic` の 3 パラメータを `Transformer` struct のフィールドに束ね、105 関数をメソッドに変換する。Phase D-2-A〜I の 9 フェーズで段階的に実施。

- [ ] **D-2**: Transformer struct 導入（105 関数のメソッド化 + ラッパー遷移 + current_file_dir 除去）
  - **依存**: D5 完了後

### Phase E: 最終検証

- [ ] **E1**: `cargo test` 全 GREEN
- [ ] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **E3**: `cargo fmt --all --check` 通過
- [ ] **E4**: Hono ベンチマーク実行、結果が改善していることを確認
- [ ] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット
