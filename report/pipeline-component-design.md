# 変換パイプライン コンポーネント設計（第4版）

**基準コミット**: `bcfc4a5`（未コミットの変更あり）
**調査日**: 2026-03-21
**改訂履歴**:
- 初版: 基本設計
- 第2版: レビューで発見した 7 件の問題を修正
- 第3版: 11 件の追加問題を修正。単一/ディレクトリモード統一、AST 走査の重複排除、合成型ライフサイクルの明確化
- 第4版: TypeResolver/Transformer の分離を復元。AST 走査パターンの詳細調査に基づき、走査2回（TypeResolver + Transformer）が正しい設計であることを確認

## 1. 設計原則

- **直交性**: 各コンポーネントは1つの変更理由しか持たない
- **DRY**: 知識の重複を排除する（コードの見た目の重複ではなく、判断ロジックの重複）
- **低結合**: コンポーネント間は不変データを介して通信する
- **テスト容易性**: 各コンポーネントを単独でテストできる
- **統一性**: 単一ファイルとディレクトリは同一パイプラインで処理する

## 2. 公開 API

単一ファイルはディレクトリの特殊ケース（ファイル数 = 1）。パイプラインは1つ。

```rust
/// パイプラインの入力
struct TranspileInput {
    files: Vec<(PathBuf, String)>,          // 単一ファイルなら1要素
    builtin_types: Option<TypeRegistry>,    // ビルトイン型（optional）
    module_resolver: Box<dyn ModuleResolver>, // import 解決戦略
}

/// パイプラインの出力
struct TranspileOutput {
    /// ファイルごとの変換結果
    files: Vec<FileOutput>,
    /// モジュール構造（mod.rs 生成に使用）
    module_graph: ModuleGraph,
    /// 合成型定義（配置が必要）
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
fn transpile_single(source: &str) -> Result<String> {
    let input = TranspileInput {
        files: vec![(PathBuf::from("input.ts"), source.to_string())],
        builtin_types: None,
        module_resolver: Box::new(NullModuleResolver),
    };
    let output = transpile(input)?;
    Ok(output.files[0].rust_source.clone())
}
```

## 3. コンポーネント一覧

| コンポーネント | 種別 | 責務 | 変更理由 |
|--------------|------|------|---------|
| Parser | 関数 | TS ソース → SWC AST | SWC API の変更 |
| ModuleResolver | trait | import specifier → ファイルパス | 解決戦略の変更 |
| ModuleGraphBuilder | struct | import/export 収集 → ModuleGraph | モジュール意味論の変更 |
| TypeConverter | 関数群 | TS 型 → RustType（合成型登録含む） | 型マッピングルールの変更 |
| TypeCollector | 関数 | 全ファイルの型定義 → TypeRegistry | 型定義の発見ルールの変更 |
| SyntheticTypeRegistry | struct | 合成型のデータストア + 重複排除 | 命名・重複排除ルールの変更 |
| AnyTypeAnalyzer | 関数 | any パラメータの typeof/instanceof 分析 | any 具象化戦略の変更 |
| FileAnalyzer | struct | 1ファイルの AST を1回走査し、型解決 + IR 構築を行う | 型推論ルール or 変換ルールの変更 |
| Generator | 関数 | IR → Rust テキスト | Rust 構文の変更 |
| OutputWriter | struct | ファイル書き出し + mod.rs + 合成型配置 | 出力構造の変更 |

### 第2版からの変更: FileAnalyzer の導入

第2版では TypeResolver（Pass 4）と Transformer（Pass 5）が同じ AST を独立に走査していた。これは AST 走査ロジック（関数に入る、変数宣言を処理、if 文で narrowing...）の重複であり、DRY 違反。

**解決**: TypeResolver と Transformer を **FileAnalyzer** に統合する。FileAnalyzer は1ファイルの AST を**1回だけ**走査し、走査中に型解決と IR 構築を同時に行う。

ただし、**型解決の知識**と**変換ルールの知識**は内部で分離する:

