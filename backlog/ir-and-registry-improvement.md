# IR 型システム改善と Registry 2パス化

## 背景・動機

アーキテクチャレビュー（`report/architecture-review.md`）で以下の基盤的な問題が特定された:

### IR の Named 型への文字列エンコード

trait パラメータ変換（`wrap_trait_for_param` / `wrap_trait_for_value`）で `&dyn Greeter` や `Box<dyn Greeter>` を `RustType::Named { name: "&dyn Greeter" }` として文字列にエンコードしている。これにより:

- 型の構造情報が失われ、generator 側で参照型・trait object・Box 型を判定できない
- `is_derivable_type` が `Named { name: "&dyn Greeter" }` を derivable と誤判定する
- 将来の型操作（例: `Option<&dyn Trait>` への対応）で文字列操作が必要になる

該当コード: `transformer/mod.rs` 35行目 `format!("&dyn {name}")`, 54行目 `format!("Box<dyn {name}>")`

### Registry の循環依存と型解決の不正確さ

`build_registry()` が型注釈を変換する際、空の `TypeRegistry::new()` を `convert_ts_type()` に渡している（registry.rs の11箇所以上）。これにより:

- 同一モジュール内で後方参照される型（例: `interface A { b: B }` の `B` が `A` より後に宣言）のフィールド型が空 registry で解決される
- intersection 型のフィールドマージで、宣言順依存の不正確さが生じうる

### register_interface の二重管理

`register()` と `register_interface()` が分離しており、呼び出し側が `register_interface()` を忘れると `is_trait_type()` が誤判定する。

## ゴール

1. `RustType` に参照型（`Ref`）と trait object 型（`DynTrait`）の構造的な表現を追加し、文字列エンコードを排除する
2. `is_derivable_type` が trait object 参照を含む型を正しく非 derivable と判定する
3. Registry 構築が2パスになり、後方参照される型が正確に解決される
4. `register_interface` が `register` に統合される

## スコープ

### 対象

- `RustType` に `Ref(Box<RustType>)` と `DynTrait(String)` バリアントを追加
- `generate_type` で `Ref` → `&T`、`DynTrait` → `dyn T` として生成
- `is_derivable_type` で `DynTrait` → `false` として判定
- `wrap_trait_for_param` を `Ref(DynTrait(name))` に変更
- `wrap_trait_for_value` を `Named { name: "Box", type_args: [DynTrait(name)] }` に変更
- Registry 構築を2パス化
- `TypeDef::Struct` に `is_interface: bool` フラグ追加、`interface_names` セット廃止
- `TypeDef` のコンストラクタヘルパー導入

### 対象外

- `Option<&dyn Trait>` や `Vec<Box<dyn Trait>>` のネスト対応（現時点で需要なし。IR は構造的に表現可能だが、変換ロジックの対応は別 PRD）
- ジェネリクスの型パラメータ解決（I-100 として保留中）
- 型推論の改善（PRD 3 で対応）

## 設計

### IR の拡張

```rust
pub enum RustType {
    // 既存バリアント（変更なし）
    Unit, String, F64, Bool, Option(Box<RustType>), Vec(Box<RustType>),
    Fn { params, return_type }, Result { ok, err },
    Tuple(Vec<RustType>), Any, Never,
    Named { name: String, type_args: Vec<RustType> },
    // 新規
    /// 参照型: `&T`（例: `&dyn Greeter`）
    Ref(Box<RustType>),
    /// Trait object 型: `dyn T`（例: `dyn Greeter`）
    /// `Ref(DynTrait("Greeter"))` → `&dyn Greeter`
    /// `Named { name: "Box", type_args: [DynTrait("Greeter")] }` → `Box<dyn Greeter>`
    DynTrait(String),
}
```

