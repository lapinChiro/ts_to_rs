# 低カバレッジ7ファイルのテスト観点補完（81観点）

## Background

CI のカバレッジ閾値 89% に対し、実測 88.61% で CI が失敗している。テスト自体は全て通過（1276 passed; 0 failed）しており、純粋にカバレッジ不足が原因。

調査の結果、カバレッジが特に低い7ファイル全てに共通して **ユニットテストが完全に欠如** していることが判明した。これらのカバレッジは全てスナップショット/E2E テスト経由の間接的なものであり、以下のリスクがある：

1. **サイレントな意味変更を検出できない**: `extract_narrowing_guard` のキーワード除外ロジック等、間接テストでは到達しにくい分岐がある
2. **リファクタリング耐性が低い**: 間接テストは内部構造の変更でテスト対象の分岐を通らなくなる可能性がある
3. **エラーパスが未検証**: `Result::Err` を返す防御的分岐や `None` を返す early return が直接テストされていない

テストケース設計技法（同値分割、境界値分析、分岐網羅 C1、デシジョンテーブル、AST バリアント網羅）を適用した体系的レビューにより、81の不足テスト観点を特定した。

## Goal

- 7ファイル全てにユニットテストを追加し、特定した81テスト観点を全てカバーする
- `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` が通る
- 全テストが `test_<target>_<condition>_<expected_result>` 命名規約に従う
- 0 errors, 0 warnings を維持

## Scope

### In Scope

- 7ファイルに対するユニットテスト追加（81観点）
- 既存テストとの重複がないことの確認
- テスト追加に必要なヘルパー関数の追加（既存パターンに準拠）

### Out of Scope

- 実装コードの変更（テスト追加のみ）
- スナップショットテストや E2E テストの追加
- 7ファイル以外のカバレッジ改善
- カバレッジ閾値のラチェットアップ（89% → 90% 等は別途判断）

## Design

### Technical Approach

既存のテストパターンに完全に準拠する：

- **Transformer メソッドのテスト**: `src/transformer/{module}/tests/` 配下のサブモジュールに追加。`TctxFixture` + `parse_*` ヘルパーを使用し、SWC パーサー経由で AST を構築
- **純粋関数のテスト**: 同ファイル内の `#[cfg(test)] mod tests` に追加（`extract_narrowing_guard`, `collect_du_field_accesses_from_stmts`, `generate_truthiness_condition` 等）
- **TypeConverter のテスト**: `src/pipeline/type_converter/tests.rs` に追加。`parse_type_annotation` + `build_registry` パターンを使用
- **TypeResolver のテスト**: `src/pipeline/type_resolver/tests/` 配下に新規サブモジュール追加。`resolve()` / `resolve_with_reg()` ヘルパーを使用
- **Registry のテスト**: `src/registry/tests/` 配下に新規サブモジュール追加

### Design Integrity Review

- **Higher-level consistency**: テストコードのみの追加であり、既存のテスト配置規約（`testing.md`）に準拠。パイプラインの依存方向に影響しない
- **DRY / Orthogonality / Coupling**: 各テストファイル内で共通のヘルパー関数を定義し、テスト間の重複を排除。ただし、テストファイル間でのヘルパー共有は coupling 増加を避けるため行わない（既存パターンと同じ）
- **Broken windows**: 既存テストで発見された問題はないが、7ファイル全てにユニットテストが欠如していること自体が broken window である。本 PRD で解消する

Verified, no issues beyond the broken window (missing tests) being fixed by this PRD.

### Impact Area

テスト追加先（新規作成または既存ファイルへの追記）:

| 対象ソース | テスト配置先 | 操作 |
|---|---|---|
| `transformer/functions/destructuring.rs` | `transformer/functions/tests/destructuring.rs` | 既存ファイルに追記 |
| `transformer/classes/inheritance.rs` | `transformer/tests/classes.rs` | 既存ファイルに追記 |
| `transformer/statements/helpers.rs` | `transformer/statements/tests/helpers.rs` | 新規作成 + `mod.rs` に `mod helpers;` 追加 |
| `transformer/expressions/patterns.rs` | `transformer/expressions/tests/type_guards.rs` | 既存ファイルに追記 |
| `pipeline/type_converter/type_aliases.rs` | `pipeline/type_converter/tests.rs` | 既存ファイルに追記 |
| `pipeline/type_resolver/du_analysis.rs` | `pipeline/type_resolver/tests/du_analysis.rs` | 新規作成 + `mod.rs` に `mod du_analysis;` 追加 |
| `registry/interfaces.rs` | `registry/tests/interfaces.rs` | 新規作成 + `mod.rs` に `mod interfaces;` 追加 |

## Task List

### T1: transformer/statements/helpers.rs のテスト（9観点）

- **Work**: `src/transformer/statements/tests/helpers.rs` を新規作成し、`src/transformer/statements/tests/mod.rs` に `mod helpers;` を追加。以下の9観点のテストを実装：

  **`extract_conditional_assignment` (5観点)**:
  1. `test_extract_conditional_assignment_bare_assignment_returns_some` — `if (x = expr)` で `var_name` と `rhs` を検証
  2. `test_extract_conditional_assignment_comparison_with_left_assign_returns_outer` — `(x = expr) > 0` で `assign_on_left: true` を検証
  3. `test_extract_conditional_assignment_comparison_with_right_assign_returns_outer` — `0 < (x = expr)` で `assign_on_left: false` を検証
  4. `test_extract_conditional_assignment_no_assignment_returns_none` — `x > 0` で `None` を検証
  5. `test_extract_conditional_assignment_nested_parens_unwraps` — `(((x = expr)))` でパーレン展開を検証

  **`generate_truthiness_condition` / `generate_falsy_condition` (4観点)**:
  6. `test_generate_truthiness_condition_f64_generates_not_eq_zero` — `F64` → `var != 0.0`
  7. `test_generate_truthiness_condition_string_generates_not_is_empty` — `String` → `!var.is_empty()`
  8. `test_generate_truthiness_condition_bool_generates_ident` — `Bool` → `var`
  9. `test_generate_falsy_condition_is_inverse_of_truthiness` — 全型について truthiness と falsy が論理的に逆であることを検証（対称性テスト）

- **Completion criteria**: 9テスト全てパス。`helpers.rs` のカバレッジが 65% → 90%+ に改善
- **Depends on**: なし
- **Prerequisites**: なし

### T2: transformer/expressions/patterns.rs のテスト（20観点）

- **Work**: `src/transformer/expressions/tests/type_guards.rs` に以下のテストを追記。既存の type guard テスト群の末尾に追加：

  **`try_convert_undefined_comparison` (4観点)**:
  1. `test_undefined_comparison_reversed_order_right_undefined_returns_is_none` — `undefined === x` （逆順）
  2. `test_undefined_comparison_reversed_order_neq_returns_is_some` — `undefined !== x`（逆順 neq）
  3. `test_undefined_comparison_non_equality_op_returns_none` — `x > undefined` → None
  4. `test_undefined_comparison_neither_side_undefined_returns_none` — `x === y` → None

  **`convert_in_operator` (5観点)**:
  5. `test_in_operator_hashmap_generates_contains_key` — HashMap 型に対して `contains_key()` 生成
  6. `test_in_operator_struct_known_field_returns_true` — struct に field が存在 → `true`
  7. `test_in_operator_struct_unknown_field_returns_false` — struct に field なし → `false`
  8. `test_in_operator_enum_tag_field_returns_true` — DU enum の tag field → `true`
  9. `test_in_operator_non_string_key_returns_todo` — 非文字列キー → `todo!()`

  **`convert_instanceof` (5観点)**:
  10. `test_instanceof_matching_type_returns_true` — `x instanceof Foo` で x の型が Foo → `true`
  11. `test_instanceof_non_matching_type_returns_false` — 型が異なる → `false`
  12. `test_instanceof_option_matching_returns_is_some` — `Option<Foo>` で `instanceof Foo` → `is_some()`
  13. `test_instanceof_option_non_matching_returns_false` — `Option<Bar>` で `instanceof Foo` → `false`
  14. `test_instanceof_unknown_type_returns_todo` — 型不明 → `todo!()`

  **`extract_narrowing_guard` (6観点)** — 純粋関数のため `patterns.rs` 内の `#[cfg(test)] mod tests` に配置:
  15. `test_extract_narrowing_guard_typeof_returns_typeof_guard` — `typeof x === "string"` → `Typeof`
  16. `test_extract_narrowing_guard_null_check_returns_non_nullish` — `x !== null` → `NonNullish(is_neq=true)`
  17. `test_extract_narrowing_guard_reversed_null_returns_non_nullish` — `null !== x` → `NonNullish`
  18. `test_extract_narrowing_guard_instanceof_returns_instanceof_guard` — `x instanceof Foo` → `InstanceOf`
  19. `test_extract_narrowing_guard_keyword_ident_returns_none` — `undefined` / `true` / `false` → None（**サイレント意味変更防止**）
  20. `test_extract_narrowing_guard_non_bin_non_ident_returns_none` — 数値リテラル等 → None

- **Completion criteria**: 20テスト全てパス。`patterns.rs` のカバレッジが 70% → 85%+ に改善
- **Depends on**: なし
- **Prerequisites**: なし

### T3: transformer/functions/destructuring.rs のテスト（10観点）

- **Work**: `src/transformer/functions/tests/destructuring.rs` に以下のテストを追記：

  **デフォルト値の3分岐 (3観点)**:
  1. `test_object_destructuring_param_default_string_lit_generates_unwrap_or_else` — `{ x = "hello" }` → `unwrap_or_else`
  2. `test_object_destructuring_param_default_to_string_generates_unwrap_or_else` — `{ x = val.toString() }` → `unwrap_or_else`（to_string パス）
  3. `test_object_destructuring_param_default_other_generates_unwrap_or` — `{ x = 42 }` → `unwrap_or`（既存テストと重複確認の上、不足分を追加）

  **ネストと rest パターン (4観点)**:
  4. `test_object_destructuring_param_nested_object_generates_recursive_expansion` — `{ a: { b, c } }: T` → 再帰的フィールドアクセス展開
  5. `test_object_destructuring_param_rest_generates_synthetic_struct` — `{ x, ...rest }: Point` → 合成構造体初期化
  6. `test_object_destructuring_param_rest_excludes_explicit_fields` — rest の remaining_fields に明示フィールドが含まれない
  7. `test_object_destructuring_param_rest_unknown_type_returns_error` — registry に型がない → UnsupportedSyntaxError

  **`lookup_field_type` (3観点)**:
  8. `test_object_destructuring_param_nested_with_known_type_resolves_field_types` — `lookup_field_type` が `Named` 型から正しくフィールド型を取得
  9. `test_object_destructuring_param_nested_option_type_unwraps_inner` — `Option<Named>` 型のアンラップ
  10. `test_object_destructuring_param_nested_unknown_type_skips_field_lookup` — 非 Named/Option 型 → field type が None（フォールバック動作を検証）

- **Completion criteria**: 10テスト全てパス。`destructuring.rs` のカバレッジが 61% → 85%+ に改善
- **Depends on**: なし
- **Prerequisites**: `reg_with_outer_inner()` 等の既存ヘルパーを活用可能

### T4: transformer/classes/inheritance.rs のテスト（8観点）

- **Work**: `src/transformer/tests/classes.rs` に以下のテストを追記：

  **`rewrite_super_constructor` (4観点)** — テスト用ヘルパーとして `ClassInfo` と `Method` を直接構築：
  1. `test_rewrite_super_constructor_merges_into_tail_struct_init` — body に `TailExpr(StructInit)` がある場合、super fields がマージされる
  2. `test_rewrite_super_constructor_merges_into_return_struct_init` — body に `Return(Some(StructInit))` がある場合のマージ
  3. `test_rewrite_super_constructor_no_struct_init_creates_new` — body に StructInit がない場合、super fields で新規 StructInit 作成
  4. `test_rewrite_super_constructor_no_super_call_preserves_body` — super() 呼び出しなし → body がそのまま保持

  **`transform_class_with_inheritance` (4観点)** — `transform_module` 経由で統合テスト：
  5. `test_transform_class_abstract_generates_trait` — `abstract class` → Trait 生成
  6. `test_transform_class_parent_generates_trait_and_struct` — 親クラス（他のクラスに extends される）→ struct + trait + impl
  7. `test_transform_class_child_of_abstract_generates_impl_trait` — `extends AbstractClass` → impl AbstractClass for Child
  8. `test_transform_class_child_with_implements_generates_all` — `extends Parent implements Interface` → struct + impl + trait impl

- **Completion criteria**: 8テスト全てパス。`inheritance.rs` のカバレッジが 64% → 85%+ に改善
- **Depends on**: なし
- **Prerequisites**: なし

### T5: pipeline/type_converter/type_aliases.rs のテスト（15観点）

- **Work**: `src/pipeline/type_converter/tests.rs` に以下のテストを追記：

  **`convert_type_alias_items` — conditional type (3観点)**:
  1. `test_convert_type_alias_conditional_type_infer_pattern_generates_associated_type` — `type X<T> = T extends Promise<infer U> ? U : never` → `<T as Promise>::Output`
  2. `test_convert_type_alias_conditional_type_true_false_literal_generates_bool` — `type X<T> = T extends Y ? true : false` → `bool`
  3. `test_convert_type_alias_conditional_type_fallback_uses_true_branch` — 変換失敗時に true branch 型 + Comment を生成

  **`try_convert_keyof_typeof_alias` (3観点)**:
  4. `test_convert_type_alias_keyof_typeof_struct_generates_string_enum` — `type K = keyof typeof myStruct` → struct のフィールド名から enum 生成
  5. `test_convert_type_alias_keyof_typeof_enum_generates_string_enum` — `type K = keyof typeof myEnum` → enum の string_values から enum 生成
  6. `test_convert_type_alias_keyof_typeof_unknown_returns_none` — registry に型がない → 通常の型変換にフォールスルー

  **`convert_type_alias` — TsTypeLit 3-way 分類 (5観点)**:
  7. `test_convert_type_alias_call_signature_only_generates_fn_type` — `type F = { (x: string): number }` → TypeAlias(Fn)
  8. `test_convert_type_alias_methods_only_generates_trait` — `type T = { foo(): void; bar(): string }` → Trait
  9. `test_convert_type_alias_properties_generates_struct` — `type T = { x: number; y: string }` → Struct
  10. `test_convert_type_alias_index_signature_generates_hashmap` — `type T = { [key: string]: number }` → TypeAlias(HashMap)
  11. `test_convert_type_alias_index_signature_no_type_returns_error` — 型アノテーションなし index signature → Error

  **その他の型形式 (4観点)**:
  12. `test_convert_type_alias_function_type_generates_fn_alias` — `type F = (x: string) => number` → TypeAlias(Fn)
  13. `test_convert_type_alias_tuple_type_generates_tuple` — `type T = [string, number]` → TypeAlias(Tuple)
  14. `test_convert_type_alias_single_string_literal_generates_enum` — `type X = "only"` → Enum with 1 variant
  15. `test_convert_type_alias_unsupported_fn_param_pattern_returns_error` — 関数型パラメータが Ident 以外 → Error

- **Completion criteria**: 15テスト全てパス。`type_aliases.rs` のカバレッジが 73% → 85%+ に改善
- **Depends on**: なし
- **Prerequisites**: registry に struct/enum を事前登録するヘルパーが必要（keyof typeof テスト用）

### T6: pipeline/type_resolver/du_analysis.rs のテスト（12観点）

- **Work**: `src/pipeline/type_resolver/tests/du_analysis.rs` を新規作成し、`src/pipeline/type_resolver/tests/mod.rs` に `mod du_analysis;` を追加：

  **`detect_du_switch_bindings` (6観点)** — `resolve_with_reg()` 経由で TS ソースを解析し `du_field_bindings` を検証：
  1. `test_du_switch_bindings_basic_records_field_access` — `switch (s.kind) { case "circle": s.radius }` → `DuFieldBinding { var_name: "radius" }` が記録される
  2. `test_du_switch_bindings_non_member_discriminant_skips` — `switch (x)` → binding なし
  3. `test_du_switch_bindings_non_enum_type_skips` — discriminant の型が enum でない → binding なし
  4. `test_du_switch_bindings_tag_mismatch_skips` — enum の tag_field と discriminant のフィールド名が異なる → binding なし
  5. `test_du_switch_bindings_fall_through_accumulates_variants` — 空 body の fall-through で複数バリアントが蓄積される
  6. `test_du_switch_bindings_field_not_in_variant_skips` — variant に存在しないフィールドアクセス → binding に含まれない

  **`collect_du_field_accesses_from_stmts` (3観点)** — 純粋関数テスト（`du_analysis.rs` 内 `#[cfg(test)] mod tests` に配置）：
  7. `test_collect_du_field_accesses_member_access_collects_field` — `s.radius` → `["radius"]`
  8. `test_collect_du_field_accesses_tag_field_excluded` — `s.kind`（tag field）→ 収集されない（**サイレント意味変更防止**）
  9. `test_collect_du_field_accesses_deduplicates` — 同一フィールドの複数アクセス → 重複なし

  **`collect_du_field_accesses_from_expr_inner` — AST バリアント網羅 (3観点)**:
  10. `test_collect_du_field_accesses_nested_in_call_args` — `console.log(s.radius)` → call 引数内のアクセスを収集
  11. `test_collect_du_field_accesses_in_template_literal` — `` `${s.name}` `` → テンプレート内のアクセスを収集
  12. `test_collect_du_field_accesses_in_conditional_expr` — `cond ? s.a : s.b` → 両分岐のアクセスを収集

- **Completion criteria**: 12テスト全てパス。`du_analysis.rs` のカバレッジが 73% → 90%+ に改善
- **Depends on**: なし
- **Prerequisites**: `build_shape_registry()` ヘルパーが既存 (`type_resolver/tests/mod.rs`)。DU enum の TypeDef 構築が必要

### T7: registry/interfaces.rs のテスト（7観点）

- **Work**: `src/registry/tests/interfaces.rs` を新規作成し、`src/registry/tests/mod.rs` に `mod interfaces;` を追加：

  **`collect_interface_fields` (2観点)**:
  1. `test_collect_interface_fields_property_signatures_collected` — `interface I { x: number; y: string }` → `[("x", F64), ("y", String)]`
  2. `test_collect_interface_fields_non_property_members_skipped` — method signature 混在 → property のみ収集

  **`collect_interface_methods` (3観点)**:
  3. `test_collect_interface_methods_ident_param_with_type_collected` — `interface I { foo(x: string): number }` → params と return_type を検証
  4. `test_collect_interface_methods_rest_param_collected` — `interface I { foo(...args: string[]): void }` → rest param と `has_rest: true` を検証
  5. `test_collect_interface_methods_overload_accumulates` — 同名メソッドが `Vec` に蓄積される

  **`collect_property_signature` (2観点)**:
  6. `test_collect_property_signature_optional_wraps_in_option` — `x?: number` → `Option<F64>`
  7. `test_collect_property_signature_non_ident_key_returns_none` — computed key `[expr]: T` → `None`

- **Completion criteria**: 7テスト全てパス。`interfaces.rs` のカバレッジが 78% → 95%+ に改善
- **Depends on**: なし
- **Prerequisites**: `parse_typescript()` で interface 宣言を解析し、TsInterfaceDecl を抽出するヘルパーが必要

### T8: 品質検証とカバレッジ確認

- **Work**: 全テスト追加後に以下を実行：
  1. `cargo fix --allow-dirty --allow-staged`
  2. `cargo fmt --all --check`
  3. `cargo clippy --all-targets --all-features -- -D warnings`
  4. `cargo test` — 全テストパス確認
  5. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` — カバレッジ閾値クリア確認
  6. `./scripts/check-file-lines.sh` — ファイル行数制限チェック

- **Completion criteria**: 全コマンドが exit code 0。カバレッジ 89% 以上
- **Depends on**: T1, T2, T3, T4, T5, T6, T7
- **Prerequisites**: なし

## Test Plan

本 PRD 自体がテスト追加の PRD であるため、テスト計画 = タスクリストの各テスト観点。

### テスト設計技法の適用状況

| 技法 | 適用箇所 | 観点数 |
|---|---|---|
| **同値分割** | 全ファイルの入力型パーティション、AST バリアント分類 | 38 |
| **分岐網羅 (C1)** | 各関数の if/match/early return 分岐 | 22 |
| **デシジョンテーブル** | `transform_class_with_inheritance`、`convert_in_operator`、`convert_type_alias` | 12 |
| **境界値分析** | 空コレクション、fall-through 蓄積、overload 蓄積 | 5 |
| **対称性テスト** | truthiness/falsy 逆転、undefined 左右対称 | 4 |

### サイレント意味変更の検出テスト（最重要）

以下の3テストは、バグがあっても実行時まで検出できないサイレントな意味変更を防ぐ：

1. **T2-19**: `extract_narrowing_guard` のキーワード除外 — `undefined` を変数として narrowing するとサイレントに常時マッチ
2. **T6-8**: `collect_du_field_accesses` の tag field 除外 — tag field がバインディングに含まれるとパターン破壊
3. **T1-9**: `generate_falsy_condition` の対称性 — 型ごとの条件が truthiness と論理的に逆でないと制御フロー反転

## Completion Criteria

1. 81テスト観点が全てテストコードとして実装されている
2. `cargo test` で全テストパス（既存 + 新規）
3. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` が exit code 0
4. `cargo clippy --all-targets --all-features -- -D warnings` が exit code 0
5. `cargo fmt --all --check` が exit code 0
6. `./scripts/check-file-lines.sh` が exit code 0
7. 全テストが `test_<target>_<condition>_<expected_result>` 命名規約に従っている
