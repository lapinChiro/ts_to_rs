# I-100: ジェネリック型の基盤 + 具体化

## 背景・動機

TypeScript のジェネリクスは型システムの中核であり、`Container<T>`, `ReadableStream<Uint8Array>`, `Promise<string>` 等が広く使われる。現在のトランスパイラは:

- 型パラメータ名の抽出（`extract_type_params`）は行うが、`TypeDef` に格納しない
- 型引数（`Array<string>` の `string`）は `RustType::Named { type_args }` に保持するが、フィールド型への代入（substitution）を行わない
- trait 変換時に型パラメータを破棄している（`let _ = type_params;`）
- `TypeRegistry` にジェネリクスのインスタンス化メカニズムがない

これにより I-101（ジェネリック intersection）、I-181（call signature ジェネリクス）、I-58（Trait の type_params）が全てブロックされている。

## ゴール

1. `TypeDef::Struct` と `TypeDef::Enum` が型パラメータ情報を保持する
2. `Item::Trait` が型パラメータを出力し、`trait Processor<T>` が生成される
3. `TypeRegistry` がジェネリック型のインスタンス化（`Container<string>` → フィールド型の `T` → `String` 代入）を実行できる
4. 型パラメータの制約（`T extends Foo`）が Rust の trait bound（`T: Foo`）として生成される
5. ジェネリック関数・メソッドの型パラメータが Rust コードに反映される

## スコープ

### 対象

- **TypeDef 拡張**: `type_params: Vec<TypeParam>` を `Struct`, `Enum` に追加（`TypeParam` は名前 + オプショナルな制約）
- **TypeRegistry のインスタンス化**: `instantiate(name, args) → TypeDef` メソッド（型引数で型パラメータを代入した TypeDef を返す）
- **RustType の代入**: `RustType::substitute(params, args)` メソッド（型パラメータ名を具体型に置換）
- **Item::Trait の型パラメータ**: 型パラメータの出力と Rust コード生成
- **制約の収集と生成**: `TsTypeParam::constraint` → `T: Bound` の変換
- **I-58 統合**: Trait の type_params 未対応を同時に解消

### 対象外

- ジェネリックメソッド呼び出し時の型引数推論（TypeScript のコンテキストに依存し、明示的な型引数のみ対応）
- 高階型（Higher Kinded Types）
- variance（共変・反変）の追跡

## 設計

### 技術的アプローチ

#### 1. 型パラメータの IR 表現

```rust
/// ジェネリック型パラメータ
pub struct TypeParam {
    pub name: String,
    pub constraint: Option<RustType>,  // T: Foo の Foo
}
```

`TypeDef::Struct` に追加:
```rust
Struct {
    type_params: Vec<TypeParam>,  // 新規追加
    fields: Vec<(String, RustType)>,
    methods: HashMap<String, MethodSignature>,
    extends: Vec<String>,
    is_interface: bool,
}
```

#### 2. 型パラメータの収集

`collect_interface_methods` と並行して、`TsTypeParamDecl` から `TypeParam` を収集:

```rust
fn collect_type_params(decl: Option<&TsTypeParamDecl>, reg: &TypeRegistry) -> Vec<TypeParam> {
    decl.map(|d| d.params.iter().map(|p| TypeParam {
        name: p.name.sym.to_string(),
        constraint: p.constraint.as_ref()
            .and_then(|c| convert_ts_type(c, &mut vec![], reg).ok()),
    }).collect()).unwrap_or_default()
}
```

#### 3. RustType の代入（substitution）

```rust
impl RustType {
    /// 型パラメータ名を具体型に置換する
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> RustType {
        match self {
            RustType::Named { name, type_args } => {
                if let Some(concrete) = bindings.get(name.as_str()) {
                    concrete.clone()
                } else {
                    RustType::Named {
                        name: name.clone(),
                        type_args: type_args.iter().map(|a| a.substitute(bindings)).collect(),
                    }
                }
            }
            RustType::Vec(inner) => RustType::Vec(Box::new(inner.substitute(bindings))),
            RustType::Option(inner) => RustType::Option(Box::new(inner.substitute(bindings))),
            RustType::Ref(inner) => RustType::Ref(Box::new(inner.substitute(bindings))),
            // ... 他のバリアントも再帰的に処理
            other => other.clone(),
        }
    }
}
```