`Ref` と `DynTrait` を分離する理由: `&dyn Trait` は `&` (参照) + `dyn Trait` (trait object) の2つの独立した概念の合成。`Ref` は trait object 以外の参照（将来的に `&str` 等）にも使える。`DynTrait` は `Box<dyn Trait>` でも使われるため、`Ref` の内側に限定されない。

### generator の更新

```rust
RustType::Ref(inner) => format!("&{}", generate_type(inner)),
RustType::DynTrait(name) => format!("dyn {name}"),
```

### is_derivable_type の更新

```rust
RustType::Ref(inner) => is_derivable_type(inner),
RustType::DynTrait(_) => false, // trait object は Clone/PartialEq を実装しない
```

### uses_param の更新

```rust
RustType::Ref(inner) => inner.uses_param(param),
RustType::DynTrait(name) => name == param,
```

### trait ラッピングの修正

```rust
// wrap_trait_for_param: パラメータ位置
RustType::Ref(Box::new(RustType::DynTrait(name.clone())))

// wrap_trait_for_value: 変数・戻り値位置
RustType::Named {
    name: "Box".to_string(),
    type_args: vec![RustType::DynTrait(name.clone())],
}
```

### Registry 2パス化

```rust
pub fn build_registry(module: &ast::Module) -> TypeRegistry {
    let mut reg = TypeRegistry::new();

    // パス 1: 型名と種別を収集。フィールド・メソッドは空のまま登録
    for item in &module.body {
        collect_type_name(&mut reg, item);
    }
    // パス 1 完了時点で、全ての型名が Named で解決可能になる

    // パス 2: 構築済み registry を使って型の中身を解決
    for item in &module.body {
        collect_type_def(&mut reg, item);
    }

    reg
}
```

パス 1 では型名のみを空の `TypeDef` として登録する。パス 2 の `convert_ts_type` 呼び出し時には、パス 1 で登録された型名が registry に存在するため、`Named { name: "Bar" }` として正しく解決される。

パス 1 で登録する placeholder:
```rust
fn collect_type_name(reg: &mut TypeRegistry, item: &ModuleItem) {
    // interface → TypeDef::new_interface(vec![], HashMap::new(), vec![])
    // type alias → TypeDef::new_struct(vec![], HashMap::new(), vec![])
    // class → TypeDef::new_struct(vec![], HashMap::new(), vec![])
    // enum → TypeDef::Enum { variants: vec![], ... }
    // function → TypeDef::Function { params: vec![], ... }
}
```

パス 2 では既存の `collect_decl` を修正し、空 registry ではなく `&reg` を渡す:
```rust
fn collect_type_def(reg: &mut TypeRegistry, item: &ModuleItem) {
    // 既存の collect_decl と同じロジックだが、
    // convert_ts_type(..., &TypeRegistry::new()) を
    // convert_ts_type(..., reg) に置換
}
```

### TypeDef コンストラクタヘルパー

```rust
impl TypeDef {
    pub fn new_struct(fields: Vec<(String, RustType)>, methods: HashMap<...>, extends: Vec<String>) -> Self {
        TypeDef::Struct { fields, methods, extends, is_interface: false }
    }
    pub fn new_interface(fields: Vec<(String, RustType)>, methods: HashMap<...>, extends: Vec<String>) -> Self {
        TypeDef::Struct { fields, methods, extends, is_interface: true }
    }
}
```

