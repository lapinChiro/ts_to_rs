//! TsTypeInfo::Intersection / TypeLiteral → RustType 解決。
//!
//! TypeScript の intersection 型とインライン型リテラルを Rust 型に変換する。
//! - `A & B` → フィールドマージした synthetic struct
//! - `{ key: T; }` → synthetic inline struct
//! - `{ [key: string]: T }` → HashMap<String, T>
//! - `{ [K in keyof T]: T[K] }` → T（identity mapped type の簡約）

use crate::ir::{EnumVariant, Item, RustType, StructField, Visibility};
use crate::pipeline::synthetic_registry::SyntheticTypeKind;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::ts_type_info::{TsLiteralKind, TsTypeInfo, TsTypeLiteralInfo};

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
        let name = synthetic.generate_name("Intersection");
        synthetic.push_item(
            name.clone(),
            SyntheticTypeKind::InlineStruct,
            Item::Struct {
                vis: Visibility::Public,
                name: name.clone(),
                fields: vec![],
                type_params: vec![],
            },
        );
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
                for method in &lit.methods {
                    let params = method
                        .params
                        .iter()
                        .filter_map(|p| {
                            let ty = resolve_ts_type(&p.ty, reg, synthetic).ok()?;
                            Some(crate::ir::Param {
                                name: p.name.clone(),
                                ty: Some(ty),
                            })
                        })
                        .collect();
                    let return_type = method
                        .return_type
                        .as_ref()
                        .and_then(|rt| resolve_ts_type(rt, reg, synthetic).ok());
                    methods.push(crate::ir::Method {
                        vis: Visibility::Public,
                        name: method.name.clone(),
                        params,
                        return_type,
                        body: None,
                        has_self: true,
                        has_mut_self: false,
                    });
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

    // synthetic struct 登録
    let name = synthetic.generate_name("Intersection");
    synthetic.push_item(
        name.clone(),
        SyntheticTypeKind::InlineStruct,
        Item::Struct {
            vis: Visibility::Public,
            name: name.clone(),
            fields: merged_fields,
            type_params: vec![],
        },
    );

    // メソッドがある場合は impl ブロックも登録
    if !methods.is_empty() {
        let impl_name = format!("{name}Impl");
        synthetic.push_item(
            impl_name,
            SyntheticTypeKind::InlineStruct,
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

    // フィールドとメソッドを変換
    let field_defs: Vec<(String, RustType)> = lit
        .fields
        .iter()
        .filter_map(|f| {
            let ty = resolve_ts_type(&f.ty, reg, synthetic).ok()?;
            let ty = if f.optional {
                RustType::Option(Box::new(ty))
            } else {
                ty
            };
            Some((f.name.clone(), ty))
        })
        .collect();

    let struct_name = synthetic.register_inline_struct(&field_defs);
    Ok(RustType::Named {
        name: struct_name,
        type_args: vec![],
    })
}

/// TypeLiteral のフィールド情報を StructField に変換する。
fn resolve_type_literal_fields(
    lit: &TsTypeLiteralInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<Vec<StructField>> {
    lit.fields
        .iter()
        .filter_map(|f| {
            let ty = resolve_ts_type(&f.ty, reg, synthetic).ok()?;
            let ty = if f.optional {
                RustType::Option(Box::new(ty))
            } else {
                ty
            };
            Some(StructField {
                name: crate::ir::sanitize_field_name(&f.name),
                ty,
                vis: Some(Visibility::Public),
            })
        })
        .collect::<Vec<_>>()
        .pipe_ok()
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
            for method in &lit.methods {
                let params = method
                    .params
                    .iter()
                    .filter_map(|p| {
                        let ty = resolve_ts_type(&p.ty, reg, synthetic).ok()?;
                        Some(crate::ir::Param {
                            name: p.name.clone(),
                            ty: Some(ty),
                        })
                    })
                    .collect();
                let return_type = method
                    .return_type
                    .as_ref()
                    .and_then(|rt| resolve_ts_type(rt, reg, synthetic).ok());
                base_methods.push(crate::ir::Method {
                    vis: Visibility::Public,
                    name: method.name.clone(),
                    params,
                    return_type,
                    body: None,
                    has_self: true,
                    has_mut_self: false,
                });
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
        let (variant_name, variant_fields) = if let Some(ref disc_field) = discriminant {
            extract_discriminated_variant(variant_type, disc_field, reg, synthetic)?
        } else {
            let fields = extract_variant_fields(variant_type, reg, synthetic)?;
            (format!("Variant{i}"), fields)
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
            value: None,
            data: None,
            fields: merged,
        });
    }

    let name = synthetic.generate_name("Intersection");
    let serde_tag = discriminant;
    synthetic.push_item(
        name.clone(),
        SyntheticTypeKind::UnionEnum,
        Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
            serde_tag,
            variants,
        },
    );

    // メソッドがある場合は impl ブロックも登録
    if !base_methods.is_empty() {
        let impl_name = format!("{name}Impl");
        synthetic.push_item(
            impl_name,
            SyntheticTypeKind::InlineStruct,
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
fn find_discriminant_field(variants: &[TsTypeInfo]) -> Option<String> {
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
fn extract_discriminated_variant(
    variant_type: &TsTypeInfo,
    disc_field: &str,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<(String, Vec<StructField>)> {
    if let TsTypeInfo::TypeLiteral(lit) = variant_type {
        let mut disc_value = String::new();
        let mut fields = Vec::new();

        for field in &lit.fields {
            if field.name == disc_field {
                if let TsTypeInfo::Literal(TsLiteralKind::String(s)) = &field.ty {
                    disc_value = crate::ir::string_to_pascal_case(s);
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

        Ok((disc_value, fields))
    } else {
        // TypeLiteral 以外のバリアント
        let ty = resolve_ts_type(variant_type, reg, synthetic)?;
        Ok((
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
fn extract_variant_fields(
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

/// Vec に Ok をパイプする拡張（filter_map の Result 変換用）。
trait PipeOk {
    fn pipe_ok(self) -> anyhow::Result<Self>
    where
        Self: Sized;
}
impl<T> PipeOk for Vec<T> {
    fn pipe_ok(self) -> anyhow::Result<Self> {
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ts_type_info::TsFieldInfo;
    use std::collections::HashMap;

    #[test]
    fn type_literal_to_inline_struct() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let lit = TsTypeLiteralInfo {
            fields: vec![TsFieldInfo {
                name: "x".to_string(),
                ty: TsTypeInfo::String,
                optional: false,
            }],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        };
        let result = resolve_type_literal(&lit, &reg, &mut syn).unwrap();
        match result {
            RustType::Named { name, .. } => assert!(!name.is_empty()),
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn index_signature_to_hashmap() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let lit = TsTypeLiteralInfo {
            fields: vec![],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![crate::ts_type_info::TsIndexSigInfo {
                param_name: "key".to_string(),
                param_type: TsTypeInfo::String,
                value_type: TsTypeInfo::Number,
                readonly: false,
            }],
        };
        let result = resolve_type_literal(&lit, &reg, &mut syn).unwrap();
        assert_eq!(
            result,
            RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, RustType::F64],
            }
        );
    }

    #[test]
    fn intersection_merges_fields() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();

        reg.register(
            "A".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![crate::registry::FieldDef {
                    name: "x".to_string(),
                    ty: RustType::String,
                    optional: false,
                }],
                methods: HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );

        let members = vec![
            TsTypeInfo::TypeRef {
                name: "A".to_string(),
                type_args: vec![],
            },
            TsTypeInfo::TypeLiteral(TsTypeLiteralInfo {
                fields: vec![TsFieldInfo {
                    name: "y".to_string(),
                    ty: TsTypeInfo::Number,
                    optional: false,
                }],
                methods: vec![],
                call_signatures: vec![],
                construct_signatures: vec![],
                index_signatures: vec![],
            }),
        ];

        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        match result {
            RustType::Named { name, .. } => assert!(name.contains("Intersection")),
            _ => panic!("expected Named for intersection struct"),
        }
    }

    #[test]
    fn identity_mapped_type_simplified() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            }))),
            value: Some(Box::new(TsTypeInfo::IndexedAccess {
                object: Box::new(TsTypeInfo::TypeRef {
                    name: "T".to_string(),
                    type_args: vec![],
                }),
                index: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
            has_readonly: false,
            has_optional: false,
            name_type: None,
        }];

        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        assert_eq!(
            result,
            RustType::Named {
                name: "T".to_string(),
                type_args: vec![]
            }
        );
    }

    // --- 以下、テストカバレッジ向上のため追加 ---

    fn empty_type_literal() -> TsTypeLiteralInfo {
        TsTypeLiteralInfo {
            fields: vec![],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        }
    }

    fn type_literal(fields: Vec<TsFieldInfo>) -> TsTypeLiteralInfo {
        TsTypeLiteralInfo {
            fields,
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        }
    }

    fn field(name: &str, ty: TsTypeInfo) -> TsFieldInfo {
        TsFieldInfo {
            name: name.to_string(),
            ty,
            optional: false,
        }
    }

    fn string_lit(s: &str) -> TsTypeInfo {
        TsTypeInfo::Literal(TsLiteralKind::String(s.to_string()))
    }

    #[test]
    fn empty_type_literals_filtered_from_intersection() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // {} & {} → 空メンバー → 空 struct
        let members = vec![
            TsTypeInfo::TypeLiteral(empty_type_literal()),
            TsTypeInfo::TypeLiteral(empty_type_literal()),
        ];
        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        match result {
            RustType::Named { name, .. } => assert!(name.contains("Intersection")),
            other => panic!("expected Named, got {other:?}"),
        }
    }

    #[test]
    fn identity_mapped_with_readonly_not_simplified() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            }))),
            value: Some(Box::new(TsTypeInfo::IndexedAccess {
                object: Box::new(TsTypeInfo::TypeRef {
                    name: "T".to_string(),
                    type_args: vec![],
                }),
                index: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
            has_readonly: true,
            has_optional: false,
            name_type: None,
        }];
        // readonly 修飾子がある場合、簡約されない → HashMap フォールバック
        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        assert_ne!(
            result,
            RustType::Named {
                name: "T".to_string(),
                type_args: vec![]
            }
        );
    }

    #[test]
    fn identity_mapped_with_optional_not_simplified() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            }))),
            value: Some(Box::new(TsTypeInfo::IndexedAccess {
                object: Box::new(TsTypeInfo::TypeRef {
                    name: "T".to_string(),
                    type_args: vec![],
                }),
                index: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
            has_readonly: false,
            has_optional: true,
            name_type: None,
        }];
        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        assert_ne!(
            result,
            RustType::Named {
                name: "T".to_string(),
                type_args: vec![]
            }
        );
    }

    #[test]
    fn identity_mapped_value_mismatch_not_simplified() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // value が T[K] でなく string → 簡約されない
        let members = vec![TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            }))),
            value: Some(Box::new(TsTypeInfo::String)),
            has_readonly: false,
            has_optional: false,
            name_type: None,
        }];
        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        // HashMap フォールバック
        match result {
            RustType::Named { name, .. } => assert_eq!(name, "HashMap"),
            other => panic!("expected HashMap, got {other:?}"),
        }
    }

    #[test]
    fn identity_mapped_constraint_not_keyof_not_simplified() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // constraint が keyof T でなく string → 簡約されない
        let members = vec![TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::String),
            value: Some(Box::new(TsTypeInfo::Number)),
            has_readonly: false,
            has_optional: false,
            name_type: None,
        }];
        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        match result {
            RustType::Named { name, .. } => assert_eq!(name, "HashMap"),
            other => panic!("expected HashMap, got {other:?}"),
        }
    }

    #[test]
    fn merge_fields_duplicate_error() {
        let mut base = vec![StructField {
            name: "x".to_string(),
            ty: RustType::String,
            vis: Some(Visibility::Public),
        }];
        let new = vec![StructField {
            name: "x".to_string(),
            ty: RustType::F64,
            vis: Some(Visibility::Public),
        }];
        let err = merge_fields_into(&mut base, new).unwrap_err();
        assert!(err.to_string().contains("duplicate field 'x'"));
    }

    #[test]
    fn discriminated_union_with_intersection() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();

        // { base: string } & ({ kind: "click", x: number } | { kind: "hover", y: number })
        let members = vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![field("base", TsTypeInfo::String)])),
            TsTypeInfo::Union(vec![
                TsTypeInfo::TypeLiteral(type_literal(vec![
                    field("kind", string_lit("click")),
                    field("x", TsTypeInfo::Number),
                ])),
                TsTypeInfo::TypeLiteral(type_literal(vec![
                    field("kind", string_lit("hover")),
                    field("y", TsTypeInfo::Number),
                ])),
            ]),
        ];

        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        match &result {
            RustType::Named { name, .. } => assert!(name.contains("Intersection")),
            other => panic!("expected Named, got {other:?}"),
        }

        // synthetic に登録された enum を確認
        let items = syn.all_items();
        let enum_found = items.iter().any(|item| {
            if let Item::Enum {
                serde_tag,
                variants,
                ..
            } = item
            {
                assert_eq!(serde_tag.as_deref(), Some("kind"));
                assert_eq!(variants.len(), 2);
                let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
                assert!(
                    names.contains(&"Click"),
                    "expected Click variant, got {names:?}"
                );
                assert!(
                    names.contains(&"Hover"),
                    "expected Hover variant, got {names:?}"
                );
                for v in variants {
                    let field_names: Vec<&str> = v.fields.iter().map(|f| f.name.as_str()).collect();
                    assert!(
                        field_names.contains(&"base"),
                        "variant {} should have base field, got {field_names:?}",
                        v.name
                    );
                }
                true
            } else {
                false
            }
        });
        assert!(enum_found, "discriminated union enum should be registered");
    }

    #[test]
    fn non_discriminated_union_with_intersection() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();

        // { base: string } & ({ x: number } | { y: string })
        // discriminant なし → Variant0, Variant1
        let members = vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![field("base", TsTypeInfo::String)])),
            TsTypeInfo::Union(vec![
                TsTypeInfo::TypeLiteral(type_literal(vec![field("x", TsTypeInfo::Number)])),
                TsTypeInfo::TypeLiteral(type_literal(vec![field("y", TsTypeInfo::String)])),
            ]),
        ];

        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        assert!(matches!(result, RustType::Named { .. }));

        let items = syn.all_items();
        let enum_found = items.iter().any(|item| {
            if let Item::Enum {
                serde_tag,
                variants,
                ..
            } = item
            {
                assert_eq!(*serde_tag, None, "no discriminant → no serde_tag");
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "Variant0");
                assert_eq!(variants[1].name, "Variant1");
                true
            } else {
                false
            }
        });
        assert!(enum_found);
    }

    #[test]
    fn find_discriminant_duplicate_values_returns_none() {
        // { kind: "a" } | { kind: "a" } → 重複 → None
        let variants = vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("a"))])),
            TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("a"))])),
        ];
        assert_eq!(find_discriminant_field(&variants), None);
    }

    #[test]
    fn find_discriminant_no_common_field_returns_none() {
        let variants = vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![field("x", TsTypeInfo::Number)])),
            TsTypeInfo::TypeLiteral(type_literal(vec![field("y", TsTypeInfo::String)])),
        ];
        assert_eq!(find_discriminant_field(&variants), None);
    }

    #[test]
    fn find_discriminant_valid() {
        let variants = vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![
                field("type", string_lit("text")),
                field("content", TsTypeInfo::String),
            ])),
            TsTypeInfo::TypeLiteral(type_literal(vec![
                field("type", string_lit("image")),
                field("url", TsTypeInfo::String),
            ])),
            TsTypeInfo::TypeLiteral(type_literal(vec![
                field("type", string_lit("video")),
                field("src", TsTypeInfo::String),
            ])),
        ];
        assert_eq!(find_discriminant_field(&variants), Some("type".to_string()));
    }

    #[test]
    fn find_discriminant_non_type_literal_returns_none() {
        // TypeLiteral 以外が含まれる → None
        let variants = vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("a"))])),
            TsTypeInfo::String,
        ];
        assert_eq!(find_discriminant_field(&variants), None);
    }

    #[test]
    fn unresolvable_typeref_becomes_embed_field() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();

        // A & { y: number } where A is not in registry
        let members = vec![
            TsTypeInfo::TypeRef {
                name: "Unknown".to_string(),
                type_args: vec![],
            },
            TsTypeInfo::TypeLiteral(type_literal(vec![field("y", TsTypeInfo::Number)])),
        ];

        let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
        assert!(matches!(result, RustType::Named { .. }));
    }

    #[test]
    fn extract_variant_fields_type_literal() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let lit = TsTypeInfo::TypeLiteral(type_literal(vec![
            field("a", TsTypeInfo::String),
            field("b", TsTypeInfo::Number),
        ]));
        let fields = extract_variant_fields(&lit, &reg, &mut syn).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "a");
        assert_eq!(fields[1].name, "b");
    }

    #[test]
    fn extract_variant_fields_typeref_in_registry() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "Point".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![
                    crate::registry::FieldDef {
                        name: "x".to_string(),
                        ty: RustType::F64,
                        optional: false,
                    },
                    crate::registry::FieldDef {
                        name: "y".to_string(),
                        ty: RustType::F64,
                        optional: false,
                    },
                ],
                methods: HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let ty = TsTypeInfo::TypeRef {
            name: "Point".to_string(),
            type_args: vec![],
        };
        let fields = extract_variant_fields(&ty, &reg, &mut syn).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "x");
    }

    #[test]
    fn extract_variant_fields_unknown_type() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let ty = TsTypeInfo::String;
        let fields = extract_variant_fields(&ty, &reg, &mut syn).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "_data");
    }
}
