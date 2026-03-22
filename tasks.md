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

**残存する実装不足（fallback が残っている箇所）:**
- ExprContext: expected_type の伝搬に使用。TypeResolver の expected_types カバレッジが不十分なため ExprContext がフォールバックとして必要
- TypeEnv narrowing: スコープベースの narrowing 管理。TypeResolver の narrowing_events カバレッジが不十分
- resolve_expr_type_heuristic: 式の型解決。TypeResolver の expr_types カバレッジが不十分
- convert_relative_path_to_crate_path: import パス解決。ModuleGraph.resolve_import が未使用
- tctx + reg 二重パラメータ: 105 関数に残存
- files.clone(): main.rs ディレクトリモードで不可避

## タスク一覧

### Phase A-C + リファクタリング + D0a/D0b/D7（全完了）

省略（git history 参照）

### Phase D: 残作業

**実行順序: TypeResolver 改善 → fallback 削除 → パラメータ統合 → 構造改善**

#### D-TR: TypeResolver カバレッジ改善（D2-D4 の前提）

TypeResolver のカバレッジが不十分なために ExprContext / TypeEnv narrowing / heuristic を fallback として残している。fallback は実装不足であり正当な理由ではない。TypeResolver を改善し、fallback を不要にする。

- [ ] **D-TR-1**: TypeResolver カバレッジギャップ調査
  - heuristic 無効化時に失敗したテストケースの全一覧を作成
  - 各失敗の原因を分類（expr_types 未登録 / expected_types 未登録 / narrowing_events 未登録）
  - TypeResolver が対応すべき AST パターンの一覧を作成
- [ ] **D-TR-2**: TypeResolver の expr_types カバレッジ改善
  - D-TR-1 で特定した未対応パターンを TypeResolver に追加
  - テスト追加: 各パターンに対して TypeResolver が Known を返すことを検証
- [ ] **D-TR-3**: TypeResolver の expected_types カバレッジ改善
  - Option<T> の inner type を expected_type として設定するロジックを追加
  - return 文以外の expected_type 伝搬（変数宣言、関数引数等）を改善
- [ ] **D-TR-4**: TypeResolver の narrowing_events カバレッジ改善
  - TypeEnv の push_scope/pop_scope/insert に対応する narrowing_events を生成
  - compound narrowing（nested if/switch）のカバレッジを確認
- [ ] **D-TR-verify**: heuristic 無効化で全テスト GREEN を確認

#### D2-D4: fallback 削除（D-TR 完了後）

- [ ] **D2**: ExprContext 削除 — D-TR-3 完了後、ExprContext struct と全参照箇所を削除。`convert_expr` の expected は `tctx.type_resolution.expected_type(span)` のみから取得
- [ ] **D3**: TypeEnv narrowing 削除 — D-TR-4 完了後、TypeEnv の push_scope/pop_scope/narrowing 関連コードを削除。TypeEnv は変数型追跡（insert/get）の用途のみ残す
- [ ] **D4**: resolve_expr_type_heuristic 削除 — D-TR-2 完了後、heuristic 関数を削除。`resolve_expr_type` は `tctx.type_resolution.expr_type(span)` のみから取得

#### D1: import 解決の ModuleGraph 統合

- [ ] **D1**: `convert_relative_path_to_crate_path` に ModuleGraph lookup + fallback パターンを適用
  - TransformContext の module_graph を使い `resolve_import()` を先に試す
  - 解決できない場合（NullModuleResolver 等）のみ `convert_relative_path_to_crate_path` にフォールバック

#### D5: tctx + reg 二重パラメータ統合

- [ ] **D5**: 105 関数の `reg: &TypeRegistry` を削除し `tctx.type_registry` に統一
  - 分析結果: 14 ファイル、105 関数。全箇所で `reg == tctx.type_registry`
  - `/large-scale-refactor` スキルに従う

#### D6: files.clone() 解消

- [ ] **D6**: `FileOutput` に `source: String` フィールドを追加し `files.clone()` を解消

### Phase E: 最終検証

- [ ] **E1**: `cargo test` 全 GREEN
- [ ] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **E3**: `cargo fmt --all --check` 通過
- [ ] **E4**: Hono ベンチマーク実行、結果が改善していることを確認
- [ ] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット
