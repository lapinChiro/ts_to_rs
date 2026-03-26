# I-192: 大規模ファイルの分割

## 背景・動機

プロダクションコード 6 ファイル・テストコード 5 ファイルが 1000 行を大幅に超過しており、開発効率を著しく阻害している。

- `expressions/tests.rs`（6814 行）、`type_resolver.rs`（3692 行）、`type_converter.rs`（2688 行）等
- Claude の読み書き精度はファイルサイズに強く依存し、6000 行超のファイルでは誤編集・コンテキスト圧迫・読み直しの頻度が跳ね上がる
- 人間のコードレビュー・ナビゲーションも同様に劣化する
- 現在 1000 行超のファイルは **18 個** 存在する

## ゴール

- 全ての `.rs` ファイルが 1000 行以下になる
- 800 行超のファイルは、凝集度が高く分割すると結合が増えるケースのみ許容する
- 外部 API（他モジュールからの `use` パス）は変更しない
- `cargo test` が全テスト pass、`cargo clippy` が 0 警告

## スコープ

### 対象

1000 行超の全 18 ファイル（プロダクション 6 + テスト 5 + テスト抽出 7）を分割する。

**カテゴリ A: プロダクションコード分割（prod > 1000 行）**

| # | ファイル | prod 行数 | test 行数 | 合計 |
|---|---------|----------|----------|------|
| 1 | `src/pipeline/type_resolver.rs` | 2284 | 1408 | 3692 |
| 2 | `src/pipeline/type_converter.rs` | 2590 | 98 | 2688 |
| 3 | `src/transformer/statements/mod.rs` | 2654 | (別ファイル 2766) | 2656 |
| 4 | `src/registry.rs` | 1280 | 1129 | 2409 |
| 5 | `src/transformer/classes.rs` | 1322 | 893 | 2215 |
| 6 | `src/transformer/functions/mod.rs` | 1296 | (別ファイル 1422) | 1298 |

**カテゴリ B: テストファイル分割（test > 1000 行）**

| # | ファイル | 行数 | テスト数 |
|---|---------|------|---------|
| 7 | `src/transformer/expressions/tests.rs` | 6814 | 291 |
| 8 | `src/transformer/types/tests.rs` | 3333 | 130 |
| 9 | `src/transformer/statements/tests.rs` | 2766 | 96 |
| 10 | `src/transformer/functions/tests.rs` | 1422 | 57 |
| 11 | `src/transformer/tests.rs` | 1335 | 57 |

**カテゴリ C: テスト抽出（prod < 800 行だが合計 > 1000 行）**

| # | ファイル | prod 行数 | test 行数 | 合計 |
|---|---------|----------|----------|------|
| 12 | `src/generator/mod.rs` | 613 | 836 | 1449 |
| 13 | `src/ir.rs` | 793 | 495 | 1288 |
| 14 | `src/generator/expressions.rs` | 484 | 783 | 1267 |
| 15 | `src/external_types.rs` | 395 | 676 | 1071 |
| 16 | `src/pipeline/module_graph.rs` | 566 | 472 | 1038 |
| 17 | `src/generator/statements.rs` | 239 | 780 | 1019 |
| 18 | `src/pipeline/external_struct_generator.rs` | 241 | 765 | 1006 |

### 対象外

- 800 行未満のファイルの分割
- 機能変更・リファクタリング（ロジックの変更は一切行わない）
- `pub` / `pub(crate)` の可視性変更（I-193 の対象）
- `clippy.toml` のファイル行数制限導入（I-271 の対象）

## 設計

### 技術的アプローチ

**Rust の `impl` ブロック分割パターン**を使用する。Rust では:

- 1 つの型に対して複数の `impl` ブロックを異なるファイルに配置できる
- 親モジュール（`mod.rs`）の private フィールドは子モジュールからアクセス可能（Rust の可視性ルール: private アイテムは定義モジュールとその子孫から参照可能）
- `type_resolver.rs` → `type_resolver/mod.rs` に変換しても、外部からの `use crate::pipeline::type_resolver::TypeResolver` パスは変わらない

**分割の原則**:

1. **責務ごとの分割**: 1 ファイル 1 責務。関数の依存グラフに基づき、凝集度の高い関数群を 1 つのサブモジュールにまとめる
2. **循環依存の回避**: サブモジュール間に循環依存が生じない分割にする。共有ヘルパーは `mod.rs` または専用の `helpers.rs` に配置
3. **テストの対応関係**: テストファイルの分割はプロダクションコードのモジュール構造に対応させる
4. **外部 API 不変**: `mod.rs` で re-export し、外部モジュールの `use` パスを維持する

### 分割設計

#### 1-7: 完了済み（実績は git history 参照）

- **T1**: `type_resolver.rs` (3692行) → 7 サブモジュール + テスト分割 (T1b)
- **T2**: `type_converter.rs` (2691行) → 6 サブモジュール + tests
- **T3**: `statements/mod.rs` (2656行) → 7 サブモジュール + テスト分割 (T3b)
- **T4**: `registry.rs` (2414行) → 6 サブモジュール + テスト分割 (T4b)
- **T5**: `classes.rs` (2215行) → 5 サブモジュール + tests
- **T6**: `functions/mod.rs` (1298行) → 4 サブモジュール + テスト分割 (T6b: 4 ファイル)
- **T7**: `expressions/tests.rs` (6814行) → テスト分割 (19 ファイル、論理分類ベース)

#### 8-11. テストファイル分割

各テストファイルは、テスト対象の機能単位で論理的に分類したサブモジュールに分割する。

**8. `types/tests.rs` (3333 行, 130 テスト) → `types/tests/`**:
`mod.rs`（ヘルパー）, `primitives.rs`, `interfaces.rs`, `unions.rs`, `aliases.rs`, `utility_types.rs`, `advanced.rs`

**9. `statements/tests.rs` (2766 行, 96 テスト) → `statements/tests/`**:
`mod.rs`（ヘルパー）, `variables.rs`, `control_flow.rs`, `loops.rs`, `destructuring.rs`, `switch.rs`, `error_handling.rs`, `expected_types.rs`

**10. `functions/tests.rs` (1422 行, 57 テスト) → `functions/tests/`**:
`mod.rs`（ヘルパー）, `declarations.rs`, `params.rs`, `destructuring.rs`, `async_fn.rs`, `return_handling.rs`

**11. `transformer/tests.rs` (1335 行, 57 テスト) → `transformer/tests/`**:
`mod.rs`（ヘルパー）, `imports.rs`, `exports.rs`, `types.rs`, `classes.rs`, `enums.rs`, `functions.rs`

#### 12-18. テスト抽出（prod < 800 行、total > 1000 行）

インラインの `#[cfg(test)] mod tests { ... }` を `#[cfg(test)] mod tests;` + 別ファイルに抽出する。プロダクションコードは変更しない。

| ファイル | 抽出方法 | 抽出後 prod | 抽出後 test |
|---------|---------|------------|------------|
| `generator/mod.rs` | → `generator/tests.rs` | 613 | 836 |
| `ir.rs` | → `ir/` dir + `tests.rs` | 793 | 495 |
| `generator/expressions.rs` | → `generator/expressions/` dir + `tests.rs` | 484 | 783 |
| `generator/statements.rs` | → `generator/statements/` dir + `tests.rs` | 239 | 780 |
| `external_types.rs` | → `external_types/` dir + `tests.rs` | 395 | 676 |
| `module_graph.rs` | → `module_graph/` dir + `tests.rs` | 566 | 472 |
| `external_struct_generator.rs` | → `external_struct_generator/` dir + `tests.rs` | 241 | 765 |

**注**: `ir.rs`（prod=793）は純粋なデータ定義ファイルで凝集度が極めて高い。800 行未満であり分割不要。テスト抽出のみ行う。

### 設計整合性レビュー

- **高次の整合性**: 各モジュールの外部 API は `mod.rs` の re-export で維持する。パイプライン全体（parser → transformer → generator）の構造に変更なし
- **DRY / 直交性 / 結合度**: サブモジュール間の依存は単方向のみ許容。共有ヘルパーは `helpers.rs` または `mod.rs` に集約し、循環依存を構造的に排除する
- **割れ窓**: 分割作業中に発見した既存コードの問題はロジック変更を伴うため本 PRD では修正しない。TODO に記録する

### 影響範囲

| ディレクトリ | 変更ファイル数 | 新規ファイル数 |
|------------|-------------|-------------|
| `src/pipeline/` | 4 (type_resolver, type_converter, module_graph, external_struct_generator) | ~25 |
| `src/transformer/` | 6 (statements, classes, functions, expressions, types, tests) | ~40 |
| `src/generator/` | 3 (mod, expressions, statements) | ~5 |
| `src/` | 3 (registry, ir, external_types) | ~15 |
| **合計** | **16** | **~85** |

## タスク一覧

### T1: `type_resolver.rs` → `type_resolver/` ディレクトリ化 ✅

- **作業内容**: `src/pipeline/type_resolver.rs` (3692行) を 7 サブモジュールに分割
- **結果**: `mod.rs` (146), `visitors.rs` (431), `narrowing.rs` (146), `expected_types.rs` (194), `expressions.rs` (976), `du_analysis.rs` (221), `helpers.rs` (243)。テスト 65 個全 pass。外部 API パス不変
- **依存**: なし

### T1b: `type_resolver` テスト分割 ✅

