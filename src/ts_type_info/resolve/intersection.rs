//! TsTypeInfo::Intersection / TypeLiteral → RustType 解決。
//!
//! TypeScript の intersection 型とインライン型リテラルを Rust 型に変換する。
//! - `A & B` → フィールドマージした synthetic struct
//! - `{ key: T; }` → synthetic inline struct
//! - `{ [key: string]: T }` → HashMap<String, T>
//! - `{ [K in keyof T]: T[K] }` → T（identity mapped type の簡約）

use crate::ir::{EnumVariant, Item, Method, Param, RustType, StructField, Visibility};
use crate::pipeline::synthetic_registry::SyntheticTypeKind;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::ts_type_info::{TsLiteralKind, TsMethodInfo, TsTypeInfo, TsTypeLiteralInfo};

use super::resolve_ts_type;

/// Intersection 型をアノテーション位置で解決する。
///
/// 各メンバーからフィールドを抽出・マージし、synthetic struct を登録する。
pub(crate) fn resolve_intersection(
    members: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // 空オブジェクト `{}` を除去
    let members: Vec<&TsTypeInfo> = members
        .iter()
        .filter(|m| !is_empty_type_literal(m))
        .collect();

    if members.is_empty() {
        let name = synthetic.register_inline_struct(&[]);
        return Ok(RustType::Named {
            name,
            type_args: vec![],
        });
    }

    if members.len() == 1 {
        // identity mapped type チェック（修飾子なし + name_type なしの場合のみ）
        if let TsTypeInfo::Mapped {
            type_param,
            constraint,
            value,
            has_readonly,
            has_optional,
            name_type,
        } = members[0]
        {
            if !has_readonly && !has_optional && name_type.is_none() {
                if let Some(ty) =
                    super::try_simplify_identity_mapped(type_param, constraint, value.as_deref())
                {
                    return Ok(ty);
                }
            }
        }
        return resolve_ts_type(members[0], reg, synthetic);
    }

    // union メンバーの検出と分配
    let has_union = members.iter().any(|m| matches!(m, TsTypeInfo::Union(_)));
    if has_union {
        return resolve_intersection_with_union(&members, reg, synthetic);
    }

    // フィールドとメソッドを抽出・マージ
    let mut merged_fields: Vec<StructField> = Vec::new();
    let mut methods: Vec<crate::ir::Method> = Vec::new();

    for member in &members {
        match member {
            TsTypeInfo::TypeLiteral(lit) => {
                let fields = resolve_type_literal_fields(lit, reg, synthetic)?;
                merge_fields_into(&mut merged_fields, fields)?;
                // メソッドを抽出
                for method_info in &lit.methods {
                    methods.push(resolve_method_info(method_info, reg, synthetic)?);
                }
            }
            TsTypeInfo::TypeRef { name, .. } => {
                // レジストリから struct フィールドを取得
                if let Some(TypeDef::Struct { fields, .. }) = reg.get(name) {
                    let struct_fields: Vec<StructField> = fields
                        .iter()
                        .map(|f| StructField {
                            name: crate::ir::sanitize_field_name(&f.name),
                            ty: f.ty.clone(),
                            vis: Some(Visibility::Public),
                        })
                        .collect();
                    merge_fields_into(&mut merged_fields, struct_fields)?;
                } else {
                    // 解決できない TypeRef → _i フィールドとして埋め込み
                    let ty = resolve_ts_type(member, reg, synthetic)?;
                    let field_name = format!("_{}", merged_fields.len());
                    merged_fields.push(StructField {
                        name: field_name,
                        ty,
                        vis: Some(Visibility::Public),
                    });
                }
            }
            TsTypeInfo::Mapped {
                type_param,
                constraint,
                value,
                has_readonly,
                has_optional,
                name_type,
            } => {
                // identity mapped type の簡約（修飾子なし + name_type なし）
                if !has_readonly && !has_optional && name_type.is_none() {
                    if let Some(ty) = super::try_simplify_identity_mapped(
                        type_param,
                        constraint,
                        value.as_deref(),
                    ) {
                        return Ok(ty);
                    }
                }
                // HashMap フォールバック
                let value_type = value
                    .as_ref()
                    .map(|v| resolve_ts_type(v, reg, synthetic))
                    .transpose()?
                    .unwrap_or(RustType::Any);
                return Ok(RustType::Named {
                    name: "HashMap".to_string(),
                    type_args: vec![RustType::String, value_type],
                });
            }
            _ => {
                // その他の型 → 解決して _i フィールドとして埋め込み
                let ty = resolve_ts_type(member, reg, synthetic)?;
                let field_name = format!("_{}", merged_fields.len());
                merged_fields.push(StructField {
                    name: field_name,
                    ty,
                    vis: Some(Visibility::Public),
                });
            }
        }
    }

    // synthetic struct 登録（構造的 dedup 付き）
    let (name, _is_new) = synthetic.register_intersection_struct(&merged_fields);

    // メソッドがある場合は impl ブロックも登録。
    // dedup ヒット時でも、先行登録が TypeLit（メソッドなし）の可能性があるため
    // 無条件に登録する。push_item は insert（上書き）なので重複は無害。
    if !methods.is_empty() {
        let impl_name = format!("{name}Impl");
        synthetic.push_item(
            impl_name,
            SyntheticTypeKind::ImplBlock,
            Item::Impl {
                struct_name: name.clone(),
                type_params: vec![],
                for_trait: None,
                consts: vec![],
                methods,
            },
        );
    }

    Ok(RustType::Named {
        name,
        type_args: vec![],
    })
}