既存の `TypeDef::Struct { ... }` 構築は全てヘルパーに移行する。テストコードも含む。パターンマッチ側は `..` を使っているため影響なし。

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/ir.rs` | `RustType::Ref`, `DynTrait` 追加。`uses_param` 対応 |
| `src/generator/types.rs` | `generate_type` の `Ref`, `DynTrait` 対応 |
| `src/generator/mod.rs` | `is_derivable_type` の `Ref`, `DynTrait` 対応 |
| `src/transformer/mod.rs`（or `type_env.rs`） | `wrap_trait_for_param`/`wrap_trait_for_value` の修正 |
| `src/registry.rs` | 2パス化。`is_interface` フラグ追加。コンストラクタヘルパー追加。`interface_names` 廃止 |
| `src/external_types.rs` | `register_interface` 呼び出しの除去。コンストラクタヘルパーへの移行 |
| テストファイル全般 | 期待値更新、TypeDef 構築箇所のヘルパー移行 |

## 作業ステップ

**フェーズ A: IR 拡張と generator 対応**

- [ ] 1: `RustType::Ref` と `DynTrait` を追加。`uses_param` に対応追加。コンパイル確認（未使用バリアントの警告あり）
- [ ] 2: `generate_type` に `Ref` / `DynTrait` 対応追加。ユニットテスト RED → GREEN
- [ ] 3: `is_derivable_type` に `Ref` / `DynTrait` 対応追加。ユニットテスト RED → GREEN
- [ ] 4: `wrap_trait_for_param` を `Ref(DynTrait(name))` に変更。関連テストの期待値更新。全テスト PASS 確認
- [ ] 5: `wrap_trait_for_value` を `Named { name: "Box", type_args: [DynTrait(name)] }` に変更。関連テストの期待値更新。全テスト PASS 確認

**フェーズ B: TypeDef 統合**

- [ ] 6: `TypeDef` にコンストラクタヘルパー `new_struct` / `new_interface` を追加
- [ ] 7: `TypeDef::Struct` に `is_interface: bool` を追加。全構築箇所をヘルパー経由に移行
- [ ] 8: `interface_names` セットと `register_interface()` を廃止。`is_trait_type` を `is_interface` フラグベースに変更。全テスト PASS 確認

**フェーズ C: Registry 2パス化**

- [ ] 9: 2パスの必要性を実証するテスト追加（`interface A { b: B } interface B { x: number }` の順序で宣言し、A の b フィールドが正しく解決されることを検証）。RED 確認
- [ ] 10: `build_registry` を2パスに変更。パス 1 で型名収集、パス 2 で構築済み registry を `convert_ts_type` に渡す。テスト GREEN 確認
- [ ] 11: 全テスト + clippy + fmt PASS 確認

## テスト計画

| テスト | 入力 | 期待出力 |
|-------|------|---------|
| `generate_type` Ref | `Ref(DynTrait("Greeter"))` | `"&dyn Greeter"` |
| `generate_type` DynTrait | `DynTrait("Greeter")` | `"dyn Greeter"` |
| `generate_type` Box dyn | `Named { name: "Box", type_args: [DynTrait("Greeter")] }` | `"Box<dyn Greeter>"` |
| `is_derivable_type` DynTrait | `DynTrait("Greeter")` | `false` |
| `is_derivable_type` Ref DynTrait | `Ref(DynTrait("Greeter"))` | `false` |
| `is_derivable_type` Ref String | `Ref(String)` | `true` |
| trait パラメータ | `function foo(g: Greeter)` (Greeter=trait) | `Ref(DynTrait("Greeter"))` |
| trait 戻り値 | `function make(): Greeter` | `Named { name: "Box", type_args: [DynTrait("Greeter")] }` |
| registry 後方参照 | `interface A { b: B } interface B { x: number }` | A.b が `Named { name: "B" }` で解決 |
| is_trait_type | interface with methods | `true`（is_interface フラグ経由） |
| is_trait_type for class | class with methods | `false`（is_interface = false） |

## 完了条件

- [ ] `RustType::Named` の name に `&` や `dyn ` のプレフィックスが含まれない
- [ ] `RustType::Ref` が参照型を、`RustType::DynTrait` が trait object を構造的に表現している
- [ ] `is_derivable_type` が `DynTrait` を非 derivable と正しく判定する
- [ ] registry 構築が2パスで、後方参照される型が正確に解決される
- [ ] `interface_names` セットが廃止され、`TypeDef::Struct` の `is_interface` フラグに統合されている
- [ ] 全テスト PASS、clippy 0警告、fmt PASS
