# 変換パイプライン コンポーネント設計（第4版）

**基準コミット**: `bcfc4a5`（未コミットの変更あり）
**調査日**: 2026-03-21
**改訂履歴**:
- 初版: 基本設計
- 第2版: 7 件の設計問題を修正
- 第3版: 11 件を修正。FileAnalyzer への統合
- 第4版: AST 走査パターンの詳細調査に基づき TypeResolver/Transformer の分離を復元。全セクションを再検証

## 1. 設計原則

- **直交性**: 各コンポーネントは1つの変更理由しか持たない
- **DRY**: 知識の重複を排除する（判断ロジックの重複を排除）
- **低結合**: コンポーネント間は不変データを介して通信する
- **テスト容易性**: 各コンポーネントを単独でテストできる
- **統一性**: 単一ファイルとディレクトリは同一パイプラインで処理する

## 2. AST 走査パターンの調査結果

TypeResolver と Transformer を分離するか統合するかの判断の根拠として、現在の実装の AST 走査パターンを調査した。

### 発見された事実

1. **`resolve_expr_type` は AST のみに依存し、IR に一切依存しない**（`src/transformer/expressions/type_resolution.rs:19-59`）。AST + TypeEnv + TypeRegistry だけで型を解決する
2. **型解決は変換の前に行われる**。各ノードで「型を解決 → 変換方法を決定 → IR を生成」の順
3. **narrowing は AST の構造から決定的に計算可能**。TypeResolver が自身で AST を走査すれば、Transformer と同じ制御フローの追跡を再現できる

### 結論

TypeResolver は独立パスとして事前に全ノードの型情報を計算し、結果を Transformer に不変データとして渡す。AST は2回走査される（TypeResolver + Transformer）が、Rust 実装のため走査は高速であり、責務分離によるテスト容易性・保守性のメリットが上回る。

## 3. 公開 API

```rust
struct TranspileInput {
    files: Vec<(PathBuf, String)>,
    builtin_types: Option<TypeRegistry>,
    module_resolver: Box<dyn ModuleResolver>,
}

struct TranspileOutput {
    files: Vec<FileOutput>,
    module_graph: ModuleGraph,
    synthetic_types: SyntheticTypeRegistry,
}

struct FileOutput {
    path: PathBuf,
    rust_source: String,
    unsupported: Vec<UnsupportedSyntax>,
}

/// 統一パイプライン
fn transpile(input: TranspileInput) -> Result<TranspileOutput>;

/// 単一ファイルの簡易 API
fn transpile_single(source: &str) -> Result<String>;
```

単一ファイルは `NullModuleResolver`（全 import → None）+ ファイル数1の特殊ケース。パイプラインロジックは同一。

## 4. コンポーネント一覧

| コンポーネント | 種別 | 責務 | 変更理由 |
|--------------|------|------|---------|
| Parser | 関数 | TS ソース → SWC AST | SWC API の変更 |
| ModuleResolver | trait | import specifier → ファイルパス | 解決戦略の変更 |
| ModuleGraphBuilder | struct | import/export 収集 → ModuleGraph | モジュール意味論の変更 |
| TypeConverter | 関数群 | TS 型注釈 → RustType（合成型登録を伴う） | 型マッピングルールの変更 |
| TypeCollector | 関数 | 全ファイルの型定義 → TypeRegistry | 型定義の発見ルールの変更 |
| SyntheticTypeRegistry | struct | 合成型のデータストア + 重複排除 | 命名・重複排除ルールの変更 |
| AnyTypeAnalyzer | 関数 | any パラメータの typeof/instanceof 分析 | any 具象化戦略の変更 |
| TypeResolver | struct | 式の型・期待型・narrowing を事前計算 | 型推論ルールの変更 |
| Transformer | struct | AST + 型情報 → IR | 変換ルールの変更 |
| Generator | 関数 | IR → Rust テキスト | Rust 構文の変更 |
| OutputWriter | struct | ファイル書き出し + mod.rs + 合成型配置 | 出力構造の変更 |

## 5. コンポーネント間の依存関係

```
ParsedFiles（不変、全パスで共有）
  │
  ├──→ ModuleGraphBuilder + ModuleResolver ──→ ModuleGraph（不変）
  │
  ├──→ TypeCollector + TypeConverter ──→ TypeRegistry（不変）
  │         └──→ SyntheticTypeRegistry（追記）
  │
  ├──→ AnyTypeAnalyzer ──→ SyntheticTypeRegistry（追記）
  │
  ├──→ TypeResolver ──→ FileTypeResolution（不変、per file）
  │       (参照: TypeRegistry, SyntheticTypeRegistry, ModuleGraph)
  │       (副作用: SyntheticTypeRegistry に body 内の合成型を追記)
  │
  ├──→ Transformer ──→ Vec<Item>（per file）
  │       (参照: FileTypeResolution, ModuleGraph, TypeRegistry, SyntheticTypeRegistry)
  │
  ├──→ Generator ──→ String（per file）
  │
  └──→ OutputWriter ──→ 出力ディレクトリ
```

依存は全て上から下。循環なし。

## 6. データ構造

### 6.1 ParsedFiles

```rust
struct ParsedFiles {
    files: Vec<ParsedFile>,
}
struct ParsedFile {
    path: PathBuf,
    source: String,
    module: swc_ecma_ast::Module,
}
```

### 6.2 ModuleResolver (trait)

```rust
trait ModuleResolver {
    fn resolve(&self, from_file: &Path, specifier: &str) -> Option<PathBuf>;
}
struct NullModuleResolver;       // 単一ファイル用
struct NodeModuleResolver { .. } // Node.js/Bundler 用（baseUrl/paths 対応）
```

### 6.3 ModuleGraph

```rust
struct ModuleGraph {
    file_to_module: HashMap<PathBuf, String>,
    exports: HashMap<PathBuf, HashMap<String, ExportOrigin>>,
    module_tree: ModuleTree,
}
struct ExportOrigin {
    module_path: String,  // Rust モジュールパス
    name: String,
}
```

query API: `resolve_import()`, `module_path()`, `children_of()`, `reexports_of()`

### 6.4 TypeConverter

```rust
fn convert_ts_type(
    ts_type: &TsType,
    registry: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<RustType>;
```

TypeCollector と TypeResolver の両方が使用（DRY）。union 型やインライン型を変換する際に SyntheticTypeRegistry に登録する。

### 6.5 TypeRegistry

既存と同一。TypeCollector が構築。Pass 2 完了後は不変。

### 6.6 SyntheticTypeRegistry

```rust
struct SyntheticTypeRegistry {
    types: HashMap<String, SyntheticTypeDef>,
}
```

API: `register_union()`, `register_any_enum()`, `register_inline_struct()`, `get()`, `all_items()`

**ライフサイクル**: パイプライン全体で追記される:
- Pass 2: TypeConverter がトップレベルの型注釈から登録
- Pass 3: AnyTypeAnalyzer が any-enum を登録
- Pass 4: TypeResolver が body 内の型注釈から登録

重複排除は SyntheticTypeRegistry 側で保証（冪等）。

### 6.7 AnyTypeAnalyzer

```rust
fn analyze_any_params(
    files: &ParsedFiles,
    registry: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
);
```

### 6.8 TypeResolver + FileTypeResolution

```rust
struct TypeResolver<'a> {
    registry: &'a TypeRegistry,
    synthetic: &'a mut SyntheticTypeRegistry,
    module_graph: &'a ModuleGraph,
}

impl<'a> TypeResolver<'a> {
    fn resolve_file(&mut self, file: &ParsedFile) -> FileTypeResolution;
}
```

```rust
struct FileTypeResolution {
    /// 式の型: Span → ResolvedType
    expr_types: HashMap<Span, ResolvedType>,
    /// 期待型: Span → RustType
    expected_types: HashMap<Span, RustType>,
    /// narrowing: スコープ範囲 + 変数名 + narrowing 後の型
    narrowing_events: Vec<NarrowingEvent>,
    /// 変数の mutability
    var_mutability: HashMap<VarId, bool>,
}

#[derive(Hash, Eq, PartialEq)]
struct Span { lo: u32, hi: u32 }

/// 変数の識別子（名前 + 宣言位置で一意）
#[derive(Hash, Eq, PartialEq)]
struct VarId { name: String, declared_at: Span }

enum ResolvedType {
    Known(RustType),
    Unknown,
}

struct NarrowingEvent {
    scope_start: u32,
    scope_end: u32,
    var_name: String,
    narrowed_type: RustType,
}
```