#### 4. TypeRegistry のインスタンス化

```rust
impl TypeRegistry {
    /// ジェネリック型を具体型引数でインスタンス化する
    pub fn instantiate(&self, name: &str, args: &[RustType]) -> Option<TypeDef> {
        let type_def = self.get(name)?;
        let type_params = type_def.type_params();
        if type_params.is_empty() || args.len() != type_params.len() {
            return Some(type_def.clone());  // 非ジェネリックまたは引数不一致
        }
        let bindings: HashMap<String, RustType> = type_params.iter()
            .zip(args.iter())
            .map(|(p, a)| (p.name.clone(), a.clone()))
            .collect();
        Some(type_def.substitute(&bindings))
    }
}
```

#### 5. Rust コード生成

`Item::Trait` と `Item::Struct` の型パラメータをジェネレータに反映:

```rust
// 入力: interface Processor<T extends Serializable>
// 出力: pub trait Processor<T: Serializable> { fn process(&self, input: T) -> T; }
```

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/ir.rs` | `TypeParam` 構造体追加、`Item::Trait` / `Item::Struct` に `type_params` 追加、`RustType::substitute` 実装 |
| `src/registry.rs` | `TypeDef` に `type_params` 追加、`instantiate` メソッド、`collect_type_params` 関数 |
| `src/transformer/types/mod.rs` | `convert_interface_as_trait` で型パラメータを保持（現在の `let _ = type_params;` を修正） |
| `src/generator/items.rs` | Trait / Struct の型パラメータ生成 |
| `src/generator/types.rs` | 制約の生成（`T: Bound`） |
| テストファイル | 新規テストケース追加 |

## 作業ステップ

- [ ] ステップ1（RED）: ジェネリック interface の型パラメータが Rust trait に反映されるテストを書く（`trait Processor<T>`）
- [ ] ステップ2（RED）: 型パラメータの制約が trait bound として生成されるテストを書く（`T: Serializable`）
- [ ] ステップ3（RED）: `TypeRegistry::instantiate` で `Container<string>` のフィールド型が `String` に解決されるテストを書く
- [ ] ステップ4（GREEN）: `TypeParam` 構造体と `RustType::substitute` を実装
- [ ] ステップ5（GREEN）: `TypeDef` に `type_params` を追加、`collect_type_params` を実装
- [ ] ステップ6（GREEN）: `TypeRegistry::instantiate` を実装
- [ ] ステップ7（GREEN）: `convert_interface_as_trait` で型パラメータを保持し、`Item::Trait` に渡す
- [ ] ステップ8（GREEN）: ジェネレータで型パラメータと制約を出力
- [ ] ステップ9（REFACTOR）: 既存のジェネリック関連コードとの整合性確認
- [ ] ステップ10: E2E スナップショットテスト

## テスト計画

### 単体テスト

- `interface Processor<T> { process(input: T): T; }` → `trait Processor<T> { fn process(&self, input: T) -> T; }`
- `interface Container<T extends Clone> { value: T; }` → `struct Container<T: Clone> { pub value: T }`
- `TypeRegistry::instantiate("Container", [String])` → フィールド `value: String`
- `RustType::substitute` で再帰的な代入（`Vec<T>` → `Vec<String>`）
- 型パラメータなしの型に `instantiate` しても元の TypeDef が返る
- 型引数の数が不一致の場合にインスタンス化がスキップされる

### E2E テスト

- ジェネリック interface/type alias を含む TS ファイルの変換スナップショット

## 完了条件

- 全テストパターンが GREEN
- `trait Processor<T>` 形式の Rust コードが生成される
- `TypeRegistry::instantiate` がジェネリック型を具体化できる
- I-58（Trait の type_params）が解消される
- `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- `cargo fmt --all --check` が通る
- `cargo test` が全パス
- `cargo llvm-cov` のカバレッジ閾値を満たす