/// インライン型リテラルをアノテーション位置で解決する。
///
/// `{ key: T; }` → synthetic struct、`{ [key: string]: T }` → HashMap。
pub(crate) fn resolve_type_literal(
    lit: &TsTypeLiteralInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // index signature → HashMap
    if let Some(idx) = lit.index_signatures.first() {
        let value_type = resolve_ts_type(&idx.value_type, reg, synthetic)?;
        return Ok(RustType::Named {
            name: "HashMap".to_string(),
            type_args: vec![RustType::String, value_type],
        });
    }

    // フィールドとメソッドを変換（resolve_type_literal_fields に委譲）
    // register_inline_struct は raw name を受け取り内部で sanitize するため、
    // StructField.name（sanitize 済み）ではなく TsFieldInfo.name（raw）を使用
    let fields = resolve_type_literal_fields(lit, reg, synthetic)?;
    let field_defs: Vec<(String, RustType)> = lit
        .fields
        .iter()
        .zip(fields.iter())
        .map(|(raw, resolved)| (raw.name.clone(), resolved.ty.clone()))
        .collect();

    let struct_name = synthetic.register_inline_struct(&field_defs);
    Ok(RustType::Named {
        name: struct_name,
        type_args: vec![],
    })
}

/// TypeLiteral のフィールド情報を StructField に変換する。
pub(crate) fn resolve_type_literal_fields(
    lit: &TsTypeLiteralInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<Vec<StructField>> {
    lit.fields
        .iter()
        .map(|f| {
            let ty = resolve_ts_type(&f.ty, reg, synthetic)?;
            // Optional fields become Option<T>, but avoid double-wrapping
            // when the resolved type is already Option (e.g., `name?: string | null`)
            let ty = if f.optional { ty.wrap_optional() } else { ty };
            Ok(StructField {
                name: crate::ir::sanitize_field_name(&f.name),
                ty,
                vis: Some(Visibility::Public),
            })
        })
        .collect()
}

/// フィールドをマージする。重複フィールドがある場合はエラー。
fn merge_fields_into(
    base: &mut Vec<StructField>,
    new_fields: Vec<StructField>,
) -> anyhow::Result<()> {
    let existing_names: std::collections::HashSet<String> =
        base.iter().map(|f| f.name.clone()).collect();
    for field in new_fields {
        if existing_names.contains(&field.name) {
            return Err(anyhow::anyhow!("duplicate field '{}'", field.name));
        }
        base.push(field);
    }
    Ok(())
}

/// Intersection に union メンバーが含まれる場合の分配解決。
///
/// `{ base: T } & ({ a: U } | { b: V })` → enum with variants carrying base + variant fields.
fn resolve_intersection_with_union(
    members: &[&TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // union メンバーと非 union メンバーを分離
    let mut base_members = Vec::new();
    let mut union_member = None;

    for member in members {
        if let TsTypeInfo::Union(union_members) = member {
            union_member = Some(union_members.as_slice());
        } else {
            base_members.push(*member);
        }
    }

    let union_variants = match union_member {
        Some(variants) => variants,
        None => return Err(anyhow::anyhow!("expected union member in intersection")),
    };

    // base フィールドを抽出
    let mut base_fields: Vec<StructField> = Vec::new();
    let mut base_methods: Vec<crate::ir::Method> = Vec::new();
    for member in &base_members {
        if let TsTypeInfo::TypeLiteral(lit) = member {
            let fields = resolve_type_literal_fields(lit, reg, synthetic)?;
            merge_fields_into(&mut base_fields, fields)?;
            // メソッドを抽出
            for method_info in &lit.methods {
                base_methods.push(resolve_method_info(method_info, reg, synthetic)?);
            }
        } else if let TsTypeInfo::TypeRef { name, .. } = member {
            if let Some(TypeDef::Struct { fields, .. }) = reg.get(name) {
                let struct_fields: Vec<StructField> = fields
                    .iter()
                    .map(|f| StructField {
                        name: crate::ir::sanitize_field_name(&f.name),
                        ty: f.ty.clone(),
                        vis: Some(Visibility::Public),
                    })
                    .collect();
                merge_fields_into(&mut base_fields, struct_fields)?;
            } else {
                let ty = resolve_ts_type(member, reg, synthetic)?;
                base_fields.push(StructField {
                    name: format!("_{}", base_fields.len()),
                    ty,
                    vis: Some(Visibility::Public),
                });
            }
        }
    }

    // discriminant フィールドの検出を試みる
    let discriminant = find_discriminant_field(union_variants);

    // 各 union バリアントに base フィールドをマージして enum variant を生成
    let mut variants = Vec::new();
    for (i, variant_type) in union_variants.iter().enumerate() {
        let (variant_name, variant_value, variant_fields) =
            if let Some(ref disc_field) = discriminant {
                let (raw_value, pascal_name, fields) =
                    extract_discriminated_variant(variant_type, disc_field, reg, synthetic)?;
                (
                    pascal_name,
                    Some(crate::ir::EnumValue::Str(raw_value)),
                    fields,
                )
            } else {
                let fields = extract_variant_fields(variant_type, reg, synthetic)?;
                (format!("Variant{i}"), None, fields)
            };

        // base + variant フィールドをマージ（variant が優先）
        let variant_field_names: std::collections::HashSet<String> =
            variant_fields.iter().map(|f| f.name.clone()).collect();
        let mut merged = base_fields
            .iter()
            .filter(|f| !variant_field_names.contains(&f.name))
            .cloned()
            .collect::<Vec<_>>();
        merged.extend(variant_fields);

        variants.push(EnumVariant {
            name: variant_name,
            value: variant_value,
            data: None,
            fields: merged,
        });
    }

    // intersection enum 登録（構造的 dedup 付き）
    let serde_tag = discriminant;
    let (name, _is_new) = synthetic.register_intersection_enum(serde_tag.as_deref(), variants);

    // メソッドがある場合は impl ブロックも登録。
    // dedup ヒット時でも無条件に登録する（理由は struct パスと同じ）。
    if !base_methods.is_empty() {
        let impl_name = format!("{name}Impl");
        synthetic.push_item(
            impl_name,
            SyntheticTypeKind::ImplBlock,
            Item::Impl {
                struct_name: name.clone(),
                type_params: vec![],
                for_trait: None,
                consts: vec![],
                methods: base_methods,
            },
        );
    }

    Ok(RustType::Named {
        name,
        type_args: vec![],
    })
}

/// Discriminant フィールドを検出する。
///
/// 全ての union variant が TypeLiteral で、共通の string literal フィールドがある場合に
/// そのフィールド名を返す。
pub(crate) fn find_discriminant_field(variants: &[TsTypeInfo]) -> Option<String> {
    // 全バリアントが TypeLiteral であること
    let type_lits: Vec<&TsTypeLiteralInfo> = variants
        .iter()
        .filter_map(|v| match v {
            TsTypeInfo::TypeLiteral(lit) => Some(lit),
            _ => None,
        })
        .collect();

    if type_lits.len() != variants.len() || type_lits.is_empty() {
        return None;
    }

    // 最初の variant のフィールドから discriminant 候補を探す
    for field in &type_lits[0].fields {
        if !matches!(&field.ty, TsTypeInfo::Literal(TsLiteralKind::String(_))) {
            continue;
        }

        // 全 variant に同じフィールドが string literal として存在し、値がユニークか
        let mut values = std::collections::HashSet::new();
        let all_match = type_lits.iter().all(|lit| {
            lit.fields.iter().any(|f| {
                f.name == field.name
                    && matches!(&f.ty, TsTypeInfo::Literal(TsLiteralKind::String(s)) if values.insert(s.clone()))
            })
        });

        if all_match && values.len() == type_lits.len() {
            return Some(field.name.clone());
        }
    }

    None
}

/// Discriminated union variant からバリアント名とフィールドを抽出する。
///
/// 戻り値: `(raw_value, pascal_name, fields)`
/// - `raw_value`: 元の TS 文字列リテラル値（例: `"click"`）
/// - `pascal_name`: PascalCase バリアント名（例: `"Click"`）
/// - `fields`: discriminant フィールドを除いた残りのフィールド
pub(crate) fn extract_discriminated_variant(
    variant_type: &TsTypeInfo,
    disc_field: &str,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<(String, String, Vec<StructField>)> {
    if let TsTypeInfo::TypeLiteral(lit) = variant_type {
        let mut raw_value = String::new();
        let mut fields = Vec::new();

        for field in &lit.fields {
            if field.name == disc_field {
                if let TsTypeInfo::Literal(TsLiteralKind::String(s)) = &field.ty {
                    raw_value = s.clone();
                }
                continue; // discriminant フィールド自体は含めない
            }
            let ty = resolve_ts_type(&field.ty, reg, synthetic)?;
            let ty = if field.optional {
                RustType::Option(Box::new(ty))
            } else {
                ty
            };
            fields.push(StructField {
                name: crate::ir::sanitize_field_name(&field.name),
                ty,
                vis: Some(Visibility::Public),
            });
        }

        let pascal_name = crate::ir::string_to_pascal_case(&raw_value);
        Ok((raw_value, pascal_name, fields))
    } else {
        // TypeLiteral 以外のバリアント
        let ty = resolve_ts_type(variant_type, reg, synthetic)?;
        Ok((
            String::new(),
            "Variant".to_string(),
            vec![StructField {
                name: "_data".to_string(),
                ty,
                vis: Some(Visibility::Public),
            }],
        ))
    }
}

/// Union variant からフィールドを抽出する（非 discriminated 用）。
pub(crate) fn extract_variant_fields(
    variant_type: &TsTypeInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<Vec<StructField>> {
    match variant_type {
        TsTypeInfo::TypeLiteral(lit) => resolve_type_literal_fields(lit, reg, synthetic),
        TsTypeInfo::TypeRef { name, .. } => {
            if let Some(TypeDef::Struct { fields, .. }) = reg.get(name) {
                Ok(fields
                    .iter()
                    .map(|f| StructField {
                        name: crate::ir::sanitize_field_name(&f.name),
                        ty: f.ty.clone(),
                        vis: Some(Visibility::Public),
                    })
                    .collect())
            } else {
                let ty = resolve_ts_type(variant_type, reg, synthetic)?;
                Ok(vec![StructField {
                    name: "_data".to_string(),
                    ty,
                    vis: Some(Visibility::Public),
                }])
            }
        }
        _ => {
            let ty = resolve_ts_type(variant_type, reg, synthetic)?;
            Ok(vec![StructField {
                name: "_data".to_string(),
                ty,
                vis: Some(Visibility::Public),
            }])
        }
    }
}

/// TsMethodInfo を IR の Method に変換する。
///
/// TS の `void` は `RustType::Unit` に解決される（`-> ()` として表現）。
/// generator 側で `Unit` 戻り値の省略を処理する。
pub(crate) fn resolve_method_info(
    method: &TsMethodInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<Method> {
    let params = method
        .params
        .iter()
        .map(|p| {
            let ty = resolve_ts_type(&p.ty, reg, synthetic)?;
            Ok(Param {
                name: p.name.clone(),
                ty: Some(ty),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let return_type = method
        .return_type
        .as_ref()
        .map(|rt| resolve_ts_type(rt, reg, synthetic))
        .transpose()?;
    Ok(Method {
        vis: Visibility::Public,
        name: method.name.clone(),
        params,
        return_type,
        body: None,
        has_self: true,
        has_mut_self: false,
    })
}

/// 空の型リテラル `{}` か判定する。
fn is_empty_type_literal(info: &TsTypeInfo) -> bool {
    matches!(
        info,
        TsTypeInfo::TypeLiteral(TsTypeLiteralInfo {
            fields,
            methods,
            call_signatures,
            construct_signatures,
            index_signatures,
        }) if fields.is_empty()
            && methods.is_empty()
            && call_signatures.is_empty()
            && construct_signatures.is_empty()
            && index_signatures.is_empty()
    )
}

#[cfg(test)]
#[path = "intersection_tests.rs"]
mod tests;
