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

#### 1. `src/pipeline/type_resolver.rs` (3692 行) → `src/pipeline/type_resolver/`

```
type_resolver/
├── mod.rs              # struct 定義, new(), resolve_file(), scope 管理 (~200 行)
├── visitors.rs         # visit_* メソッド群 (impl TypeResolver) (~420 行)
├── narrowing.rs        # narrowing 検出 (impl TypeResolver) (~135 行)
├── expected_types.rs   # expected type 伝播 (impl TypeResolver) (~180 行)
├── expressions.rs      # 式の型解決 (impl TypeResolver) (~960 行)
├── du_analysis.rs      # DU switch 検出・フィールド収集 (~100 行)
└── helpers.rs          # 独立ヘルパー関数 (~350 行)
```

**mod.rs**: `TypeResolver` struct 定義（フィールドは private のまま）、`Scope`/`VarInfo` struct、`new()`、`set_any_enum_overrides()`、`resolve_file()`、scope 管理メソッド（`enter_scope`, `leave_scope`, `declare_var`, `lookup_var`, `mark_var_mutable`）

**visitors.rs**: `visit_module_item`, `visit_decl`, `visit_fn_decl`, `visit_param_pat`, `visit_var_decl`, `register_pat_vars`, `visit_class_decl`, `visit_block_stmt`, `visit_stmt`, `visit_if_stmt`

**narrowing.rs**: `detect_narrowing_guard`, `extract_typeof_narrowing`, `extract_typeof_and_string`, `extract_null_check_narrowing`

**expected_types.rs**: `resolve_object_lit_fields`, `merge_object_fields`, `propagate_expected`

**expressions.rs**: `resolve_expr`, `resolve_expr_inner`, `resolve_bin_expr`, `resolve_member_expr`, `resolve_member_type`, `resolve_call_expr`, `set_call_arg_expected_types`, `collect_resolved_arg_types`, `lookup_method_sigs`, `lookup_method_params`, `resolve_method_return_type`, `resolve_new_expr`, `resolve_arrow_expr`, `resolve_fn_expr`, `resolve_array_expr`
- 960 行は 1000 行未満だが上限に近い。`resolve_expr_inner` の巨大 match が単独で ~330 行を占めるため、これ以上の分割は match アームの分断を伴い凝集度を下げる

**du_analysis.rs**: `detect_du_switch_bindings`, `collect_du_field_accesses_from_stmts`, `collect_du_field_accesses_from_stmt_inner`, `collect_du_field_accesses_from_expr_inner`, `case_body_span_range`

**helpers.rs**: `is_null_or_undefined`, `extract_prop_name`, `find_string_prop_value`, `common_named_type`, `is_null_literal`, `is_object_type`, `extract_type_name_for_registry`, `select_overload`, `unwrap_promise_and_unit`, `resolve_fn_type_info`

**依存方向**:
```
mod.rs (struct 定義 + scope)
  ↑ 全サブモジュールが super:: で参照
visitors.rs → narrowing.rs, expressions.rs, helpers.rs
narrowing.rs → helpers.rs
expected_types.rs → helpers.rs
expressions.rs → expected_types.rs, narrowing.rs, helpers.rs
du_analysis.rs → helpers.rs
```

#### 2. `src/pipeline/type_converter.rs` (2688 行) → `src/pipeline/type_converter/`

```
type_converter/
├── mod.rs              # pub API + core dispatcher (convert_ts_type, convert_type_ref) (~300 行)
├── unions.rs           # union 型変換 (~250 行)
├── interfaces.rs       # interface → struct/trait 変換 (~360 行)
├── type_aliases.rs     # type alias 変換 (~600 行)
├── utility_types.rs    # Partial/Pick/Omit 等 (~310 行)
├── intersections.rs    # intersection 型変換 (~170 行)
├── annotations.rs      # 型リテラル・関数型・indexed access (~200 行)
└── helpers.rs          # 共有ユーティリティ (~100 行)
```

**mod.rs**: `convert_type_for_position`（pub）, `convert_ts_type`（pub）, `extract_type_params`（pub）, `convert_type_ref`, `is_nullable_keyword`, `unwrap_promise`, `convert_property_signature`（pub(crate)）

**unions.rs**: `convert_union_type`, `convert_unsupported_union_member`, `convert_fn_type_to_rust`

**interfaces.rs**: `convert_interface_items`（pub）, `convert_interface`（pub）, `convert_interface_as_struct`, `convert_interface_as_fn_type`, `convert_interface_as_struct_and_trait`, `convert_interface_as_trait`, `convert_method_signature`, `collect_extends_refs`, `collect_extends_names`

**type_aliases.rs**: `convert_type_alias_items`（pub）, `convert_type_alias`（pub）, `try_convert_keyof_typeof_alias`, `try_convert_string_literal_union`, `try_convert_single_string_literal`, `try_convert_discriminated_union`, `try_convert_general_union`, `find_discriminant_field`, `extract_variant_info`, `is_string_literal_type`
- 600 行だが `convert_type_alias` の巨大 match + 7 つの `try_convert_*` が密結合しており分割不適

**utility_types.rs**: `convert_utility_partial`, `convert_utility_required`, `convert_utility_pick`, `convert_utility_omit`, `convert_utility_non_nullable`, `resolve_utility_inner_fields`, `resolve_utility_inner_with_conversion`, `extract_string_keys`, `capitalize_first`

**intersections.rs**: `try_convert_intersection_type`, `extract_intersection_members`

**annotations.rs**: `convert_type_lit_in_annotation`, `convert_intersection_in_annotation`, `convert_fn_type`, `convert_indexed_access_type`, `convert_conditional_type`, `try_convert_infer_pattern`, `extract_infer_info`, `is_true_false_literal`, `try_convert_function_type_alias`, `try_convert_tuple_type_alias`

**helpers.rs**: `string_to_pascal_case`（pub(crate)）

**依存方向**:
```
mod.rs (core dispatcher)
  ↑ unions.rs, interfaces.rs, type_aliases.rs, annotations.rs
helpers.rs ← 全モジュール
utility_types.rs ← mod.rs (convert_type_ref から呼出)
intersections.rs ← type_aliases.rs, annotations.rs
```

#### 3. `src/transformer/statements/mod.rs` (2656 行) → サブモジュール分割

```
statements/
├── mod.rs              # convert_stmt dispatcher + convert_var_decl + convert_stmt_list (~300 行)
├── control_flow.rs     # if/while/for/do-while/labeled (impl Transformer) (~500 行)
├── switch.rs           # switch 文の全変換 (impl Transformer) (~730 行)
├── error_handling.rs   # try/catch/throw + TryBodyRewrite (impl Transformer) (~300 行)
├── spread.rs           # spread array 展開 (impl Transformer) (~250 行)
├── destructuring.rs    # object/array destructuring (impl Transformer) (~150 行)
├── mutability.rs       # let mut 推論 (~130 行)
└── helpers.rs          # conditional assignment 抽出、truthiness/falsy 条件生成 (~300 行)
```

**mod.rs**: `convert_stmt`（pub(crate)、main dispatcher）、`convert_var_decl`、`convert_stmt_list`（pub(crate)）、`convert_block_or_stmt`、`convert_nested_fn_decl`、`is_object_type`、サブモジュール宣言

**control_flow.rs**: `convert_if_stmt`, `convert_if_with_conditional_assignment`, `convert_while_stmt`, `convert_while_with_conditional_assignment`, `convert_for_stmt`, `convert_for_of_stmt`, `convert_for_in_stmt`, `convert_labeled_stmt`, `convert_do_while_stmt`, `convert_for_stmt_as_loop`, `convert_update_to_stmt`, `convert_and_combine_conditions`, `build_nested_if_let`, `can_generate_if_let`, `generate_if_let`

**switch.rs**: `convert_switch_stmt`, `try_convert_typeof_switch`, `try_convert_discriminated_union_switch`, `try_convert_string_enum_switch`, `convert_switch_clean_match`, `convert_switch_fallthrough`, `is_case_terminated`, `is_literal_match_pattern`, `build_combined_guard`, `collect_du_field_accesses`, `collect_du_field_accesses_from_stmt`, `collect_du_field_accesses_from_expr`

**error_handling.rs**: `convert_try_stmt`, `convert_throw_stmt`, `extract_error_message`, `ends_with_return`, `is_err_call`, `TryBodyRewrite` struct + `rewrite()` impl

**spread.rs**: `try_expand_spread_var_decl`, `try_expand_spread_return`, `try_expand_spread_expr_stmt`, `convert_spread_segments`, `emit_spread_ops`, `has_spread_elements`, `extract_spread_array_init`

**destructuring.rs**: `try_convert_object_destructuring`, `expand_object_pat_props`, `try_convert_array_destructuring`

**mutability.rs**: `mark_mutated_vars`, `collect_mutated_vars`, `collect_mutated_vars_from_expr`, `collect_closure_assigns`, `collect_assigns_from_expr`, `MUTATING_METHODS`

**helpers.rs**: `ConditionalAssignment` struct, `OuterComparison` struct, `extract_conditional_assignment`, `unwrap_parens`, `extract_assign_target_name`, `extract_assign_from_expr`, `is_comparison_op`, `generate_truthiness_condition`, `generate_falsy_condition`

#### 4. `src/registry.rs` (2409 行) → `src/registry/`

```
registry/
├── mod.rs              # TypeDef, TypeRegistry struct + impl + build_registry() (~400 行)
├── collection.rs       # 2-pass 型収集 (collect_type_name, collect_decl 等) (~350 行)
├── interfaces.rs       # interface フィールド・メソッド収集 (~150 行)
├── unions.rs           # string literal / discriminated union 検出 (~250 行)
├── functions.rs        # 関数型・アロー関数定義収集 (~200 行)
└── enums.rs            # 合成 enum 登録 + any-narrowing enum (~200 行)
```

**mod.rs**: `MethodSignature` struct、`TypeDef` enum + impl、`TypeRegistry` struct + impl（`new`, `register`, `register_external`, `is_external`, `get`, `is_trait_type`, `instantiate`, `merge`）、`build_registry`（pub）、`build_registry_with_synthetic`（pub）

**collection.rs**: `collect_type_name`（Pass 1）、`collect_decl`（Pass 2 dispatcher）、`collect_class_info`、`collect_type_params`、`collect_type_alias_fields`

**interfaces.rs**: `collect_interface_fields`, `collect_interface_methods`, `collect_property_signature`

**unions.rs**: `try_collect_string_literal_union`, `try_collect_discriminated_union`, `find_registry_discriminant_field`, `extract_registry_variant_info`

**functions.rs**: `try_collect_fn_type_alias`, `try_collect_call_signature_fn`, `collect_fn_def_with_extras`, `collect_arrow_def_with_extras`

**enums.rs**: `register_extra_enums`, `register_single_enum`, `register_single_enum_by_name`, `register_enum_typedef`, `register_any_narrowing_enums`, `register_any_narrowing_enums_from_expr`

#### 5. `src/transformer/classes.rs` (2215 行) → `src/transformer/classes/`

```
classes/
├── mod.rs              # ClassInfo struct + extract_class_info + public API (~300 行)
├── generation.rs       # struct/impl/trait 生成 (impl Transformer) (~350 行)
├── inheritance.rs      # 継承・super constructor 処理 (impl Transformer) (~300 行)
├── members.rs          # フィールド・メソッド・プロパティ変換 (impl Transformer) (~350 行)
└── helpers.rs          # visibility 解決、parent 探索 (~150 行)
```

**mod.rs**: `ClassInfo` struct 定義、`extract_class_info`、pub API（`generate_items_for_class`, `generate_parent_class_items`, `generate_abstract_class_items`, `generate_child_of_abstract`, `generate_child_class_with_implements`, `generate_class_with_implements`）

**generation.rs**: `generate_standalone_class`, `generate_child_class`, pub API の実装本体

**inheritance.rs**: `rewrite_super_constructor`, `try_extract_super_call`, `transform_class_with_inheritance`, `pre_scan_classes`, `find_parent_class_names`

**members.rs**: `convert_constructor`, `convert_constructor_body`, `convert_class_prop`, `convert_private_prop`, `convert_class_method`, `convert_private_method`, `convert_param_pat`, `convert_ts_param_prop`, `build_param_prop_assignments`, `try_extract_this_assignment`

**helpers.rs**: `resolve_member_visibility`, `body_has_self_assignment`, `is_self_field_access`

#### 6. `src/transformer/functions/mod.rs` (1298 行) → サブモジュール分割

```
functions/
├── mod.rs              # convert_fn_decl + convert_param (impl Transformer) (~500 行)
├── closures.rs         # アロー関数・クロージャ変換 (impl Transformer) (~200 行)
├── params.rs           # destructuring params + default value 推論 (~300 行)
└── helpers.rs          # mutability/throw 検出、return wrap、promise unwrap (~300 行)
```

#### 7-11. テストファイル分割

各テストファイルは、プロダクションコードのモジュール構造に対応したサブモジュールに分割する。テスト間に依存関係はないため、テストカテゴリごとに分割する。

**7. `expressions/tests.rs` (6814 行, 291 テスト) → `expressions/tests/`**:
`mod.rs`（ヘルパー）, `literals.rs`, `closures.rs`, `calls.rs`, `constructors.rs`, `type_inference.rs`, `optional.rs`, `operators.rs`, `string_methods.rs`, `array_methods.rs`, `builtins.rs`, `type_assertions.rs`, `discriminated_unions.rs`, `computed_access.rs`, `spread_rest.rs`

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

### T1: `type_resolver.rs` → `type_resolver/` ディレクトリ化

- **作業内容**: `src/pipeline/type_resolver.rs` を `src/pipeline/type_resolver/mod.rs` にリネームし、上記設計に従い 7 サブモジュール（`visitors.rs`, `narrowing.rs`, `expected_types.rs`, `expressions.rs`, `du_analysis.rs`, `helpers.rs`）に分割する。テストは `tests.rs`（テスト行数 1408 行、1000 行超のため T1b で分割）に抽出する
- **完了条件**: 全サブモジュールが 1000 行以下。`cargo test -- type_resolver` 全 pass。外部の `use crate::pipeline::type_resolver::*` パスが不変
- **依存**: なし

### T1b: `type_resolver` テスト分割

- **作業内容**: T1 で抽出した `tests.rs`（1408 行）を `tests/` ディレクトリに分割（`variables.rs`, `narrowing.rs`, `expressions.rs` 等）
- **完了条件**: 全テストファイルが 1000 行以下。全テスト pass
- **依存**: T1

### T2: `type_converter.rs` → `type_converter/` ディレクトリ化

- **作業内容**: `src/pipeline/type_converter.rs` を上記設計に従い 8 サブモジュール（`unions.rs`, `interfaces.rs`, `type_aliases.rs`, `utility_types.rs`, `intersections.rs`, `annotations.rs`, `helpers.rs`）に分割する。テスト（98 行）は `mod.rs` 内に残置
- **完了条件**: 全サブモジュールが 1000 行以下。`cargo test` 全 pass。外部の `use crate::pipeline::type_converter::*` パスが不変
- **依存**: なし

### T3: `statements/mod.rs` サブモジュール分割

- **作業内容**: `src/transformer/statements/mod.rs` を上記設計に従い 7 サブモジュール（`control_flow.rs`, `switch.rs`, `error_handling.rs`, `spread.rs`, `destructuring.rs`, `mutability.rs`, `helpers.rs`）に分割する
- **完了条件**: 全サブモジュールが 1000 行以下。`cargo test -- statements` 全 pass
- **依存**: なし

### T3b: `statements/tests.rs` テスト分割

- **作業内容**: `src/transformer/statements/tests.rs`（2766 行）を `tests/` ディレクトリに分割（`variables.rs`, `control_flow.rs`, `loops.rs`, `destructuring.rs`, `switch.rs`, `error_handling.rs`, `expected_types.rs`）
- **完了条件**: 全テストファイルが 1000 行以下。全テスト pass
- **依存**: T3

### T4: `registry.rs` → `registry/` ディレクトリ化

- **作業内容**: `src/registry.rs` を上記設計に従い 6 サブモジュール（`collection.rs`, `interfaces.rs`, `unions.rs`, `functions.rs`, `enums.rs`）に分割する。テスト（1129 行）は `tests.rs` に抽出し、1000 行超の場合は T4b で分割
- **完了条件**: 全サブモジュールが 1000 行以下。`cargo test -- registry` 全 pass。外部の `use crate::registry::*` パスが不変
- **依存**: なし

### T4b: `registry` テスト分割（条件付き）

- **作業内容**: T4 で抽出した `tests.rs` が 1000 行超の場合、テストカテゴリごとに分割する
- **完了条件**: 全テストファイルが 1000 行以下。全テスト pass
- **依存**: T4

### T5: `classes.rs` → `classes/` ディレクトリ化

- **作業内容**: `src/transformer/classes.rs` を上記設計に従い 5 サブモジュール（`generation.rs`, `inheritance.rs`, `members.rs`, `helpers.rs`）に分割する。テスト（893 行）は `tests.rs` に抽出（1000 行未満のため分割不要）
- **完了条件**: 全サブモジュールが 1000 行以下。`cargo test -- classes` 全 pass
- **依存**: なし

### T6: `functions/mod.rs` サブモジュール分割

- **作業内容**: `src/transformer/functions/mod.rs` を上記設計に従い 4 サブモジュール（`closures.rs`, `params.rs`, `helpers.rs`）に分割する
- **完了条件**: 全サブモジュールが 1000 行以下。`cargo test -- functions` 全 pass
- **依存**: なし

### T6b: `functions/tests.rs` テスト分割

- **作業内容**: `src/transformer/functions/tests.rs`（1422 行）を `tests/` ディレクトリに分割（`declarations.rs`, `params.rs`, `destructuring.rs`, `async_fn.rs`, `return_handling.rs`）
- **完了条件**: 全テストファイルが 1000 行以下。全テスト pass
- **依存**: T6

### T7: `expressions/tests.rs` テスト分割

- **作業内容**: `src/transformer/expressions/tests.rs`（6814 行）を `tests/` ディレクトリに分割（15 サブモジュール: `mod.rs` + 14 カテゴリファイル）
- **完了条件**: 全テストファイルが 1000 行以下。全 291 テスト pass
- **依存**: なし

### T8: `types/tests.rs` テスト分割

- **作業内容**: `src/transformer/types/tests.rs`（3333 行）を `tests/` ディレクトリに分割（`primitives.rs`, `interfaces.rs`, `unions.rs`, `aliases.rs`, `utility_types.rs`, `advanced.rs`）
- **完了条件**: 全テストファイルが 1000 行以下。全 130 テスト pass
- **依存**: なし

### T9: `transformer/tests.rs` テスト分割

- **作業内容**: `src/transformer/tests.rs`（1335 行）を `tests/` ディレクトリに分割（`imports.rs`, `exports.rs`, `types.rs`, `classes.rs`, `enums.rs`, `functions.rs`）
- **完了条件**: 全テストファイルが 1000 行以下。全 57 テスト pass
- **依存**: なし

### T10: `generator/` テスト抽出

- **作業内容**: `src/generator/mod.rs`（836 行テスト）、`src/generator/expressions.rs`（783 行テスト）、`src/generator/statements.rs`（780 行テスト）のインラインテストを別ファイルに抽出する。`expressions.rs` と `statements.rs` は `.rs` → ディレクトリ化（`mod.rs` + `tests.rs`）
- **完了条件**: 全ファイルが 1000 行以下。全テスト pass
- **依存**: なし

### T11: `ir.rs` テスト抽出

- **作業内容**: `src/ir.rs`（495 行テスト）のインラインテストを別ファイルに抽出する。`ir.rs` → `ir/mod.rs` + `ir/tests.rs`
- **完了条件**: `ir/mod.rs` が 793 行（1000 行以下）。全テスト pass
- **依存**: なし

### T12: `pipeline/` テスト抽出

- **作業内容**: `src/external_types.rs`（676 行テスト）、`src/pipeline/module_graph.rs`（472 行テスト）、`src/pipeline/external_struct_generator.rs`（765 行テスト）のインラインテストを別ファイルに抽出する。各ファイルをディレクトリ化（`mod.rs` + `tests.rs`）
- **完了条件**: 全ファイルが 1000 行以下。全テスト pass
- **依存**: なし

### T13: 最終検証

- **作業内容**: 全ファイルの行数が 1000 行以下であることを検証する。`cargo test` 全 pass、`cargo clippy --all-targets --all-features -- -D warnings` 0 警告、`cargo fmt --all --check` pass を確認する
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
