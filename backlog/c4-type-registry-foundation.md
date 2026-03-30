# C-4: TypeRegistry 型登録基盤改善

## Background

OBJECT_LITERAL_NO_TYPE エラー 29件の残存原因の一部は、TypeRegistry に型情報が正しく登録されないことに起因する。C-3 の調査（`report/c3-object-literal-no-type-deep-analysis-2026-03-30.md`）で各エラーの根本原因を個別トレースし、以下の3種の TypeRegistry 登録欠陥を特定した:

1. **TypeAlias の TsTypeRef RHS が未登録**（I-307）: `type BodyCache = Partial<Body>` が空 struct として登録される
2. **callable interface が TypeDef::Struct として登録される**（I-305）: `interface GetCookie { (c, key): Cookie }` で return 型が伝播しない
3. **indexed access 複合名が型パラメータ解決で未処理**（I-308）: `E['Bindings']` → `Named("E::Bindings")` が制約解決されない

加えて、同一コードパスの以下をバッチ化する:

4. **callable interface/関数型エイリアスの rest パラメータ未収集**（I-259）: `try_collect_call_signature_fn` / `try_collect_fn_type_alias` が `TsFnParam::Rest` を無視（`has_rest: false` 固定）
5. **interface の ConstructSignature 未収集**（I-277）: `collect_interface_methods` が `TsConstructSignatureDeclaration` を無視

さらに、PRD 作成前の影響範囲レビュー（2a: Production Code Quality Review）で以下の問題を検出し、同一コードパス上のためバッチ化する:

6. **パラメータ抽出ロジックの3重DRY違反**: `interfaces.rs:54-70`（TsFnParam系）、`collection.rs:367-382`（Pat系）、`functions.rs:127-137`（Pat系）にほぼ同一のrest パラメータ抽出コードが存在
7. **callable-only 検出の2重DRY違反**: `type_converter/interfaces.rs:28-37` と `registry/functions.rs:53-65` で同一ロジックが異なるコードパターンで実装
8. **`collect_arrow_def_with_extras` の `Pat::Assign` 未処理**: `functions.rs:159-189` がデフォルトパラメータの `Option` ラップを処理しない（同ファイルの `collect_fn_def_with_extras` では処理済み）
9. **`collect_type_alias_fields` の doc comment 不正確**: 実際の処理範囲と記述が不一致

## Goal

- TypeAlias の TsTypeRef RHS（`Partial<T>`, `Required<T>`, 単純型参照）が TypeRegistry に正しくフィールド情報付きで登録される
- callable interface（call signature のみ）の return 型・param 型が `resolve_fn_type_info` で取得可能になる
- `resolve_type_params_in_type` が `"E::Bindings"` 形式の複合名を型パラメータ制約から解決できる
- 関数型の rest パラメータが TypeDef::Function に正しく収集される
- interface の construct signature が TypeDef::Struct.constructor に収集される
- パラメータ抽出ロジックが共通ヘルパーに統合され、DRY 違反が解消されている
- callable-only 判定が共通関数に統合されている
- アロー関数のデフォルトパラメータが `Option` ラップされる
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
- パラメータ抽出ヘルパーの共通化（`TsFnParam` 系 + `Pat` 系）
- `collect_arrow_def_with_extras` の `Pat::Assign` 対応
- doc comment 修正
- 全テストギャップの解消（G1-G9 + 新規機能テスト）

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

#### T2: パラメータ抽出ヘルパーの共通化

`src/registry/` 内に2つの共通ヘルパーを作成:

1. **`extract_ts_fn_param`**: `TsFnParam` → `(name, RustType)` の変換（`interfaces.rs` と `functions.rs` の `TsFnParam::Ident` / `TsFnParam::Rest` 処理を統合）
2. **`extract_pat_param`**: `Pat` → `(name, RustType)` の変換（`collection.rs` と `functions.rs` の `Pat::Ident` / `Pat::Assign` / `Pat::Rest` 処理を統合）

配置先: `src/registry/functions.rs` 内に `pub(super)` として定義（パラメータ抽出は関数型収集の責務に最も近い）。

これにより:
- `interfaces.rs:54-70` の `TsFnParam` 処理 → `extract_ts_fn_param` 呼び出しに置き換え
- `collection.rs:356-386` の class method パラメータ処理 → `extract_pat_param` 呼び出しに置き換え
- `functions.rs:107-140` の fn decl パラメータ処理 → `extract_pat_param` 呼び出しに置き換え
- `functions.rs:159-189` の arrow パラメータ処理 → `extract_pat_param` 呼び出しに置き換え

`extract_pat_param` は `Pat::Assign`（デフォルトパラメータの `Option` ラップ）も処理するため、P7（arrow の Pat::Assign 未処理）が自動的に解消される。

#### T3: callable interface の call/construct signature 収集 + callable-only DRY 化

`src/registry/interfaces.rs` に:
- `pub(crate) fn is_callable_only(members: &[TsTypeElement]) -> bool` を追加
- `collect_interface_methods` に `TsCallSignatureDecl` と `TsConstructSignatureDeclaration` のハンドラ追加

call signature は `Vec<MethodSignature>` として返し、呼び出し元（`collection.rs`）で `TypeDef::Struct.call_signatures` に設定。construct signature は `constructor` フィールドに設定。

`src/pipeline/type_converter/interfaces.rs` と `src/registry/functions.rs` の callable-only 判定を `is_callable_only` に置き換え。

#### T4: `resolve_fn_type_info` の `call_signatures` 対応

`src/pipeline/type_resolver/helpers.rs` の `resolve_fn_type_info` に `TypeDef::Struct { call_signatures, .. }` マッチを追加:

```rust
TypeDef::Struct { call_signatures, .. } if !call_signatures.is_empty() => {
    let sig = select_overload(call_signatures, arg_count);
    (sig.return_type.clone(), Some(sig.params.clone()))
}
```

#### T5: `collect_type_alias_fields` の TsTypeRef 対応

`src/registry/collection.rs` の `collect_type_alias_fields` に `TsTypeRef` ブランチ追加。`convert_ts_type` で RustType に変換し、`Named` なら registry/synthetic からフィールド取得。

intersection ブランチ内の TsTypeRef 処理（:498-506）も同じロジックに統一。型引数付き TsTypeRef（`Partial<T>` 等）を正しく展開。共通ヘルパー `resolve_type_ref_fields` を抽出してトップレベルと intersection で共有。

doc comment（P8）も修正: 対応する TsType バリアント（TsTypeLit, TsIntersectionType, TsTypeRef）を明記。

#### T6: rest パラメータ収集（I-259）

T2 のヘルパー共通化により、`extract_ts_fn_param` が `TsFnParam::Rest` を処理する。`try_collect_fn_type_alias` と `try_collect_call_signature_fn` でこのヘルパーを使用し、rest パラメータがあれば `has_rest: true` を設定。

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
- **DRY**: 3つの統合を実施:
  1. callable-only 判定の共通化（T3）で `type_converter/interfaces.rs` と `registry/functions.rs` の重複を解消
  2. パラメータ抽出ヘルパーの共通化（T2）で 3 ファイルの重複を解消
  3. `resolve_type_ref_fields` の抽出（T5）でトップレベルと intersection 内の重複を解消
- **Orthogonality**: パラメータ抽出をヘルパーに分離することで、各収集関数がパラメータのパース詳細から解放される
- **Broken windows**:
  - `collection.rs:499-504` の intersection 内 TsTypeRef が型引数を無視 → T5 で修正
  - `functions.rs:159-189` の arrow Pat::Assign 未処理 → T2 のヘルパー化で自動解消
  - `collection.rs:444-449` の doc comment 不正確 → T5 で修正

### Production Code Quality Issues

| Issue | Location | Category | Severity | Action |
|-------|----------|----------|----------|--------|
| P1 | `functions.rs:30,94` | 機能欠陥 | High | T6 で修正 |
| P2 | `type_converter/interfaces.rs` + `functions.rs` | DRY | High | T3 で修正 |
| P3 | `interfaces.rs:54-70` + `collection.rs:367-382` + `functions.rs:127-137` | DRY | High | T2 で修正 |
| P4 | `interfaces.rs:31-90` | 機能欠陥 | High | T3 で修正 |
| P5 | `helpers.rs:170-203` | 機能欠陥 | High | T4 で修正 |
| P6 | `collection.rs:445-502` | 機能欠陥 | Medium | T5 で修正 |
| P7 | `functions.rs:159-189` | 不整合 | Medium | T2 で修正 |
| P8 | `collection.rs:444-449` | doc comment | Low | T5 で修正 |

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `src/registry/mod.rs` | `TypeDef::Struct` に `call_signatures` フィールド追加 |
| `src/registry/interfaces.rs` | call/construct signature 収集、`is_callable_only` 抽出 |
| `src/registry/collection.rs` | interface 登録で call_signatures/constructor 設定、`collect_type_alias_fields` TsTypeRef 対応、doc 修正 |
| `src/registry/functions.rs` | パラメータ抽出ヘルパー追加、rest パラメータ収集、`is_callable_only` 使用、arrow Pat::Assign 修正 |
| `src/pipeline/type_resolver/helpers.rs` | `resolve_fn_type_info` 拡張 |
| `src/pipeline/type_resolver/expected_types.rs` | `resolve_type_params_impl` 複合名解決 |
| `src/pipeline/type_converter/interfaces.rs` | `is_callable_only` 使用 |
| 多数のテストファイル | `TypeDef::Struct` 構築箇所に `call_signatures: vec![]` 追加 |

### Semantic Safety Analysis

**T5 (TsTypeRef 解決)**: `collect_type_alias_fields` が `None` → `Some(fields)` に変わることで、以前は空 struct だった TypeAlias が正しいフィールド付き struct になる。これは「エラー → 正しい型」の遷移であり、既存の正しく変換されたコードには影響しない。Safe。

**T7 (複合名解決)**: `Named("E::Bindings")` が具体型に解決されることで、expected type が設定される。以前は expected type なし → 変換エラー（OBJECT_LITERAL_NO_TYPE）だったケースが、正しい expected type 付きになる。Safe。

**T4 (call_signatures)**: callable interface の return 型が `resolve_fn_type_info` から返されるようになることで、`current_fn_return_type` が設定される。以前は `None` だったため return 文の expected type が未設定だった。新たに設定されることで、return 文のオブジェクトリテラルが正しく変換される。Safe。

## Task List

### T1: `TypeDef::Struct` に `call_signatures` フィールド追加

- **Work**: `src/registry/mod.rs` の `TypeDef::Struct` に `call_signatures: Vec<MethodSignature>` を追加。`new_struct`, `new_interface` で `call_signatures: vec![]` を初期化。全テストファイルの `TypeDef::Struct` 構築箇所に `call_signatures: vec![]` を追加
- **Completion criteria**: `cargo check` 通過。既存テスト全 PASS
- **Depends on**: None

### T2: パラメータ抽出ヘルパーの共通化

- **Work**:
  - `src/registry/functions.rs` に `pub(super) fn extract_ts_fn_param(param: &TsFnParam, lookup, synthetic) -> Option<(String, RustType)>` を追加。`TsFnParam::Ident` と `TsFnParam::Rest` を処理
  - `src/registry/functions.rs` に `pub(super) fn extract_pat_param(pat: &Pat, lookup, synthetic) -> Option<(String, RustType)>` を追加。`Pat::Ident`, `Pat::Assign`（Option ラップ）, `Pat::Rest` を処理
  - `src/registry/interfaces.rs` の `collect_interface_methods` 内パラメータ処理を `extract_ts_fn_param` に置き換え
  - `src/registry/collection.rs` の `collect_class_info` 内パラメータ処理を `extract_pat_param` に置き換え
  - `src/registry/functions.rs` の `collect_fn_def_with_extras` と `collect_arrow_def_with_extras` を `extract_pat_param` に置き換え（arrow の `Pat::Assign` 対応が自動的に含まれる）
- **Completion criteria**: 重複コードが共通ヘルパーに統合されている。`cargo test` 全 PASS。arrow 関数のデフォルトパラメータが `Option` ラップされる
- **Depends on**: None

### T3: callable interface の call/construct signature 収集 + callable-only DRY 化

- **Work**:
  - `src/registry/interfaces.rs` に `pub(crate) fn is_callable_only(members: &[TsTypeElement]) -> bool` を追加
  - `collect_interface_methods` に `TsCallSignatureDecl` と `TsConstructSignatureDeclaration` のハンドラ追加。call signature は `Vec<MethodSignature>` として返し、construct signature は `MethodSignature` として返す
  - `src/registry/collection.rs` の interface 登録で `call_signatures` と `constructor` を設定
  - `src/pipeline/type_converter/interfaces.rs` の callable-only 判定を `is_callable_only` に置き換え
  - `src/registry/functions.rs` の `try_collect_call_signature_fn` の callable-only 判定を `is_callable_only` に置き換え
- **Completion criteria**: callable-only interface が `TypeDef::Struct { call_signatures: [...] }` として登録される。construct signature 付き interface が `constructor: Some(...)` として登録される。callable-only 判定が 1 箇所に統合されている
- **Depends on**: T1, T2（call signature パラメータ抽出に `extract_ts_fn_param` を使用）

### T4: `resolve_fn_type_info` の `call_signatures` 対応

- **Work**: `src/pipeline/type_resolver/helpers.rs` の `resolve_fn_type_info` に `TypeDef::Struct { call_signatures, .. }` マッチを追加。`call_signatures` が非空なら `select_overload` で最適シグネチャを選択し、`(return_type, params)` を返す
- **Completion criteria**: `const getCookie: GetCookie = (c, key?) => { return {} }` で `current_fn_return_type` が設定される。callable interface expected type テスト PASS
- **Depends on**: T3

### T5: `collect_type_alias_fields` の TsTypeRef 対応 + doc 修正

- **Work**:
  - `src/registry/collection.rs` の `collect_type_alias_fields` に `TsTypeRef` ブランチ追加。`convert_ts_type` で RustType に変換し、`Named` なら registry/synthetic からフィールド取得
  - intersection ブランチ内の TsTypeRef 処理も同じロジックに統一。型引数付き TsTypeRef（`Partial<T>` 等）を正しく展開
  - 共通ヘルパー `resolve_type_ref_fields` を抽出してトップレベルと intersection で共有
  - `collect_type_alias_fields` の doc comment を修正: 対応バリアント（TsTypeLit, TsIntersectionType, TsTypeRef）を明記
- **Completion criteria**: `type BodyCache = Partial<Body>` が全フィールド `Option` 付き struct として登録される。`type X = Named & Partial<T>` の intersection でもユーティリティ型が展開される。doc comment が正確
- **Depends on**: T1

### T6: rest パラメータ収集（I-259）

- **Work**: `src/registry/functions.rs` の `try_collect_fn_type_alias` と `try_collect_call_signature_fn` で T2 の `extract_ts_fn_param` ヘルパーを使用。rest パラメータがあれば `has_rest: true` を設定（現在の `has_rest: false` 固定を修正）
- **Completion criteria**: `type Handler = { (...args: string[]): void }` が `TypeDef::Function { has_rest: true }` として登録される。`type Fn = (...args: T[]) => U` も同様
- **Depends on**: T2

### T7: `resolve_type_params_impl` の複合名解決（I-308）

- **Work**: `src/pipeline/type_resolver/expected_types.rs` の `resolve_type_params_impl` に `"::"` 複合名の分解解決ロジック追加。ベース部分を制約から解決し、解決結果の型からフィールドを取得
- **Completion criteria**: `E['Bindings']` の expected type が `Env` の `Bindings` フィールド型に解決される。テスト PASS
- **Depends on**: None

### T8: テストギャップ解消

- **Work**: レビューで特定した全テストギャップを解消:
  - G1: interface の call signature 収集テスト
  - G2: interface の construct signature 収集テスト
  - G3: `try_collect_fn_type_alias` の rest パラメータテスト
  - G4: `try_collect_call_signature_fn` の rest パラメータテスト
  - G5: `collect_type_alias_fields` の TsTypeRef 分岐テスト
  - G6: `resolve_fn_type_info` の callable interface パステスト
  - G7: `collect_arrow_def_with_extras` の `Pat::Assign` テスト
  - G8: callable-only: 0 call sig 境界値テスト
  - G9: callable-only: mixed (call sig + property) テスト
  - intersection TsTypeLit & TsTypeLit テスト
  - intersection 全メンバーフィールドなし → None テスト
  - intersection 未登録 TsTypeRef → None テスト
  - `resolve_type_params_impl` + "::" 複合名テスト
  - パラメータ抽出ヘルパーのユニットテスト（`extract_ts_fn_param`, `extract_pat_param`）
  - E2E: callable interface の変換結果コンパイルテスト（fixture 追加）
  - E2E: `type X = Partial<T>` の変換結果コンパイルテスト（fixture 追加）
- **Completion criteria**: 全テストギャップのテスト追加。`cargo test` 全 PASS
- **Depends on**: T1-T7

## Test Plan

### 新規テスト（機能変更由来）

| テスト | 対象 | 種別 |
|-------|------|------|
| TypeAlias TsTypeRef → Struct フィールド | T5 | Registry unit |
| TypeAlias Partial<T> → 全フィールド Option | T5 | Registry unit |
| intersection 内 Partial<T> 展開 | T5 | Registry unit |
| callable interface → call_signatures 登録 | T3 | Registry unit |
| construct signature → constructor 登録 | T3 | Registry unit |
| resolve_fn_type_info + call_signatures | T4 | TypeResolver unit |
| callable interface → arrow return type 伝播 | T4 | TypeResolver integration |
| resolve_type_params_impl + "::" 複合名 | T7 | TypeResolver unit |
| rest パラメータ収集（fn type alias） | T6 | Registry unit |
| rest パラメータ収集（call signature） | T6 | Registry unit |
| extract_ts_fn_param: Ident / Rest | T2 | Registry unit |
| extract_pat_param: Ident / Assign / Rest | T2 | Registry unit |
| arrow デフォルトパラメータ → Option ラップ | T2 | Registry unit |

### 既存テストギャップ（テストカバレッジレビュー由来）

| ギャップ | パターン | 技法 | 重要度 |
|---------|---------|------|-------|
| G1 | interface call signature 収集 | 同値分割 | High |
| G2 | interface construct signature 収集 | 同値分割 | High |
| G3 | `try_collect_fn_type_alias` rest パラメータ | C1 | High |
| G4 | `try_collect_call_signature_fn` rest パラメータ | C1 | High |
| G5 | `collect_type_alias_fields` TsTypeRef 分岐 | C1 | High |
| G6 | `resolve_fn_type_info` callable interface | C1 | High |
| G7 | arrow `Pat::Assign` | C1 | Medium |
| G8 | callable-only: 0 call sig | 境界値 | Low |
| G9 | callable-only: mixed (call sig + property) | 同値分割 | Medium |
| G10 | intersection TsTypeLit & TsTypeLit | 同値分割 | Medium |
| G11 | intersection 全メンバーフィールドなし → None | 境界値 | Low |
| G12 | intersection 未登録 TsTypeRef → None | エラーパス | Medium |

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
8. パラメータ抽出ヘルパーが共通化されている（DRY 違反解消）
9. callable-only 判定が 1 箇所に統合されている
10. plan.md / TODO の更新