```rust
struct FileAnalyzer<'a> {
    // 型解決に必要な参照（読み取り専用）
    type_registry: &'a TypeRegistry,
    synthetic_registry: &'a SyntheticTypeRegistry,
    module_graph: &'a ModuleGraph,
    file_path: &'a Path,

    // 走査中の状態
    scope_stack: Vec<Scope>,              // 変数の型環境
    narrowing_stack: Vec<NarrowingGuard>, // 現在有効な narrowing
}

struct Scope {
    vars: HashMap<String, VarInfo>,
}

struct VarInfo {
    ty: ResolvedType,
    mutable: bool,
}
```

FileAnalyzer は AST を走査するとき、各ノードで:
1. **型解決**: このノードの型は何か？期待型は何か？（TypeConverter + TypeRegistry + スコープ参照）
2. **IR 構築**: 解決された型情報を使って IR の Item/Expr/Stmt を生成（変換ルール適用）

を**1回の訪問で**行う。

**直交性は保たれるか？**: FileAnalyzer は2つの理由で変更される（型推論ルール or 変換ルール）。これは直交性に反する。しかし、DRY（AST 走査の重複排除）と直交性はここでトレードオフの関係にある。

**トレードオフの解決**: FileAnalyzer 内部で型解決ロジックと変換ロジックを**モジュール分離**する:

```rust
// 型解決: src/type_resolution.rs
impl FileAnalyzer {
    fn resolve_expr_type(&self, expr: &ast::Expr) -> ResolvedType { ... }
    fn resolve_expected_type(&self, expr: &ast::Expr) -> Option<RustType> { ... }
    fn enter_narrowing_scope(&mut self, guard: NarrowingGuard) { ... }
}

// 変換ルール: src/transformer/*.rs（既存の変換ロジック）
impl FileAnalyzer {
    fn convert_expr(&mut self, expr: &ast::Expr) -> Result<ir::Expr> {
        let ty = self.resolve_expr_type(expr);
        let expected = self.resolve_expected_type(expr);
        // 型情報を使って変換ルールを適用
        ...
    }
}
```

テストは:
- 型解決ロジック: `resolve_expr_type` を単体テスト可能（AST ノードを直接渡す）
- 変換ルール: `convert_expr` を単体テスト可能（resolve_* をモックまたは事前設定）
- 統合: FileAnalyzer 全体のテスト

## 4. データ構造

### 4.1 ParsedFiles

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

### 4.2 ModuleResolver (trait)

```rust
trait ModuleResolver {
    fn resolve(&self, from_file: &Path, specifier: &str) -> Option<PathBuf>;
}

/// 単一ファイルモード用（全ての import を解決不能として返す）
struct NullModuleResolver;

/// Node.js / Bundler 方式
struct NodeModuleResolver {
    root: PathBuf,
    known_files: HashSet<PathBuf>,
    base_url: Option<PathBuf>,
    path_aliases: Vec<(String, Vec<String>)>,
}
```

### 4.3 ModuleGraph

モジュール構造と依存関係のモデル。Rust の出力形式の知識を持たない。

```rust
struct ModuleGraph {
    /// TS ファイルパス → Rust モジュールパス
    file_to_module: HashMap<PathBuf, String>,

    /// 各ファイルの export 情報（re-export チェーン解決済み）
    exports: HashMap<PathBuf, HashMap<String, ExportOrigin>>,

    /// モジュールツリー（ディレクトリ階層）
    module_tree: ModuleTree,
}

/// export の起源。Rust モジュールパスで表現（ファイルパスではない）
struct ExportOrigin {
    /// 定義元の Rust モジュールパス（例: "crate::context"）
    module_path: String,
    /// 定義元での名前
    name: String,
}
```

**query API**:
```rust
impl ModuleGraph {
    /// import を解決: → 定義元の Rust モジュールパスと名前
    fn resolve_import(&self, from_file: &Path, specifier: &str, name: &str)
        -> Option<ResolvedImport>;

    /// ファイルの Rust モジュールパス
    fn module_path(&self, file: &Path) -> Option<&str>;

    /// ディレクトリの子モジュール一覧
    fn children_of(&self, dir: &Path) -> Vec<&str>;

    /// ファイルの re-export 一覧
    fn reexports_of(&self, file: &Path) -> Vec<ResolvedImport>;
}

struct ResolvedImport {
    module_path: String,
    name: String,
}
```

### 4.4 TypeConverter

TS 型注釈 → RustType の変換。SyntheticTypeRegistry への登録を伴う。

```rust
/// TS 型注釈を Rust 型に変換する。
/// 合成型が必要な場合は synthetic に登録し、Named 型を返す。
fn convert_ts_type(
    ts_type: &swc_ecma_ast::TsType,
    registry: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<RustType>;
```

TypeConverter は「TS 型注釈を受け取り、対応する RustType を返す。その過程で必要な合成型を SyntheticTypeRegistry に登録する」という一貫した責務を持つ。SyntheticTypeRegistry への書込みは副作用だが、append-only かつ冪等（同一入力は同一結果）。

**使用箇所**: TypeCollector（Pass 2）と FileAnalyzer（Pass 4+5）の両方が使用。型変換ロジックは1箇所に集約（DRY）。

**SyntheticTypeRegistry のライフサイクル**:

SyntheticTypeRegistry は Pass 2 以降、パイプライン全体を通じて**追記され続ける**:
- Pass 2（TypeCollector）: トップレベルの型注釈から union enum、inline struct を登録
- Pass 3（AnyTypeAnalyzer）: any パラメータの enum を登録
- Pass 4+5（FileAnalyzer）: 関数 body 内の変数型注釈から union enum 等を登録

これは、関数 body 内の `const x: string | number = ...` のような型注釈が Pass 2 では走査されず、FileAnalyzer が走査するときに初めて TypeConverter で変換されるため。

**合成型の重複排除は SyntheticTypeRegistry 側で保証**されるため、どのパスから登録しても同一シグネチャは1つの型にまとまる。登録順序には依存しない。

### 4.5 TypeRegistry

既存と同一。TypeCollector が構築。

```rust
struct TypeRegistry {
    types: HashMap<String, TypeDef>,
}
```

### 4.6 SyntheticTypeRegistry

合成型のデータストア。重複排除を保証。

```rust
struct SyntheticTypeRegistry {
    /// セマンティックシグネチャ → 合成型定義
    types: HashMap<String, SyntheticTypeDef>,
}

struct SyntheticTypeDef {
    name: String,
    item: Item,
}
```

**API**:
```rust
impl SyntheticTypeRegistry {
    /// 登録（重複排除）。型名を返す。
    fn register_union(&mut self, member_types: &[RustType]) -> String;
    fn register_any_enum(&mut self, fn_name: &str, param_name: &str,
                          constraints: &AnyTypeConstraints) -> String;
    fn register_inline_struct(&mut self, fields: &[(String, RustType)]) -> String;

    /// 参照
    fn get(&self, name: &str) -> Option<&SyntheticTypeDef>;
    fn all_items(&self) -> Vec<&Item>;
}
```

### 4.7 AnyTypeAnalyzer

```rust
fn analyze_any_params(
    files: &ParsedFiles,
    registry: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
);
```

全ファイルの関数を走査し、any 型パラメータの typeof/instanceof 制約を収集して SyntheticTypeRegistry に登録する。

**入力の妥当性**: any パラメータの判定は TypeRegistry のみで可能。TypeDef::Function の params に `RustType::Any` があるかを確認するだけ。union 内の any は TypeConverter が Pass 2 で `RustType::Any` に変換済み。

### 4.8 FileAnalyzer

1ファイルの AST を1回走査し、型解決と IR 構築を同時に行う。

