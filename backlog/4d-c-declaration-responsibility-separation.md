# Batch 4d-C: declaration 変換の責務分離

## Background

Batch 4d-B で `convert_ts_type` は `SWC → TsTypeInfo → resolve_ts_type` の 2 ステップに統一された。しかし、declaration 変換関数（`type_converter/unions.rs`, `type_converter/intersections.rs`）は依然として SWC AST を直接走査して型解決を行っている。これにより以下の DRY 違反が存在する:

1. **find_discriminant_field の二重実装**: `type_converter/unions.rs:69-139` と `resolve/intersection.rs:445-480` に同一アルゴリズムが別 AST 表現で存在
2. **extract_variant_info / extract_discriminated_variant の二重実装**: `unions.rs:150-194` と `resolve/intersection.rs:483-526`
3. **extract_variant_fields の二重実装**: `intersections.rs:257-310` と `resolve/intersection.rs:529-564`
4. **extract_intersection_members の SWC 直接走査**: `intersections.rs:153-232` が `convert_property_signature` で SWC AST を直接操作。resolve の `resolve_type_literal_fields` と同一知識
5. **try_convert_general_union 内の SWC 直接操作**: `unions.rs:400-433` の TsTypeLit/intersection 分岐

## Goal

1. `find_discriminant_field` の実装を `resolve/intersection.rs` の 1 箇所に統一する
2. declaration 変換内の SWC AST 直接走査による型解決を排除し、TsTypeInfo → resolve 経由に統一する
3. 全テストパス、clippy 0 warnings、ベンチマーク変化なし

## Scope

### In Scope

- `unions.rs::find_discriminant_field` の削除、resolve 版を `pub(crate)` にして統一
- `unions.rs::extract_variant_info` の削除、resolve の `extract_discriminated_variant` を拡張・統一
- `intersections.rs::extract_intersection_members` の TsTypeInfo 経由への書き換え
- `intersections.rs::extract_variant_fields` の削除、resolve 版を `pub(crate)` にして統一
- `intersections.rs::distribute_intersection_with_union` の TsTypeInfo 経由への書き換え
- `unions.rs::try_convert_general_union` 内の TsTypeLit/intersection 分岐の TsTypeInfo 経由化

### Out of Scope

- `convert_type_alias` のマスターディスパッチャ構造の変更（型分類の責務は type_converter に残す）
- `convert_interface_items` の移行（interface は declaration 構造に依存するため現状維持）
- type_converter モジュール自体のリネームや構造変更
- `convert_property_signature` の全面廃止（interface 等から使われ続ける）
- `convert_method_signature` の TsTypeInfo 移行（interface 変換で SWC AST 依存）

## Design

### 設計原則

declaration 変換関数が SWC AST を参照してよいのは **型分類**（「この type_ann は Union か Intersection か Literal か」の判定）のみ。型解決（フィールド抽出、discriminant 検出、型の RustType 変換）は全て `convert_to_ts_type_info` → resolve 関数経由に統一する。

### 重要な設計判断

#### extract_discriminated_variant の戻り値拡張

**現状の差異**:
- SWC 版 `extract_variant_info` → `(raw_value: String, fields: Vec<StructField>)` — 生の discriminant 値を返す
- resolve 版 `extract_discriminated_variant` → `(pascal_name: String, fields: Vec<StructField>)` — PascalCase 名を返し、`EnumVariant::value` は `None`

**問題**: declaration 変換では `Item::Enum` の `EnumVariant::value` に `Some(EnumValue::Str(raw_value))` が必要（serde の `#[serde(rename = "raw_value")]` に対応）。resolve 版は `value: None` で登録しており、serde tag 付き enum の discriminant 値が失われている。

**解決策**: `extract_discriminated_variant` の戻り値を `(raw_value: String, pascal_name: String, fields: Vec<StructField>)` に拡張する。

- `raw_value`: 元の TS 文字列リテラル値（例: `"click"`）
- `pascal_name`: PascalCase バリアント名（例: `"Click"`）
- `fields`: discriminant フィールドを除いた残りのフィールド

resolve 側の呼び出し元（`resolve_intersection_with_union`）も `value: Some(EnumValue::Str(raw_value))` を設定するよう修正。これは resolve 側のバグ修正でもある（synthetic enum でも serde で正しくデシリアライズするには raw value が必要）。

