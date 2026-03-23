# P8: 統合 + 既存 API の置き換え

## 背景・動機

P1〜P7 で新パイプラインの全コンポーネントが実装された。しかし、既存の `lib.rs` 公開 API（`transpile()`, `transpile_collecting()` 等）と `main.rs` のディレクトリ/単一ファイルモードは、まだ旧ロジックを呼んでいる。本 PRD で統一パイプライン `transpile(TranspileInput) -> TranspileOutput` を組み立て、既存 API をそのラッパーに置き換え、不要コードを削除する。

これにより:
1. 単一ファイルモードとディレクトリモードが同一パイプラインを通り、挙動の不一致が解消される
2. 旧コードパス（`convert_relative_path_to_crate_path`, 分散した合成型生成、`ExprContext` 等）が削除され、保守コストが減る
3. 全 E2E テストが新パイプライン上で GREEN であることで、移行の正しさが保証される

## ゴール

1. 統一パイプライン `transpile(TranspileInput) -> TranspileOutput` を P1〜P7 の全コンポーネントを接続して組み立てる
2. 既存 `lib.rs` 公開 API を統一パイプラインのラッパーに置換する
3. 既存 `main.rs` のディレクトリ/単一ファイルモードを統一パイプライン呼び出しに置換する
4. 不要コードを削除する
5. 全 E2E テストが変更なしで GREEN。ベンチマーク結果が改善。

## スコープ

### スコープ内

- 統一パイプラインの組み立て（`src/pipeline/mod.rs`）:
  - Pass 0: `parse_files()` → `ParsedFiles`
  - Pass 1: `ModuleGraphBuilder` + `ModuleResolver` → `ModuleGraph`
  - Pass 2: `TypeCollector` + `TypeConverter` → `TypeRegistry` + `SyntheticTypeRegistry`
  - Pass 3: `AnyTypeAnalyzer` → `SyntheticTypeRegistry` に追記
  - Pass 4: `TypeResolver` → `FileTypeResolution`（per file）
  - Pass 5: `Transformer` + `TransformContext` → `Vec<Item>`（per file）
  - Pass 6: `Generator` → `String`（per file）
  - Pass 7: `OutputWriter` → 出力
- 既存 `lib.rs` 公開 API の置換 → **Phase B で実施済み**:
  - 公開 API を `transpile()` と `transpile_collecting()` の 2 関数に整理。両方とも `run_single_file_pipeline()` + `extract_single_output()` 内部関数を使用
  - 旧ラッパー API（`transpile_with_registry` / `transpile_with_registry_and_path` / `transpile_collecting_with_registry` / `transpile_collecting_with_registry_and_path`）と `build_shared_registry` は削除済み
- 既存 `main.rs` の置換 → **Phase C で実施済み**:
  - ディレクトリモード: `TranspileInput` + `NodeModuleResolver` → `transpile_pipeline` → `OutputWriter`
  - 単一ファイルモード: `TranspileInput` + `NullModuleResolver` → `transpile_pipeline` → ファイル書き出し
- AnyTypeAnalyzer の SyntheticTypeRegistry 完全統合（P4 から繰り越し）:
  - `generate_any_enum`（`src/transformer/any_narrowing.rs:84`）が現在 `(Item, RustType)` を直接返し、呼び出し元（`src/transformer/functions/mod.rs:106,136,1173`、`src/registry.rs:1067,1100`）が Item を `items` に push する方式
  - 統一パイプラインでは、`generate_any_enum` が SyntheticTypeRegistry に登録し、Item は返さない方式に変更。Transformer は SyntheticTypeRegistry から `all_items()` で取得する
  - **二重定義に注意**: `generate_any_enum` が Item を返すのと SyntheticTypeRegistry に登録するのを同時に行うと、Item が2回出力される。必ず片方のみにする
- I-212（同一 union 型の enum 重複定義）の完全解消（P4 から繰り越し）:
  - Transformer 内の合成型直接 push が全て SyntheticTypeRegistry 経由になった時点で達成される
  - コンパイルテスト `type-narrowing` のスキップ解除もここで行う
- 不要コードの削除（完了済み / 残作業の状態）:
  - `convert_relative_path_to_crate_path` → **D1 で削除済み**（ModuleGraph lookup + fallback に置換）
  - `ExprContext` → **Phase 2 で削除済み**（TypeResolver の expected_types に一本化）
  - `resolve_expr_type` / `resolve_expr_type_heuristic` → **Phase 3-2 で削除済み**（TypeResolver の expr_types に一本化）
  - `set_expected_types_in_nested_calls` → **Phase 3-5 で削除済み**（resolve_call_expr の再帰で自然に解消）
  - `TypeEnv` の narrowing スコープ管理 → **Phase 4 で削除予定**。narrowing_events で完全カバー済み（D-TR-1 で検証）だが push_scope/pop_scope の削除は未実施
  - `tctx` + `reg` の二重パラメータ → **D5: 未着手**。105 関数 + テストコード
  - 分散した合成型生成 → **D0a で削除済み**
  - P1 のブリッジ実装 → **Phase A で削除済み**
