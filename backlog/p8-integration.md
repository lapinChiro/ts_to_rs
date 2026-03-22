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
- 不要コードの削除:
  - `convert_relative_path_to_crate_path`（`src/directory.rs`）→ `ModuleGraph` に置換済み
  - `ExprContext`（`src/transformer/expressions/mod.rs`）→ P6 で FileTypeResolution をフォールバックとして参照するよう変更済み（P8 Phase B で優先順位を修正: ExprContext 優先、FileTypeResolution フォールバック）。ExprContext は Transformer が文脈に基づいて設定する「精密な」expected であり、FileTypeResolution は TypeResolver の事前計算値。Option<T> unwrap のように文脈を変えて再帰する場合に ExprContext 優先が必須（Phase B で発見した無限ループバグの教訓）。削除するには TypeResolver が全 ExprContext 伝搬ケースをカバーする必要がある
  - `TypeEnv` の narrowing スコープ管理（`src/transformer/type_env.rs`）→ P6 で `FileTypeResolution.narrowed_type()` を優先参照するよう変更済み。TypeEnv はフォールバックとして併存中。TypeEnv 自体は変数型追跡（`insert`/`get`）にも使われるため、narrowing 以外の用途が残る場合は構造体は残す
  - `resolve_expr_type`（`src/transformer/expressions/type_resolution.rs`）→ P6 で `FileTypeResolution.expr_types` を優先参照するよう変更済み。ヒューリスティクス（`resolve_expr_type_heuristic`）がフォールバックとして併存中。フォールバックが発火するケースを Hono ベンチマークで計測し、0 件であることを確認してから削除すること
  - `tctx` + `reg` の二重パラメータ（全 Transformer 関数）→ P6 で `tctx.type_registry` と `reg` が同一の参照を持つ冗長な構造のまま残存。P8 で `reg` パラメータを削除し `tctx.type_registry` に統一する。影響範囲: 105 関数 + 全テストコード
  - 分散した合成型生成（Transformer 内の直接 `Item::Enum` push）→ `SyntheticTypeRegistry` に集約済み
  - P1 で作成したブリッジ実装 → **Phase A で本実装に置換済み**。旧ブリッジコードは存在しない
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

| 削除対象 | ファイル | 置換先 | 現在の状態（Phase D 完了時点） |
|---------|---------|--------|------|
| `convert_relative_path_to_crate_path` | `src/transformer/mod.rs` | `ModuleGraph.resolve_import()` | **D1: ModuleGraph lookup + fallback パターンを適用すべき**。TransformContext は module_graph を持っているが未使用。resolve_import() を先に試し、解決不可時に fallback |
| `transpile_directory` (旧実装) | `src/main.rs` | 統一パイプライン + `OutputWriter` | **Phase C で削除済み** |
| `build_shared_registry` | `src/lib.rs` | `transpile_pipeline` 内の型収集 | **リファクタリングで削除済み** |
| `transpile_with_registry` 系 4 関数 | `src/lib.rs` | `transpile()` / `transpile_collecting()` | **リファクタリングで削除済み** |
| `ExprContext` | `src/transformer/expressions/mod.rs` | `TransformContext` + `expected_types` | **D2: TypeResolver が Option unwrap 後の inner type も設定すれば削除可能**。現状は再帰防止に必須 |
| `TypeEnv` の narrowing 管理 | `src/transformer/type_env.rs` | `narrowing_events` | **D3: TypeResolver の narrowing_events カバレッジ 100% で削除可能**。現状は不十分 |
| `resolve_expr_type_heuristic` | `src/transformer/expressions/type_resolution.rs` | `TypeResolver` | **D4: TypeResolver の expr_types カバレッジ 100% で削除可能**。現状は不十分 |
| `tctx` + `reg` 二重パラメータ | 全 Transformer 関数（105 関数） | `tctx.type_registry` に統一 | **D5: 未着手**。分析・設計済み（tasks.md 参照） |
| 合成型の直接 Item push | `src/transformer/functions/mod.rs` 等 | `SyntheticTypeRegistry` | **D0a で解消済み**（`build_any_enum_variants` + `register_any_enum`） |
| P1 のブリッジ実装 | `src/pipeline/mod.rs` | 本 PRD の本実装 | **Phase A で削除済み** |

### 影響ファイル（D1, D2-D4, D5, D6 の残作業）

- **D1**: `src/transformer/mod.rs`（import 解決に ModuleGraph lookup + fallback を適用）
- **D2-D4**: `src/pipeline/type_resolver.rs`（TypeResolver のカバレッジ改善: expected_types の Option inner type 設定、narrowing_events カバレッジ、expr_types カバレッジ）→ カバレッジ 100% 達成後に ExprContext / TypeEnv narrowing / heuristic を削除
- **D5**: 全 Transformer 関数 105 個（14 ファイル）+ 全テストコード — `reg: &TypeRegistry` パラメータを削除し `tctx.type_registry` に統一
- **D6**: `src/pipeline/types.rs`（`FileOutput` に `source: String` フィールド追加）+ `src/pipeline/mod.rs`（ソース文字列を移送）+ `src/main.rs`（`files.clone()` 削除）

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

1. `ExprContext` の削除（参照箇所がゼロであることを確認）
2. `TypeEnv` の narrowing 管理の削除
3. `resolve_expr_type` の削除（フォールバックが不要になっている場合）
4. `convert_relative_path_to_crate_path` の削除
5. `transpile_directory` 旧実装の削除
6. 分散した合成型生成の残骸の削除
7. P1 のブリッジ実装の削除
8. 各削除後に `cargo test` が GREEN であることを確認

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

**注記**: 以下の条件には P2〜P7 で「コンポーネントは実装済みだが既存コードとの統合は P8 に委ねた」項目が含まれる。具体的には:
- P2 の「`convert_relative_path_to_crate_path` が `ModuleGraph` に置き換えられている」→ P6 で Transformer に統合し、P8 で旧コードを削除
- P3 の「`SyntheticTypeRegistry` で合成型が一元管理されている」→ P4 で TypeCollector が使用開始し、P8 で分散生成を完全削除
- P4 の「I-212 が構造的に解消されている」→ P8 で全パスが SyntheticTypeRegistry を使うようになった時点で完全達成

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