#### メソッド抽出の扱い

`extract_intersection_members`（`intersections.rs:175-177`）と `resolve_intersection`（`resolve/intersection.rs:87-112`）はどちらも `TsMethodSignature` / `TsMethodInfo` からメソッドを抽出する。

resolve 版は TsTypeInfo の `TsMethodInfo` から `Method` を構築する inline ロジックを持つ（`resolve/intersection.rs:87-112`）。declaration 変換では `convert_method_signature`（SWC ベース、`interfaces.rs:379-428`）を使用。

**判断**: メソッド抽出は `convert_method_signature` のままにする。理由:
1. `convert_method_signature` は interface 変換でも使用され、SWC AST 依存が残る
2. resolve 版の inline メソッド変換は `filter_map` でエラーを握りつぶしており、declaration 変換より寛容
3. メソッド抽出の統一は interface 変換の TsTypeInfo 移行と一緒に行うべき（別 PRD）

ただし、`extract_intersection_members` を TsTypeInfo 経由にする際、メソッドは TsMethodInfo から変換する必要がある。resolve の inline ロジックを共通関数に切り出すか、`convert_method_signature` を TsTypeInfo ベースに移行するかの二択。

**結論**: resolve の inline メソッド変換ロジックを `resolve_method_info` として `pub(crate)` 関数に切り出す。declaration 変換の `extract_intersection_members` からも呼ぶ。

### 関数の公開範囲変更と新設

`src/ts_type_info/resolve/intersection.rs` の以下を `pub(crate)` に変更:
- `find_discriminant_field` (L445) — 現在 `fn`（private）
- `extract_discriminated_variant` (L483) — 現在 `fn`（private）、戻り値を 3-tuple に拡張
- `extract_variant_fields` (L529) — 現在 `fn`（private）
- `resolve_type_literal_fields` (L254) — 現在 `fn`（private）

新設:
- `resolve_method_info(&TsMethodInfo, &TypeRegistry, &mut SyntheticTypeRegistry) -> Result<Method>` — resolve の inline メソッド変換を関数化

### 各関数の変更詳細

#### T1: find_discriminant_field の統一

**変更前** (`unions.rs:69-139`):
```rust
pub(super) fn find_discriminant_field(type_lits: &[&swc_ecma_ast::TsTypeLit]) -> Option<String> {
    // SWC TsTypeLit を直接走査
}
```

**変更後**: この関数を削除。`try_convert_discriminated_union` で union を TsTypeInfo に変換し、resolve 版を呼ぶ。

`try_convert_discriminated_union` の変更:
```rust
// Before (L19-45):
let union = match decl.type_ann.as_ref() {
    TsType::TsUnionOrIntersectionType(TsUnionType(u)) => u,
    _ => return Ok(None),
};
let type_lits: Vec<&TsTypeLit> = union.types.iter()...;
let discriminant_field = find_discriminant_field(&type_lits);

// After:
let union_info = match convert_to_ts_type_info(decl.type_ann.as_ref())? {
    TsTypeInfo::Union(members) => members,
    _ => return Ok(None),
};
// 全メンバーが TypeLiteral であることを確認 (2個以上)
if union_info.len() < 2 || !union_info.iter().all(|m| matches!(m, TsTypeInfo::TypeLiteral(_))) {
    return Ok(None);
}
let discriminant_field = crate::ts_type_info::resolve::intersection::find_discriminant_field(&union_info);
```

#### T2: extract_discriminated_variant の拡張と統一

**変更前** (`resolve/intersection.rs:483-526`):
```rust
fn extract_discriminated_variant(
    variant_type: &TsTypeInfo, disc_field: &str, reg, synthetic
) -> Result<(String, Vec<StructField>)> {
    // returns (pascal_name, fields)
    disc_value = crate::ir::string_to_pascal_case(s);
}
```

**変更後**:
```rust
pub(crate) fn extract_discriminated_variant(
    variant_type: &TsTypeInfo, disc_field: &str, reg, synthetic
) -> Result<(String, String, Vec<StructField>)> {
    // returns (raw_value, pascal_name, fields)
    let raw_value = s.clone();
    let pascal_name = crate::ir::string_to_pascal_case(s);
    Ok((raw_value, pascal_name, fields))
}
```

**resolve 側の呼び出し元修正** (`resolve/intersection.rs:381-382`):
```rust
// Before:
let (variant_name, variant_fields) = extract_discriminated_variant(...)?;
variants.push(EnumVariant { name: variant_name, value: None, ... });

// After:
let (raw_value, variant_name, variant_fields) = extract_discriminated_variant(...)?;
variants.push(EnumVariant {
    name: variant_name,
    value: Some(crate::ir::EnumValue::Str(raw_value)),
    ...
});
```

**declaration 変換側** (`unions.rs:try_convert_discriminated_union`):
```rust
// Before (L49-57):
let (discriminant_value, other_fields) = extract_variant_info(type_lit, &discriminant_field, synthetic, reg)?;
variants.push(EnumVariant {
    name: string_to_pascal_case(&discriminant_value),
    value: Some(EnumValue::Str(discriminant_value)),
    ...
});

// After:
let (raw_value, pascal_name, other_fields) =
    crate::ts_type_info::resolve::intersection::extract_discriminated_variant(
        &member, &discriminant_field, reg, synthetic
    )?;
variants.push(EnumVariant {
    name: pascal_name,
    value: Some(EnumValue::Str(raw_value)),
    ...
});
```

削除対象:
- `unions.rs::extract_variant_info` (L150-194)
- `unions.rs::is_string_literal_type` (L142-147)

#### T3: extract_intersection_members の TsTypeInfo 移行

**変更前** (`intersections.rs:153-232`):
SWC `TsTypeLit` → `convert_property_signature` でフィールド抽出、`TsTypeRef` → registry lookup。

**変更後**:
各メンバーの SWC `TsType` を `convert_to_ts_type_info` で変換し、TsTypeInfo パターンで処理:

```rust
fn extract_intersection_members(
    members: &[&TsType],
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(Vec<StructField>, Vec<Method>)> {
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    for (i, ty) in members.iter().enumerate() {
        let info = convert_to_ts_type_info(ty)?;
        match &info {
            TsTypeInfo::TypeLiteral(lit) => {
                let lit_fields = crate::ts_type_info::resolve::intersection::resolve_type_literal_fields(lit, reg, synthetic)?;
                // 重複チェック
                for field in lit_fields {
                    if fields.iter().any(|f: &StructField| f.name == field.name) {
                        return Err(anyhow!("duplicate field '{}' in intersection type", field.name));
                    }
                    fields.push(field);
                }
                // メソッド抽出
                for method_info in &lit.methods {
                    methods.push(crate::ts_type_info::resolve::intersection::resolve_method_info(method_info, reg, synthetic)?);
                }
            }
            TsTypeInfo::TypeRef { name, .. } => {
                if let Some(TypeDef::Struct { fields: resolved_fields, .. }) = reg.get(name) {
                    // 既存ロジックと同等
                    for field in resolved_fields {
                        let sanitized = sanitize_field_name(&field.name);
                        if fields.iter().any(|f: &StructField| f.name == sanitized) {
                            return Err(anyhow!("duplicate field '{}' in intersection type", field.name));
                        }
                        fields.push(StructField { vis: None, name: sanitized, ty: field.ty.clone() });
                    }
                } else {
                    let rust_type = resolve_ts_type(&info, reg, synthetic)?;
                    fields.push(StructField { vis: None, name: format!("_{i}"), ty: rust_type });
                }
            }
            TsTypeInfo::String | TsTypeInfo::Number | TsTypeInfo::Boolean
            | TsTypeInfo::Any | TsTypeInfo::Unknown | TsTypeInfo::Object
            | TsTypeInfo::Void | TsTypeInfo::Null | TsTypeInfo::Undefined
            | TsTypeInfo::Never | TsTypeInfo::BigInt | TsTypeInfo::Symbol => continue,
            _ => {
                let rust_type = resolve_ts_type(&info, reg, synthetic).unwrap_or(RustType::Any);
                fields.push(StructField { vis: None, name: format!("_{i}"), ty: rust_type });
            }
        }
    }
    Ok((fields, methods))
}
```

#### T4: distribute_intersection_with_union の TsTypeInfo 移行

**変更前** (`intersections.rs:315-374`):
SWC `TsUnionType` から `TsTypeLit` を抽出、`find_discriminant_field` と `extract_variant_info` を SWC ベースで呼ぶ。

**変更後**:
```rust
fn distribute_intersection_with_union(
    base_fields: Vec<StructField>,
    union_info: &[TsTypeInfo],  // TsTypeInfo::Union のメンバー (SWC TsUnionType から変換済み)
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(Vec<EnumVariant>, Option<String>)> {
    let serde_tag = crate::ts_type_info::resolve::intersection::find_discriminant_field(union_info);
    let mut variants = Vec::new();

    if let Some(ref discriminant_field) = serde_tag {
        for variant in union_info {
            let (raw_value, pascal_name, variant_fields) =
                crate::ts_type_info::resolve::intersection::extract_discriminated_variant(
                    variant, discriminant_field, reg, synthetic
                )?;
            let merged = merge_fields(&base_fields, variant_fields);
            variants.push(EnumVariant {
                name: pascal_name,
                value: Some(EnumValue::Str(raw_value)),
                data: None,
                fields: merged,
            });
        }
    } else {
        for (idx, variant) in union_info.iter().enumerate() {
            let variant_fields =
                crate::ts_type_info::resolve::intersection::extract_variant_fields(variant, reg, synthetic)?;
            let merged = merge_fields(&base_fields, variant_fields);
            variants.push(EnumVariant {
                name: format!("Variant{idx}"),
                value: None,
                data: None,
                fields: merged,
            });
        }
    }

    Ok((variants, serde_tag))
}
```

シグネチャ変更: `union: &swc_ecma_ast::TsUnionType` → `union_info: &[TsTypeInfo]`

呼び出し元（`try_convert_intersection_type` L472-473）も変更:
```rust
// Before:
let (variants, serde_tag) = distribute_intersection_with_union(extra_base, first_union, synthetic, reg)?;

// After:
let first_union_info = match convert_to_ts_type_info(union_members[0])? {
    TsTypeInfo::Union(members) => members,
    _ => unreachable!(),
};
let (variants, serde_tag) = distribute_intersection_with_union(extra_base, &first_union_info, synthetic, reg)?;
```

#### T5: try_convert_general_union の TsTypeLit/intersection 分岐の移行

**変更前** (`unions.rs:400-433`):
```rust
TsType::TsTypeLit(lit) => {
    let mut fields = Vec::new();
    for member in &lit.members {
        if let TsTypeElement::TsPropertySignature(prop) = member {
            fields.push(convert_property_signature(prop, synthetic, reg)?);
        }
    }
    // ...
}
TsType::TsUnionOrIntersectionType(TsIntersectionType(intersection)) => {
    // SWC AST 直接走査
}
```

**変更後**: 各メンバーを TsTypeInfo に変換し resolve でフィールド抽出:
```rust
TsType::TsTypeLit(_) => {
    let info = convert_to_ts_type_info(ty)?;
    if let TsTypeInfo::TypeLiteral(lit) = &info {
        let fields = crate::ts_type_info::resolve::intersection::resolve_type_literal_fields(lit, synthetic, reg)?;
        // ...
    }
}
TsType::TsUnionOrIntersectionType(TsIntersectionType(_)) => {
    let info = convert_to_ts_type_info(ty)?;
    if let TsTypeInfo::Intersection(members) = &info {
        // resolve の intersection 解決でフィールド抽出
        // ...
    }
}
```

### StructField の vis フィールドの扱い

**差異**: resolve の `resolve_type_literal_fields` は `vis: Some(Visibility::Public)` を設定する。declaration 変換の `convert_property_signature` は `vis: None` を設定する。

**分析**: `vis: None` は generator 側で構造体のデフォルト可視性を適用する意味。`vis: Some(Public)` は明示的に public を指定。declaration 変換で生成する struct のフィールドは最終的に pub になるため、挙動は同一。

**対応**: resolve の `resolve_type_literal_fields` が返す `vis: Some(Public)` をそのまま使用する。declaration 変換側で上書きは不要。

### Semantic Safety Analysis

型解決パスが SWC AST 直接 → TsTypeInfo 経由に変わるが、`convert_ts_type()` は既に内部で `convert_to_ts_type_info → resolve_ts_type` を呼んでおり、結果は同一。ベンチマーク前後比較で確認する。

#### Batch 4d-B で確認済みの挙動差異

1. **Record<K,V> の key 型**: Batch 4d-B で修正済み。旧パスと resolve パスの挙動は統一された
2. **Qualified type name (`Namespace.Type`)**: 旧パスはエラー、resolve パスは受理。前進方向
3. **Missing type parameters (e.g., `Array` without args)**: 実質的影響なし