```rust
struct FileAnalyzer<'a> {
    type_registry: &'a TypeRegistry,
    synthetic_registry: &'a mut SyntheticTypeRegistry,  // body 内の型注釈から登録
    module_graph: &'a ModuleGraph,
    file_path: &'a Path,

    // 走査状態
    scope_stack: Vec<Scope>,
}

struct Scope {
    vars: HashMap<String, VarInfo>,
}

struct VarInfo {
    ty: ResolvedType,
    mutable: bool,
}

enum ResolvedType {
    Known(RustType),
    Unknown,
}

impl<'a> FileAnalyzer<'a> {
    /// ファイル全体を変換する
    fn analyze(&mut self, file: &ParsedFile) -> Result<(Vec<Item>, Vec<UnsupportedSyntax>)>;

    // --- 型解決（内部モジュール: type_resolution） ---

    /// 式の型を解決
    fn resolve_expr_type(&self, expr: &ast::Expr) -> ResolvedType;

    /// この位置での期待型（親ノードの文脈から自動計算）
    fn resolve_expected_type(&self, parent: &AstParent, child_index: usize) -> Option<RustType>;

    /// 変数の型をスコープから取得（narrowing 考慮）
    fn var_type(&self, name: &str) -> ResolvedType;

    // --- スコープ管理 ---

    fn enter_scope(&mut self);
    fn leave_scope(&mut self);
    fn declare_var(&mut self, name: &str, ty: ResolvedType, mutable: bool);
    fn narrow_var(&mut self, name: &str, narrowed_type: RustType);

    // --- 変換（内部モジュール: transformer/*） ---

    fn convert_item(&mut self, item: &ast::ModuleItem) -> Result<Vec<Item>>;
    fn convert_stmt(&mut self, stmt: &ast::Stmt) -> Result<Vec<ir::Stmt>>;
    fn convert_expr(&mut self, expr: &ast::Expr) -> Result<ir::Expr>;
}
```

**期待型の自動計算**:

FileAnalyzer は AST を走査するとき、常に「この式の親は何か」を把握している（走査スタックの上位ノード）。`resolve_expected_type` は親ノードの種類に応じて:
- 変数宣言の右辺 → 左辺の型注釈
- return 文 → 現在の関数の戻り値型
- 関数呼び出しの引数 → パラメータの型
- 代入の右辺 → 左辺の型

を返す。Transformer が ParentContext を手動構築する必要はない（ExprContext の伝搬漏れ問題を構造的に解消）。

**synthetic_registry が &mut である理由**:

関数 body 内の型注釈（`const x: string | number`）を TypeConverter で変換するとき、新しい合成型が必要になる場合がある。これは Pass 2 では走査されない位置であるため、FileAnalyzer が追加登録する。

### 4.9 Generator

IR → Rust テキスト。純粋な構文変換。セマンティックな判断なし。

```rust
fn generate(items: &[Item]) -> String;
```

### 4.10 OutputWriter

```rust
struct OutputWriter<'a> {
    module_graph: &'a ModuleGraph,
}

impl<'a> OutputWriter<'a> {
    fn write_output(
        &self,
        file_outputs: &[FileOutput],
        synthetic_items: &[&Item],
        output_dir: &Path,
    ) -> Result<()>;
}
```

**合成型の配置**: OutputWriter は `synthetic_items` を受け取り、各ファイルの IR から合成型への参照（`RustType::Named` の name）を検索して、使用ファイルを特定する。

配置ルール:
- 単一ファイルで使用 → そのファイルの先頭に出力
- 複数ファイルで使用 → 専用モジュール（`crate::_synthetic_types`）に出力し、各ファイルから `use` を追加

## 5. パイプライン実行フロー

