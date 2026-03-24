# D-2: Transformer struct 導入 — 詳細設計 + タスク一覧

## 目的

`tctx: &TransformContext`, `type_env: &(mut) TypeEnv`, `synthetic: &mut SyntheticTypeRegistry` の 3 パラメータが 91 関数を貫通している。これらを `Transformer` struct のフィールドに束ね、全関数をメソッドに変換する。

## 設計

### Transformer struct

```rust
/// 変換処理の状態を保持する構造体。
///
/// 不変コンテキスト (`tctx`) と可変状態 (`type_env`, `synthetic`) を束ね、
/// 全変換関数をメソッドとして提供する。各サブモジュールに `impl Transformer`
/// ブロックを配置し、ファイル構成を変更せずにメソッド化する。
pub(crate) struct Transformer<'a> {
    /// 不変コンテキスト（TypeRegistry, ModuleGraph, TypeResolution, file path）
    pub(crate) tctx: &'a TransformContext<'a>,
    /// ローカル変数の型追跡（可変 — ブロックスコープで push_scope / pop_scope）
    pub(crate) type_env: &'a mut TypeEnv,
    /// 合成型レジストリ（可変 — 変換中に型が追加される）
    pub(crate) synthetic: &'a mut SyntheticTypeRegistry,
}

impl<'a> Transformer<'a> {
    /// `tctx.type_registry` へのショートカット。
    pub(crate) fn reg(&self) -> &'a TypeRegistry {
        self.tctx.type_registry
    }
}
```

### borrow checker との整合性

**実機検証済み。** 以下の 3 パターンが全てコンパイル・実行可能であることを確認:

1. **`self.tctx.type_registry.get()` → `self.convert_expr()`**: `tctx` は `&'a TransformContext`（`Copy`）。返り値の lifetime は `'a` であり `&mut self` の reborrow と独立。衝突なし。

2. **`self.type_env.get().cloned()` → `self.convert_expr()`**: `.cloned()` で値をコピーし、reborrow が終了する。衝突なし。

3. **`self.type_env.get()` in match arm → `self.convert_expr()`**: match arm の中で参照を消費し、arm の外で `self.convert_expr()` を呼ぶ。NLL により衝突なし。

**コードベース検証**: `type_env.get()` の参照が `convert_*` 呼び出しをまたぐケースは**ゼロ**。全て `.cloned()` / `.is_some()` / match arm 内消費で、既に上記パターンに適合している。

### メソッド化のシグネチャ変換

```rust
// Before: free function
pub(super) fn convert_expr(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> { ... }

// After: method on Transformer
impl<'a> Transformer<'a> {
    pub(crate) fn convert_expr(&mut self, expr: &ast::Expr) -> Result<Expr> { ... }
}
```

本体の変更:
- `type_env.xxx()` → `self.type_env.xxx()`
- `synthetic.xxx()` → `self.synthetic.xxx()`
- `tctx.xxx` → `self.tctx.xxx`
- `convert_expr(child, tctx, type_env, synthetic)` → `self.convert_expr(child)`
- `reg.xxx()` → `self.reg().xxx()`（`let reg = self.reg();` パターンは使わず、直接 `self.reg()` を呼ぶ）

### メソッド化の対象判定基準

**基準**: 変換プロセスの一部であるか。パラメータの種類（tctx/type_env/synthetic を取るかどうか）ではなく、**概念的に Transformer の責務であるか**で判断する。

#### Transformer メソッドにする関数（102 関数）

| カテゴリ | 関数数 | 説明 |
|---------|--------|------|
| tctx + type_env + synthetic を取る関数 | 91 | 現在の分析通り |
| tctx のみを取る変換関数 | 11 | `convert_lit`, `convert_in_operator`, `resolve_enum_type_name`, `needs_trait_box_coercion`, `resolve_typeof_to_enum_variant`, `resolve_instanceof_to_enum_variant`, `extract_fn_return_type`, `extract_fn_param_types`, `transform_import`, `transform_export_named`, `resolve_import_path_with_fallback` |

#### Transformer メソッドにしない関数

| 関数 | 理由 |
|------|------|
| `wrap_trait_for_position` | 型ラッピングの純粋ユーティリティ。Transformer の状態に依存しない |
| `lookup_string_enum_variant` | TypeRegistry のルックアップユーティリティ |
| `convert_ts_type` 等 (pipeline/) | pipeline pass。Transformer と無関係 |
| `if_let_pattern` | `NarrowingGuard` のメソッド。別の型の責務 |
| `convert_ident_to_param` | `synthetic` + `reg` を使うが、Transformer メソッドにする（後述） |

**注**: `convert_ident_to_param` は `synthetic` と `reg` を使用しており、呼び出し元は全て Transformer メソッド（`classes.rs` 内）。変換プロセスの一部であるため、Transformer メソッドにする。`resolve_member_access` と `resolve_field_type` も同様に、呼び出し元が全て Transformer メソッドであるため、Transformer メソッドにする。

最終的な対象: **91 + 11 + 3 = 105 関数**

### `current_file_dir` パラメータの除去

`current_file_dir: Option<&str>` は `tctx.file_path.parent().and_then(|p| p.to_str())` と等価。`Transformer` にヘルパーメソッドを追加し、パラメータを除去する。

```rust
impl<'a> Transformer<'a> {
    fn current_file_dir(&self) -> Option<&'a str> {
        self.tctx.file_path.parent().and_then(|p| p.to_str())
    }
}
```

影響: `transform_module_with_path`, `transform_module_collecting_with_path`, `transform_module_item` の 3 関数 + その呼び出し元。

### 遷移戦略: ラッパー関数

一括変換ではなく、ファイルごとに段階的にメソッド化する。各ファイル変換後に `cargo check` が通る状態を維持する。

1. ファイル X の関数をメソッドに変換
2. 旧シグネチャの free function ラッパーを作成（他ファイルからの呼び出し用）
3. ファイル X 内の呼び出しは `self.method()` に変更
4. `cargo check` 通過を確認
5. 次のファイルへ

ラッパーの実装パターン:

```rust
// &mut TypeEnv を取る関数のラッパー（直接委譲）
pub fn convert_stmt(
    stmt: &ast::Stmt,
    tctx: &TransformContext<'_>,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<Stmt>> {
    Transformer { tctx, type_env, synthetic }.convert_stmt(stmt, return_type)
}

// &TypeEnv を取る関数のラッパー（clone 経由）
pub fn convert_expr(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let mut env = type_env.clone();
    Transformer { tctx, type_env: &mut env, synthetic }.convert_expr(expr)
}
```

`&TypeEnv` ラッパーの `clone()` が安全な理由:
- `&TypeEnv` を取る関数は type_env を**読み取りのみ**
- clone した env は元と同じ値を持ち、読み取り結果は同一
- 変換中に sub-call が env を変更しても、それは clone 上の変更で元には影響しない（元も影響されない設計 — push_scope/pop_scope は常にバランス）

## 実装タスク

### Phase D-2-A: Transformer struct 定義

- [x] `src/transformer/mod.rs` に `Transformer` struct を定義
- [x] `reg()` ヘルパーメソッドを追加
- [x] `current_file_dir()` ヘルパーメソッドを追加
- [x] `cargo check` 通過確認（dead_code 警告のみ、エラー 0）

### Phase D-2-B: expressions モジュールのメソッド化

変換順序: 依存の少ないファイルから。各ファイルで関数をメソッド化 + ラッパー作成。

- [x] **B-1**: `expressions/literals.rs` — `convert_lit`（1 関数）
- [x] **B-2**: `expressions/assignments.rs` — 1 関数
- [x] **B-3**: `expressions/binary.rs` — 2 関数
- [x] **B-4**: `expressions/data_literals.rs` — 5 関数
- [x] **B-5**: `expressions/member_access.rs` — 6 関数
- [x] **B-6**: `expressions/calls.rs` — 2 関数メソッド化 + 6 関数ラッパー付き
- [x] **B-7**: `expressions/functions.rs` — 4 関数
- [x] **B-8**: `expressions/patterns.rs` — 5 関数メソッド化（tctx のみの 4 関数はラッパー付き free function）
- [x] **B-9**: `expressions/mod.rs` — 2 関数メソッド化 + 3 関数ラッパー付き
- [x] **B-10**: rust-analyzer diagnostics エラー 0 確認（全 expressions ラッパー経由で動作）

各サブタスクの手順:
1. ファイルを読む
2. 対象関数を `impl<'a> Transformer<'a> { }` ブロック内に移動
3. シグネチャから `tctx`, `type_env`, `synthetic` を削除、`&mut self` を追加
4. 本体の `type_env` → `self.type_env`、`synthetic` → `self.synthetic`、`tctx` → `self.tctx`
5. 同ファイル内の呼び出しを `self.method()` に変更
6. `impl` ブロックの外にラッパー free function を作成
7. `cargo check` 通過確認

### Phase D-2-C: statements モジュールのメソッド化

- [x] **C-1**: `statements/mod.rs` — 36 関数メソッド化 + ラッパー
- [x] **C-2**: rust-analyzer diagnostics エラー 0 確認

### Phase D-2-D: functions, classes モジュールのメソッド化

- [x] **D-1**: `functions/mod.rs` — 7 関数 + `extract_fn_return_type`, `extract_fn_param_types`（計 9 関数）
- [x] **D-2**: `classes.rs` — 11 関数 + `convert_ident_to_param`（`mod.rs` から移動。呼び出し元が全て classes なため）
- [x] **D-3**: `cargo check` 通過確認

### Phase D-2-E: mod.rs のメソッド化 + entry point 変更

- [x] **E-1**: `mod.rs` の `transform_module_with_path`, `transform_module_collecting_with_path`, `transform_module_item`, `transform_decl`, `transform_import`, `transform_export_named`, `resolve_import_path_with_fallback` — 7 関数
- [x] **E-2**: entry point (`transform_module`, `transform_module_collecting`, `transform_module_with_context`) を `Transformer` 構築 + メソッド呼び出しに変更
- [x] **E-3**: `pipeline/mod.rs` の呼び出しを更新
- [x] **E-4**: `cargo check` 通過確認

### Phase D-2-F: ラッパー削除 + API 整理

全ファイルがメソッド化された後、ラッパー free function を削除し、外部 API を整理する。

#### F-0: ファクトリメソッド導入（ラッパー削除の前提）

現在、Transformer 構築ボイラープレート（`TypeEnv::new()` → struct 構築）が以下 6 箇所に重複:

- `mod.rs`: `transform_module`, `transform_module_collecting`, `transform_module_with_context`
- `mod.rs`: `transform_module_with_path` ラッパー, `transform_module_collecting_with_path` ラッパー
- `pipeline/mod.rs`（現在はラッパー経由だが、F-5 でラッパー削除後に直接構築が必要になる）

ラッパー削除後、外部呼び出し元（pipeline, entry point, テスト）は Transformer を直接構築する必要がある。`drop(t)` の明示的呼び出しも API 設計の不良。

**対策**: ファクトリメソッドを導入し、構築ボイラープレートを一元化する。

```rust
impl<'a> Transformer<'a> {
    /// モジュール変換用の Transformer を構築する。
    /// TypeEnv は内部で作成し、返り値の lifetime に束縛する。
    pub(crate) fn for_module(
        tctx: &'a TransformContext<'a>,
        synthetic: &'a mut SyntheticTypeRegistry,
    ) -> Self { ... }
}
```

**設計判断**: `TypeEnv` を Transformer のフィールドとして所有（`type_env: TypeEnv`）に変更するか、現状の参照（`&'a mut TypeEnv`）を維持するか。

- **所有に変更する場合**: ファクトリメソッドが TypeEnv を内部で作成でき、外部に構築詳細が漏れない。`drop(t)` も不要になる（Transformer が drop されれば TypeEnv も自動 drop）。ただし、サブ Transformer パターン（後述 F-3b）で TypeEnv の共有が必要な箇所がないか要確認。
- **参照を維持する場合**: ファクトリメソッドは使えない（ローカル変数への参照を返せない）。マクロまたは closure ベースの API（`Transformer::with_module(tctx, synthetic, |t| { ... })`）が代替案。

**D-2-E の検証結果**: `convert_fn_decl`, `transform_class_with_inheritance`, `convert_var_decl_arrow_fns` は全て `self.type_env` を**使用していない**。各関数が独自のローカル TypeEnv を作成している。mod.rs のメソッド群（`transform_module_with_path` 等）も type_env を直接操作しない。**TypeEnv を所有に変更しても既存のセマンティクスに影響なし**。

- [ ] **F-0a**: `Transformer` の `type_env` フィールドを `TypeEnv`（所有）に変更するか `&'a mut TypeEnv`（参照）を維持するか決定。上記の検証結果を踏まえ、所有への変更を推奨
- [ ] **F-0b**: ファクトリメソッド `Transformer::for_module()` を実装
- [ ] **F-0c**: entry point 3 関数（`transform_module`, `transform_module_collecting`, `transform_module_with_context`）をファクトリメソッド経由に統一
- [ ] **F-0d**: `cargo check` 通過確認

#### F-1〜F-5: ラッパー削除

- [ ] **F-1**: `expressions/` の全ラッパーを削除
- [ ] **F-2**: `statements/mod.rs` のラッパーを削除
- [ ] **F-3**: `functions/mod.rs` のラッパーを削除
- [ ] **F-4**: `classes.rs` のラッパーを削除
- [ ] **F-5**: `mod.rs` のラッパー（`transform_module_with_path`, `transform_module_collecting_with_path`）を削除

#### F-3b: ローカル TypeEnv / SyntheticTypeRegistry でラッパーを呼んでいる箇所の変換

以下の 8 箇所は、Transformer メソッド内部で**ラッパー free function を意図的にローカルな TypeEnv や SyntheticTypeRegistry で呼んでいる**。ラッパー削除後はサブ Transformer を構築して呼び出す形に書き換える必要がある。

| # | ファイル:行 | メソッド | 呼んでいるラッパー | ローカル変数 |
|---|------------|---------|------------------|-------------|
| 1 | `functions/mod.rs:183` | `convert_fn_decl` | `convert_stmt_list` | `fn_type_env`, `local_synthetic` |
| 2 | `classes.rs:579` | `convert_static_prop` | `convert_expr` | `TypeEnv::new()` |
| 3 | `classes.rs:791` | `convert_constructor_body` | `convert_expr` | `type_env`（ローカル） |
| 4 | `classes.rs:794` | `convert_constructor_body` | `convert_stmt` | `type_env`（ローカル） |
| 5 | `functions/mod.rs:533` | `convert_arrow_fn_with_inferred_type` | `convert_expr` | `TypeEnv::new()`, `dummy_tctx` |
| 6 | `functions/mod.rs:614` | `convert_object_destructuring_param` | `convert_expr` | `TypeEnv::new()` |
| 7 | `functions/mod.rs:747` | `convert_fn_rest_params_to_struct` | `convert_expr` | `TypeEnv::new()` |
| 8 | `statements/mod.rs:3427` | `convert_nested_fn_decl` | `convert_stmt_list` | `fn_type_env` |

**変換パターン**: ラッパー呼び出しをサブ Transformer 構築に置き換える。

```rust
// Before (wrapper)
let result = convert_stmt_list(&stmts, self.tctx, ret, &mut fn_type_env, &mut local_synthetic)?;

// After (sub-Transformer)
let result = Transformer {
    tctx: self.tctx,
    type_env: &mut fn_type_env,      // or TypeEnv field if owned
    synthetic: &mut local_synthetic,
}.convert_stmt_list(&stmts, ret)?;
```

`#5`（`convert_arrow_fn_with_inferred_type`）は `dummy_tctx` も作成している特殊ケース。ラッパー削除後もサブ Transformer でそのまま維持する。

**注意**: `convert_fn_decl` の `local_synthetic` 分離パターン（D-2-D の設計判断）はそのまま維持すること。成功時のみ `self.synthetic.merge(local_synthetic)` する設計を崩さない。

- [ ] **F-3b-1**: 上記 8 箇所をサブ Transformer 構築パターンに書き換え
- [ ] **F-3b-2**: `cargo check` 通過確認

#### F-6〜F-7: 呼び出し元更新 + 検証

- [ ] **F-6**: `pipeline/mod.rs` をファクトリメソッド経由の Transformer メソッド呼び出しに更新（現在はラッパー経由。F-5 でラッパー削除するため）
- [ ] **F-7**: 全呼び出し元をメソッド呼び出しに更新（テスト含む）
- [ ] **F-8**: `cargo check` 通過確認

### Phase D-2-G: `current_file_dir` パラメータ除去

`Transformer::current_file_dir()` メソッドは D-2-A で既に追加済み。

- [ ] **G-1**: `transform_module_with_path`, `transform_module_collecting_with_path`, `transform_module_item` から `current_file_dir` パラメータを削除し、本体で `self.current_file_dir()` を使用
- [ ] **G-2**: `transform_import`, `transform_export_named`, `resolve_import_path_with_fallback` から `current_file_dir` パラメータを削除し `self.current_file_dir()` を使用
- [ ] **G-3**: entry point の `transform_module_with_path` / `transform_module_collecting_with_path` 呼び出し箇所から `current_file_dir` 引数を削除
- [ ] **G-4**: `cargo check` 通過確認

**注意**: G の実施後、`transform_module_with_path` と `transform_module_with_context` は同じシグネチャ（`module` と `synthetic` のみ）になる。統合を検討する（ただし D-2 のスコープ外。TODO に記録する）。

### Phase D-2-H: テスト更新

- [ ] **H-1**: `expressions/tests.rs` — テストヘルパーが `Transformer` を構築してメソッド呼び出し
- [ ] **H-2**: `statements/tests.rs` — 同上
- [ ] **H-3**: `functions/tests.rs` — 同上
- [ ] **H-4**: `classes.rs` 内テスト — 同上
- [ ] **H-5**: `context.rs` 内テスト — 同上
- [ ] **H-6**: `tests.rs` — 同上
- [ ] **H-7**: `test_fixtures.rs` — `TctxFixture::transform()` が `Transformer` を使用するよう更新
- [ ] **H-8**: `cargo test` 全 GREEN

**注意**: H はラッパー削除（F）の後に実施する。F でラッパーが消えるため、テストコードもラッパー経由からメソッド呼び出しに移行する必要がある。ただし F-7 でテストを含む呼び出し元を更新するため、H は F-7 の完了確認 + 追加テスト修正が主な作業になる。

### Phase D-2-I: クリーンアップ + 最終検証

- [ ] **I-1**: 不要な `use` 文の削除
- [ ] **I-2**: 不要な `let reg = tctx.type_registry;` / `let reg = self.reg();` の削除（直接 `self.reg()` を使用）
- [ ] **I-3**: 不要な `let tctx = self.tctx;` の削除（直接 `self.tctx` を使用）
- [ ] **I-4**: ラッパー関数の残骸がないことを確認（`grep` で `"Wrapper:"` / `"transition period"` を検索）
- [ ] **I-5**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **I-6**: `cargo fmt --all --check` 通過
- [ ] **I-7**: `cargo test` 全 GREEN
- [ ] **I-8**: `tasks.md`, `plan.md`, `backlog/p8-integration.md` を更新
- [ ] **I-9**: Transformer struct の doc コメントが正確であることを確認

## 完了条件

1. 105 関数が `Transformer` のメソッドになっている
2. ラッパー free function が存在しない
3. `tctx`, `type_env`, `synthetic` がメソッドのパラメータとして現れない（`self` 経由でアクセス）
4. `current_file_dir` パラメータが存在しない（`self.current_file_dir()` メソッド経由）
5. Transformer 構築ボイラープレートがファクトリメソッドに集約されている
6. `pipeline/mod.rs` が Transformer のフィールド構造に直接依存していない
7. `cargo test` 全 GREEN
8. `cargo clippy` 0 エラー・0 警告

## リスクと対策

### リスク 1: ラッパーの `type_env.clone()` によるセマンティクスの変化

`&TypeEnv` を取るラッパーは `clone()` 経由で Transformer を構築する。clone は遷移期間のみ使用。

**対策**: ラッパー削除（Phase F）後は clone が不要になる。遷移期間中のテストで Green を維持し、セマンティクスの変化がないことを検証。

### リスク 2: `&mut self` と `self.type_env.get()` の borrow 衝突

**対策**: コードベース検証で `type_env.get()` が `convert_*` 呼び出しをまたぐケースがゼロであることを確認済み。万一発見した場合は `.cloned()` を追加。

### リスク 3: `impl Transformer` ブロックの可視性

各サブモジュールの `impl Transformer` ブロック内のメソッドは `pub(crate)` にする。`pub(super)` だった関数は `pub(crate)` に昇格する（Transformer は crate 全体で使用されるため）。

**対策**: 可視性の変更は意味的に正しい（メソッドは型を通じてアクセスされるため、モジュール可視性ではなく型の可視性が支配的）。

### リスク 4: サブ Transformer パターンと TypeEnv 所有の両立（Phase F-0 で検討）

`convert_fn_decl` 等がローカル TypeEnv でサブ Transformer を構築するパターンがある。TypeEnv を所有フィールドに変更した場合でも、サブ Transformer は別の TypeEnv（ローカル）を所有できるため問題ない。

**検証済み**: `self.type_env` を使用しているメソッドは statements / expressions モジュールの変換関数のみ。mod.rs, functions, classes のメソッドは `self.type_env` を一切使用しておらず、独自のローカル TypeEnv を作成している。