#### 本 PRD 固有のリスク

1. **discriminant raw value の伝播**: T2 で `extract_discriminated_variant` の戻り値を拡張し、raw value を保持する。resolve 側の `value: None` も修正するため、synthetic enum でも正しい serde rename が付く（改善）
2. **メソッド抽出のエラーハンドリング**: resolve 版は `filter_map` でエラーを握りつぶす。新設の `resolve_method_info` は `Result` を返す設計にし、declaration 変換のエラーハンドリングを維持する

### Design Integrity Review

- **凝集度**: 各関数の責務は明確。`find_discriminant_field` は判定のみ、`extract_discriminated_variant` は抽出のみ
- **責務分離**: 型分類（SWC AST match）と型解決（TsTypeInfo → resolve）が明確に分離される
- **DRY**: 5 つの二重実装を全て解消。型解決の知識が resolve に集約される
- **結合度**: type_converter → resolve の依存が増えるが、これは意図的（resolve が型解決の唯一の窓口）

## Task List

### T1: resolve 関数の公開と extract_discriminated_variant 拡張 + resolve_method_info 新設

- **Work**:
  - `src/ts_type_info/resolve/intersection.rs`:
    - `find_discriminant_field` (L445) → `pub(crate)`
    - `extract_discriminated_variant` (L483) → `pub(crate)` + 戻り値を `(String, String, Vec<StructField>)` に拡張（raw_value, pascal_name, fields）
    - `extract_variant_fields` (L529) → `pub(crate)`
    - `resolve_type_literal_fields` (L254) → `pub(crate)`
    - `resolve_intersection_with_union` (L381-382): `extract_discriminated_variant` の新戻り値に合わせて `value: Some(EnumValue::Str(raw_value))` を設定
    - resolve の inline メソッド変換ロジック（L87-112）を `pub(crate) fn resolve_method_info` として切り出し
  - 既存テストの修正（戻り値の型変更に伴う）
- **Completion criteria**: `cargo check` パス、resolve のテスト全パス
- **Depends on**: なし

### T2: try_convert_discriminated_union の TsTypeInfo 移行

- **Work**:
  - `src/pipeline/type_converter/unions.rs`:
    - `try_convert_discriminated_union` (L13-66): SWC union → `convert_to_ts_type_info` → `find_discriminant_field` + `extract_discriminated_variant`
    - `find_discriminant_field` (L69-139) 削除
    - `is_string_literal_type` (L142-147) 削除
    - `extract_variant_info` (L150-194) 削除
  - `mod.rs` の `use` 文更新（`find_discriminant_field`, `extract_variant_info` の import 削除）
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス、`grep -rn "fn find_discriminant_field" src/` が resolve の 1 件のみ
- **Depends on**: T1

### T3: distribute_intersection_with_union の TsTypeInfo 移行

- **Work**:
  - `src/pipeline/type_converter/intersections.rs`:
    - `distribute_intersection_with_union` (L315-374): シグネチャを `&[TsTypeInfo]` に変更、resolve の関数を使用
    - `extract_variant_fields` (L257-310) 削除
    - `try_convert_intersection_type` (L448-473) の呼び出し元を変更: union メンバーを TsTypeInfo に変換
  - `mod.rs` の import 更新
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス
- **Depends on**: T1

### T4: extract_intersection_members の TsTypeInfo 移行

- **Work**:
  - `src/pipeline/type_converter/intersections.rs`:
    - `extract_intersection_members` (L153-232): 各メンバーを `convert_to_ts_type_info` で変換、`resolve_type_literal_fields` と `resolve_method_info` を使用
    - `convert_property_signature` の使用をこの関数内から排除
    - `convert_method_signature` の使用を `resolve_method_info` に置換
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス
- **Depends on**: T1

### T5: try_convert_general_union の TsTypeLit/intersection 分岐の移行

- **Work**:
  - `src/pipeline/type_converter/unions.rs`:
    - `try_convert_general_union` の `TsType::TsTypeLit` 分岐 (L400-413): TsTypeInfo 変換 → `resolve_type_literal_fields`
    - `TsType::TsUnionOrIntersectionType(TsIntersectionType)` 分岐 (L414-433): TsTypeInfo 変換 → resolve 経由
