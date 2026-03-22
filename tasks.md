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

## 現在の状況（Phase C + リファクタリング完了時点）

- `transpile_pipeline()` は全コンポーネント接続の本実装（Phase A）
- `TranspileOutput` に `module_graph` + `synthetic_items` フィールド追加済み
- `FileOutput.unsupported` は `UnsupportedSyntaxError` 型（パイプライン内部型に統一）
- `transpile_single()` 簡易 API 追加済み
- lib.rs の公開 API は `transpile()` と `transpile_collecting()` の 2 関数のみ。両方とも `run_single_file_pipeline()` + `extract_single_output()` 内部関数を使用。旧ラッパー API（`transpile_with_registry` 等 4 関数）と `build_shared_registry` は削除済み
- main.rs は `TranspileInput` + `transpile_pipeline` + `OutputWriter` を直接使用。旧 API への依存なし
- per-file synthetic は items に prepend（旧 API 互換）+ `TranspileOutput.synthetic_items` にも蓄積（OutputWriter 用）
- **expected_type 優先順位**: ExprContext 優先、FileTypeResolution フォールバック（Phase B で修正）
- **TypeResolver の Promise unwrap**: `unwrap_promise_and_unit()` で全 4 箇所適用済み
- DRY: `byte_pos_to_line_col`, `resolve_unsupported`, `run_rustfmt` は lib.rs に集約。main.rs / output_writer.rs は lib.rs の公開関数を使用
- `find_common_root` のユニットテスト 6 件追加済み
- compile_test の `assert_compiles_directory` は `transpile_pipeline` ベースに書き換え済み
- ExprContext / TypeEnv narrowing / resolve_expr_type_heuristic は P6 でフォールバックとして併存中
- tctx + reg 二重パラメータが 105 関数に存在
- `generate_any_enum` は Transformer 内で直接呼ばれ、SyntheticTypeRegistry 経由ではない
- `files.clone()` が main.rs ディレクトリモードに存在（TranspileInput が所有権を取るため不可避。FileOutput にソース文字列を含めるリファクタリングで解消可能）

## タスク一覧

### Phase A: 統一パイプライン本実装（完了）

- [x] **A1**: `TranspileOutput` に `module_graph` と `synthetic_items` フィールドを追加
- [x] **A2**: `transpile_pipeline()` を全コンポーネント接続の本実装に置換（Pass 0→1→2→3→4-5）
- [x] **A3**: `transpile_single()` API を追加
- [x] **A-verify**: 既存テスト全 GREEN

### Phase B: lib.rs API ラッパー化（完了）

- [x] **B1**: `transpile_pipeline()` で per-file synthetic を items に含める修正
- [x] **B2-B4**: lib.rs の公開 API を統一パイプライン経由に置換
  - **修正した重大バグ 1**: expected_type 優先順位（Option<T> unwrap 再帰で無限ループ）
  - **修正した重大バグ 2**: TypeResolver が Promise<T> を unwrap せず expected_type に登録
- [x] **B-verify**: 全テスト GREEN、clippy 0

### Phase C: main.rs 統一（完了）

- [x] **C1-C3**: main.rs を全面書き換え。`TranspileInput` + `transpile_pipeline` + `OutputWriter` を直接使用
- [x] **C-verify**: 全テスト GREEN。Hono ベンチマーク: ディレクトリコンパイル 91.8%→98.7%

### リファクタリング（完了）

- [x] `unwrap()` → `ok_or_else` + エラーメッセージ（lib.rs 3箇所 + main.rs 1箇所）
- [x] DRY: `byte_pos_to_line_col`, `resolve_unsupported`, `run_rustfmt` を lib.rs に集約
- [x] DRY: `transpile_strict` / `transpile_collecting_impl` 内部関数抽出 → `run_single_file_pipeline` + `extract_single_output` に統合
- [x] 死んだ API 削除: `build_shared_registry`, `transpile_with_registry`, `transpile_with_registry_and_path`, `transpile_collecting_with_registry`, `transpile_collecting_with_registry_and_path`
- [x] `find_common_root` テスト 6 件追加
- [x] compile_test の `assert_compiles_directory` を `transpile_pipeline` ベースに書き換え

### Phase D: 統合残課題 + 不要コード削除

**PRD スコープ内で Phase A-C で未着手の項目:**
- AnyTypeAnalyzer の SyntheticTypeRegistry 完全統合（PRD 40-43行）
- I-212（同一 union 型の enum 重複定義）の解消（PRD 44-46行）

- [ ] **D0a**: AnyTypeAnalyzer 統合 — `generate_any_enum` が `(Item, RustType)` を返す方式から、`SyntheticTypeRegistry::register_any_enum` に登録する方式に変更。Transformer 内の `items.push(any_enum_item)` を削除し、per-file synthetic 経由で出力する。**二重定義に注意**（PRD 43行の警告）
- [ ] **D0b**: I-212 解消 — D0a 完了後、同一 union 型の enum が SyntheticTypeRegistry の dedup で一元管理される。compile test `type-narrowing` のスキップを解除して GREEN を確認
- [ ] **D1**: `convert_relative_path_to_crate_path` の使用箇所確認と削除。Transformer 内で import パス変換に使われている（4箇所）。Transformer が ModuleGraph.resolve_import を使うようになれば不要になるが、P8 のスコープ内かどうかを評価する
- [ ] **D2**: ExprContext の削除可否を検証
  - ExprContext は Option<T> unwrap 再帰で必須（Phase B の教訓）。削除するには TypeResolver が Option unwrap 後の inner type も expected_type として設定する必要がある
  - Hono ベンチでフォールバック発火数を計測し、削除可否を判断
- [ ] **D3**: TypeEnv narrowing の削除可否を検証
  - TypeEnv 自体は変数型追跡（insert/get）に使われるため構造体は残す
- [ ] **D4**: resolve_expr_type_heuristic の削除可否を検証
  - Hono ベンチでフォールバック発火数を計測し、0 件なら削除可能
- [ ] **D5**: tctx + reg 二重パラメータの統合（105 関数 + 全テスト）。`/large-scale-refactor` スキルに従う
- [ ] **D6**: `files.clone()` の解消（main.rs ディレクトリモード）。FileOutput にソース文字列を含めるか、TranspileInput が参照を受け取るリファクタリング

### Phase E: 最終検証

- [ ] **E1**: `cargo test` 全 GREEN
- [ ] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **E3**: `cargo fmt --all --check` 通過
- [ ] **E4**: Hono ベンチマーク実行、結果が改善していることを確認
- [ ] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット
