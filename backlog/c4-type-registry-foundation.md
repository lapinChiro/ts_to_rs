# C-4: TypeRegistry 型登録基盤改善

## Background

OBJECT_LITERAL_NO_TYPE エラー 29件の残存原因の一部は、TypeRegistry に型情報が正しく登録されないことに起因する。C-3 の調査（`report/c3-object-literal-no-type-deep-analysis-2026-03-30.md`）で各エラーの根本原因を個別トレースし、以下の3種の TypeRegistry 登録欠陥を特定した:

1. **TypeAlias の TsTypeRef RHS が未登録**（I-307）: `type BodyCache = Partial<Body>` が空 struct として登録される
2. **callable interface が TypeDef::Struct として登録される**（I-305）: `interface GetCookie { (c, key): Cookie }` で return 型が伝播しない
3. **indexed access 複合名が型パラメータ解決で未処理**（I-308）: `E['Bindings']` → `Named("E::Bindings")` が制約解決されない

加えて、同一コードパスの以下2件をバッチ化する:

4. **callable interface/関数型エイリアスの rest パラメータ未収集**（I-259）: `try_collect_call_signature_fn` が `TsFnParam::Rest` を無視
5. **interface の ConstructSignature 未収集**（I-277）: `collect_interface_methods` が `TsConstructSignatureDeclaration` を無視

## Goal

- TypeAlias の TsTypeRef RHS（`Partial<T>`, `Required<T>`, 単純型参照）が TypeRegistry に正しくフィールド情報付きで登録される
- callable interface（call signature のみ）の return 型・param 型が `resolve_fn_type_info` で取得可能になる
- `resolve_type_params_in_type` が `"E::Bindings"` 形式の複合名を型パラメータ制約から解決できる
- 関数型の rest パラメータが TypeDef::Function に正しく収集される
- interface の construct signature が TypeDef::Struct.constructor に収集される
- ベンチマーク: OBJECT_LITERAL_NO_TYPE 29→25 以下（-4件以上）

## Scope

### In Scope

- `collect_type_alias_fields` に TsTypeRef ブランチ追加（トップレベル + intersection 内）
- `TypeDef::Struct` に `call_signatures: Vec<MethodSignature>` フィールド追加
- callable interface の call signature を `call_signatures` フィールドに収集
- `resolve_fn_type_info` が `TypeDef::Struct.call_signatures` から関数型情報を抽出
- callable-only 判定の DRY 化（`is_callable_only` 共通関数）
- `resolve_type_params_impl` に `"::"` 複合名の分解解決ロジック追加
- `try_collect_call_signature_fn` / `try_collect_fn_type_alias` に `TsFnParam::Rest` 対応
- `collect_interface_methods` に `TsConstructSignatureDeclaration` 対応
- 全テストギャップの解消

### Out of Scope

- I-300 imported 関数の TypeRegistry 登録（マルチファイルモードの問題）
- I-301 匿名構造体自動生成（C-5 で対応。C-4 の基盤修正で対象件数が変わる可能性あり）
- I-306 `.map()` callback 型伝播（`propagate_expected` の拡張は独立した設計）
- I-263 外部型エイリアスの TypeRegistry 登録（コードパスが独立）
- `"::"` 複合名の IR 構造化（消費箇所 5+ のため別PRD規模）

## Design

### Technical Approach

#### T1: `TypeDef::Struct` に `call_signatures` フィールド追加

`src/registry/mod.rs` の `TypeDef::Struct` に:
```rust
TypeDef::Struct {
    // ... existing fields ...
    /// Call signatures for callable interfaces.
    /// e.g., `interface GetCookie { (c: Context): Cookie; (c: Context, key: string): string }`
    call_signatures: Vec<MethodSignature>,
}
```

`TypeDef::new_struct` と `TypeDef::new_interface` の両方で `call_signatures: vec![]` を初期化。

#### T2: callable interface の call signature 収集

`src/registry/interfaces.rs` の `collect_interface_methods` に `TsCallSignatureDecl` ハンドラを追加。収集結果を呼び出し元（`collection.rs`）で `TypeDef::Struct.call_signatures` に設定。

同時に `TsConstructSignatureDeclaration` ハンドラも追加し（I-277）、収集結果を `constructor` フィールドに設定。

#### T3: `resolve_fn_type_info` の拡張

`src/pipeline/type_resolver/helpers.rs` の `resolve_fn_type_info` に `TypeDef::Struct` + `call_signatures` のマッチを追加:

```rust
RustType::Named { name, .. } => {
    if let Some(type_def) = registry.get(name) {
        match type_def {
            TypeDef::Function { return_type, params, .. } => { /* existing */ }
            TypeDef::Struct { call_signatures, .. } if !call_signatures.is_empty() => {
                let sig = select_overload(call_signatures, ...);
                (sig.return_type, Some(sig.params))
            }
            _ => (None, None)
        }
    }
}
```

#### T4: callable-only 判定の DRY 化

`src/registry/interfaces.rs` に共通関数:
```rust
pub(super) fn is_callable_only(members: &[TsTypeElement]) -> bool
```
`src/pipeline/type_converter/interfaces.rs` と `src/registry/functions.rs` の重複判定を置き換え。

#### T5: `collect_type_alias_fields` の TsTypeRef 対応

`src/registry/collection.rs` の `collect_type_alias_fields` で、`_ => None` を `TsTypeRef` ブランチに拡張。`convert_ts_type` で RustType に変換し、結果が `Named` なら registry/synthetic からフィールドを取得。

intersection ブランチ内の TsTypeRef 処理（:498-506）も同じロジックに統一。現在は型引数を無視して `reg.get(name)` で直接取得しているが、`Partial<T>` 等のユーティリティ型が展開されない。

#### T6: rest パラメータ収集（I-259）

`src/registry/functions.rs` の `try_collect_fn_type_alias`（:22）と `try_collect_call_signature_fn`（:72）に `TsFnParam::Rest` 対応を追加。`has_rest` フラグを正しく設定。

#### T7: `resolve_type_params_impl` の複合名解決（I-308）

`src/pipeline/type_resolver/expected_types.rs` の `resolve_type_params_impl` で、`Named { name, type_args: [] }` かつ `name.contains("::")` の場合にベース部分を制約から解決:

```rust
if type_args.is_empty() && name.contains("::") {
    if let Some((base, field)) = name.split_once("::") {
        if let Some(constraint) = self.type_param_constraints.get(base) {
            let resolved_base = self.resolve_type_params_impl(constraint, depth + 1);
            // resolved_base の field を lookup
        }
    }
}
```

### Design Integrity Review

- **Higher-level consistency**: `TypeDef::Struct` に `call_signatures` を追加することで、callable interface が Struct のまま関数型情報も保持できる。`TypeDef::Function` への変換ではなく拡張なので、既存の Struct 前提のコードパスに影響しない
- **DRY**: callable-only 判定の共通化（T4）で `functions.rs` と `interfaces.rs` の重複を解消。`collect_type_alias_fields` の TsTypeRef 解決ロジックはトップレベルと intersection 内で共有（T5）
- **Orthogonality**: 各修正は独立したコードパスに作用し、相互依存は T1（構造変更）→ T2, T3（使用箇所）のみ
- **Broken windows**: `collection.rs:499-504` の intersection 内 TsTypeRef が型引数を無視している問題を T5 で修正

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `src/registry/mod.rs` | `TypeDef::Struct` に `call_signatures` フィールド追加 |
| `src/registry/interfaces.rs` | call signature + construct signature 収集、`is_callable_only` 抽出 |
| `src/registry/collection.rs` | interface 登録で call_signatures/constructor 設定、`collect_type_alias_fields` TsTypeRef 対応 |
| `src/registry/functions.rs` | rest パラメータ収集、`is_callable_only` 使用 |
| `src/pipeline/type_resolver/helpers.rs` | `resolve_fn_type_info` 拡張 |
| `src/pipeline/type_resolver/expected_types.rs` | `resolve_type_params_impl` 複合名解決 |
| `src/pipeline/type_converter/interfaces.rs` | `is_callable_only` 使用 |
| 多数のテストファイル | `TypeDef::Struct` 構築箇所に `call_signatures: vec![]` 追加 |

### Semantic Safety Analysis

**T5 (TsTypeRef 解決)**: `collect_type_alias_fields` が `None` → `Some(fields)` に変わることで、以前は空 struct だった TypeAlias が正しいフィールド付き struct になる。これは「エラー → 正しい型」の遷移であり、既存の正しく変換されたコードには影響しない。Safe。

**T7 (複合名解決)**: `Named("E::Bindings")` が具体型に解決されることで、expected type が設定される。以前は expected type なし → 変換エラー（OBJECT_LITERAL_NO_TYPE）だったケースが、正しい expected type 付きになる。Safe。

**T3 (call_signatures)**: callable interface の return 型が `resolve_fn_type_info` から返されるようになることで、`current_fn_return_type` が設定される。以前は `None` だったため return 文の expected type が未設定だった。新たに設定されることで、return 文のオブジェクトリテラルが正しく変換される。Safe。

## Task List

### T1: `TypeDef::Struct` に `call_signatures` フィールド追加

- **Work**: `src/registry/mod.rs` の `TypeDef::Struct` に `call_signatures: Vec<MethodSignature>` を追加。`new_struct`, `new_interface` で `call_signatures: vec![]` を初期化。全テストファイルの `TypeDef::Struct` 構築箇所に `call_signatures: vec![]` を追加
- **Completion criteria**: `cargo check` 通過。既存テスト全 PASS
- **Depends on**: None

### T2: callable interface の call/construct signature 収集 + DRY 化

- **Work**:
  - `src/registry/interfaces.rs` に `is_callable_only(members: &[TsTypeElement]) -> bool` を追加
  - `collect_interface_methods` に `TsCallSignatureDecl` と `TsConstructSignatureDeclaration` のハンドラ追加。call signature は `Vec<MethodSignature>` として返し、construct signature は既存の `MethodSignature` 形式で返す
  - `src/registry/collection.rs` の interface 登録で `call_signatures` と `constructor` を設定
  - `src/pipeline/type_converter/interfaces.rs` の callable-only 判定を `is_callable_only` に置き換え
  - `src/registry/functions.rs` の `try_collect_call_signature_fn` の callable-only 判定を `is_callable_only` に置き換え
- **Completion criteria**: callable-only interface が `TypeDef::Struct { call_signatures: [...] }` として登録される。construct signature 付き interface が `constructor: Some(...)` として登録される。`cargo test` 全 PASS
- **Depends on**: T1

### T3: `resolve_fn_type_info` の `call_signatures` 対応

- **Work**: `src/pipeline/type_resolver/helpers.rs` の `resolve_fn_type_info` に `TypeDef::Struct { call_signatures, .. }` マッチを追加。`call_signatures` が非空なら `select_overload` で最適シグネチャを選択し、`(return_type, params)` を返す
- **Completion criteria**: `const getCookie: GetCookie = (c, key?) => { return {} }` で `current_fn_return_type` が設定される。callable interface expected type テスト PASS
- **Depends on**: T2

### T4: `collect_type_alias_fields` の TsTypeRef 対応

- **Work**:
  - `src/registry/collection.rs` の `collect_type_alias_fields` に `TsTypeRef` ブランチ追加。`convert_ts_type` で RustType に変換し、`Named` なら registry/synthetic からフィールド取得
  - intersection ブランチ内の TsTypeRef 処理（:498-506）も同じロジックに統一。型引数付き TsTypeRef（`Partial<T>` 等）を正しく展開
  - 共通ヘルパー `resolve_type_ref_fields` を抽出してトップレベルと intersection で共有
- **Completion criteria**: `type BodyCache = Partial<Body>` が全フィールド `Option` 付き struct として登録される。`type X = Named & Partial<T>` の intersection でもユーティリティ型が展開される
- **Depends on**: T1

### T5: rest パラメータ収集（I-259）

- **Work**: `src/registry/functions.rs` の `try_collect_fn_type_alias`（:22）と `try_collect_call_signature_fn`（:72）に `TsFnParam::Rest` 対応を追加。`has_rest: true` を設定
- **Completion criteria**: `type Handler = { (...args: string[]): void }` が `TypeDef::Function { has_rest: true }` として登録される
- **Depends on**: None

### T6: `resolve_type_params_impl` の複合名解決（I-308）

- **Work**: `src/pipeline/type_resolver/expected_types.rs` の `resolve_type_params_impl` に `"::"` 複合名の分解解決ロジック追加。ベース部分を制約から解決し、解決結果の型からフィールドを取得
- **Completion criteria**: `E['Bindings']` の expected type が `Env` の `Bindings` フィールド型に解決される。テスト PASS
- **Depends on**: None

### T7: テストギャップ解消

- **Work**: 調査で特定した全テストギャップを解消:
  - `collect_type_alias_fields` TsTypeRef RHS テスト
  - intersection 内 TsTypeLit & TsTypeLit テスト
  - interface callable-only → call_signatures 登録テスト
  - interface construct signature 登録テスト
  - `resolve_fn_type_info` + Struct call_signatures テスト
  - callable interface expected type → arrow return type 伝播テスト
  - `resolve_type_params_impl` + "::" 複合名テスト
  - rest パラメータ収集テスト
  - E2E: callable interface の変換結果コンパイルテスト（fixture 追加）
- **Completion criteria**: 全テストギャップのテスト追加。`cargo test` 全 PASS
- **Depends on**: T1-T6

## Test Plan

### 新規テスト（機能変更由来）

| テスト | 対象 | 種別 |
|-------|------|------|
| TypeAlias TsTypeRef → Struct フィールド | T4 | Registry unit |
| TypeAlias Partial<T> → 全フィールド Option | T4 | Registry unit |
| intersection 内 Partial<T> 展開 | T4 | Registry unit |
| callable interface → call_signatures 登録 | T2 | Registry unit |
| construct signature → constructor 登録 | T2 | Registry unit |
| resolve_fn_type_info + call_signatures | T3 | TypeResolver unit |
| callable interface → arrow return type 伝播 | T3 | TypeResolver integration |
| resolve_type_params_impl + "::" 複合名 | T6 | TypeResolver unit |
| rest パラメータ収集 | T5 | Registry unit |

### 既存テストギャップ（テストカバレッジレビュー由来）

| ギャップ | パターン | 技法 | 重要度 |
|---------|---------|------|-------|
| intersection TsTypeLit & TsTypeLit | 未テストの同値分割 | Equivalence | 中 |
| intersection 全メンバーフィールドなし → None | 境界値 | Boundary | 低 |
| intersection 未登録 TsTypeRef → None | エラーパス | C1 | 中 |
| is_callable_only: 0 call sig | 境界値 | Boundary | 低 |
| is_callable_only: call sig + property (mixed) | 同値分割 | Equivalence | 中 |

### E2E テスト

| フィクスチャ | 内容 |
|------------|------|
| `callable-interface.input.ts` | callable interface → 関数型変換のコンパイル検証 |
| `type-alias-utility.input.ts` | `type X = Partial<T>` の変換結果コンパイル検証 |

## Completion Criteria

1. 全テスト PASS（既存 + 新規）
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 エラー
3. `cargo fmt --all --check` 通過
4. ベンチマーク: OBJECT_LITERAL_NO_TYPE 29→25 以下
5. `resolve_fn_type_info` が callable interface の return type を正しく返す
6. `type BodyCache = Partial<Body>` が正しいフィールド付き TypeDef::Struct として登録される
7. `E['Bindings']` の expected type が `Env` の `Bindings` フィールド型に解決される
8. plan.md / TODO の更新