- **Completion criteria**: `cargo check` パス、`cargo test` 全パス
- **Depends on**: T1

### T6: 品質確認 + ベンチマーク

- **Work**:
  - `cargo fix --allow-dirty --allow-staged && cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test > /tmp/test-result.txt 2>&1` → 全パス確認
  - `./scripts/hono-bench.sh` → 110/158 クリーン維持確認
  - `./scripts/check-file-lines.sh` → 全ファイル 1000 行以下確認
- **Completion criteria**: 0 errors, 0 warnings, 全テストパス、ベンチマーク 110/158 維持
- **Depends on**: T1-T5

## Test Plan

- **リファクタリングの正しさ**: 既存テスト全パスが証明（出力が変わらないことの保証）
- **extract_discriminated_variant の戻り値変更**: 既存テスト（resolve/intersection.rs L596-1100+）の修正で raw value 含む 3-tuple を検証
- **resolve_method_info の新設**: ユニットテスト 2 件追加（通常メソッド、optional パラメータ付きメソッド）
- **ベンチマーク前後比較**: 110/158 から変化なし

## Completion Criteria

1. `cargo test` 全テストパス
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
3. `cargo fmt --all --check` パス
4. `find_discriminant_field` の定義が `resolve/intersection.rs` の 1 箇所のみ
5. `extract_variant_info` が `unions.rs` に存在しない
6. `extract_variant_fields` が `intersections.rs` に存在しない
7. `extract_intersection_members` 内で `convert_property_signature` を使用していない
8. Hono ベンチマーク: 110/158 クリーン維持（±0）
9. type_converter 内の SWC AST 走査が「型分類」目的のみに限定されている

## コード参照一覧

以下は本 PRD で変更する全ファイル・関数の行番号（base commit: `fca5f85`）:

### 変更対象（resolve 側）
- `src/ts_type_info/resolve/intersection.rs:445-480` — `find_discriminant_field` → pub(crate)
- `src/ts_type_info/resolve/intersection.rs:483-526` — `extract_discriminated_variant` → pub(crate) + 戻り値拡張
- `src/ts_type_info/resolve/intersection.rs:529-564` — `extract_variant_fields` → pub(crate)
- `src/ts_type_info/resolve/intersection.rs:254-276` — `resolve_type_literal_fields` → pub(crate)
- `src/ts_type_info/resolve/intersection.rs:87-112` — inline メソッド変換 → `resolve_method_info` として切り出し
- `src/ts_type_info/resolve/intersection.rs:381-382` — `resolve_intersection_with_union` の EnumVariant::value 修正

### 変更対象（type_converter 側）
- `src/pipeline/type_converter/unions.rs:13-66` — `try_convert_discriminated_union` 書き換え
- `src/pipeline/type_converter/unions.rs:69-147` — `find_discriminant_field` + `is_string_literal_type` 削除
- `src/pipeline/type_converter/unions.rs:150-194` — `extract_variant_info` 削除
- `src/pipeline/type_converter/unions.rs:400-433` — `try_convert_general_union` TsTypeLit/intersection 分岐の書き換え
- `src/pipeline/type_converter/intersections.rs:153-232` — `extract_intersection_members` 書き換え
- `src/pipeline/type_converter/intersections.rs:257-310` — `extract_variant_fields` 削除
- `src/pipeline/type_converter/intersections.rs:315-374` — `distribute_intersection_with_union` 書き換え
- `src/pipeline/type_converter/intersections.rs:448-473` — `try_convert_intersection_type` 呼び出し元変更
- `src/pipeline/type_converter/mod.rs:24,26-29,321` — import 文更新

### 参照のみ（変更なし）
- `src/ts_type_info/mod.rs:19-130` — TsTypeInfo enum 定義
- `src/ts_type_info/mod.rs:147-158` — TsFieldInfo 定義
- `src/ts_type_info/mod.rs:164-176` — TsTypeLiteralInfo 定義
- `src/ts_type_info/mod.rs:181-193` — TsMethodInfo 定義
- `src/ts_type_info/mod.rs:240` — `convert_to_ts_type_info` エントリポイント
- `src/pipeline/type_converter/utilities.rs:28-58` — `convert_property_signature`（interface 等から引き続き使用）
- `src/pipeline/type_converter/interfaces.rs:379-428` — `convert_method_signature`（interface 変換で引き続き使用）
