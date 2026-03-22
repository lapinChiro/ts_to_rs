# P6 TransformContext 貫通 — tasks.md

## 分析結果

### 変更対象関数: 108 関数（15 ファイル）

| ファイル | 関数数 | 関数名 |
|----------|--------|--------|
| `mod.rs` | 6 | transform_module, transform_module_with_path, transform_module_collecting, transform_module_collecting_with_path, transform_module_item, transform_decl |
| `expressions/mod.rs` | 4 | convert_expr, needs_trait_box_coercion, convert_template_literal, convert_cond_expr |
| `expressions/binary.rs` | 2 | convert_bin_expr, convert_unary_expr |
| `expressions/calls.rs` | 8 | convert_call_expr, convert_global_builtin, convert_number_static_call, convert_fs_call, convert_math_call, convert_new_expr, convert_call_args, convert_call_args_with_types |
| `expressions/member_access.rs` | 5 | resolve_member_access, convert_opt_chain_expr, extract_method_from_callee, convert_member_expr, convert_du_standalone_field_access |
| `expressions/data_literals.rs` | 5 | convert_discriminated_union_object_lit, try_convert_as_hashmap, convert_object_lit, convert_array_lit, convert_spread_array_to_block |
| `expressions/functions.rs` | 4 | convert_function_param_pat, convert_fn_expr, convert_arrow_expr, convert_arrow_expr_with_return_type |
| `expressions/assignments.rs` | 1 | convert_assign_expr |
| `expressions/patterns.rs` | 9 | try_convert_undefined_comparison, try_convert_enum_string_comparison, resolve_enum_type_name, try_convert_typeof_comparison, convert_in_operator, convert_instanceof, resolve_typeof_to_enum_variant, resolve_instanceof_to_enum_variant + 1 closure/helper |
| `expressions/type_resolution.rs` | 7 | resolve_expr_type, resolve_bin_expr_type, resolve_call_return_type, resolve_method_return_type, resolve_new_expr_type, resolve_field_type, convert_ts_as_expr |
| `expressions/literals.rs` | 1 | convert_lit |
| `statements/mod.rs` | 35 | convert_stmt, convert_var_decl, convert_if_with_conditional_assignment, convert_while_with_conditional_assignment, convert_if_stmt, convert_and_combine_conditions, build_nested_if_let, can_generate_if_let, generate_if_let, convert_for_stmt, convert_for_of_stmt, convert_for_in_stmt, convert_labeled_stmt, convert_while_stmt, convert_try_stmt, convert_throw_stmt, extract_error_message, convert_stmt_list, convert_spread_segments, try_expand_spread_var_decl, is_null_or_undefined_expr, try_expand_spread_return, try_expand_spread_expr_stmt, try_convert_object_destructuring, expand_object_pat_props, convert_switch_stmt, try_convert_typeof_switch, try_convert_discriminated_union_switch, convert_switch_clean_match, convert_switch_fallthrough, convert_do_while_stmt, try_convert_array_destructuring, convert_for_stmt_as_loop, convert_update_to_stmt, convert_block_or_stmt, convert_nested_fn_decl |
| `functions/mod.rs` | 9 | convert_fn_decl, convert_ts_type_with_fallback, convert_param, convert_default_param, convert_object_destructuring_param, expand_fn_param_object_props, convert_var_decl_arrow_fns, extract_fn_return_type, extract_fn_param_types |
| `classes.rs` | 11 | extract_class_info, convert_class_decl, convert_static_prop, convert_class_prop, convert_constructor, convert_ts_param_prop, convert_constructor_body, convert_class_method, convert_param_pat, pre_scan_classes, transform_class_with_inheritance |
| `type_env.rs` | 1 | wrap_trait_for_position |

### 追加で変更が必要なファイル

- `mod.rs` の `convert_ident_to_param`（`reg` の位置が他と異なる: `synthetic, reg` の順）
- テストファイル 6 件: `expressions/tests.rs`, `statements/tests.rs`, `functions/tests.rs`, `types/tests.rs`, `tests.rs`, `classes.rs` 内テスト

### 依存関係

全 108 関数は互いに呼び出し合っており、**全て同時に変更しないとコンパイルが通らない**。

例外: `cargo check --lib` と `cargo check --tests` は分離可能。プロダクションコードを先に全変更し、テストコードを後で変更できる。

### テスト呼び出し箇所

テストファイル内で変更対象関数を呼ぶ箇所: 約 744 箇所。

## 設計

### 変換パターン 1: 関数シグネチャ

```rust
// BEFORE
fn convert_foo(
    expr: &ast::Expr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {

// AFTER
fn convert_foo(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
```