- **作業内容**: T1 で抽出した `tests.rs` (1405行) を `tests/` ディレクトリに 3 サブモジュールに分割
- **結果**: `tests/mod.rs` (93, 共有ヘルパー), `tests/basics.rs` (425), `tests/expected_types.rs` (434), `tests/complex_features.rs` (453)。テスト 65 個全 pass
- **PRD 設計との差異**: ファイル名を `variables.rs`/`narrowing.rs`/`expressions.rs` から `basics.rs`/`expected_types.rs`/`complex_features.rs` に変更。テスト内容の論理的カテゴリに対応させた
- **依存**: T1

### T2: `type_converter.rs` → `type_converter/` ディレクトリ化 ✅

- **作業内容**: `src/pipeline/type_converter.rs` (2691行) を 6 サブモジュール + tests に分割（PRD 設計の 8 ファイルから最適化）
- **結果**: `mod.rs` (289), `interfaces.rs` (433), `intersections.rs` (325), `type_aliases.rs` (518), `unions.rs` (585), `utilities.rs` (467), `tests.rs` (95)。テスト 4 個全 pass。外部 API パス不変
- **PRD 設計との差異**: 詳細は上記設計セクション参照
- **依存**: なし

### T3: `statements/mod.rs` サブモジュール分割 ✅

- **結果**: 7 サブモジュールに分割。`mod.rs` (198), `control_flow.rs` (753), `switch.rs` (727), `error_handling.rs` (294), `spread.rs` (202), `destructuring.rs` (239), `mutability.rs` (132), `helpers.rs` (180)。全テスト pass

### T3b: `statements/tests.rs` テスト分割 ✅

- **結果**: `tests/` に 7 サブモジュール分割。96 テスト全 pass
- **依存**: T3

### T4: `registry.rs` → `registry/` ディレクトリ化 ✅

- **結果**: 6 サブモジュール + tests に分割。`mod.rs` (350), `collection.rs` (400), `interfaces.rs` (98), `unions.rs` (194), `functions.rs` (177), `enums.rs` (132), `tests.rs` (1131→T4bで分割)。全テスト pass

### T4b: `registry` テスト分割 ✅

- **結果**: tests.rs (1131行) → `tests/` に 4 サブモジュール分割。52 テスト全 pass
- **依存**: T4

### T5: `classes.rs` → `classes/` ディレクトリ化 ✅

- **結果**: 5 サブモジュール + tests に分割。`mod.rs` (171), `generation.rs` (329), `inheritance.rs` (181), `members.rs` (513), `helpers.rs` (170), `tests.rs` (894)。全テスト pass

### T6: `functions/mod.rs` サブモジュール分割 ✅

- **作業内容**: `src/transformer/functions/mod.rs` (1298行) を 4 サブモジュールに分割
- **結果**: `mod.rs` (236), `helpers.rs` (272), `params.rs` (278), `destructuring.rs` (381), `arrow_fns.rs` (173)。全テスト pass。外部 API パス不変
- **PRD 設計との差異**: `closures.rs` → `arrow_fns.rs` に名称変更（アロー関数変換に特化）。`destructuring.rs` を `params.rs` から分離して凝集度を向上
- **依存**: なし

### T6b: `functions/tests.rs` テスト分割 ✅

- **作業内容**: `src/transformer/functions/tests.rs`（1422 行）を `tests/` ディレクトリに 4 サブモジュール分割
- **結果**: `tests/mod.rs` (23), `fn_decl.rs` (400), `params.rs` (559), `destructuring.rs` (322), `helpers.rs` (125)。57 テスト全 pass
- **PRD 設計との差異**: `declarations.rs`/`async_fn.rs`/`return_handling.rs` → `fn_decl.rs`/`helpers.rs` に統合。async/return テストは fn_decl の一部として凝集度が高い
- **依存**: T6

### T7: `expressions/tests.rs` テスト分割 ✅

- **作業内容**: `src/transformer/expressions/tests.rs`（6814 行）を `tests/` ディレクトリに 19 サブモジュール分割
- **結果**: `mod.rs` (134, 共有ヘルパー), `literals.rs` (185), `binary_unary.rs` (519), `ternary.rs` (109), `calls.rs` (726), `math_number.rs` (397), `arrows.rs` (396), `fn_exprs.rs` (167), `arrays.rs` (874), `objects.rs` (489), `strings.rs` (311), `regex.rs` (305), `member_access.rs` (183), `optional_chaining.rs` (315), `optional_semantics.rs` (188), `type_guards.rs` (389), `enums.rs` (337), `expected_type.rs` (410), `update_exprs.rs` (115), `builtins.rs` (142)。全 291 テスト pass
- **PRD 設計との差異**: 15 ファイル → 19 ファイルに増加。テスト対象の機能単位で論理的に分類（リテラル、演算子、呼び出し、Math/Number、配列メソッド等）。PRD の `closures.rs`/`constructors.rs`/`type_inference.rs` 等は実際のテスト内容に基づき `arrows.rs`/`calls.rs`/`expected_type.rs` 等に再分類
- **依存**: なし