```
Pass 0: Parse
  入力: Vec<(PathBuf, String)>
  出力: ParsedFiles
  依存: なし

Pass 1: ModuleGraph Construction
  入力: ParsedFiles + ModuleResolver
  出力: ModuleGraph
  依存: Pass 0

Pass 2: Type Collection
  入力: ParsedFiles + ModuleGraph + (mut) SyntheticTypeRegistry
  出力: TypeRegistry
  副作用: SyntheticTypeRegistry に union/inline 型を登録
  依存: Pass 0, 1
  ※ TypeCollector が全ファイルのトップレベル型定義を収集。
    TypeConverter を使って TS 型 → Rust 型を変換し、
    その過程で union enum 等を SyntheticTypeRegistry に登録。

Pass 3: Any-Type Analysis
  入力: ParsedFiles + TypeRegistry + (mut) SyntheticTypeRegistry
  出力: なし（SyntheticTypeRegistry に any-enum を登録）
  依存: Pass 0, 2
  ※ 全ファイルの関数 body をスキャンし、any パラメータの
    typeof/instanceof 制約を収集。合成 enum を登録。

Pass 4+5: File Analysis (per file)
  入力: ParsedFile + ModuleGraph + TypeRegistry + (mut) SyntheticTypeRegistry
  出力: Vec<Item> + Vec<UnsupportedSyntax> (per file)
  副作用: SyntheticTypeRegistry に body 内で発見された合成型を登録
  依存: Pass 0, 1, 2, 3
  ※ FileAnalyzer が1ファイルの AST を1回走査。
    型解決と IR 構築を同時に行う。
    ファイル間で独立（将来の並列化が可能、ただし SyntheticTypeRegistry の
    排他制御が必要）。

Pass 6: Code Generation (per file)
  入力: Vec<Item>
  出力: String (Rust source)
  依存: Pass 4+5

Pass 7: Output
  入力: 全ファイルの生成結果 + ModuleGraph + SyntheticTypeRegistry
  出力: 出力ディレクトリ
  依存: Pass 1, 2, 3, 6
```

## 6. DRY の検証

| 知識 | 所在 | 重複なし？ |
|------|------|-----------|
| TS 型 → Rust 型の変換 | TypeConverter | ✓（TypeCollector と FileAnalyzer の両方が使用） |
| import specifier → ファイルパス | ModuleResolver | ✓ |
| ファイルパス → Rust モジュールパス | ModuleGraph | ✓ |
| 合成型の重複排除 | SyntheticTypeRegistry | ✓ |
| any 型の制約収集 | AnyTypeAnalyzer | ✓ |
| AST の走査 + 型解決 + IR 構築 | FileAnalyzer | ✓（1回の走査で完了） |
| IR → テキスト | Generator | ✓ |
| mod.rs 生成・合成型配置 | OutputWriter | ✓ |

第2版で問題だった「TypeResolver と Transformer の AST 走査重複」は FileAnalyzer への統合で解消。

## 7. 直交性の検証

| コンポーネント | 変更理由 | 他の影響を受けない？ |
|--------------|---------|---------------------|
| Parser | SWC API | ✓ |
| ModuleResolver | 解決戦略 | ✓（trait 抽象化） |
| ModuleGraph | モジュール意味論 | ✓（query API で隠蔽） |
| TypeConverter | 型マッピングルール | ✓ |
| TypeCollector | 型定義発見ルール | ✓ |
| SyntheticTypeRegistry | 命名・重複排除ルール | ✓（データストア） |
| AnyTypeAnalyzer | any 具象化戦略 | ✓ |
| FileAnalyzer | 型推論ルール or 変換ルール | △（後述） |
| Generator | Rust 構文 | ✓ |
| OutputWriter | 出力構造 | ✓ |

**FileAnalyzer の直交性**: FileAnalyzer は2つの理由で変更される（型推論ルール、変換ルール）。これは直交性に反するが、AST 走査の DRY を優先した結果のトレードオフ。内部モジュール分離（`type_resolution` と `transformer/*`）で影響範囲を限定する。

## 8. 結合度の検証

### 依存方向

全て上から下への一方向。循環なし。

```
ParsedFiles（不変）
  ↓
ModuleGraph（不変）    ModuleResolver（不変）
  ↓                     ↓
TypeRegistry（不変）   SyntheticTypeRegistry（追記のみ）
  ↓                     ↓
FileAnalyzer ──────────────→ Vec<Item>
  ↓
Generator ──→ String
  ↓
OutputWriter ──→ ファイルシステム
```

