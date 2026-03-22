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

## 前提状況

- `transpile_pipeline()` は現在ブリッジ実装（`transpile_collecting_with_registry` に委譲）
- `TranspileOutput` に `module_graph` / `synthetic_items` フィールドが未追加
- `TypeCollector::collect()` / `AnyTypeAnalyzer::analyze()` は独立 struct として存在しない。型収集は `build_registry()` で行う
- `generate_any_enum` は Transformer 内で直接呼ばれ、SyntheticTypeRegistry 経由ではない
- ExprContext / TypeEnv narrowing / resolve_expr_type_heuristic は P6 でフォールバックとして併存中
- tctx + reg 二重パラメータが 105 関数に存在

## タスク一覧

### Phase A: 統一パイプライン本実装

- [x] **A1**: `TranspileOutput` に `module_graph` と `synthetic_items` フィールドを追加（`src/pipeline/types.rs`）
  - `FileOutput.unsupported` の型を `UnsupportedSyntaxError` に変更（パイプライン内部型に統一）
- [x] **A2**: `transpile_pipeline()` を全コンポーネント接続の本実装に置換
  - Pass 0: `parse_files()` → `ParsedFiles`
  - Pass 1: `ModuleGraphBuilder` + `find_common_root()` → `ModuleGraph`
  - Pass 2: 全ファイルの `build_registry()` をマージ → `TypeRegistry`
  - Pass 3: `TypeResolver` → `FileTypeResolution`（per file）。SyntheticTypeRegistry に合成型が蓄積
  - Pass 4-5: `TransformContext` + `transform_module_collecting_with_path()` → `Vec<Item>` + `generate()` → `String`（per file）。per-file の SyntheticTypeRegistry を作成し、完了後にマージ
  - `TranspileOutput` に module_graph と synthetic_items を設定
- [x] **A3**: `transpile_single()` API を追加
- [x] **A-verify**: 既存テスト全 GREEN（1092 lib + 60 E2E + 69 integration + 3 compile + 2 doc）

### Phase B: lib.rs API ラッパー化

**Phase A → B の引継ぎ事項:**

新パイプラインの `transpile_pipeline()` は合成型をファイル出力に含めない設計（`TranspileOutput.synthetic_items` で別途返す）。旧 API（`transpile()` / `transpile_collecting()`）は合成型をファイル出力に含めていた（`synthetic.into_items()` + `items` → `generate()`）。Phase B でラッパー化する際に、この差分を吸収する必要がある:

- 方式 A: ラッパー内で `synthetic_items` を `generate()` に渡して結合（旧互換）
- 方式 B: `transpile_pipeline()` 内で per-file の合成型を items に含める（パイプライン側で対応）

方式 B が理想的。理由: ラッパーで結合すると合成型の配置ロジック（OutputWriter の責務）と重複する。パイプライン内で per-file synthetic をファイルの items に含めれば、単一ファイルモードでは旧互換、ディレクトリモードでは OutputWriter が配置を最適化する。

**具体的な修正**: `transpile_pipeline()` の Pass 4-5 で `file_synthetic.into_items()` を `items` に prepend する。`synthetic.merge(file_synthetic)` も維持し、`TranspileOutput.synthetic_items` にも入れる（OutputWriter 用）。

- [ ] **B1**: `transpile_pipeline()` で per-file synthetic を items に含める修正
- [ ] **B2**: `transpile()` を `transpile_single()` 経由に置換
- [ ] **B3**: `transpile_collecting()` を統一パイプライン経由に置換
- [ ] **B4**: 他の公開 API（`transpile_with_registry` 等）を統一パイプライン経由に置換
- [ ] **B-verify**: 全スナップショットテスト + E2E テストが GREEN

### Phase C: main.rs 統一

- [ ] **C1**: 単一ファイルモードを統一パイプライン経由に置換
- [ ] **C2**: ディレクトリモードを統一パイプライン + OutputWriter 経由に置換
- [ ] **C-verify**: Hono ベンチマーク実行、結果確認

### Phase D: 不要コード削除

- [ ] **D1**: P1 ブリッジ実装の残骸を削除（旧 `transpile_pipeline` のコメント等）
- [ ] **D2**: `convert_relative_path_to_crate_path` の使用箇所確認と削除
- [ ] **D3**: ExprContext の削除可否を検証（TypeResolver expected_types カバレッジ確認）
  - 削除可能: ExprContext struct + 全参照箇所を削除
  - 削除不可: フォールバック併存を維持し、理由を記録
- [ ] **D4**: TypeEnv narrowing の削除可否を検証
- [ ] **D5**: resolve_expr_type_heuristic の削除可否を検証（Hono ベンチでフォールバック発火 0 件を確認）
- [ ] **D6**: tctx + reg 二重パラメータの統合（reg を TransformContext に統合、105 関数 + 全テスト）

### Phase E: 最終検証

- [ ] **E1**: `cargo test` 全 GREEN
- [ ] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **E3**: `cargo fmt --all --check` 通過
- [ ] **E4**: Hono ベンチマーク実行、結果が改善していることを確認
- [ ] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット
