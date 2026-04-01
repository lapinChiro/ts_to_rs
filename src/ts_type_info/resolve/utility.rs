//! ユーティリティ型（Partial, Required, Pick, Omit, NonNullable）の解決。
//!
//! TypeScript の組み込みユーティリティ型を Rust 型に変換する。
//! フィールド操作を伴うため、TypeRegistry からの型定義参照が必要。

use crate::ir::{Item, RustType, StructField, Visibility};
use crate::pipeline::synthetic_registry::SyntheticTypeKind;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{FieldDef, TypeDef, TypeRegistry};
use crate::ts_type_info::{TsLiteralKind, TsTypeInfo};

use super::resolve_ts_type;

/// `Partial<T>` を解決する。全フィールドを Option ラップ。
pub(crate) fn resolve_partial(
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    let (inner_name, fields) =
        match resolve_inner_fields_with_conversion(type_args, reg, synthetic)? {
            Some(resolved) => resolved,
            None => {
                if let Some(arg) = type_args.first() {
                    return resolve_ts_type(arg, reg, synthetic);
                }
                return Ok(RustType::Any);
            }
        };

    let partial_fields: Vec<StructField> = fields
        .into_iter()
        .map(|f| {
            let ty = if matches!(f.ty, RustType::Option(_)) {
                f.ty
            } else {
                RustType::Option(Box::new(f.ty))
            };
            StructField {
                name: f.name,
                ty,
                vis: Some(Visibility::Public),
            }
        })
        .collect();

    let name = format!("Partial{inner_name}");
    synthetic.push_item(
        name.clone(),
        SyntheticTypeKind::InlineStruct,
        Item::Struct {
            vis: Visibility::Public,
            name: name.clone(),
            fields: partial_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name,
        type_args: vec![],
    })
}

/// `Required<T>` を解決する。全フィールドから Option を除去。
pub(crate) fn resolve_required(
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    let (inner_name, fields) =
        match resolve_inner_fields_with_conversion(type_args, reg, synthetic)? {
            Some(resolved) => resolved,
            None => {
                if let Some(arg) = type_args.first() {
                    return resolve_ts_type(arg, reg, synthetic);
                }
                return Ok(RustType::Any);
            }
        };

    let required_fields: Vec<StructField> = fields
        .into_iter()
        .map(|f| {
            let ty = match f.ty {
                RustType::Option(inner) => *inner,
                other => other,
            };
            StructField {
                name: f.name,
                ty,
                vis: Some(Visibility::Public),
            }
        })
        .collect();

    let name = format!("Required{inner_name}");
    synthetic.push_item(
        name.clone(),
        SyntheticTypeKind::InlineStruct,
        Item::Struct {
            vis: Visibility::Public,
            name: name.clone(),
            fields: required_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name,
        type_args: vec![],
    })
}

/// `Pick<T, K>` を解決する。指定キーのフィールドのみ抽出。
pub(crate) fn resolve_pick(
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    if type_args.len() < 2 {
        return Err(anyhow::anyhow!("Pick requires 2 type arguments"));
    }

    let (inner_name, fields) = match resolve_inner_fields(type_args, reg, synthetic) {
        Some(resolved) => resolved,
        None => return Ok(RustType::Any),
    };

    let keys = extract_string_keys(&type_args[1]);
    let picked_fields: Vec<StructField> = fields
        .into_iter()
        .filter(|f| keys.contains(&f.name))
        .map(|f| StructField {
            name: f.name,
            ty: f.ty,
            vis: Some(Visibility::Public),
        })
        .collect();

    let keys_suffix: String = keys
        .iter()
        .map(|k| capitalize_first(k))
        .collect::<Vec<_>>()
        .join("");
    let name = format!("Pick{inner_name}{keys_suffix}");
    synthetic.push_item(
        name.clone(),
        SyntheticTypeKind::InlineStruct,
        Item::Struct {
            vis: Visibility::Public,
            name: name.clone(),
            fields: picked_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name,
        type_args: vec![],
    })
}

/// `Omit<T, K>` を解決する。指定キーのフィールドを除外。
pub(crate) fn resolve_omit(
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    if type_args.len() < 2 {
        return Err(anyhow::anyhow!("Omit requires 2 type arguments"));
    }

    let (inner_name, fields) = match resolve_inner_fields(type_args, reg, synthetic) {
        Some(resolved) => resolved,
        None => return Ok(RustType::Any),
    };

    let keys = extract_string_keys(&type_args[1]);
    let omitted_fields: Vec<StructField> = fields
        .into_iter()
        .filter(|f| !keys.contains(&f.name))
        .map(|f| StructField {
            name: f.name,
            ty: f.ty,
            vis: Some(Visibility::Public),
        })
        .collect();

    let keys_suffix: String = keys
        .iter()
        .map(|k| capitalize_first(k))
        .collect::<Vec<_>>()
        .join("");
    let name = format!("Omit{inner_name}{keys_suffix}");
    synthetic.push_item(
        name.clone(),
        SyntheticTypeKind::InlineStruct,
        Item::Struct {
            vis: Visibility::Public,
            name: name.clone(),
            fields: omitted_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name,
        type_args: vec![],
    })
}

/// `NonNullable<T>` を解決する。Option ラップを除去。
pub(crate) fn resolve_non_nullable(
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    if type_args.is_empty() {
        return Err(anyhow::anyhow!("NonNullable requires 1 type argument"));
    }

    let ty = resolve_ts_type(&type_args[0], reg, synthetic)?;
    Ok(match ty {
        RustType::Option(inner) => *inner,
        other => other,
    })
}

/// 内部型のフィールド情報を解決する。
///
/// 最初の型引数が TypeRef の場合、レジストリまたは synthetic レジストリから
/// フィールド情報を取得する。
fn resolve_inner_fields(
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) -> Option<(String, Vec<FieldDef>)> {
    let inner_name = match type_args.first()? {
        TsTypeInfo::TypeRef { name, .. } => name.clone(),
        _ => return None,
    };

    // レジストリから検索
    if let Some(TypeDef::Struct { fields, .. }) = reg.get(&inner_name) {
        return Some((inner_name, fields.clone()));
    }

    // synthetic レジストリから検索
    if let Some(syn_def) = synthetic.get(&inner_name) {
        if let Item::Struct { fields, .. } = &syn_def.item {
            let field_defs = fields
                .iter()
                .map(|f| FieldDef {
                    name: f.name.clone(),
                    ty: f.ty.clone(),
                    optional: matches!(f.ty, RustType::Option(_)),
                })
                .collect();
            return Some((inner_name, field_defs));
        }
    }

    None
}

/// フィールド解決のフォールバック付き版。
///
/// `resolve_inner_fields` で見つからない場合、型引数を `resolve_ts_type` で変換し、
/// 結果の Named 型を synthetic レジストリから再検索する。
/// `Partial<Pick<T, K>>` のような nested utility 型に対応。
fn resolve_inner_fields_with_conversion(
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<Option<(String, Vec<FieldDef>)>> {
    // まず直接検索
    if let Some(result) = resolve_inner_fields(type_args, reg, synthetic) {
        return Ok(Some(result));
    }

    // 型引数を変換して再検索
    let arg = match type_args.first() {
        Some(arg) => arg,
        None => return Ok(None),
    };

    let resolved = resolve_ts_type(arg, reg, synthetic)?;
    if let RustType::Named { name, .. } = &resolved {
        // 変換後の名前で synthetic を検索
        if let Some(syn_def) = synthetic.get(name) {
            if let Item::Struct { fields, .. } = &syn_def.item {
                let field_defs = fields
                    .iter()
                    .map(|f| FieldDef {
                        name: f.name.clone(),
                        ty: f.ty.clone(),
                        optional: matches!(f.ty, RustType::Option(_)),
                    })
                    .collect();
                return Ok(Some((name.clone(), field_defs)));
            }
        }
    }

    Ok(None)
}

/// TsTypeInfo から string key リストを抽出する。
///
/// string literal → 単一キー、union → 再帰的にキー収集。
fn extract_string_keys(info: &TsTypeInfo) -> Vec<String> {
    match info {
        TsTypeInfo::Literal(TsLiteralKind::String(s)) => vec![s.clone()],
        TsTypeInfo::Union(members) => members.iter().flat_map(extract_string_keys).collect(),
        _ => vec![],
    }
}

/// 先頭を大文字にする。
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_registry_with_foo() -> TypeRegistry {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Foo".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![
                    FieldDef {
                        name: "name".to_string(),
                        ty: RustType::String,
                        optional: false,
                    },
                    FieldDef {
                        name: "age".to_string(),
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
        reg
    }

    #[test]
    fn partial_wraps_in_option() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_partial(
            &[TsTypeInfo::TypeRef {
                name: "Foo".to_string(),
                type_args: vec![],
            }],
            &reg,
            &mut syn,
        )
        .unwrap();

        match result {
            RustType::Named { name, .. } => assert_eq!(name, "PartialFoo"),
            _ => panic!("expected Named"),
        }
        // synthetic に登録されているか確認
        assert!(syn.get("PartialFoo").is_some());
    }

    #[test]
    fn pick_selects_fields() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_pick(
            &[
                TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                },
                TsTypeInfo::Literal(TsLiteralKind::String("name".to_string())),
            ],
            &reg,
            &mut syn,
        )
        .unwrap();

        match result {
            RustType::Named { name, .. } => assert!(name.starts_with("PickFoo")),
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn non_nullable_strips_option() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // number | null → Option<f64> → NonNullable → f64
        let result = resolve_non_nullable(
            &[TsTypeInfo::Union(vec![
                TsTypeInfo::Number,
                TsTypeInfo::Null,
            ])],
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::F64);
    }
}