- `transpile_single(source: &str) -> Result<String>` の簡易 API → **Phase A で実装済み**（`src/pipeline/mod.rs`）

### スコープ外

- 個別コンポーネントの機能追加（P1〜P7 で完了済み）
- 新しい変換ルールの追加
- パフォーマンス最適化（将来の課題として TODO に記録）

## 設計

`report/pipeline-component-design.md` セクション 3（公開 API）、セクション 7（パイプライン実行フロー）に準拠。

### 統一パイプライン

```rust
// src/pipeline/mod.rs

/// 統一パイプライン。全モードで同一のコードパスを通る。
pub fn transpile(input: TranspileInput) -> Result<TranspileOutput> {
    // Pass 0: Parse
    let parsed = parse_files(input.files)?;

    // Pass 1: Module Graph
    let module_graph = ModuleGraphBuilder::new(&parsed, &*input.module_resolver).build();

    // Pass 2: Type Collection
    let mut synthetic = SyntheticTypeRegistry::new();
    let builtin = input.builtin_types.unwrap_or_default();
    let registry = TypeCollector::collect(&parsed, &module_graph, &mut synthetic, &builtin);

    // Pass 3: Any-Type Analysis
    AnyTypeAnalyzer::analyze(&parsed, &registry, &mut synthetic);

    // Pass 4: Type Resolution (all files first)
    // TypeResolver が &mut SyntheticTypeRegistry を必要とするため、
    // 全ファイルの型解決を先に完了させてから Transformation に進む。
    // これにより Pass 5-6 では SyntheticTypeRegistry が不変になり、
    // borrow checker の問題を回避できる。
    let mut type_resolutions = Vec::new();
    for file in &parsed.files {
        let type_resolution = {
            let mut resolver = TypeResolver::new(&registry, &mut synthetic, &module_graph);
            resolver.resolve_file(file)
        };
        type_resolutions.push(type_resolution);
    }
    // SyntheticTypeRegistry is now immutable

    // Pass 4-5: Transformation + Code Generation (per file)
    // Phase A で実装済み。実際のコードは src/pipeline/mod.rs:82-110 を参照。
    // - TransformContext (module_graph, type_registry, type_resolution, file_path)
    // - per-file SyntheticTypeRegistry → items に prepend + 共有 synthetic にマージ
    // - transform_module_collecting_with_path → generate
    // Phase D で tctx + reg 二重パラメータ統合、synthetic の TransformContext 統合を予定。
    let mut file_outputs = Vec::new();
    for (file, type_resolution) in parsed.files.iter().zip(type_resolutions.iter()) {
        let ctx = TransformContext::new(&module_graph, &registry, type_resolution, &file.path);
        let mut file_synthetic = SyntheticTypeRegistry::new();
        let (items, unsupported) = transform_module_collecting_with_path(
            &file.module, &ctx, &registry, ctx.file_path.parent().and_then(|p| p.to_str()),
            &mut file_synthetic,
        )?;
        let file_synthetic_items = file_synthetic.all_items().into_iter().cloned().collect::<Vec<_>>();
        synthetic.merge(file_synthetic);
        let mut all_items = file_synthetic_items;
        all_items.extend(items);
        let rust_source = generate(&all_items);
        file_outputs.push(FileOutput {
            path: file.path.with_extension("rs"),
            rust_source,
            unsupported,
        });
    }

    Ok(TranspileOutput {
        files: file_outputs,
        module_graph,
        synthetic_items: synthetic.all_items(),
    })
}

/// 単一ファイルの簡易 API。
pub fn transpile_single(source: &str) -> Result<String> {
    let input = TranspileInput {
        files: vec![(PathBuf::from("input.ts"), source.to_string())],
        builtin_types: None,
        module_resolver: Box::new(NullModuleResolver),
    };
    let output = transpile(input)?;
    Ok(output.files.into_iter().next().unwrap().rust_source)
}
```

### 既存 API のラッパー化

```rust
// src/lib.rs

/// 後方互換 API。内部で統一パイプラインを呼ぶ。
pub fn transpile(source: &str) -> Result<String> {
    pipeline::transpile_single(source)
}

pub fn transpile_collecting(source: &str) -> Result<(String, Vec<UnsupportedSyntax>)> {
    let input = TranspileInput {
        files: vec![(PathBuf::from("input.ts"), source.to_string())],
        builtin_types: None,
        module_resolver: Box::new(NullModuleResolver),
    };
    let output = pipeline::transpile(input)?;
    let file = output.files.into_iter().next().unwrap();
    Ok((file.rust_source, file.unsupported))
}
```

### main.rs の置換

```rust
// src/main.rs（概要）

// ディレクトリモード
let files = collect_ts_files(input_dir)?;
let sources: Vec<(PathBuf, String)> = files.iter()
    .map(|p| (p.clone(), fs::read_to_string(p).unwrap()))
    .collect();
let input = TranspileInput {
    files: sources,
    builtin_types: Some(builtin_registry),
    module_resolver: Box::new(NodeModuleResolver::new(input_dir)),
};
let output = pipeline::transpile(input)?;
OutputWriter::new(&output.module_graph)
    .write_to_directory(output_dir, &output.files, &output.synthetic_items, true)?;

// 単一ファイルモード
let rust_source = pipeline::transpile_single(&source)?;
```

### 削除対象コード

| 削除対象 | ファイル | 置換先 | 現在の状態 |
|---------|---------|--------|------|
| `convert_relative_path_to_crate_path` | `src/transformer/mod.rs` | `ModuleGraph.resolve_import()` | **D1 で完了済み** |
| `transpile_directory` (旧実装) | `src/main.rs` | 統一パイプライン + `OutputWriter` | **Phase C で削除済み** |
| `build_shared_registry` | `src/lib.rs` | `transpile_pipeline` 内の型収集 | **リファクタリングで削除済み** |
| `transpile_with_registry` 系 4 関数 | `src/lib.rs` | `transpile()` / `transpile_collecting()` | **リファクタリングで削除済み** |
| `ExprContext` | `src/transformer/expressions/mod.rs` | `TransformContext` + `expected_types` | **Phase 2 で削除済み** |
| `TypeEnv` の narrowing 管理 | `src/transformer/type_env.rs` | `narrowing_events` | **Phase 4 で削除予定**。TypeResolver の narrowing_events カバレッジは D-TR-1 で 100% 確認済みだが、push_scope/pop_scope の削除は未実施 |
| `resolve_expr_type_heuristic` | `src/transformer/expressions/type_resolution.rs` | `TypeResolver` | **Phase 3-2 で削除済み** |
| `tctx` + `reg` 二重パラメータ | 全 Transformer 関数（105 関数） | `tctx.type_registry` に統一 | **D5: 未着手**。分析・設計済み（tasks.md 参照） |
| 合成型の直接 Item push | `src/transformer/functions/mod.rs` 等 | `SyntheticTypeRegistry` | **D0a で解消済み** |
| P1 のブリッジ実装 | `src/pipeline/mod.rs` | 本 PRD の本実装 | **Phase A で削除済み** |

### 残作業

- **D5**: 全 Transformer 関数 105 個（14 ファイル）+ 全テストコード — `reg: &TypeRegistry` パラメータを削除し `tctx.type_registry` に統一
- **Phase 3-7**: `ast_produces_option` 削除（TypeResolver Cond/OptChain expr_type 強化）
- **Phase 4**: TypeEnv 簡素化（narrowing 用 push_scope/pop_scope 削除）
- **Phase E**: 最終検証

## 作業ステップ

### Step 1: テスト設計（RED）

1. 統一パイプラインの E2E テスト:
   - 単一ファイル: `transpile_single` で既存の全スナップショットテストと同じ結果
   - ディレクトリ: 複数ファイル + import 関係を持つ入力 → 正しい mod.rs + use 文 + 変換結果
2. 後方互換 API のテスト:
   - `transpile(source)` が既存と同じ結果を返す
   - `transpile_collecting(source)` が既存と同じ結果を返す
3. 不要コード削除のテスト:
   - `ExprContext`, `TypeEnv`, `resolve_expr_type` への参照がコンパイルエラーにならないことの確認（削除前に参照箇所をゼロにする）

### Step 2: 統一パイプラインの本実装（GREEN）

1. `src/pipeline/mod.rs` のブリッジ実装を、P1〜P7 の全コンポーネントを接続する本実装に置換
2. Pass 0〜7 を順に接続
3. 単一ファイルの E2E テストを GREEN にする

### Step 3: 既存 API のラッパー化（GREEN）

1. `src/lib.rs` の `transpile()`, `transpile_collecting()` を統一パイプラインのラッパーに置換
2. 後方互換テストを GREEN にする
3. 全スナップショットテストが通ることを確認

### Step 4: main.rs の置換（GREEN）

1. ディレクトリモードを統一パイプライン + OutputWriter に置換
2. 単一ファイルモードを `transpile_single` に置換
3. 手動テスト: 実際の TS ファイルで動作確認

