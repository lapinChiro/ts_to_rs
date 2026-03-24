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
    /// ローカル変数の型追跡（所有 — ブロックスコープで push_scope / pop_scope）
    pub(crate) type_env: TypeEnv,
    /// 合成型レジストリ（可変 — 変換中に型が追加される）
    pub(crate) synthetic: &'a mut SyntheticTypeRegistry,
}

impl<'a> Transformer<'a> {
    /// モジュール変換用の Transformer を構築する。
    pub(crate) fn for_module(
        tctx: &'a TransformContext<'a>,
        synthetic: &'a mut SyntheticTypeRegistry,
    ) -> Self { ... }

    /// `tctx.type_registry` へのショートカット。
    pub(crate) fn reg(&self) -> &'a TypeRegistry {
        self.tctx.type_registry
    }
}
```

**設計判断（F-0a で確定）**: `type_env` を所有フィールド（`TypeEnv`）にした。理由:
- ファクトリメソッド `for_module()` で TypeEnv を内部作成でき、外部に構築詳細が漏れない
- `convert_fn_decl` 等は `self.type_env` を使用せず独自のローカル TypeEnv を作成するため、所有化によるセマンティクスの変化はゼロ（D-2-E で検証済み）
- サブ Transformer パターンでは、ローカル TypeEnv を move で渡せるため自然

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

#### Transformer メソッドにする関数（106 関数）

| カテゴリ | 関数数 | 説明 |
|---------|--------|------|
| tctx + type_env + synthetic を取る関数 | 91 | 元の分析通り |
| tctx のみを取る変換関数 | 11 | `convert_lit`, `convert_in_operator`, `resolve_enum_type_name`, `needs_trait_box_coercion`, `resolve_typeof_to_enum_variant`, `resolve_instanceof_to_enum_variant`, `extract_fn_return_type`, `extract_fn_param_types`, `transform_import`, `transform_export_named`, `resolve_import_path_with_fallback` |
| synthetic のみを取る変換関数 | 1 | `convert_default_value`（F-3 品質修正で追加） |
| 呼び出し元が全て Transformer メソッド | 3 | `convert_ident_to_param`, `resolve_member_access`, `resolve_field_type` |

#### Transformer メソッドにしない関数

| 関数 | 理由 |
|------|------|
| `wrap_trait_for_position` | 型ラッピングの純粋ユーティリティ。Transformer の状態に依存しない |
| `lookup_string_enum_variant` | TypeRegistry のルックアップユーティリティ |
| `convert_ts_type` 等 (pipeline/) | pipeline pass。Transformer と無関係 |
| `if_let_pattern` | `NarrowingGuard` のメソッド。別の型の責務 |
| `convert_ident_to_param` | `synthetic` + `reg` を使うが、Transformer メソッドにする（後述） |

**注**: `convert_ident_to_param` は `synthetic` と `reg` を使用しており、呼び出し元は全て Transformer メソッド（`classes.rs` 内）。変換プロセスの一部であるため、Transformer メソッドにする。`resolve_member_access` と `resolve_field_type` も同様に、呼び出し元が全て Transformer メソッドであるため、Transformer メソッドにする。

最終的な対象: **91 + 11 + 3 + 1 = 106 関数**（`convert_default_value` を F-3 品質修正で追加。`synthetic` パラメータを取り dummy_tctx を構築していた free function をメソッド化し、`self.convert_expr()` の直接呼び出しに変更）

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

ラッパーの実装パターン（F-0 で TypeEnv 所有化後の現在の状態）:

```rust
// &mut TypeEnv を取る関数のラッパー（take+restore パターン）
pub fn convert_stmt(
    stmt: &ast::Stmt,
    tctx: &TransformContext<'_>,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<Stmt>> {
    let mut t = Transformer { tctx, type_env: std::mem::take(type_env), synthetic };
    let result = t.convert_stmt(stmt, return_type);
    *type_env = t.type_env;
    result
}

// &TypeEnv を取る関数のラッパー（clone 経由）
pub fn convert_expr(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let env = type_env.clone();
    Transformer { tctx, type_env: env, synthetic }.convert_expr(expr)
}
```

take+restore パターンの安全性:
- `std::mem::take(type_env)` で TypeEnv を取得（元は Default = 空になる）
- Transformer メソッド呼び出し後、`*type_env = t.type_env` で復元（Err 時も実行される）
- エラー伝搬は `result` 変数経由で復元後に行われるため、TypeEnv は常に正しく戻る

`&TypeEnv` ラッパーの `clone()` が安全な理由:
- `&TypeEnv` を取る関数は type_env を**読み取りのみ**
- clone した env は元と同じ値を持ち、読み取り結果は同一

**これらのラッパーは F-1〜F-5 で全て削除される。** 削除後は `self.method()` の直接呼び出しになり、take+restore / clone は不要になる。

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

**設計判断（確定）**: `TypeEnv` を所有フィールド（`type_env: TypeEnv`）に変更した。検討過程と根拠は設計セクションの「設計判断（F-0a で確定）」を参照。

- [x] **F-0a**: `Transformer` の `type_env` フィールドを `TypeEnv`（所有）に変更。`&'a mut TypeEnv` ラッパーは `std::mem::take` + 復元パターンで過渡的に対応
- [x] **F-0b**: ファクトリメソッド `Transformer::for_module()` を実装
- [x] **F-0c**: entry point 3 関数（`transform_module`, `transform_module_collecting`, `transform_module_with_context`）をファクトリメソッド経由に統一
- [x] **F-0d**: `cargo check` 通過確認（0 エラー、全テスト GREEN）

