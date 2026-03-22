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

## 現在の状況（Phase B 完了時点）

- `transpile_pipeline()` は全コンポーネント接続の本実装（Phase A で置換済み）
- `TranspileOutput` に `module_graph` + `synthetic_items` フィールド追加済み
- `FileOutput.unsupported` は `UnsupportedSyntaxError` 型（パイプライン内部型に統一）
- `transpile_single()` 簡易 API 追加済み
- lib.rs の全公開 API が統一パイプライン経由（Phase B で置換済み）
- per-file synthetic は items に prepend（旧 API 互換）+ `TranspileOutput.synthetic_items` にも蓄積（OutputWriter 用）
- **expected_type 優先順位**: ExprContext 優先、FileTypeResolution フォールバック（Phase B で修正。逆にすると Option<T> unwrap 再帰で無限ループ）
- **TypeResolver の Promise unwrap**: `unwrap_promise_and_unit()` ヘルパーで全 4 箇所（fn_decl, class method, arrow, fn_expr）で適用済み
- main.rs はまだ旧パイプライン（`transpile_with_registry` 等を直接呼ぶ）。ただし内部的には統一パイプライン経由
- ExprContext / TypeEnv narrowing / resolve_expr_type_heuristic は P6 でフォールバックとして併存中
- tctx + reg 二重パラメータが 105 関数に存在
- `generate_any_enum` は Transformer 内で直接呼ばれ、SyntheticTypeRegistry 経由ではない

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

- [x] **B1**: `transpile_pipeline()` で per-file synthetic を items に含める修正
- [x] **B2-B4**: lib.rs の全公開 API を統一パイプライン経由に置換
  - `transpile()` → unsupported 検出でエラー（strict mode）
  - `transpile_with_registry()` / `transpile_with_registry_and_path()` → 同上
  - `transpile_collecting()` / `transpile_collecting_with_registry()` / `transpile_collecting_with_registry_and_path()` → unsupported 収集
  - **修正した重大バグ**: C3 で `tctx.type_resolution.expected_type` を優先していたため、Option<T> unwrap 再帰で無限ループ。`ctx.expected` を優先し FileTypeResolution をフォールバックに変更
  - **修正した重大バグ**: TypeResolver が `Promise<T>` を unwrap せずに expected_type に登録していた。`unwrap_promise_and_unit()` ヘルパーを追加し全 4 箇所で適用
- [x] **B-verify**: 全テスト GREEN（lib 1092 + CLI 3 + compile 2 + doc 2 + E2E 60 + integration 69）、clippy 0

### Phase C: main.rs 統一

**注意**: main.rs は現在 `transpile_with_registry` / `transpile_with_registry_and_path` を呼んでおり、これらは Phase B で統一パイプライン経由に変更済み。したがって main.rs は **既に間接的に統一パイプラインを使っている**。Phase C の本質は main.rs のコードを直接 `TranspileInput` / `transpile_pipeline` / `OutputWriter` を使うようにリファクタリングし、旧 API への依存を断ち切ること。

- [ ] **C1**: 単一ファイルモード（`src/main.rs:268`）を `transpile_pipeline` + unsupported チェックに置換。`transpile_with_registry` を介さず直接 `TranspileInput` を構築
- [ ] **C2**: ディレクトリモード（`src/main.rs:295-377`）を `transpile_pipeline` + `OutputWriter::write_to_directory` に置換。`transpile_directory_common` / `transpile_with_registry_and_path` のループを `transpile_pipeline` の1回呼び出しに集約。mod.rs 生成を `OutputWriter.generate_mod_rs` に委譲
- [ ] **C3**: collecting モード（`src/main.rs:69-75`）も `transpile_pipeline` 経由に置換
- [ ] **C-verify**: Hono ベンチマーク実行、結果確認。全テスト GREEN

### Phase D: 不要コード削除

**前提**: Phase C で main.rs が統一パイプライン直接呼び出しに変わった後、旧 API のラッパー（lib.rs の `transpile_with_registry` 等）が main.rs から使われなくなる。

- [ ] **D1**: lib.rs の旧ラッパー API の整理。main.rs が使わなくなった関数は `pub(crate)` に変更するか削除。ただし外部クレートとして使用される可能性を考慮（`transpile()` / `transpile_collecting()` は公開 API として維持）
- [ ] **D2**: `convert_relative_path_to_crate_path` の使用箇所確認と削除。Phase C でディレクトリモードが OutputWriter に移行すれば不要になる
- [ ] **D3**: ExprContext の削除可否を検証
  - **検証方法**: Hono ベンチマーク実行時に、`ctx.expected` がフォールバックで使われるケース（= `tctx.type_resolution.expected_type(span)` が `None` だが `ctx.expected` が `Some`）をログ出力し、0 件なら削除可能
  - **注意**: Phase B で発見した通り、ExprContext は Option<T> unwrap 再帰で必須。ExprContext を削除するには TypeResolver が Option unwrap 後の inner type も expected_type として設定する必要がある
  - 削除可能: ExprContext struct + 全参照箇所を削除
  - 削除不可: フォールバック併存を維持し、理由を記録
- [ ] **D4**: TypeEnv narrowing の削除可否を検証
  - **検証方法**: `narrowed_type()` のフォールバック（`type_env.get()` のみで型が取れるケース）をログ出力し、0 件なら narrowing 関連コードを削除可能
  - TypeEnv 自体は変数型追跡（insert/get）に使われるため構造体は残す
- [ ] **D5**: resolve_expr_type_heuristic の削除可否を検証
  - **検証方法**: `resolve_expr_type` で `tctx.type_resolution.expr_type(span)` が Unknown/未登録で heuristic にフォールバックするケースを Hono ベンチで計測し、0 件なら削除可能
- [ ] **D6**: tctx + reg 二重パラメータの統合（reg を TransformContext に統合、105 関数 + 全テスト）。`/large-scale-refactor` スキルに従う

### Phase E: 最終検証

- [ ] **E1**: `cargo test` 全 GREEN
- [ ] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **E3**: `cargo fmt --all --check` 通過
- [ ] **E4**: Hono ベンチマーク実行、結果が改善していることを確認
- [ ] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット
