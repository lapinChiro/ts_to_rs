# Batch 4d-C: declaration 変換の責務分離

## Background

PRD-A（クリーンアップ）と PRD-B（ユーティリティ型統一）の完了後も、type_converter の declaration 変換関数（intersection, union, type alias, interface）には以下の問題が残る:

1. **find_discriminant_field の二重実装**: `type_converter/unions.rs:69` と `resolve/intersection.rs:480` に同一アルゴリズムが別表現で存在
2. **SWC AST 直接走査による型解決バイパス**: intersection/union のメンバー処理で SWC AST を直接 match し、`convert_ts_type` を経由せず型を判定・抽出する箇所が残る
3. **convert_property_signature の SWC AST 依存**: StructField 生成が SWC `TsPropertySignature` に直接依存。`TsFieldInfo` ベースの代替が可能

PRD-C の目標は、declaration 変換関数の責務を「**型分類（何の Item を作るかの判定）**」と「**Item 構築（フィールド/メソッド/バリアントの組み立て）**」に限定し、**型解決は全て resolve 経由**に統一すること。

## Goal

1. `find_discriminant_field` の実装を `resolve/intersection.rs` の1箇所に統一する
2. declaration 変換内の SWC AST 直接走査による型解決を排除し、`convert_ts_type()` → `resolve_ts_type()` 経由に統一する
3. `convert_property_signature` を `TsFieldInfo` ベースの汎用関数で補完する
4. 全テストパス、clippy 0 warnings、ベンチマーク変化なし

## Scope

### In Scope

- `unions.rs::find_discriminant_field` の削除。`resolve/intersection.rs::find_discriminant_field` を `pub(crate)` にして統一
- `unions.rs::extract_variant_info` の SWC AST 走査を TsTypeInfo ベースに移行
- `intersections.rs::extract_intersection_members` の SWC AST 走査を TsTypeInfo ベースに移行
- `utilities.rs` に `convert_field_info(TsFieldInfo) → StructField` 関数を追加し、`convert_property_signature` の呼び出し元を段階的に移行
- `unions.rs::try_convert_discriminated_union` を TsTypeInfo ベースの discriminant 検出に移行

### Out of Scope

- `convert_type_alias` のマスターディスパッチャ構造の変更（型分類の責務は type_converter に残す）
- `convert_interface_items` の Struct/Trait/Mixed 判定の移行（SWC AST の declaration 構造に依存するため現状維持）
- type_converter モジュール自体のリネーム（将来 PRD で検討）

## Design

### Technical Approach

#### 設計原則

declaration 変換関数が SWC AST を参照してよいのは **型分類**（「この declaration は Struct か Enum か Trait か」の判定）のみ。型解決（「`string` は `RustType::String`」「`Foo<T>` は `RustType::Named{...}`」）は必ず `convert_ts_type` → `resolve_ts_type` を経由する。

#### T1: find_discriminant_field の統一

`resolve/intersection.rs:480` の `find_discriminant_field` を `pub(crate)` に変更する。

`unions.rs` の `try_convert_discriminated_union` で、SWC AST の `TsUnionType` を `convert_to_ts_type_info` で `TsTypeInfo::Union` に変換してから、`resolve/intersection.rs::find_discriminant_field` を呼び出す。

```
Before: SWC TsTypeLit[] → unions.rs::find_discriminant_field (SWC AST 走査)
After:  SWC TsUnionType → TsTypeInfo::Union → resolve::find_discriminant_field (TsTypeInfo 走査)
```

`unions.rs` の `find_discriminant_field` と `is_string_literal_type` ヘルパーを削除。

#### T2: extract_variant_info の TsTypeInfo 移行

`unions.rs::extract_variant_info` は SWC `TsTypeLit` を走査してバリアント名とフィールドを抽出する。これを TsTypeInfo ベースに移行する。

`resolve/intersection.rs` の `extract_discriminated_variant` が同等の機能を持つ（TsTypeInfo ベース）。これを `pub(crate)` にして `unions.rs` から呼び出す。

#### T3: extract_intersection_members の型解決部分を resolve 経由に

`intersections.rs::extract_intersection_members` は SWC AST を直接走査して:
- `TsTypeLit` → `convert_property_signature` でフィールド抽出
- `TsTypeRef` → registry lookup でフィールド取得
- その他 → `convert_ts_type` で RustType 変換

「TsTypeLit → フィールド抽出」と「TsTypeRef → registry lookup」の部分を、TsTypeInfo に変換してから resolve の関数を使う形に移行する。

#### T4: convert_field_info 関数の追加

`utilities.rs` に TsFieldInfo ベースの StructField 変換関数を追加:

```rust
pub(crate) fn convert_field_info(
    field: &TsFieldInfo,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<StructField> {
    let mut ty = resolve_ts_type(&field.ty, reg, synthetic)?;
    if field.optional && !matches!(ty, RustType::Option(_)) {
        ty = RustType::Option(Box::new(ty));
    }
    Ok(StructField {
        vis: None,
        name: crate::ir::sanitize_field_name(&field.name),
        ty,
    })
}
```

既存の `convert_property_signature` は互換性のため維持するが、新規コードでは `convert_field_info` を使用。将来的に `convert_property_signature` の呼び出し元を全て移行後に削除。

#### T5: unions.rs の型分類ロジックの TsTypeInfo 対応

`try_convert_string_literal_union` と `try_convert_general_union` で、union メンバーの型分類（string literal か？keyword か？TypeRef か？）を SWC AST match で行っている。これを TsTypeInfo の enum variant match に置き換える。

具体的には、`convert_type_alias` → `try_convert_*_union` の入り口で `convert_to_ts_type_info` を呼び、TsTypeInfo ベースで判定を行う。SWC AST match は declaration-level の判定（`type_ann` が `TsUnionType` か？）にのみ使用。

### Design Integrity Review

- **Higher-level consistency**: convert_ts_type → resolve_ts_type の統一パスに完全に合わせる変更。Batch 4b の移行を完結させる
- **DRY**: find_discriminant_field の二重実装を解消。extract_variant_info の重複も解消
- **Orthogonality**: 型分類（SWC AST 依存）と型解決（TsTypeInfo 依存）の責務が明確に分離される
- **Coupling**: type_converter → resolve の依存が増えるが、これは意図的（resolve が型解決の唯一の窓口）
- **Broken windows**: `convert_property_signature` と `convert_field_info` の並存は一時的。将来の完全移行で解消

### Impact Area

- `src/pipeline/type_converter/unions.rs` — find_discriminant_field 削除、extract_variant_info 移行、try_convert_* 修正
- `src/pipeline/type_converter/intersections.rs` — extract_intersection_members の型解決部分修正
- `src/pipeline/type_converter/utilities.rs` — convert_field_info 追加
- `src/ts_type_info/resolve/intersection.rs` — find_discriminant_field, extract_discriminated_variant を pub(crate) に

### Semantic Safety Analysis

型解決パスが SWC AST 直接 → TsTypeInfo 経由に変わるが、`convert_ts_type()` は内部で `convert_to_ts_type_info → resolve_ts_type` を呼んでおり、結果は同一。ベンチマーク前後比較で確認する。

## Task List

### T1: find_discriminant_field の統一

- **Work**:
  - `src/ts_type_info/resolve/intersection.rs:480` の `find_discriminant_field` を `pub(crate)` に変更
  - `src/pipeline/type_converter/unions.rs` の `find_discriminant_field`（行69-139）と `is_string_literal_type`（行142-147）を削除
  - `try_convert_discriminated_union` で union メンバーを `convert_to_ts_type_info` で変換し、`resolve::intersection::find_discriminant_field` を呼び出すよう修正
- **Completion criteria**: `cargo check` パス、`grep -rn "fn find_discriminant_field" src/` が `resolve/intersection.rs` の1件のみ
- **Depends on**: PRD-B 完了

### T2: extract_variant_info の TsTypeInfo 移行

- **Work**:
  - `src/ts_type_info/resolve/intersection.rs` の `extract_discriminated_variant` を `pub(crate)` に変更
  - `unions.rs` の `extract_variant_info`（行149-194）を削除
  - `try_convert_discriminated_union` で TsTypeInfo ベースの `extract_discriminated_variant` を使用するよう修正
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス
- **Depends on**: T1

### T3: extract_intersection_members の型解決移行

- **Work**:
  - `intersections.rs::extract_intersection_members` の `TsTypeLit` → `convert_property_signature` のパスを、TsTypeInfo 変換 → `resolve_type_literal_fields` に移行
  - `TsTypeRef` → registry lookup のパスを `convert_ts_type` 経由に統一
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス
- **Depends on**: PRD-B 完了

### T4: convert_field_info 関数の追加

- **Work**:
  - `src/pipeline/type_converter/utilities.rs` に `pub(crate) fn convert_field_info(field: &TsFieldInfo, ...) -> Result<StructField>` を追加
  - T3 で移行した箇所から `convert_field_info` を使用
  - 既存テストは維持、`convert_field_info` のユニットテストを追加（3件: 通常フィールド、optional フィールド、option 二重ラップ回避）
- **Completion criteria**: `cargo test` 全パス、`convert_field_info` のテスト3件追加
- **Depends on**: T3

### T5: unions.rs の型分類 TsTypeInfo 対応

- **Work**:
  - `try_convert_string_literal_union` で union メンバーの string literal 判定を TsTypeInfo ベースに移行
  - `try_convert_general_union` で union メンバーの型分類（keyword/TypeRef/TypeLit 等）を TsTypeInfo ベースに移行
  - SWC AST match は declaration-level の判定（`TsUnionType` か？）にのみ残す
- **Completion criteria**: `cargo test` 全パス、unions.rs 内の `TsKeywordType` / `TsLitType` / `TsTypeLit` パターンマッチが declaration 判定のみに限定
- **Depends on**: T2

### T6: ベンチマークと品質確認

- **Work**:
  - `cargo fix && cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test`
  - `./scripts/hono-bench.sh` でベンチマーク実行
  - 変換率が 110/158 から変化しないことを確認
- **Completion criteria**: 0 errors, 0 warnings, 全テストパス、ベンチマーク 110/158 維持
- **Depends on**: T1-T5

## Test Plan

- 既存テスト全パスが変更の正しさの証明（リファクタリング）
- `convert_field_info` の新規テスト3件
- ベンチマーク前後比較で回帰がないことを確認

## Completion Criteria

1. `cargo test` 全テストパス
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
3. `cargo fmt --all --check` パス
4. `find_discriminant_field` の定義が `resolve/intersection.rs` の1箇所のみ
5. `extract_variant_info` が `unions.rs` に存在しない
6. `convert_field_info` が `utilities.rs` に存在し、テスト3件がパス
7. Hono ベンチマーク: 110/158 クリーン維持（±0）
8. type_converter 内の SWC AST 走査が「型分類」目的のみに限定されている