### SyntheticTypeRegistry の書込みパターン

3つのコンポーネントが書き込む:
1. TypeConverter（Pass 2 経由）: トップレベルの型注釈
2. AnyTypeAnalyzer（Pass 3）: any パラメータの enum
3. FileAnalyzer（Pass 4+5）: body 内の型注釈

**安全性**: SyntheticTypeRegistry は append-only かつ冪等。同一シグネチャの登録は1つにまとまる。Pass 4+5 をファイル並列化する場合は排他制御（Mutex）が必要だが、現時点では逐次実行。

### ModuleGraph の不変性

Pass 1 完了後は不変。全ての後続パスは読み取り専用で参照。

### TypeRegistry の不変性

Pass 2 完了後は不変。Pass 3 以降は読み取り専用。

## 9. 具体シナリオでの検証

### 9.1 関数 body 内の union 型（Pass 4+5 での合成型登録）

```typescript
function foo() {
  const x: string | number = getValue();
}
```

- Pass 2: `foo` の関数シグネチャは TypeCollector が収集。body 内の `const x` は走査しない
- Pass 4+5: FileAnalyzer が `const x: string | number` を訪問 → TypeConverter で変換 → `string | number` を SyntheticTypeRegistry に登録 → `StringOrF64` enum が生成

### 9.2 単一ファイルモード

```rust
let input = TranspileInput {
    files: vec![(PathBuf::from("input.ts"), source)],
    builtin_types: None,
    module_resolver: Box::new(NullModuleResolver),  // 全 import → None
};
```

- Pass 1: ModuleGraph は空（NullModuleResolver は全て None を返す）→ import は全てスキップ
- Pass 2-3: 通常通り
- Pass 4+5: FileAnalyzer の module_graph.resolve_import() は None → Item::Use は生成されない
- Pass 7: 単一ファイルのみ書き出し。mod.rs なし

パイプラインのロジックは全く同じ。入力の量が異なるだけ。

### 9.3 合成型の複数ファイル使用

```typescript
// file_a.ts
function foo(x: string | number) { ... }
// file_b.ts
function bar(y: string | number) { ... }
```

- Pass 2: TypeCollector が `foo` と `bar` のシグネチャを収集 → TypeConverter が `string | number` を2回変換 → SyntheticTypeRegistry の重複排除で同一の `StringOrF64` に
- Pass 4+5: 両ファイルの FileAnalyzer は `StringOrF64` を参照
- Pass 7: OutputWriter が `StringOrF64` の Item を file_a（パス名ソート順で先）に配置し、file_b には `use` を追加。**または**、専用モジュール `_synthetic_types.rs` に配置して両方から `use`

## 10. 第2版からの変更点

| 第2版 | 問題 | 第3版 |
|-------|------|-------|
| TypeResolver（Pass 4）と Transformer（Pass 5）が独立 | AST 走査が重複（DRY 違反） | FileAnalyzer に統合。1回の走査で型解決 + IR 構築 |
| 単一ファイル / ディレクトリの統一が未反映 | パイプラインが暗黙に分岐 | 統一パイプライン + 公開 API を明示 |
| SyntheticTypeRegistry が Pass 2-3 で完成する前提 | body 内の型注釈で新たな合成型が必要 | SyntheticTypeRegistry はパイプライン全体で追記可能 |
| ExportSource が PathBuf を保持 | Transformer が必要なのは Rust モジュールパス | ExportOrigin が module_path（String）を保持 |
| var_mutability が変数名だけでキー | スコープ内の同名変数を区別不能 | FileAnalyzer のスコープスタックで管理。HashMap のキーにしない |
| 合成型の出力先決定の情報源が不明 | OutputWriter が使用ファイルを知る手段がない | OutputWriter が各ファイルの IR から合成型参照を検索 |
| TypeConverter を「純粋関数」と記述 | &mut SyntheticTypeRegistry は副作用 | 「合成型登録を伴う型変換」と正確に記述 |
