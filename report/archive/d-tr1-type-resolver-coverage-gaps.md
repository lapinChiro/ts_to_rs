# D-TR-1: TypeResolver カバレッジギャップ調査

**基準コミット**: `6cf059c`
**調査日**: 2026-03-22

## 概要

TypeResolver のカバレッジ不足により、3 つのフォールバックが残存している:

1. **resolve_expr_type_heuristic** — 式の型解決 (`src/transformer/expressions/type_resolution.rs:46`)
2. **ExprContext** — expected_type の伝搬 (`src/transformer/expressions/mod.rs:42`)
3. **TypeEnv narrowing** — スコープベースの narrowing (`src/transformer/type_env.rs:74-84`)

各フォールバックを無効化してテストを実行し、カバレッジギャップを特定した。

## 調査結果サマリ

| フォールバック | 無効化方法 | 失敗テスト数 | 実際のギャップ |
|---|---|---|---|
| resolve_expr_type_heuristic | 常に None を返す | 50 | **大半はテスト構造の問題** |
| ExprContext | with_expected → None | 47 | **expected_types の伝搬不足** |
| TypeEnv narrowing | push_scope/pop_scope を no-op | 4 | **TypeEnv 自体のユニットテストのみ** |

## 詳細分析

### 1. resolve_expr_type_heuristic（50 テスト失敗）

#### 重要な発見: テスト構造の問題

heuristic 無効化で失敗する 50 テストの大多数は、**テストが TypeResolver を使わない**（空の `FileTypeResolution` を `TransformContext` に渡している）ため、heuristic に依存している。

TypeResolver の `resolve_expr` (`type_resolver.rs:568-685`) は heuristic (`type_resolution.rs:46-99`) と同等以上のパターンを既にカバーしている:

| AST パターン | TypeResolver | heuristic | 差異 |
|---|---|---|---|
| Ident → 変数型 | `lookup_var` (scope chain) | `type_env.get` | 同等 |
| Lit (Str/Num/Bool) | ✅ | ✅ | 同等 |
| Binary expr | ✅ (詳細な演算子分岐) | ✅ | TypeResolver の方が完全 |
| Member access | ✅ (TypeRegistry lookup) | ✅ | 同等 |
| Indexed access (Vec/Tuple) | ✅ | ✅ | 同等 |
| Call expr | ✅ (scope + registry) | ✅ | 同等 |
| New expr | ✅ | ✅ | 同等 |
| TsAs / type assertion | ✅ | ✅ | 同等 |
| Paren delegation | ✅ | ✅ | 同等 |
| Template literal | ✅ | ✅ | 同等 |
| Assign expr | ✅ (+ mutability tracking) | ❌ | TypeResolver のみ |
| Cond expr (ternary) | ✅ | ❌ | TypeResolver のみ |
| Unary expr | ✅ | ❌ | TypeResolver のみ |
| Await expr | ✅ | ❌ | TypeResolver のみ |
| TsNonNull | ✅ | ❌ | TypeResolver のみ |
| Array expr | ✅ | ❌ | TypeResolver のみ |
| Arrow/Fn expr | ✅ | ❌ | TypeResolver のみ |

**結論**: TypeResolver の `resolve_expr` は heuristic を完全にスーパーセットとしてカバーしている。heuristic が「必要」に見える原因は、テストが TypeResolver を経由しないことにある。

#### heuristic-only の失敗テスト分類（44 件: heuristic のみで失敗、ExprContext では失敗しない）

| カテゴリ | テスト数 | TypeResolver 対応状況 |
|---|---|---|
| Identifier lookup (TypeEnv) | 1 | ✅ TypeResolver の scope chain で対応済み |
| Literal type inference | 1 | ✅ 対応済み |
| Binary operator type | 3 | ✅ 対応済み |
| Member access (TypeRegistry) | 4 | ✅ 対応済み |
| Indexed/Tuple access | 4 | ✅ 対応済み |
| Call return type | 3 | ✅ 対応済み |
| New expr type | 1 | ✅ 対応済み |
| Type assertion | 1 | ✅ 対応済み |
| Paren delegation | 1 | ✅ 対応済み |
| Typeof + 変数型解決 | 8 | ⚠️ typeof 判定ロジックは Transformer 側にあり、TypeResolver は変数型のみ提供 |
| Optional chain + 変数型 | 3 | ⚠️ TypeResolver は Unknown を返す（`type_resolver.rs:655-663`） |
| Binary with enum type | 2 | ✅ 変数型は対応済み（enum 判定は Transformer 側） |
| Unary plus + 変数型 | 1 | ✅ 変数型は対応済み |
| In operator + struct 型 | 2 | ✅ 変数型は対応済み（field 判定は Transformer 側） |
| DU patterns | 3 | ✅ 変数型は対応済み（DU 判定は Transformer 側） |
| Conditional assign | 2 | ✅ call return type は対応済み |
| Object destructuring | 1 | ✅ 変数型は対応済み |
| Fallback テスト自体 | 3 | N/A（heuristic のフォールバック動作をテスト） |

#### TypeResolver に残る実際のギャップ

1. **Optional chain (`x?.y`)**: TypeResolver は Unknown を返す (`type_resolver.rs:655-663`)。base の型から field 型を解決すべき
2. **heuristic フォールバックテスト 3 件**: `test_resolve_expr_type_falls_back_when_resolution_unknown`, `test_resolve_expr_type_falls_back_when_span_not_in_resolution`, `test_resolve_expr_type_narrowing_from_file_resolution_overrides_type_env` — これらは heuristic の存在自体をテストしているため、heuristic 削除時にテスト自体を削除/書き換えが必要

### 2. ExprContext（47 テスト失敗）

#### 真のカバレッジギャップ: expected_type の再帰的伝搬

TypeResolver が設定する `expected_types` は以下の 3 パターンのみ:

1. **変数宣言の型注釈 → 初期化式** (`type_resolver.rs:211-213`)
2. **関数 return type → return 文** (`type_resolver.rs:346-348`)
3. **関数パラメータ型 → 呼び出し引数** (`type_resolver.rs:814-837`)

一方、ExprContext は **再帰的に** expected type を伝搬する。TypeResolver がカバーしていない伝搬パターン:

| 伝搬パターン | ExprContext の設定箇所 | テスト数 | TypeResolver 対応 |
|---|---|---|---|
| Object literal → struct 名 | `expressions/mod.rs` convert_expr → object lit 分岐 | 14 | ❌ 未対応 |
| Array literal → 要素型 | `expressions/mod.rs` convert_expr → array lit 分岐 | 6 | ❌ 未対応 |
| Option<T> → inner T への unwrap 再帰 | `expressions/mod.rs:90-112` | 1 | ❌ 未対応 |
| String 期待 → `.to_string()` 付与 | `expressions/mod.rs:124` convert_lit | 2 | ❌ 未対応（注: TypeResolver が expected 設定すれば Transformer 側で処理可能） |
| Enum 期待 → string literal → variant | `expressions/mod.rs` convert_lit | 2 | ❌ 未対応 |
| 変数宣言 → 初期化式 | `statements/mod.rs:250-255` | 8 | ✅ 対応済み |
| Return 文 → 返り値 | `statements/mod.rs:57-60` | 6 | ✅ 対応済み |
| 関数引数 → 引数式 | `expressions/` 各所 | 5 | ⚠️ 部分対応（registry 関数のみ。TypeEnv の Fn 型は未対応） |

#### ExprContext-only の失敗テスト分類（41 件）

| カテゴリ | テスト数 | TypeResolver expected_types で解決可能か |
|---|---|---|
| Object literal struct 名推定 | 14 | ✅ variable annotation / param type から object literal span に expected 設定可能 |
| Return statement | 6 | ✅ 対応済み（テスト構造の問題） |
| Array element type | 6 | ✅ container expected type から element span に expected 設定可能 |
| Function parameter | 5 | ⚠️ TypeEnv の Fn 型からの param 解決が未対応 |
| Variable declaration | 8 | ✅ 対応済み（テスト構造の問題） |
| String coercion | 2 | ✅ expected_type 設定で Transformer 側が処理 |
| Enum variant conversion | 2 | ✅ expected_type 設定で Transformer 側が処理 |
| Option wrapping | 1 | ✅ expected_type 設定で Transformer 側が処理 |

### 3. TypeEnv narrowing（4 テスト失敗）

**結論: FileTypeResolution の narrowing_events がほぼ完全にカバー済み。**

失敗した 4 テストは全て TypeEnv 自体のユニットテスト（`test_type_env_nested_scopes_three_levels` 等）であり、変換ロジックのテストではない。

TypeResolver の `detect_narrowing_guard` (`type_resolver.rs:450-496`) が以下のパターンを検出:
- `typeof x === "string"` → x を String にナローイング
- `x !== null` → Option<T> を T にナローイング
- `x instanceof Foo` → x を Foo にナローイング

これらが `FileTypeResolution.narrowed_type()` (`type_resolution.rs:118`) として提供され、heuristic 内で TypeEnv より優先参照される (`type_resolution.rs:56-59`)。

### 4. 両方で失敗するテスト（6 件）

| テスト | 原因 |
|---|---|
| test_binary_number_plus_string_generates_format | 式の型解決(heuristic) + 期待型からの string concat 判定(ExprContext) の両方が必要 |
| test_convert_method_call_string_arg_gets_to_string_with_registry | method のパラメータ型からの expected 伝搬(ExprContext) + 引数の型解決(heuristic) |
| test_convert_nullish_coalescing_rhs_string_gets_to_string_when_lhs_is_option_string | 左辺の型解決(heuristic) + 右辺への expected 伝搬(ExprContext) |
| test_convert_opt_chain_method_call_propagates_param_types | opt chain の型解決(heuristic) + method param expected(ExprContext) |
| test_convert_switch_case_propagates_discriminant_type_for_string_enum | discriminant の型解決(heuristic) + case 値への expected(ExprContext) |
| test_convert_switch_discriminated_union_to_enum_match | DU 型の解決(heuristic) + case body の expected(ExprContext) |

## TypeResolver が対応すべき改善項目

### 優先度 High: D-TR-2 (expr_types)

| # | パターン | 現状 | 対応方針 |
|---|---|---|---|
| E-1 | Optional chain (`x?.y`) | Unknown を返す | base 型の field 型を解決して返す |
| E-2 | Method call の param expected | registry 関数のみ | method の param types も expected に設定 |

### 優先度 High: D-TR-3 (expected_types)

| # | パターン | 現状 | 対応方針 |
|---|---|---|---|
| X-1 | Object literal に struct 名を伝搬 | ❌ | variable annotation / param / return の expected を object literal span に設定。再帰的に field span にも設定 |
| X-2 | Array literal に要素型を伝搬 | ❌ | Vec<T> expected を受けたら各要素 span に T を設定。Tuple の場合は各要素に対応する型を設定 |
| X-3 | Option<T> → inner T への再帰 | ❌ | Option<T> expected を受けた non-null literal に T を expected として設定 |
| X-4 | TypeEnv の Fn 型から param expected | ❌ | scope 内の Fn 型変数呼び出し時に param types を args に設定 |
| X-5 | Method call の param expected | ❌ | obj 型から method signature を引き、param types を args に設定 |
| X-6 | switch case の discriminant 型伝搬 | ❌ | switch の discriminant 型を各 case test の expected に設定 |

### 優先度 Low: D-TR-4 (narrowing_events)

TypeEnv narrowing は FileTypeResolution で既にカバー済み。追加の narrowing_events は不要。

## 削除計画の見直し

### D-TR 後のフォールバック削除判断

1. **resolve_expr_type_heuristic (D4)**: TypeResolver の `resolve_expr` が完全にスーパーセットであるため、**テストを TypeResolver 経由に書き換えれば heuristic は削除可能**。ただし Optional chain (E-1) の改善が前提。

2. **ExprContext (D2)**: TypeResolver の `expected_types` に X-1 〜 X-6 を実装する必要がある。**最大のギャップは object literal への struct 名伝搬 (X-1)**。これは再帰的な伝搬が必要で、TypeResolver の AST walk に「現在の expected type」を持ち回る仕組みが必要。

3. **TypeEnv narrowing (D3)**: **即座に削除可能**（push_scope/pop_scope の narrowing 用途のみ削除。変数型追跡用途は維持）。ただし、heuristic 内で TypeEnv.get() をフォールバックとして使っているため、heuristic 削除 (D4) と同時に行う。

### 推奨実行順序

```
D-TR-2: Optional chain の expr_types 改善 (E-1)
    ↓
D-TR-3: expected_types の再帰的伝搬 (X-1 〜 X-6)
    ↓
D-TR-verify: テストを TypeResolver 経由に書き換え + heuristic 無効化で全 GREEN
    ↓
D3: TypeEnv narrowing 削除（即座に可能だが D4 と同時が効率的）
    ↓
D4: resolve_expr_type_heuristic 削除
    ↓
D2: ExprContext 削除
```

## 全失敗テスト一覧

### heuristic 無効化で失敗（50 件）

<details>
<summary>テスト一覧</summary>

#### heuristic-only（44 件: ExprContext では失敗しない）

1. `transformer::expressions::tests::test_resolve_expr_type_ident_registered_returns_type`
2. `transformer::expressions::tests::test_resolve_expr_type_number_literal_returns_f64`
3. `transformer::expressions::tests::test_resolve_expr_type_comparison_returns_bool`
4. `transformer::expressions::tests::test_resolve_expr_type_equality_returns_bool`
5. `transformer::expressions::tests::test_resolve_expr_type_logical_and_returns_operand_type`
6. `transformer::expressions::tests::test_resolve_expr_type_member_field_found_returns_field_type`
7. `transformer::expressions::tests::test_resolve_expr_type_member_chain_returns_nested_type`
8. `transformer::expressions::tests::test_resolve_expr_type_member_option_named_returns_field_type`
9. `transformer::expressions::tests::test_resolve_expr_type_paren_delegates_to_inner`
10. `transformer::expressions::tests::test_resolve_expr_type_ts_as_returns_target_type`
11. `transformer::expressions::tests::test_resolve_expr_type_tuple_index_returns_element_type`
12. `transformer::expressions::tests::test_resolve_expr_type_index_vec_returns_element_type`
13. `transformer::expressions::tests::test_resolve_expr_type_call_registry_fn_returns_return_type`
14. `transformer::expressions::tests::test_resolve_expr_type_call_registry_fn_no_return_type_returns_unit`
15. `transformer::expressions::tests::test_resolve_expr_type_call_fn_type_in_env_returns_return_type`
16. `transformer::expressions::tests::test_resolve_expr_type_new_registered_returns_named_type`
17. `transformer::expressions::type_resolution::tests::test_resolve_expr_type_falls_back_when_resolution_unknown`
18. `transformer::expressions::type_resolution::tests::test_resolve_expr_type_falls_back_when_span_not_in_resolution`
19. `transformer::expressions::type_resolution::tests::test_resolve_expr_type_narrowing_from_file_resolution_overrides_type_env`
20. `transformer::expressions::tests::test_convert_bin_expr_enum_var_eq_string_literal_converts_rhs`
21. `transformer::expressions::tests::test_convert_bin_expr_string_literal_ne_enum_var_converts_lhs`
22. `transformer::expressions::tests::test_convert_expr_unary_plus_string_returns_parse`
23. `transformer::expressions::tests::test_convert_member_expr_tuple_literal_index_generates_field_access`
24. `transformer::expressions::tests::test_convert_member_expr_tuple_second_index_generates_field_access`
25. `transformer::expressions::tests::test_convert_member_expr_discriminant_field_to_method_call`
26. `transformer::expressions::tests::test_convert_nullish_coalescing_non_option_returns_left`
27. `transformer::expressions::tests::test_convert_opt_chain_nested_option_uses_and_then`
28. `transformer::expressions::tests::test_convert_opt_chain_non_option_type_returns_plain_access`
29. `transformer::expressions::tests::test_convert_typeof_option_type_returns_runtime_if`
30. `transformer::expressions::tests::test_convert_typeof_static_number_returns_string_lit`
31. `transformer::expressions::tests::test_typeof_equals_string_known_type_resolves_true`
32. `transformer::expressions::tests::test_typeof_equals_number_known_type_resolves_true`
33. `transformer::expressions::tests::test_typeof_equals_string_mismatched_type_resolves_false`
34. `transformer::expressions::tests::test_typeof_not_equals_string_known_type_resolves_false`
35. `transformer::expressions::tests::test_typeof_equals_undefined_option_resolves_is_none`
36. `transformer::expressions::tests::test_typeof_standalone_known_type_resolves_string_lit`
37. `transformer::expressions::tests::test_convert_du_standalone_field_access_generates_match_expr`
38. `transformer::expressions::tests::test_in_operator_struct_field_exists_generates_true`
39. `transformer::expressions::tests::test_in_operator_struct_field_missing_generates_false`
40. `transformer::statements::tests::test_cond_assign_if_option_type_generates_if_let_some`
41. `transformer::statements::tests::test_cond_assign_while_option_type_generates_while_let_some`
42. `transformer::statements::tests::test_convert_du_switch_field_access_single_field_becomes_binding`
43. `transformer::statements::tests::test_convert_du_switch_field_access_multiple_fields_become_bindings`
44. `transformer::statements::tests::test_object_destructuring_rest_with_type_expands_remaining_fields`

#### 両方で失敗（6 件）

45. `transformer::expressions::tests::test_binary_number_plus_string_generates_format`
46. `transformer::expressions::tests::test_convert_method_call_string_arg_gets_to_string_with_registry`
47. `transformer::expressions::tests::test_convert_nullish_coalescing_rhs_string_gets_to_string_when_lhs_is_option_string`
48. `transformer::expressions::tests::test_convert_opt_chain_method_call_propagates_param_types`
49. `transformer::statements::tests::test_convert_switch_case_propagates_discriminant_type_for_string_enum`
50. `transformer::statements::tests::test_convert_switch_discriminated_union_to_enum_match`

</details>

### ExprContext 無効化で失敗（47 件）

<details>
<summary>テスト一覧</summary>

#### ExprContext-only（41 件: heuristic では失敗しない）

1. `transformer::classes::tests::test_convert_static_prop_propagates_type_annotation`
2. `transformer::context::tests::test_expr_type_unknown_fallback_to_heuristics`
3. `transformer::expressions::tests::test_convert_array_lit_elements_get_expected_element_type`
4. `transformer::expressions::tests::test_convert_assign_expr_propagates_type_from_type_env`
5. `transformer::expressions::tests::test_convert_bin_expr_expected_string_enables_concat`
6. `transformer::expressions::tests::test_convert_call_args_string_literal_to_enum_variant`
7. `transformer::expressions::tests::test_convert_call_expr_typeenv_fn_provides_param_expected`
8. `transformer::expressions::tests::test_convert_expr_array_nested_vec_string_expected`
9. `transformer::expressions::tests::test_convert_expr_array_string_with_vec_string_expected`
10. `transformer::expressions::tests::test_convert_expr_array_with_tuple_expected_generates_tuple`
11. `transformer::expressions::tests::test_convert_expr_call_resolves_object_arg_from_registry`
12. `transformer::expressions::tests::test_convert_expr_nested_array_with_vec_tuple_expected`
13. `transformer::expressions::tests::test_convert_expr_object_literal_empty`
14. `transformer::expressions::tests::test_convert_expr_object_literal_mixed_field_types`
15. `transformer::expressions::tests::test_convert_expr_object_literal_nested_resolves_field_type_from_registry`
16. `transformer::expressions::tests::test_convert_expr_object_literal_single_field`
17. `transformer::expressions::tests::test_convert_expr_object_literal_with_type_hint_basic`
18. `transformer::expressions::tests::test_convert_expr_object_shorthand_mixed_with_key_value`
19. `transformer::expressions::tests::test_convert_expr_object_shorthand_single`
20. `transformer::expressions::tests::test_convert_expr_object_shorthand_with_registry_field_type`
21. `transformer::expressions::tests::test_convert_expr_object_spread_last_position_expands_remaining_fields`
22. `transformer::expressions::tests::test_convert_expr_object_spread_middle_position_expands_remaining_fields`
23. `transformer::expressions::tests::test_convert_expr_object_spread_with_override`
24. `transformer::expressions::tests::test_convert_expr_string_lit_with_string_expected_adds_to_string`
25. `transformer::expressions::tests::test_convert_hashmap_propagates_value_type`
26. `transformer::expressions::tests::test_convert_lit_string_to_enum_variant_when_expected_is_string_literal_union`
27. `transformer::expressions::tests::test_convert_object_lit_discriminated_union_to_enum_variant`
28. `transformer::expressions::tests::test_convert_object_lit_discriminated_union_unit_variant`
29. `transformer::expressions::tests::test_convert_object_spread_multiple_registered_generates_merged_fields`
30. `transformer::expressions::tests::test_convert_object_spread_unregistered_type_generates_struct_update`
31. `transformer::expressions::tests::test_convert_opt_chain_method_call_propagates_param_types`
32. `transformer::expressions::tests::test_new_expr_string_arg_gets_to_string`
33. `transformer::expressions::tests::test_object_lit_omitted_optional_field_gets_none`
34. `transformer::expressions::tests::test_option_expected_wraps_literal_in_some`
35. `transformer::expressions::tests::test_self_field_string_concat_gets_clone`
36. `transformer::functions::tests::test_convert_fn_decl_throw_wraps_return_in_ok`
37. `transformer::statements::tests::test_convert_stmt_return_string_with_string_return_type`
38. `transformer::statements::tests::test_convert_stmt_var_decl_object_literal_with_type_annotation`
39. `transformer::statements::tests::test_convert_stmt_var_decl_string_array_type_annotation`
40. `transformer::statements::tests::test_convert_stmt_var_decl_string_type_annotation_adds_to_string`
41. `transformer::tests::test_transform_var_type_alias_arrow_propagates_return_type`
42. `transformer::tests::test_transform_var_type_arrow_propagates_return_type`

#### 両方で失敗（6 件: 上記と同じ）

</details>

### TypeEnv narrowing 無効化で失敗（4 件）

全て TypeEnv 自体のユニットテスト:

1. `transformer::tests::test_type_env_nested_scopes_three_levels`
2. `transformer::tests::test_type_env_pop_scope_removes_child_variables`
3. `transformer::tests::test_type_env_shadow_in_child_scope_hides_parent`
4. `transformer::tests::test_type_env_update_nonexistent_inserts_in_current_scope`