**TypeResolver の走査**:

TypeResolver は自身で AST を走査する。走査中に:
1. 関数宣言 → パラメータ型をスコープに登録
2. 変数宣言 → 型注釈 or 初期化式から型を推定しスコープに登録。TypeConverter を使って型変換（合成型登録を伴う場合あり）
3. 式 → TypeConverter + TypeRegistry + スコープから型を解決し expr_types に格納
4. 期待型 → 親ノードをスタック管理し、子の期待型を自動計算して expected_types に格納
5. if 文の narrowing ガード → NarrowingEvent を生成
6. mutability → 変数宣言の kind + body 内の代入検出で判定

**期待型の自動計算**:

TypeResolver は AST をトップダウンで走査するため、各式を訪問するとき親ノードを把握している:

```
変数宣言 `const x: T = expr` を訪問中:
  → expr の期待型は T（型注釈から）
  → expected_types[expr.span] = T

return 文 `return expr` を訪問中:
  → expr の期待型は現在の関数の戻り値型
  → expected_types[expr.span] = fn_return_type

関数呼び出し `foo(expr)` を訪問中:
  → expr の期待型は foo のパラメータ型
  → expected_types[expr.span] = param_type
```

Transformer は `expected_types[span]` を lookup するだけ。ParentContext の手動構築は不要。

### 6.9 Transformer

```rust
struct TransformContext<'a> {
    module_graph: &'a ModuleGraph,
    type_registry: &'a TypeRegistry,
    synthetic_registry: &'a SyntheticTypeRegistry,
    type_resolution: &'a FileTypeResolution,
    file_path: &'a Path,
}
```

Transformer の責務は **TS の意味論を Rust の意味論に変換すること**。型解決は一切行わない。

| 判断 | 情報源 | Transformer の動作 |
|------|--------|-------------------|
| import パス | `module_graph.resolve_import()` | lookup |
| 式の型 | `type_resolution.expr_types[span]` | lookup |
| 期待型 | `type_resolution.expected_types[span]` | lookup |
| narrowing | `type_resolution.narrowing_events` | 範囲チェック |
| mutability | `type_resolution.var_mutability[var_id]` | lookup |
| 文字列結合 | expr_types で左右の型を取得 | **変換ルールを適用** |
| メソッド変換 | expr_types でレシーバ型を取得 | **変換ルールを適用** |

**Unknown のフォールバック**: TypeResolver が Unknown を返した場合、Transformer は IR の構造から推測する（現在のヒューリスティクス）。将来的に TypeResolver の精度が上がれば Unknown は減る。

### 6.10 Generator

```rust
fn generate(items: &[Item]) -> String;
```

純粋な構文変換。セマンティックな判断なし。現在 Generator が行っている `.as_str()` 付加、enum 分類、regex import スキャンは Transformer に移動。

### 6.11 OutputWriter

```rust
struct OutputWriter<'a> {
    module_graph: &'a ModuleGraph,
}
```

- ファイル書き出し
- mod.rs 生成: `module_graph.children_of()` + `module_graph.reexports_of()` で `pub mod` + `pub use` を生成
- 合成型配置: 各ファイルの IR から合成型への参照を検索し、配置先を決定
- rustfmt 実行

## 7. パイプライン実行フロー

```
Pass 0: Parse
  入力: Vec<(PathBuf, String)>
  出力: ParsedFiles（不変）

Pass 1: Module Graph Construction
  入力: ParsedFiles + ModuleResolver
  出力: ModuleGraph（不変）

Pass 2: Type Collection
  入力: ParsedFiles + ModuleGraph
  出力: TypeRegistry（不変）
  副作用: SyntheticTypeRegistry にトップレベルの合成型を登録

Pass 3: Any-Type Analysis
  入力: ParsedFiles + TypeRegistry
  副作用: SyntheticTypeRegistry に any-enum を登録

Pass 4: Type Resolution (per file)
  入力: ParsedFile + TypeRegistry + SyntheticTypeRegistry + ModuleGraph
  出力: FileTypeResolution（不変、per file）
  副作用: SyntheticTypeRegistry に body 内の合成型を登録

Pass 5: Transformation (per file)
  入力: ParsedFile + FileTypeResolution + ModuleGraph + TypeRegistry + SyntheticTypeRegistry
  出力: Vec<Item>（per file）

Pass 6: Code Generation (per file)
  入力: Vec<Item>
  出力: String

Pass 7: Output
  入力: 全ファイルの生成結果 + ModuleGraph + SyntheticTypeRegistry
  出力: 出力ディレクトリ
```

