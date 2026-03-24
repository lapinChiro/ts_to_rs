# I-218: ジェネリクス基盤の残課題

## 背景・動機

ジェネリクス基盤（`TypeParam`/`instantiate`/`substitute`）は構築済みだが、interface のみが完全対応で、class・type alias・discriminated union enum では型パラメータの収集・置換が未実装。`type_params` を参照する箇所が 169 箇所、`Item::Impl` 生成が 43 箇所あり、後続のジェネリクス依存機能（I-112c, I-101 等）を先に実装すると「type_params が空のときの回避策」が蓄積する。

現在の不整合:

| 宣言形式 | type_params 収集 | substitute_types |
|---|---|---|
| interface | ✅ | ✅ (Struct) |
| class | ❌ 無視される | ✅ (Struct) |
| type alias (struct) | ❌ 無視される | ✅ (Struct) |
| type alias (DU enum) | ❌ 無視される | ❌ clone のみ |
| Item::Impl | フィールド自体がない | — |

## ゴール

1. `class Foo<T> { value: T }` の TypeDef に `type_params: [T]` が格納される
2. `type Pair<A, B> = { first: A, second: B }` の TypeDef に `type_params: [A, B]` が格納される
3. `TypeRegistry::instantiate("Result", &[RustType::String])` が `TypeDef::Enum` の `variant_fields` 内の型パラメータを正しく置換する
4. `impl<T> Foo<T> { ... }` が生成される（ジェネリック class の impl ブロック）
5. 全テスト GREEN、clippy 0 警告、ベンチマーク結果維持

## スコープ

### 対象

- `collect_class_info` での class 型パラメータ収集
- `collect_decl` の `TsTypeAlias` 分岐での型パラメータ収集
- `substitute_types` の `TypeDef::Enum` 対応
- `Item::Impl` への `type_params` フィールド追加
- Generator の `Item::Impl` 処理での `impl<T>` 生成
- `ClassInfo` への `type_params` フィールド追加と伝播
- `make_struct` / `make_impl` ヘルパーへの `type_params` パラメータ追加
- `type_converter.rs` の `convert_interface_as_struct_and_trait` での Impl type_params 設定

### 対象外

- ジェネリック intersection（I-101 のスコープ）
- call signature のジェネリック型パラメータ（I-181 のスコープ）
- 型注釈なし変数のオブジェクトリテラル推論（I-112c のスコープ）
- ジェネリクスの制約推論（`T extends X` の高度な推論）

## 設計

### 技術的アプローチ

4 つの独立した欠落を、interface の既存実装をパターンとして補完する。

#### 1. class 型パラメータ収集

`collect_class_info`（`src/registry.rs:500`）に `collect_type_params(class.class.type_params.as_deref(), lookup, synthetic)` を追加し、`TypeDef::Struct` の `type_params` に格納する。

`new_struct` は `type_params: vec![]` をハードコードしている。2 つの選択肢:
- A) `new_struct` にパラメータを追加する
- B) `new_struct` は非ジェネリック用に残し、`collect_class_info` で直接 `TypeDef::Struct { type_params, ... }` を構築する

**B を採用**: `new_struct` の呼び出し元は 3 箇所（`collect_class_info`, `collect_decl` の type alias, テスト）。パラメータを追加すると全呼び出し元に影響する。しかし、`new_struct` は「非ジェネリックの struct を簡潔に作る」ヘルパーとして存続価値がある。ジェネリック対応が必要な箇所は直接 `TypeDef::Struct { ... }` を構築する方がインテントが明確。

#### 2. type alias 型パラメータ収集

`collect_decl` の `TsTypeAlias` 分岐（`src/registry.rs:402`）で、各パスに `collect_type_params` を追加する。

type alias は 4 つの分岐がある:
1. string literal union → `TypeDef::Enum`（type_params 不要 — `type Color = "red" | "blue"` にジェネリクスはない）
2. discriminated union → `TypeDef::Enum`（type_params が必要 — `type Result<T> = { kind: "ok", value: T } | ...`）
3. fn type alias → `TypeDef::Function`（type_params が IR 側で処理済み — `Item::TypeAlias` が `type_params` を持つ）
4. struct 型（intersection 等）→ `TypeDef::Struct`（type_params が必要）

分岐 2 と 4 で `collect_type_params` を呼び出し、TypeDef に格納する。分岐 2 は `try_collect_discriminated_union` の返り値（`TypeDef::Enum`）に type_params を設定する必要がある。

`try_collect_discriminated_union`（`src/registry.rs`）の返り値の TypeDef::Enum は type_params を持つが、現在は `vec![]` で初期化されている。呼び出し元で上書きするか、引数で渡す。

**呼び出し元で上書きを採用**: `try_collect_discriminated_union` は型パラメータの概念に無関係な関数（union のバリアント解析に集中すべき）。呼び出し元の `collect_decl` で `TypeDef::Enum { type_params, .. }` のフィールドを設定する方が責務が明確。

#### 3. substitute_types の Enum 対応

`TypeDef::substitute_types`（`src/registry.rs:108`）の `other => other.clone()` 分岐を `TypeDef::Enum` の明示的な処理に置き換える。`variant_fields` 内の各フィールドの `RustType` に対して `substitute` を呼ぶ。`variants` の `EnumVariant::value` の型（`EnumValue::Typed` の型）も置換する。

#### 4. Item::Impl の type_params 追加

**IR 変更**: `Item::Impl` に `type_params: Vec<TypeParam>` フィールドを追加。

**Generator 変更**: `generate_type_params(type_params)` を使って `impl<T> StructName<T>` を生成。`for_trait` がある場合は `impl<T> TraitName for StructName<T>` になる。struct_name に型引数を付加する必要がある（`StructName` → `StructName<T>`）。

**Transformer 変更**:
- `ClassInfo` に `type_params: Vec<TypeParam>` を追加。`extract_class_info` で `class_decl.class.type_params` から収集
- `make_struct` / `make_impl` のシグネチャを変更し、`type_params` を受け取る
- `generate_standalone_class` / `generate_parent_class_items` / `generate_child_class` / `generate_abstract_class_items` で `ClassInfo.type_params` を伝播
- `type_converter.rs` の `convert_interface_as_struct_and_trait` で Impl に type_params を設定

### 設計整合性レビュー

- **高次の整合性**: ジェネリクス基盤（`TypeParam`/`instantiate`/`substitute`）のアーキテクチャに完全に合致。interface で確立されたパターンを class/type alias に拡張するだけで、新しいアーキテクチャの導入はない
- **DRY**: `collect_type_params` が共通ユーティリティとして既に存在。新たな重複は発生しない
- **直交性**: 4 つのサブ課題は互いに独立して実装・テスト可能。ただし T4（Item::Impl）は T1（class type_params）の結果を使うため論理的な依存がある
- **結合度**: `TypeDef` の `type_params` フィールドは既存のパターンの延長。新しい結合は発生しない
- **割れ窓**: `new_struct` / `new_interface` が `type_params: vec![]` をハードコードしている点は設計上の割れ窓だが、非ジェネリック用の簡便ヘルパーとしての存在価値があるため現状維持。ジェネリック対応が必要な箇所では直接 `TypeDef::Struct { ... }` を構築する

### 影響範囲

| ファイル | 変更内容 |
|---|---|
| `src/registry.rs` | `collect_class_info` に type_params 収集追加。`collect_decl` TsTypeAlias 分岐に type_params 収集追加。`substitute_types` に Enum 対応追加 |
| `src/ir.rs` | `Item::Impl` に `type_params: Vec<TypeParam>` 追加 |
| `src/generator/mod.rs` | `Item::Impl` の生成で `impl<T> StructName<T>` 対応 |
| `src/transformer/classes.rs` | `ClassInfo` に `type_params` 追加。`make_struct`/`make_impl` に `type_params` パラメータ追加。全ヘルパー関数で type_params を伝播 |
| `src/pipeline/type_converter.rs` | `convert_interface_as_struct_and_trait` の Impl に type_params 設定 |
| テストファイル | `Item::Impl` 構築箇所に `type_params: vec![]` 追加 |

## タスク一覧

### T1: class 型パラメータ収集

- **作業内容**: `src/registry.rs` の `collect_class_info` に `collect_type_params(class.class.type_params.as_deref(), lookup, synthetic)` を追加。返り値を `TypeDef::new_struct(...)` から `TypeDef::Struct { type_params, fields, methods, extends: vec![], is_interface: false }` に変更
- **完了条件**: `class Foo<T> { value: T }` を含むソースから `TypeRegistry` を構築したとき、`TypeDef::Struct { type_params: [TypeParam { name: "T", .. }], .. }` が登録される。テスト 2 件追加（単一/複数型パラメータ + 制約付き）。全テスト GREEN
- **依存**: なし

### T2: type alias 型パラメータ収集

- **作業内容**: `src/registry.rs` の `collect_decl` で `TsTypeAlias` 分岐の以下を修正:
  - discriminated union パス: `try_collect_discriminated_union` の返り値 `TypeDef::Enum` の `type_params` を `collect_type_params(alias.type_params.as_deref(), ...)` で上書き
  - struct パス（intersection 等）: `TypeDef::new_struct(...)` を `TypeDef::Struct { type_params, ... }` に変更
- **完了条件**: `type Pair<A, B> = { first: A, second: B }` で `type_params: [A, B]` が登録される。`type Result<T> = { kind: "ok", value: T } | { kind: "error", msg: string }` で DU Enum に `type_params: [T]` が登録される。テスト 2 件追加。全テスト GREEN
- **依存**: なし

### T3: substitute_types の Enum 対応

- **作業内容**: `src/registry.rs` の `substitute_types` で `other => other.clone()` を `TypeDef::Enum` の明示的処理に変更。`variant_fields` 内の各 `(String, RustType)` の `RustType` に `substitute(bindings)` を適用。`variants` の `EnumValue::Typed` の型にも `substitute` を適用
- **完了条件**: `type Result<T> = { kind: "ok", value: T } | { kind: "error", msg: string }` を `instantiate("Result", &[RustType::String])` した場合、`variant_fields["Ok"]` の `value` フィールドが `RustType::String` に置換される。テスト 2 件追加（基本 + 複数型パラメータ）。全テスト GREEN
- **依存**: T2（DU enum に type_params が設定されている前提）

### T4: Item::Impl に type_params フィールド追加

- **作業内容**:
  - `src/ir.rs` の `Item::Impl` に `type_params: Vec<TypeParam>` フィールドを追加
  - コードベース全体の `Item::Impl { ... }` 構築箇所に `type_params: vec![]` を追加（コンパイルを通す）
  - `src/generator/mod.rs` の `Item::Impl` 処理を変更: `generate_type_params(type_params)` で `impl<T>` を生成。`struct_name` に型引数を付加して `impl<T> StructName<T>` を生成。`for_trait` がある場合は `impl<T> TraitName for StructName<T>`
- **完了条件**: `Item::Impl { type_params: vec![TypeParam { name: "T", .. }], struct_name: "Foo", .. }` から `impl<T> Foo<T> { ... }` が生成される。trait impl の場合は `impl<T> TraitName for Foo<T> { ... }` が生成される。テスト 3 件追加（基本/制約付き/trait impl）。全テスト GREEN
- **依存**: なし（IR + Generator の変更のみ）

### T5: ClassInfo への type_params 追加と伝播

- **作業内容**:
  - `src/transformer/classes.rs` の `ClassInfo` に `type_params: Vec<TypeParam>` フィールドを追加
  - `extract_class_info` で `class_decl.class.type_params` から `collect_type_params` 相当のロジックで収集（Transformer に `collect_type_params` は直接使えないため、`convert_ts_type` ベースで型パラメータを構築）
  - `make_struct` に `type_params: Vec<TypeParam>` パラメータを追加し、`Item::Struct { type_params, .. }` に設定
  - `make_impl` に `type_params: Vec<TypeParam>` パラメータを追加し、`Item::Impl { type_params, .. }` に設定
  - `generate_standalone_class` / `generate_parent_class_items` / `generate_child_class` / `generate_abstract_class_items` で `ClassInfo.type_params` を `make_struct`/`make_impl` に伝播
  - `type_converter.rs` の `convert_interface_as_struct_and_trait` で `Item::Impl` に `type_params` を設定
- **完了条件**: `class Box<T> { value: T; constructor(val: T) { this.value = val; } }` から `pub struct Box<T> { pub value: T }` + `impl<T> Box<T> { pub fn new(val: T) -> Self { ... } }` が生成される。E2E テストまたは snapshot テストで検証。全テスト GREEN
- **依存**: T1（class type_params が TypeRegistry に登録済み）, T4（Item::Impl に type_params フィールドあり）

### T6: 品質チェック + ベンチマーク

- **作業内容**: `cargo fix` → `cargo fmt` → `cargo clippy` → `cargo test` → Hono ベンチマーク
- **完了条件**: 全品質チェック通過。ベンチマーク結果が 86 clean / 132 errors 以上
- **依存**: T1-T5

## テスト計画

| テスト | 検証内容 | 期待結果 |
|---|---|---|
| `test_collect_class_type_params_single` | `class Foo<T>` の type_params 収集 | `type_params: [T]` |
| `test_collect_class_type_params_with_constraint` | `class Foo<T extends Bar>` | `type_params: [T: Bar]` |
| `test_collect_type_alias_struct_type_params` | `type Pair<A, B> = { ... }` | `type_params: [A, B]` |
| `test_collect_type_alias_du_enum_type_params` | `type Result<T> = { kind: "ok", value: T } \| ...` | Enum に `type_params: [T]` |
| `test_substitute_types_enum_variant_fields` | DU Enum の instantiate | `variant_fields` 内の型が置換される |
| `test_substitute_types_enum_multiple_params` | 複数型パラメータの DU Enum | 全パラメータが正しく置換される |
| `test_generate_impl_with_type_params` | `Item::Impl { type_params: [T] }` の生成 | `impl<T> Foo<T> { ... }` |
| `test_generate_impl_with_constraint` | 制約付き type_params | `impl<T: Clone> Foo<T> { ... }` |
| `test_generate_impl_for_trait_with_type_params` | trait impl + type_params | `impl<T> TraitName for Foo<T> { ... }` |
| `test_class_generic_e2e` | class 全体の変換 | Struct + Impl が正しいジェネリクス付きで生成 |
| 既存テスト全体 | 後方互換性 | 全テスト GREEN |
| Hono ベンチマーク | 変換品質の維持 | 86 clean / 132 errors 以上 |

## 完了条件

- [ ] `class Foo<T>` の TypeDef に `type_params` が格納される
- [ ] `type Pair<A, B> = { ... }` の TypeDef に `type_params` が格納される
- [ ] `TypeRegistry::instantiate` が `TypeDef::Enum` の `variant_fields` を正しく置換する
- [ ] `Item::Impl` に `type_params` フィールドがあり、Generator が `impl<T> StructName<T>` を生成する
- [ ] ジェネリック class の Struct + Impl が正しい型パラメータ付きで生成される
- [ ] `cargo test` 全 GREEN
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] `cargo fmt --all --check` 通過
- [ ] Hono ベンチマーク結果が 86 clean / 132 errors 以上