### T8: `types/tests.rs` テスト分割 ✅

- **作業内容**: `src/transformer/types/tests.rs`（3333 行）を `tests/` ディレクトリに 7 サブモジュール分割
- **結果**: `tests/mod.rs` (61, 共有ヘルパー), `primitives.rs` (313), `collections.rs` (421), `interfaces.rs` (437), `type_aliases.rs` (566), `unions.rs` (813), `intersections.rs` (421), `structural_transforms.rs` (338)。全 130 テスト pass
- **PRD 設計との差異**: `aliases.rs`/`utility_types.rs`/`advanced.rs` → `type_aliases.rs`/`collections.rs`/`structural_transforms.rs` に再分類。テスト対象の論理的凝集度に基づく分類（プリミティブ型、コレクション型、インターフェース、型エイリアス、ユニオン型、インターセクション型、構造型変換）
- **依存**: なし

### T9: `transformer/tests.rs` テスト分割 ✅

- **作業内容**: `src/transformer/tests.rs`（1335 行）を `tests/` ディレクトリに 6 サブモジュール分割
- **結果**: `tests/mod.rs` (15), `imports_and_exports.rs` (317), `module_items.rs` (219), `enums.rs` (136), `classes.rs` (318), `variable_type_propagation.rs` (278), `error_handling.rs` (68)。全 57 テスト pass
- **PRD 設計との差異**: 7 サブモジュール → 6 サブモジュール。`imports.rs`/`exports.rs` → `imports_and_exports.rs` に統合（同一ロジック）。`types.rs`/`functions.rs` → `module_items.rs`/`variable_type_propagation.rs` に再分類。テスト内容の論理的凝集度に基づく
- **依存**: なし

### T10: `generator/` テスト抽出 ✅

- **作業内容**: `src/generator/mod.rs`（836 行テスト）、`src/generator/expressions.rs`（783 行テスト）、`src/generator/statements.rs`（780 行テスト）のインラインテストを別ファイルに抽出。`expressions.rs` と `statements.rs` はディレクトリ化
- **結果**: `mod.rs` (576) + `tests.rs` (828)、`expressions/mod.rs` (486) + `expressions/tests.rs` (771)、`statements/mod.rs` (241) + `statements/tests.rs` (774)。全テスト pass
- **依存**: なし

### T11: `ir.rs` テスト抽出

- **作業内容**: `src/ir.rs`（495 行テスト）のインラインテストを別ファイルに抽出する。`ir.rs` → `ir/mod.rs` + `ir/tests.rs`
- **完了条件**: `ir/mod.rs` が 793 行（1000 行以下）。全テスト pass
- **依存**: なし

### T12: `pipeline/` テスト抽出

- **作業内容**: `src/external_types.rs`（676 行テスト）、`src/pipeline/module_graph.rs`（472 行テスト）、`src/pipeline/external_struct_generator.rs`（765 行テスト）のインラインテストを別ファイルに抽出する。各ファイルをディレクトリ化（`mod.rs` + `tests.rs`）
- **完了条件**: 全ファイルが 1000 行以下。全テスト pass
- **依存**: なし

### T13: 最終検証＆clippy.toml最適化

- **作業内容**: 全ファイルの行数が 1000 行以下であることを検証する。さらに、clippy.tomlに最適なファイル行数制限のルールを追加する。`cargo test` 全 pass、`cargo clippy --all-targets --all-features -- -D warnings` 0 警告、`cargo fmt --all --check` pass を確認する
- **完了条件**: 0 エラー・0 警告。1000 行超のファイルが 0 個
- **依存**: T1-T12 全て

## テスト計画

- **正常系**: 各タスクの完了後に `cargo test` で全テスト pass を確認
- **回帰テスト**: `cargo test` の全テスト数が分割前後で変わらないことを確認（テストの消失・重複を検出）
- **コンパイルチェック**: `cargo check` で型エラー・import エラーがないことを確認
- **ベンチマーク**: 最終検証で `./scripts/hono-bench.sh` を実行し、変換結果に変化がないことを確認

## 完了条件

1. 全 `.rs` ファイルが 1000 行以下（800 行超は凝集度の根拠があること）
2. `cargo test` 全 pass（テスト数が分割前と一致）
3. `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
4. `cargo fmt --all --check` pass
5. 外部モジュールの `use` パスに変更がない
6. Hono ベンチマーク結果が分割前と同一