`tctx` を `reg` の **直前** に挿入する。`_tctx` ではなく `tctx` を使う（後で lookup に使うため）。

### 変換パターン 2: 呼び出し元

```rust
// BEFORE
convert_foo(expr, reg, type_env, synthetic)

// AFTER
convert_foo(expr, tctx, reg, type_env, synthetic)
```

`reg` 引数の **直前** に `tctx` を挿入する。

### 変換パターン 3: エントリポイント（tctx の生成元）

```rust
// transform_module（既存 API — デフォルト tctx を生成）
pub fn transform_module(module: &Module, reg: &TypeRegistry) -> Result<Vec<Item>> {
    let mut synthetic = SyntheticTypeRegistry::new();
    let mg = ModuleGraph::empty();
    let resolution = FileTypeResolution::empty();
    let tctx = TransformContext::new(&mg, reg, &resolution, Path::new(""));
    let mut items = transform_module_with_path(module, &tctx, reg, None, &mut synthetic)?;
    // ...
}

// transform_module_with_context（新 API — 引数の tctx を使用）
pub fn transform_module_with_context(
    module: &Module,
    ctx: &TransformContext<'_>,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<Item>> {
    let current_file_dir = ctx.file_path.parent().and_then(|p| p.to_str());
    transform_module_with_path(module, ctx, ctx.type_registry, current_file_dir, synthetic)
}
```

### 変換パターン 4: convert_ident_to_param（特殊シグネチャ）

```rust
// BEFORE: reg が synthetic の後にある
pub fn convert_ident_to_param(
    ident: &ast::BindingIdent,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Param>

// AFTER: tctx を synthetic の前に挿入
pub fn convert_ident_to_param(
    ident: &ast::BindingIdent,
    tctx: &TransformContext<'_>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Param>
```

### 変換パターン 5: テスト内の呼び出し

テスト内ではデフォルト tctx を生成するヘルパーを使う:

```rust
// テストヘルパー（各テストモジュールで定義）
fn default_tctx() -> (ModuleGraph, FileTypeResolution, TransformContext<'static>) {
    // TransformContext は借用を含むので、所有者を一緒に返す
    // ... 設計は実装時に詳細化
}
```

実際にはテストではフィクスチャを使う:

```rust
struct TctxFixture {
    mg: ModuleGraph,
    resolution: FileTypeResolution,
    reg: TypeRegistry,
}

impl TctxFixture {
    fn new(reg: TypeRegistry) -> Self {
        Self { mg: ModuleGraph::empty(), resolution: FileTypeResolution::empty(), reg }
    }
    fn tctx(&self) -> TransformContext<'_> {
        TransformContext::new(&self.mg, &self.reg, &self.resolution, Path::new("test.ts"))
    }
}
```

### エッジケース

1. **`resolve_expr_type` 内の再帰呼び出し**: `resolve_expr_type` は自身を再帰呼び出しする。`tctx` をそのまま転送すればよい
2. **`pre_scan_classes`**: transform_module_item の前に呼ばれる。tctx は transform_module_with_path で生成済みなので渡せる
3. **`wrap_trait_for_position`（type_env.rs）**: `reg` を取るが `convert_expr` 等を呼ばない末端関数。tctx を追加するが使わない（`_tctx`）
4. **`convert_lit`（literals.rs）**: 同上。末端関数。`_tctx` で追加
5. **`is_null_or_undefined_expr`**: `reg` を取るが型チェックのみ。末端関数
6. **`extract_fn_return_type`, `extract_fn_param_types`**: `reg` から型定義を取得するのみ。末端関数
7. **クロージャ内の `convert_expr` 呼び出し**: 例えば `arms.iter().map(|arm| convert_expr(...))`。クロージャは `tctx` を借用キャプチャする。Rust のクロージャ借用ルールで問題ないはず（`tctx` は `&` 参照、`synthetic` は `&mut` 参照で排他）

### 必要なインポート

各ファイルの先頭に追加:
```rust
use crate::transformer::context::TransformContext;
```

`mod.rs` では既に `context` モジュールを公開しているので:
```rust
use context::TransformContext;
```

`expressions/` 配下では:
```rust
use crate::transformer::context::TransformContext;
```

## 実装タスク

**注意**: タスク A1〜A15 は全て完了しないと `cargo check --lib` が通らない。A1 から順に実施するが、中間検証は A15 完了後。

### Phase A: プロダクションコードのシグネチャ変更

- [x] **A1-A16**: 全 15 ファイル、108 関数のシグネチャ変更 + 呼び出し元更新完了
  - `src/lib.rs` も追加で変更（`transform_module_with_path` / `transform_module_collecting_with_path` の呼び出し元）
  - `transform_module` / `transform_module_collecting` は公開 API のため tctx パラメータ追加せず、内部でデフォルト tctx 生成
  - `convert_ident_to_param` / `wrap_trait_for_position` は transformer 外から呼ばれるため tctx 追加対象外
  - `types/` モジュールの関数（`convert_type_for_position` 等）も対象外（pipeline 層の関数）
- [x] **A-verify**: `cargo check --lib` 0 エラー確認 ✓

### Phase A2: clippy 修正（プロダクションコードのみ）

- [x] **A2-1**: unused `tctx` → `_tctx` にリネーム（14 箇所） ✓
- [x] **A2-2**: redundant closures 修正 — inject_regex に反映済み ✓
- [x] **A2-verify**: `cargo clippy --lib -- -D warnings` で too_many_arguments(7) 以外 0 ✓

### Phase B: テストコード修正（462 箇所）

**前提**: Phase A 完了済み。`cargo check --lib` 0 エラー、`cargo clippy --lib` 0 エラー。
テストコードのみコンパイルエラー（462 箇所）。`reg` パラメータは残したまま `tctx` を並存。
`clippy.toml` で `too-many-arguments-threshold = 10` に設定済み。

**修正パターン**:
各テストモジュールの先頭にデフォルト tctx 生成コードを追加:
```rust
use crate::transformer::context::TransformContext;
use crate::pipeline::{ModuleGraph, type_resolution::FileTypeResolution};

// 各テスト関数内で:
let mg = ModuleGraph::empty();
let res = FileTypeResolution::empty();
let tctx = TransformContext::new(&mg, &reg, &res, std::path::Path::new("test.ts"));
```
呼び出し箇所で `reg` の前に `tctx` を追加（single-line: `, reg,` → `, tctx, reg,`、multi-line: `reg,` の前の行に `tctx,` 挿入）。

**スクリプト活用**: Phase A で成功した v4 スクリプト（single-line のみ）+ multi-line fix スクリプトを踏襲。
**注意**: `.claude/rules/bulk-edit-safety.md` に従い dry run → 確認 → 実行。

- [x] **B1**: `src/transformer/context.rs` テスト修正（1 箇所）
- [x] **B2**: `src/transformer/expressions/type_resolution.rs` テスト修正（6 箇所）
- [x] **B3**: `src/transformer/tests.rs` 修正（6 箇所）
- [x] **B4**: `src/transformer/classes.rs` 内テスト修正（28 箇所）
- [x] **B5**: `src/transformer/functions/tests.rs` 修正（43 箇所）
- [x] **B6**: `src/transformer/statements/tests.rs` 修正（73 箇所）
- [x] **B7**: `src/transformer/expressions/tests.rs` 修正（305 箇所）
- [x] **B-verify**: `cargo test --lib` 全 GREEN（1078 件）+ `cargo clippy --all-targets -- -D warnings` 0 + E2E(60)/integration(69)/compile(3) テストも全 GREEN
- [x] **B-commit**: `[WIP] P6: Phase B — テストコード tctx 対応`

### Phase B2: TctxFixture リファクタリング（DRY 改善）

**目的**: 362 箇所に重複する 4 行フィクスチャ（`let reg/mg/res/tctx`）を `TctxFixture` 構造体に抽出し、DRY を達成する。
`context.rs` の `Fixture` パターンが既に理想的な実装例。

**TctxFixture 設計**:
```rust
struct TctxFixture {
    mg: ModuleGraph,
    reg: TypeRegistry,
    res: FileTypeResolution,
}

impl TctxFixture {
    fn new() -> Self {
        Self { mg: ModuleGraph::empty(), reg: TypeRegistry::new(), res: FileTypeResolution::empty() }
    }
    fn with_reg(reg: TypeRegistry) -> Self {
        Self { mg: ModuleGraph::empty(), reg, res: FileTypeResolution::empty() }
    }
    fn tctx(&self) -> TransformContext<'_> {
        TransformContext::new(&self.mg, &self.reg, &self.res, Path::new("test.ts"))
    }
    fn reg(&self) -> &TypeRegistry { &self.reg }
}
```

**変換パターン A（空レジストリ — 395 箇所）:**
```rust
// Before (4 lines):
let reg = TypeRegistry::new();
let mg = ModuleGraph::empty();
let res = FileTypeResolution::empty();
let tctx = TransformContext::new(&mg, &reg, &res, Path::new("test.ts"));
// ... convert_expr(&swc_expr, &tctx, &reg, ...);

// After (2 lines):
let f = TctxFixture::new();
let tctx = f.tctx();
// ... convert_expr(&swc_expr, &tctx, f.reg(), ...);
```

**変換パターン B（カスタムレジストリ — 60 箇所）:**
```rust
// Before:
let mut reg = TypeRegistry::new();
reg.register("Foo", ...);
let mg = ModuleGraph::empty();
let res = FileTypeResolution::empty();
let tctx = TransformContext::new(&mg, &reg, &res, Path::new("test.ts"));
// ... convert_expr(&swc_expr, &tctx, &reg, ...);

// After:
let mut reg = TypeRegistry::new();
reg.register("Foo", ...);
let f = TctxFixture::with_reg(reg);
let tctx = f.tctx();
// ... convert_expr(&swc_expr, &tctx, f.reg(), ...);
```

**タスク（小さいファイルから順に）:**
- [x] **B2-1**: `type_resolution.rs` テストモジュール — TctxFixture 定義 + 6 箇所修正（1空+5カスタム）
- [x] **B2-2**: `tests.rs` — TctxFixture 定義 + 6 箇所修正（4空+2カスタム）
- [x] **B2-3**: `classes.rs` テストモジュール — TctxFixture 定義 + 28 箇所修正（27空+1カスタム）
- [x] **B2-4**: `functions/tests.rs` — TctxFixture 定義 + 43 箇所修正（40空+1ヘルパー+2カスタム）
- [x] **B2-5**: `statements/tests.rs` — TctxFixture 定義 + 69 箇所修正（60空+9カスタム）。ヘルパー `convert_single_stmt`/`convert_stmts_with_env` は借用制約のためインラインのまま
- [x] **B2-6**: `expressions/tests.rs` — TctxFixture 定義 + 305 箇所修正（263空+42カスタム）
- [x] **B2-verify**: `cargo test --lib` 1078 GREEN + `cargo clippy --all-targets -- -D warnings` 0 + `cargo fmt` 通過
- [x] **B2-commit**: `[WIP] P6: Phase B2 — TctxFixture リファクタリング`

### Phase C: FileTypeResolution lookup の実装

- [x] **C1+C2**: `resolve_expr_type` 自体の先頭に FileTypeResolution lookup を追加。Known なら即返却、Unknown/未登録なら `resolve_expr_type_heuristic` にフォールバック。呼び出し側の変更不要（再帰呼び出しも自動的に lookup を試みる）。テスト 3 件追加（1081 GREEN）
- [x] **C3**: `convert_expr` 内で `ExprContext::expected` を使う前に `tctx.type_resolution.expected_type(span)` を確認するロジック追加
- [ ] **C4**: TypeEnv の narrowing 参照箇所で `tctx.type_resolution.narrowed_type()` を先に確認するロジック追加
- [ ] **C5**: Generator の enum 分類（`has_data_variants` / `is_numeric_enum` / `generate_enum` in `src/generator/mod.rs`）が TS セマンティクスの判断を含むか評価し、含む場合は Transformer に移動。IR の EnumValue/data で十分な場合はそのままで PRD 完了条件を満たす判断をユーザーに確認
- [ ] **C-verify**: `cargo test` 全 GREEN。新規テスト（context.rs 内）も全 GREEN
- [ ] **C-commit**: `[WIP] P6: Phase C — FileTypeResolution lookup + enum 分類評価`

### Phase D: 最終検証

- [ ] **D1**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **D2**: `cargo fmt --all --check` 通過
- [ ] **D3**: Hono ベンチマーク実行。結果が悪化していない

## 見直し結果

### 網羅性: OK
- 分析 108 関数 + convert_ident_to_param = 109 関数
- タスク A2〜A16 の合計 = 109 関数。全てカバー済み

### 依存関係の整合性: OK
- Phase A は全て同時変更が必要（中間状態でコンパイル不可）
- Phase B は Phase A 完了後に実施可能
- Phase C は Phase B 完了後に実施可能

### コンパイル可能性: 要注意
- Phase A の 15 タスクは **全て完了しないと `cargo check --lib` が通らない**
- タスク順序は「小さいファイルから大きいファイルへ」だが、検証は A-verify でまとめて行う
- Phase B の各タスクは独立して検証可能（テストファイルごとに `cargo test --lib <module>` で確認）

### エッジケースの漏れ: なし
- `convert_ident_to_param` の特殊シグネチャ → 設計パターン 4 で対応済み
- 末端関数（`wrap_trait_for_position`, `convert_lit` 等）→ `_tctx` で対応
- クロージャ内の借用 → 問題なし（`&TransformContext` は Copy 可能な参照）

### テストへの影響: OK
- テストファイル 6 件（expressions/tests.rs, statements/tests.rs, functions/tests.rs, types/tests.rs, tests.rs, classes.rs 内テスト）
- TctxFixture パターンで統一的に対応