#### F-1〜F-5: ラッパー削除

F-0 で TypeEnv を所有に変更した結果、以下の過渡的パターンが導入されている。F-1〜F-5 でラッパーを削除し、これらを全て解消する。

**F-0 で導入された過渡的パターン（F-1〜F-5 で解消）**:

1. **take+restore パターン（22 箇所、statements/mod.rs）**: `&mut TypeEnv` を受け取るラッパーが `std::mem::take(type_env)` で TypeEnv を取得し、Transformer に渡し、処理後に `*type_env = t.type_env` で復元する。ラッパー削除後は `self.convert_xxx()` の直接呼び出しになり、`self.type_env` を直接操作するため不要になる。
2. **clone パターン（約 35 箇所、expressions/ 等）**: `&TypeEnv` を受け取るラッパーが `type_env.clone()` で所有値を作成し Transformer に渡す。式変換は TypeEnv を読み取り専用で使用するため clone 自体は安全だが、無駄なコピー。ラッパー削除後は `self.type_env` を直接参照するため不要になる。

**ラッパー削除の手順**: 各ファイルについて以下を行う。
1. ラッパー free function を削除
2. ラッパーの呼び出し元を `self.method()` の直接呼び出しに書き換え
   - take+restore 呼び出し元: `convert_stmt(stmt, self.tctx, ret, &mut self.type_env, self.synthetic)` → `self.convert_stmt(stmt, ret)`
   - clone 呼び出し元: `convert_expr(expr, self.tctx, &self.type_env, self.synthetic)` → `self.convert_expr(expr)`
3. `cargo check` 通過確認

- [x] **F-1**: `expressions/` の全ラッパーを削除（clone パターン約 35 箇所が解消される）+ ヘルパー free function 9 個をメソッド化 + 外部呼び出し元（statements/、functions/、classes/、テスト）更新
- [x] **F-2**: `statements/mod.rs` のラッパー36個を全削除（take+restore 22 箇所 + clone 約 15 箇所が解消）。ローカル変数抽出パターン（`let tctx = self.tctx;` 等）9箇所を除去。inline Transformer 構築パターン12箇所を `self.convert_expr()` に置換。外部呼び出し元（`classes.rs`, `expressions/functions.rs`, `functions/mod.rs`）をサブ Transformer パターンに更新。テストファイルも全てメソッド呼び出しに更新
- [x] **F-3**: `functions/mod.rs` のラッパー6個を削除。`arrow_type_env.clone()` を排除（`Vec<(String, RustType)>` で enum_overrides を記録し move で渡す）。`expressions/functions.rs` の呼び出し元2箇所を `self.method()` に更新。`tests.rs` の extract_fn_* テスト6箇所と `functions/tests.rs` の convert_fn_decl テスト43箇所を Transformer 構築に更新。clippy redundant_field_names はラッパー削除で解消
- [x] **F-4**: `classes.rs` のラッパー4個を削除。不要な use 文（SyntheticTypeRegistry, TransformContext）を除去。テスト28箇所（convert_class_decl 24箇所 + extract_class_info 4箇所）を Transformer 構築に更新。clippy redundant_field_names はラッパー削除で解消
- [x] **F-5**: `mod.rs` のラッパー2個（`transform_module_with_path`, `transform_module_collecting_with_path`）を削除。外部呼び出し元2箇所（`context.rs`, `pipeline/mod.rs`）を `Transformer::for_module()` 経由に更新

#### F-3b: ローカル TypeEnv / SyntheticTypeRegistry でラッパーを呼んでいる箇所の変換

以下の 8 箇所は、Transformer メソッド内部で**ラッパー free function を意図的にローカルな TypeEnv や SyntheticTypeRegistry で呼んでいる**。ラッパー削除後はサブ Transformer を構築して呼び出す形に書き換える必要がある。

| # | ファイル:行 | メソッド | 呼んでいるラッパー | ローカル変数 |
|---|------------|---------|------------------|-------------|
| 1 | `functions/mod.rs:183` | `convert_fn_decl` | `convert_stmt_list` | `fn_type_env`, `local_synthetic` | **F-2 で対応済み**（サブ Transformer 構築） |
| 2 | `classes.rs:579` | `convert_static_prop` | `convert_expr` | `TypeEnv::new()` | **F-1 で対応済み**（サブ Transformer 構築） |
| 3 | `classes.rs:795` | `convert_constructor_body` | `convert_expr` | `type_env`（ローカル） | **F-2 で対応済み**（ループ全体をサブ Transformer で統一、clone 排除） |
| 4 | `classes.rs:803` | `convert_constructor_body` | `convert_stmt` | `type_env`（ローカル） | **F-2 で #3 と同時に解消** |
| 5 | `functions/mod.rs:533` | `convert_arrow_fn_with_inferred_type` | `convert_expr` | `TypeEnv::new()`, `dummy_tctx` | **F-1 で対応済み**（サブ Transformer 構築） |
| 6 | `functions/mod.rs:614` | `convert_object_destructuring_param` | `convert_expr` | `TypeEnv::new()` | **F-1 で対応済み**（サブ Transformer 構築） |
| 7 | `functions/mod.rs:747` | `convert_fn_rest_params_to_struct` | `convert_expr` | `TypeEnv::new()` | **F-1 で対応済み**（サブ Transformer 構築） |
| 8 | `statements/mod.rs:3427` | `convert_nested_fn_decl` | `convert_stmt_list` | `fn_type_env` | **F-2 で対応済み**（サブ Transformer 構築） |

**変換パターン**: ラッパー呼び出しをサブ Transformer 構築に置き換える。TypeEnv は所有フィールドなので、ローカル変数を move で渡す。

```rust
// Before (wrapper)
let result = convert_stmt_list(&stmts, self.tctx, ret, &mut fn_type_env, &mut local_synthetic)?;

// After (sub-Transformer with owned TypeEnv)
let result = Transformer {
    tctx: self.tctx,
    type_env: fn_type_env,             // move（所有値）
    synthetic: &mut local_synthetic,
}.convert_stmt_list(&stmts, ret)?;
```

`#5`（`convert_arrow_fn_with_inferred_type`）は `dummy_tctx` も作成している特殊ケース。ラッパー削除後もサブ Transformer でそのまま維持する。

**注意**: `convert_fn_decl` の `local_synthetic` 分離パターン（D-2-D の設計判断）はそのまま維持すること。成功時のみ `self.synthetic.merge(local_synthetic)` する設計を崩さない。

- [x] **F-3b-1**: 上記 8 箇所をサブ Transformer 構築パターンに書き換え（#2,3,5,6,7 は F-1、#1,4,8 は F-2 で対応）
- [x] **F-3b-2**: `cargo check` 通過確認

#### F-6〜F-7: 呼び出し元更新 + 検証

- [x] **F-6**: F-5 で `pipeline/mod.rs` を `Transformer::for_module()` 経由に更新済み
- [x] **F-7**: F-3〜F-5 で全呼び出し元（テスト含む）をメソッド呼び出しに更新済み
- [x] **F-8**: `cargo check` 通過確認済み（全 1216 テスト GREEN）

### Phase D-2-G: `current_file_dir` パラメータ除去

`Transformer::current_file_dir()` メソッドは D-2-A で既に追加済み。

- [ ] **G-1**: `transform_module_with_path`, `transform_module_collecting_with_path`, `transform_module_item` から `current_file_dir` パラメータを削除し、本体で `self.current_file_dir()` を使用
- [ ] **G-2**: `transform_import`, `transform_export_named`, `resolve_import_path_with_fallback` から `current_file_dir` パラメータを削除し `self.current_file_dir()` を使用
- [ ] **G-3**: entry point の `transform_module_with_path` / `transform_module_collecting_with_path` 呼び出し箇所から `current_file_dir` 引数を削除
- [ ] **G-4**: `cargo check` 通過確認

**注意**: G の実施後、`transform_module_with_path` と `transform_module_with_context` は同じシグネチャ（`module` と `synthetic` のみ）になる。統合を検討する（ただし D-2 のスコープ外。TODO に記録する）。

### Phase D-2-H: テスト更新

- [ ] **H-1**: `expressions/tests.rs` — テストヘルパーが `Transformer` を構築してメソッド呼び出し
- [x] **H-2**: `statements/tests.rs` — F-2 で対応済み（`Transformer::for_module()` + サブ Transformer パターンに全面更新）
- [x] **H-3**: `functions/tests.rs` — F-3 で対応済み（43箇所を `Transformer::for_module()` 経由に更新）
- [x] **H-4**: `classes.rs` 内テスト — F-4 で対応済み（28箇所を `Transformer::for_module()` + `transform_class_with_inheritance` に更新）
- [ ] **H-5**: `context.rs` 内テスト — F-5 で `transform_module_with_path` を `Transformer::for_module()` 経由に更新済み。追加テスト修正が必要か確認
- [x] **H-6**: `tests.rs` — F-3 で対応済み（`extract_fn_return_type` / `extract_fn_param_types` テスト6箇所を `Transformer::for_module()` 経由に更新）
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

1. 106 関数が `Transformer` のメソッドになっている
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

### リスク 4: サブ Transformer パターンと TypeEnv 所有の両立 ✅ 解決済み

F-0a で TypeEnv を所有に変更。サブ Transformer は独自のローカル TypeEnv を move で所有するため問題なし（F-0 実装時に検証済み）。
