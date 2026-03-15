# TypeEnv（型環境）のデータ構造とシグネチャ導入

## 背景・動機

現在のトランスフォーマーは各文を独立に変換し、ローカル変数や関数パラメータの型情報を後続の文・式に伝搬しない。`const x: Foo = ...` の後に `x.field` を変換する際、`x` の型が `Foo` であることを知る手段がない。

これが optional chaining の型判定、builtin API の参照モデル、const のミュータビリティ解析など、多数の課題の共通原因となっている。

## ゴール

- `convert_stmt_list` が変数名 → `RustType` のスコープ付きマップ（TypeEnv）を管理する
- `convert_fn_decl` が関数パラメータの型を TypeEnv に登録してから本体を変換する
- `convert_stmt` / `convert_expr` が TypeEnv を受け取るシグネチャになる
- 既存の全テストが変更なく通過する（振る舞いの変更はなし、シグネチャのみ）

## スコープ

### 対象

- `TypeEnv` 構造体の定義（変数名 → `RustType` のマップ、スコープ push/pop）
- `convert_stmt_list` に `TypeEnv` パラメータを追加し、`Stmt::Let` 処理時にエントリを登録
- `convert_stmt` に `TypeEnv` パラメータを追加（透過的に渡す）
- `convert_expr` に `TypeEnv` パラメータを追加（透過的に渡す）
- `convert_fn_decl` でパラメータ型を TypeEnv に登録してから本体変換

### 対象外

- TypeEnv を使った式の型解決（次の PRD `type-env-expr-resolution` で対応）
- TypeEnv を使った変換ロジックの変更（optional chaining 等は別 PRD）
- ネストスコープ（ブロック式、クロージャ内の変数シャドウイング）— 初版ではフラットマップ

## 設計

### 技術的アプローチ

#### TypeEnv 構造体

```rust
/// ローカル変数の型情報を保持する型環境。
///
/// 変数宣言時にエントリを追加し、後続の式変換で参照する。
#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    vars: HashMap<String, RustType>,
}

impl TypeEnv {
    pub fn new() -> Self { Self::default() }

    /// 変数の型を登録する。同名の変数が既にある場合は上書き（シャドウイング）。
    pub fn insert(&mut self, name: String, ty: RustType) { ... }

    /// 変数名から型を取得する。
    pub fn get(&self, name: &str) -> Option<&RustType> { ... }
}
```

#### シグネチャ変更

```rust
// Before
pub fn convert_stmt_list(stmts, reg, return_type) -> Result<Vec<Stmt>>
pub fn convert_stmt(stmt, reg, return_type) -> Result<Vec<Stmt>>
pub fn convert_expr(expr, reg, expected) -> Result<Expr>

// After
pub fn convert_stmt_list(stmts, reg, return_type, type_env: &mut TypeEnv) -> Result<Vec<Stmt>>
pub fn convert_stmt(stmt, reg, return_type, type_env: &mut TypeEnv) -> Result<Vec<Stmt>>
pub fn convert_expr(expr, reg, expected, type_env: &TypeEnv) -> Result<Expr>
```

`convert_stmt_list` は `Stmt::Let` の処理後に `type_env.insert(name, ty)` を呼ぶ。

`convert_fn_decl` は本体変換前に `TypeEnv::new()` を作り、パラメータの型を登録してから `convert_stmt_list` に渡す。

#### エントリの登録タイミング

- `convert_var_decl` が `Stmt::Let { name, ty: Some(ty), .. }` を返した後、`convert_stmt_list` が `type_env.insert(name, ty)` を実行
- `convert_fn_decl` が params を変換した後、`type_env.insert(param.name, param.ty)` を実行
- 分割代入（`try_convert_object_destructuring`, `try_convert_array_destructuring`）で生成された各 `Stmt::Let` についても同様

### 影響範囲

- `src/transformer/mod.rs` — `TypeEnv` の定義、`transform_module` からの初期化
- `src/transformer/statements/mod.rs` — `convert_stmt`, `convert_stmt_list` のシグネチャ変更
- `src/transformer/expressions/mod.rs` — `convert_expr` 他のシグネチャ変更
- `src/transformer/functions/mod.rs` — `convert_fn_decl` での TypeEnv 初期化
- `src/transformer/classes.rs` — メソッド変換での TypeEnv 伝搬
- 全テストファイル — `TypeEnv::new()` の追加

## 作業ステップ

- [ ] ステップ1: `TypeEnv` 構造体を `src/transformer/mod.rs` に定義 + ユニットテスト（insert/get）
- [ ] ステップ2: `convert_expr` のシグネチャに `type_env: &TypeEnv` を追加。全呼び出し元を更新（`&TypeEnv::new()` を渡す）
- [ ] ステップ3: `convert_stmt` のシグネチャに `type_env: &mut TypeEnv` を追加。全呼び出し元を更新
- [ ] ステップ4: `convert_stmt_list` のシグネチャに `type_env: &mut TypeEnv` を追加。`Stmt::Let` 処理後に `type_env.insert` を実行
- [ ] ステップ5: `convert_fn_decl` で `TypeEnv::new()` を作り、パラメータ型を登録してから本体変換
- [ ] ステップ6: 全テスト・clippy・fmt 通過を確認

## テスト計画

- `TypeEnv::insert` / `get` の基本動作
- `TypeEnv::insert` の同名上書き（シャドウイング）
- 既存テスト全件の回帰テスト（振る舞い変更なし）

## 完了条件

- `TypeEnv` 構造体が定義され、`insert` / `get` のユニットテストがある
- `convert_stmt_list` が `Stmt::Let` 処理後に型情報を TypeEnv に登録している
- `convert_fn_decl` がパラメータ型を TypeEnv に登録している
- 既存テスト全件が通過する
- `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- `cargo fmt --all --check` 通過
