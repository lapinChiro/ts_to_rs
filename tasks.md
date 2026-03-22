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
- ExprContext: プロダクションコードからは除去済み。ExprContext struct 自体と 29 件のテストが未修正（Phase 2 作業中）
- heuristic / TypeEnv narrowing: runtime fallback として残存（Phase 3-4 で削除予定）
- tctx + reg 二重パラメータ: 112 関数に残存（D5 で削除予定）

## タスク一覧

### Phase A-C + リファクタリング + D0a/D0b/D7（全完了）

省略（git history 参照）

### Phase D: 残作業

**実施順序と依存関係:**

```
D1 (import 解決)          ─┐
D6 (files.clone 解消)     ─┼─→ 型解決統一と独立。先に実施
                           │
D-TR〜D4 (型解決の統一)   ─┤
  Phase 1: TypeResolver    │
  Phase 2: ExprContext 削除│
  Phase 3: Heuristic 削除  │
  Phase 4: TypeEnv 簡素化  │
                           │
D5 (reg パラメータ削除)   ─┘─→ Phase 2 完了後が効率的（シグネチャ安定後）
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
- [ ] **Phase 2** (2-1〜2-5): ExprContext 削除 — **作業中（詳細手順は `tasks.type-resolution-unification.md` Phase 2 セクション参照）**
  - [x] 2-1: `convert_expr` シグネチャ変更 + `convert_expr_with_expected` 追加
  - [x] 2-2/2-3: プロダクションコードから ExprContext 呼び出しを一括除去
  - [ ] Step A: アドホック修正 3 箇所の精査（assignments.rs, type_resolution.rs, member_access.rs）
  - [ ] Step B: 失敗テスト 29 件の修正（`convert_expr_with_expected` への移行）
  - [ ] Step C: ExprContext struct 削除 + unused variable 警告修正
- [ ] **Phase 3** (3-1〜3-5): Heuristic 削除 — `resolve_expr_type` の約 30 呼び出しを `FileTypeResolution.expr_type` に置換
- [ ] **Phase 4** (4-1〜4-3): TypeEnv 簡素化 — narrowing 用 push_scope/pop_scope 削除、update() 削除

#### D5: tctx + reg 二重パラメータ統合

Phase 2（ExprContext 削除）で `ctx` パラメータが消えた後、シグネチャが安定した状態で実施する。Phase 2 より前に実施すると、シグネチャ変更が二度手間になる。

- [ ] **D5**: 112 関数の `reg: &TypeRegistry` を削除し `tctx.type_registry` に統一
  - 分析結果: 14 ファイル、112 関数。全箇所で `reg == tctx.type_registry`
  - `/large-scale-refactor` スキルに従う
  - **依存**: Phase 2 完了後

### Phase E: 最終検証

- [ ] **E1**: `cargo test` 全 GREEN
- [ ] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **E3**: `cargo fmt --all --check` 通過
- [ ] **E4**: Hono ベンチマーク実行、結果が改善していることを確認
- [ ] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット
