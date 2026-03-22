# Expected Type 二重伝搬の調査レポート

**基準コミット**: b361149（未コミットの Phase 2 変更を含む状態で調査）

## 要約

Phase 2（ExprContext 削除）の過程で、expected type の伝搬が 2 つの独立したルートに分裂した。TypeResolver の `propagate_expected` と Transformer の手動伝搬（`convert_expr_with_expected` 経由）が同じ知識を異なる場所に重複して保持しており、DRY 違反・テスト信頼性の低下・後続 Phase との不整合を引き起こしている。

## 背景: 2 つの伝搬ルート

### ルート 1: TypeResolver の `propagate_expected`（production pipeline で使用）

`src/pipeline/type_resolver.rs:599-681` に定義。AST を走査し、`FileTypeResolution.expected_types` マップに span → RustType のエントリを設定する。

**呼び出し元（6 箇所）:**

| 行 | コンテキスト | 伝搬内容 |
|---|---|---|
| 214 | `visit_var_decl`（変数宣言 + 型注釈） | 型注釈 → initializer |
| 322 | `visit_class_body`（クラスプロパティ + 型注釈） | 型注釈 → initializer |
| 358 | `visit_stmt`（return 文） | 関数の return type → return value |
| 717 | `resolve_expr`（代入文） | LHS 変数型 → RHS |
| 846 | `resolve_bin_expr`（`??` 演算子） | Option inner type → RHS |
| 994 | `set_call_arg_expected_types`（関数/メソッド呼び出し） | param type → argument |

**`propagate_expected` 内部パターン（5 つ + 再帰）:**

| パターン | 行 | トリガー | 子への伝搬 | 再帰 |
|---|---|---|---|---|
| P-1 | 601-638 | Object lit + Named struct | 各フィールド値に field type | Yes |
| P-3 | 641-650 | Array lit + Vec\<T\> | 各要素に T | Yes |
| P-4 | 641-657 | Array lit + Tuple | 各要素に positional type | Yes |
| P-5 | 661-665 | Paren expr | inner expr に同じ type | Yes |
| P-6 | 667-678 | Cond (ternary) | cons/alt 両方に同じ type | Yes |

### ルート 2: Transformer の手動伝搬（`convert_expr_with_expected` 経由）

Phase 2 で ExprContext 削除時に追加。各 Transformer 関数が `convert_expr_with_expected` を呼び出し、親コンテキストから導出した expected type を子要素に渡す。

**全 19 箇所の手動伝搬サイト:**

| # | ファイル:行 | 関数 | 導出元 | 子要素 |
|---|---|---|---|---|
| 1 | data_literals.rs:75 | convert_discriminated_union_object_lit | variant_fields_map → field type | DU KeyValue value |
| 2 | data_literals.rs:85-90 | 同上 | 同上 | DU Shorthand value |
| 3 | data_literals.rs:150-152 | try_convert_as_hashmap | expected HashMap → type_args[1] | HashMap value |
| 4 | data_literals.rs:242 | convert_object_lit | struct_fields → field type | Object KeyValue value |
| 5 | data_literals.rs:256 | 同上 | 同上 | Object Shorthand value |
| 6 | data_literals.rs:382-388 | convert_array_lit | Tuple → positional type | Tuple element |
| 7 | data_literals.rs:409-416 | 同上 | Vec\<T\> → T | Array element |
| 8 | data_literals.rs:482 | convert_spread_array_to_block | element_type param | Spread array element |
| 9 | calls.rs:589-591 | convert_call_args_with_types | param_types → param type | Regular call arg |
| 10 | calls.rs:660 | 同上 | rest param → Vec inner | Rest arg (mixed) |
| 11 | calls.rs:681 | 同上 | 同上 | Rest arg (literal) |
| 12 | member_access.rs:188-190 | opt chain handler | method_sig → param type | Opt chain method arg |
| 13 | assignments.rs:39-45 | convert_assign_expr | TypeEnv → target var type | Assignment RHS |
| 14 | binary.rs:74-81 | convert_bin_expr (??) | Option inner type | Nullish coalescing RHS |
| 15 | functions.rs:312 | convert_arrow_expr_with_return_type | return type annotation | Arrow expr body |
| 16 | functions.rs:337 | 同上 | 同上 | Arrow expr body (expanded) |
| 17 | statements/mod.rs:62 | convert_stmt (return) | return_type | Return value |
| 18 | statements/mod.rs:243 | convert_stmt (var decl) | type annotation | Initializer |
| 19 | classes.rs:583 | convert_static_prop | type annotation | Static prop initializer |

## 重複分析

### 完全に重複（7 箇所）

| 伝搬パターン | TypeResolver | Transformer |
|---|---|---|
| Object lit fields → struct field type | P-1 (601-638) | #4, #5 (data_literals.rs:242,256) |
| Array elements → Vec inner type | P-3 (641-650) | #7 (data_literals.rs:409) |
| Tuple elements → positional type | P-4 (641-657) | #6 (data_literals.rs:382) |
| Function call args → param type | set_call_arg_expected_types (994) | #9 (calls.rs:589) |
| Variable init → type annotation | visit_var_decl (214) | #18 (statements/mod.rs:243) |
| Assignment RHS → LHS type | resolve_expr (717) | #13 (assignments.rs:39) |
| Nullish coalescing RHS → Option inner | resolve_bin_expr (846) | #14 (binary.rs:74) |

### Transformer にのみ存在（TypeResolver のギャップ — 7 箇所）

| 伝搬パターン | Transformer | 影響 |
|---|---|---|
| DU object lit fields → variant field type | #1, #2 (data_literals.rs:75,85) | production で DU フィールド値に expected が設定されない |
| HashMap value → value type | #3 (data_literals.rs:150) | production で HashMap value に expected が設定されない |
| Spread array element → element type | #8 (data_literals.rs:482) | production で spread array 要素に expected が設定されない |
| Rest args → Vec element type | #10, #11 (calls.rs:660,681) | production で rest 引数に expected が設定されない |
| Opt chain method args → param type | #12 (member_access.rs:188) | production で opt chain メソッド引数に expected が設定されない |
| Arrow expr body → return type | #15, #16 (functions.rs:312,337) | production で arrow body に expected が設定されない |
| Return stmt → return type | #17 (statements/mod.rs:62) | visit_stmt:358 がカバー — 要確認 |
| Static prop → type annotation | #19 (classes.rs:583) | visit_class_body:322 がカバー — 要確認 |

**注**: #17 (return stmt) と #19 (static prop) は TypeResolver にも呼び出し元があるが、完全に同等かは精査が必要。特に return stmt の Option\<T\> unwrap ロジック（inner T を渡す vs Option\<T\> をそのまま渡す）に差異がある可能性がある。

### TypeResolver にのみ存在

| 伝搬パターン | TypeResolver |
|---|---|
| Paren expr → inner | P-5 (661-665) |
| Cond → both branches | P-6 (667-678) |
| Class property init → type annotation | visit_class_body (322) |
| Switch case test → discriminant type | visit_stmt (415-425) |

Transformer ではこれらを `convert_expr_with_expected` 経由で伝搬していないが、production pipeline では TypeResolver が `expected_types` マップに設定するため、`convert_expr_with_expected` の `expected_override.or_else(|| tctx.type_resolution.expected_type(span))` フォールバックで取得される。

## 問題の影響

### 1. DRY 違反

7 箇所で同じ伝搬ロジックが 2 箇所に存在する。TypeResolver の propagate_expected を修正した場合、Transformer 側の手動伝搬との整合性が崩れる。逆も同様。

### 2. unit test が production path を検証していない

unit test は `convert_expr_with_expected` に `expected_override` を直接渡す（50+ テスト）。これは Transformer の手動伝搬をテストしているが、TypeResolver の `propagate_expected` は一切テストしていない。

- TypeResolver のテストは `context.rs` に 3 件のみ（resolve_types ヘルパー経由）
- expression unit tests は TypeResolver を一切使用していない
- TypeResolver にバグがあっても unit test は通る

### 3. TypeResolver のギャップ（production バグの可能性）

Transformer にのみ存在する 5 パターンは、TypeResolver の `propagate_expected` がカバーしていない。production pipeline ではこれらの expected type が設定されないため、以下の変換が production では正しく動作しない可能性がある:

- DU フィールド値の `.to_string()` 付与
- HashMap 値の `.to_string()` 付与
- Spread array 要素の型変換
- Rest 引数の型変換
- Opt chain メソッド引数の型変換
- Arrow expression body の return type 伝搬

### 4. Phase 3 との不整合

Phase 3 は `resolve_expr_type` を削除し `tctx.type_resolution.expr_type(span)` に一本化する計画だが、Transformer の手動伝搬はこの計画に含まれていない。Phase 3 完了後も手動伝搬は残り続け、二重性は解消されない。

## 理想的な実装

### 目標状態

1. **伝搬ロジックは TypeResolver の `propagate_expected` にのみ存在する**
2. **Transformer は `tctx.type_resolution.expected_type(span)` から読むだけ**
3. **`convert_expr_with_expected` の `expected_override` は不要になる**（または TypeResolver を使わないテスト専用のエスケープハッチとしてのみ存続）
4. **unit test は TypeResolver 経由で expected type を設定する**

### 到達方法

**Step 1**: TypeResolver の `propagate_expected` のギャップを埋める（5 パターン追加）
**Step 2**: unit test を TypeResolver 経由に移行する（テストヘルパー整備 + 50+ テスト書き換え）
**Step 3**: Transformer の手動伝搬を削除する（19 箇所）
**Step 4**: `convert_expr_with_expected` の `expected_override` パラメータを削除し `convert_expr` に統一する

### Cat B コメント箇所（追加の expected type 伝搬候補）

調査中に発見した `Cat B` コメント（「field type could be looked up from struct definition」）は、現在 `convert_expr` を使用しているが expected type を伝搬すべき箇所:

| ファイル:行 | コンテキスト |
|---|---|
| functions/mod.rs:617-619 | Struct field default value |
| functions/mod.rs:754-756 | Class field default value |
| classes.rs:802-803 | Class field assignment |
| statements/mod.rs:2045-2046 | Object destructuring default |

これらは Phase 2.5 のスコープ外だが、将来的に TypeResolver の propagate_expected がカバーすべきパターン。

## ファイル参照

| ファイル | 関連内容 |
|---|---|
| src/pipeline/type_resolver.rs:599-681 | propagate_expected 本体 |
| src/pipeline/type_resolver.rs:956-997 | set_call_arg_expected_types |
| src/pipeline/type_resolver.rs:214,322,358,717,846 | propagate_expected 呼び出し元 |
| src/pipeline/type_resolution.rs:62-131 | FileTypeResolution 構造 |
| src/transformer/test_fixtures.rs:17-86 | TctxFixture 定義 |
| src/transformer/context.rs:59-72 | resolve_types ヘルパー（TypeResolver テスト用） |
| src/transformer/expressions/mod.rs:72-83 | convert_expr_with_expected 定義 |