### Step 5: 不要コードの削除

削除済み:
- ~~`ExprContext` の削除~~ → Phase 2 で完了
- ~~`resolve_expr_type` / `resolve_expr_type_heuristic` の削除~~ → Phase 3-2 で完了
- ~~`convert_relative_path_to_crate_path` の削除~~ → D1 で完了
- ~~`transpile_directory` 旧実装の削除~~ → Phase C で完了
- ~~分散した合成型生成の残骸の削除~~ → D0a で完了
- ~~P1 のブリッジ実装の削除~~ → Phase A で完了
- ~~`set_expected_types_in_nested_calls` の削除~~ → Phase 3-5 で完了

残り:
1. `TypeEnv` の narrowing 管理の削除 → Phase 4 で対応
2. `tctx` + `reg` 二重パラメータの統合 → D5 で対応
3. 各削除後に `cargo test` が GREEN であることを確認

### Step 6: 全テスト + ベンチマーク（検証）

1. `cargo test` で全テストが GREEN
2. `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
3. `cargo fmt --all --check` が OK
4. Hono ベンチマーク実行:
   - 結果が改善していることを確認（I-222 解消等）
   - `bench-history.jsonl` に記録

### Step 7: リファクタリング（REFACTOR）

1. 未使用の `use` 文の整理
2. ドキュメントコメントの整備
3. `directory.rs` が空になった場合はファイル自体を削除

## テスト計画

| テスト | 検証内容 | 期待結果 |
|--------|---------|---------|
| `test_pipeline_single_file` | 統一パイプラインの単一ファイル変換 | 既存と同じ Rust コード |
| `test_pipeline_multi_file` | 統一パイプラインの複数ファイル変換 | 各ファイルが正しく変換 |
| `test_pipeline_with_imports` | import 関係を持つファイル群 | 正しい use 文 + mod.rs |
| `test_transpile_backward_compat` | `transpile()` の後方互換 | 既存と同じ結果 |
| `test_transpile_collecting_backward_compat` | `transpile_collecting()` の後方互換 | 既存と同じ結果 |
| `test_transpile_single_api` | `transpile_single()` の動作 | 正しい Rust コード |
| `test_no_dead_code` | 不要コードが削除されている | `ExprContext`, 旧 `TypeEnv` narrowing, `convert_relative_path_to_crate_path` への参照なし |
| 既存スナップショットテスト全体 | 出力の後方互換性 | 全スナップショットが一致（または意図的な改善） |
| 既存テスト全体 | 後方互換性 | `cargo test` が全 GREEN |
| Hono ベンチマーク | 変換品質の改善 | clean_pct が改善。I-222 由来のエラーが解消 |
| `test_resolve_hono_representative_file` | Hono の代表ファイルに対して TypeResolver を実行し、Unknown の割合が許容範囲内であることを検証 | P5 のテスト計画に記載されていたが、Hono ソース（`/tmp/hono-clean/`）への依存があるためベンチマーク環境でのみ実行可能。P8 の統合テストとして実施する |

## 完了条件

**注記**: P2〜P7 からの繰り越し項目の進捗:
- `convert_relative_path_to_crate_path` → **D1 で削除済み**
- `SyntheticTypeRegistry` で合成型一元管理 → **D0a で達成済み**
- I-212（enum 重複定義）→ **P8 で構造的に解消済み**
- `ExprContext` → **Phase 2 で削除済み**
- `resolve_expr_type_heuristic` → **Phase 3-2 で削除済み**
- `TypeEnv` narrowing → **Phase 4 で削除予定**
- `tctx` + `reg` 二重パラメータ → **D5 で統合予定**

- [ ] 統一パイプライン `transpile(TranspileInput) -> TranspileOutput` が P1〜P7 の全コンポーネントを接続して動作する
- [ ] 既存 `lib.rs` 公開 API（`transpile()`, `transpile_collecting()` 等）が統一パイプラインのラッパーになっている
- [ ] 既存 `main.rs` のディレクトリ/単一ファイルモードが統一パイプライン呼び出しになっている
- [ ] `transpile_single()` の簡易 API が提供されている
- [ ] 不要コードが削除されている（`ExprContext`, 旧 `TypeEnv` narrowing, `convert_relative_path_to_crate_path`, 分散合成型生成）
- [ ] 既存の全 E2E テストが変更なしで GREEN
- [ ] 既存の全スナップショットテストが一致（または意図的な改善として差分が説明可能）
- [ ] `cargo test` で全テストが GREEN
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] Hono ベンチマーク結果が改善している（clean_pct の向上、I-222 エラーの解消）
- [ ] `bench-history.jsonl` に結果が記録されている
- [ ] pub な型・関数に `///` ドキュメントコメントがある