## 8. 検証

### 8.1 DRY

| 知識 | 所在 | 重複なし？ |
|------|------|-----------|
| TS 型 → Rust 型の変換 | TypeConverter | ✓（TypeCollector と TypeResolver が使用） |
| import specifier → ファイルパス | ModuleResolver | ✓ |
| ファイルパス → Rust モジュールパス | ModuleGraph | ✓ |
| 合成型の重複排除 | SyntheticTypeRegistry | ✓ |
| any 型の制約収集 | AnyTypeAnalyzer | ✓ |
| 式の型推論 | TypeResolver | ✓ |
| 期待型の計算 | TypeResolver | ✓ |
| TS → Rust 意味論変換 | Transformer | ✓ |
| IR → テキスト | Generator | ✓ |
| mod.rs 生成 | OutputWriter | ✓ |

**AST 走査は2回（TypeResolver + Transformer）だが、これは知識の重複ではない**: TypeResolver は「型情報の計算」、Transformer は「IR の構築」を行う。走査メカニズムは同じだが、各ノードで行う処理（知識）は異なる。

### 8.2 直交性

| コンポーネント | 変更理由（唯一） |
|--------------|----------------|
| Parser | SWC API |
| ModuleResolver | 解決戦略 |
| ModuleGraph | モジュール意味論 |
| TypeConverter | 型マッピングルール |
| TypeCollector | 型定義発見ルール |
| SyntheticTypeRegistry | 命名・重複排除ルール |
| AnyTypeAnalyzer | any 具象化戦略 |
| TypeResolver | 型推論・narrowing ルール |
| Transformer | 変換ルール |
| Generator | Rust 構文 |
| OutputWriter | 出力構造 |

全コンポーネントが唯一の変更理由を持つ。第3版の FileAnalyzer は2つの変更理由（型推論 + 変換）を持っていたが、分離により解消。

### 8.3 結合度

- 全依存が上から下への一方向。循環なし
- SyntheticTypeRegistry への書込みは3箇所（TypeConverter, AnyTypeAnalyzer, TypeResolver）だが、append-only かつ冪等
- TypeResolver の出力（FileTypeResolution）は不変データとして Transformer に渡される
- Transformer は TypeResolver に依存しない（FileTypeResolution に依存する）

### 8.4 テスト容易性

| コンポーネント | 単体テスト方法 |
|--------------|--------------|
| ModuleResolver | ファイルシステムの mock + specifier → PathBuf のテスト |
| ModuleGraph | import/export 情報を直接構築 → query API のテスト |
| TypeConverter | TsType AST ノード → RustType のテスト |
| TypeResolver | AST + TypeRegistry → FileTypeResolution のテスト（Transformer なしで実行可能） |
| Transformer | AST + FileTypeResolution → Vec\<Item\> のテスト（TypeResolver なしで実行可能） |
| Generator | Vec\<Item\> → String のテスト |

TypeResolver と Transformer の分離により、型推論のバグと変換ルールのバグを独立にテスト・デバッグできる。

## 9. 第3版からの変更点

| 第3版 | 問題 | 第4版 |
|-------|------|-------|
| FileAnalyzer に TypeResolver と Transformer を統合 | 2つの関心事（型推論 + 変換ルール）が混在。直交性違反 | TypeResolver と Transformer を分離。AST は2回走査するが、Rust 実装のため実用上問題なし |
| var_mutability が `HashMap<String, bool>` | 同名変数のスコープ区別不能 | `HashMap<VarId, bool>` に変更。VarId は名前 + 宣言位置の組 |
