//! TsTypeInfo::IndexedAccess → RustType 解決。
//!
//! TypeScript の `T[K]` 型を Rust 型に変換する。
//! - `T["field"]` → フィールド型参照
//! - `T[number]` → 配列要素型
//! - `T[keyof typeof X]` → 値型の union
//! - `T[keyof T]` → 全フィールド値型の union

use crate::ir::RustType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::ts_type_info::{TsLiteralKind, TsTypeInfo};

/// Indexed access 型を解決する。
///
/// `T[K]` の形式を解析し、レジストリ参照によりフィールド型を解決する。
pub(crate) fn resolve_indexed_access(
    object: &TsTypeInfo,
    index: &TsTypeInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // TypeLiteral の場合は直接フィールド解決
    if let TsTypeInfo::TypeLiteral(lit) = object {
        if let TsTypeInfo::Literal(TsLiteralKind::String(field_name)) = index {
            // inline struct のフィールドから直接検索
            for field in &lit.fields {
                if field.name == *field_name {
                    return super::resolve_ts_type(&field.ty, reg, synthetic);
                }
            }
        }
        // TypeLiteral を synthetic struct に変換し、そこからフィールド解決
        let resolved = super::resolve_ts_type(object, reg, synthetic)?;
        if let RustType::Named { name, .. } = &resolved {
            if let TsTypeInfo::Literal(TsLiteralKind::String(field_name)) = index {
                return Ok(RustType::Named {
                    name: format!("{name}::{field_name}"),
                    type_args: vec![],
                });
            }
        }
        return Ok(RustType::Any);
    }

    // オブジェクト型名を抽出
    let obj_name = match extract_type_name(object) {
        Some(name) => name,
        None => {
            // typeof X の場合
            if let TsTypeInfo::TypeQuery(name) = object {
                // レジストリ未登録なら Any にフォールバック
                if reg.get(name).is_none() {
                    return Ok(RustType::Any);
                }
                name.clone()
            } else if let TsTypeInfo::IndexedAccess {
                object: inner_obj,
                index: inner_idx,
            } = object
            {
                // ネストされた indexed access: 内側を先に解決
                let inner_resolved = resolve_indexed_access(inner_obj, inner_idx, reg, synthetic)?;
                if let RustType::Named { name, .. } = inner_resolved {
                    name
                } else {
                    return Ok(RustType::Any);
                }
            } else {
                // 解決できない複雑なオブジェクト型はフォールバック
                return Ok(RustType::Any);
            }
        }
    };

    // インデックス型による分岐
    match index {
        // T[number] → 配列要素型
        TsTypeInfo::Number => resolve_number_index(&obj_name, synthetic, reg),

        // T[keyof typeof X]
        TsTypeInfo::KeyOf(inner) => match inner.as_ref() {
            TsTypeInfo::TypeQuery(name) if reg.get(name).is_some() => {
                resolve_keyof_typeof_index(name, synthetic, reg)
            }
            TsTypeInfo::TypeRef { name, .. } if reg.get(name).is_some() => {
                resolve_type_param_indexed_access(name, reg, synthetic)
            }
            _ => resolve_type_param_indexed_access(&obj_name, reg, synthetic),
        },

        // T["fieldName"] → フィールド型参照
        TsTypeInfo::Literal(TsLiteralKind::String(field_name)) => {
            match lookup_field_type(&obj_name, field_name, reg, synthetic) {
                Some(ty) => Ok(ty),
                // レジストリに登録されていない場合は associated type 形式
                None => Ok(RustType::Named {
                    name: format!("{obj_name}::{field_name}"),
                    type_args: vec![],
                }),
            }
        }

        // T[number literal] → Any
        TsTypeInfo::Literal(TsLiteralKind::Number(_)) => Ok(RustType::Any),

        // T['a' | 'b'] → union key access → フィールド型の union
        TsTypeInfo::Union(members) => {
            let mut types = Vec::new();
            for member in members {
                let ty = resolve_indexed_access(object, member, reg, synthetic)?;
                if !types.contains(&ty) {
                    types.push(ty);
                }
            }
            match types.len() {
                0 => Ok(RustType::Any),
                1 => Ok(types.into_iter().next().expect("len == 1")),
                _ => {
                    let name = synthetic.register_union(&types);
                    Ok(RustType::Named {
                        name,
                        type_args: vec![],
                    })
                }
            }
        }

        // T[K] where K is a type parameter → 全フィールド値型の union
        TsTypeInfo::TypeRef { name, .. } if reg.get(name).is_none() => {
            resolve_type_param_indexed_access(&obj_name, reg, synthetic)
        }

        _ => Ok(RustType::Any),
    }
}

/// 型名を抽出する（TypeRef から）。
fn extract_type_name(info: &TsTypeInfo) -> Option<String> {
    match info {
        TsTypeInfo::TypeRef { name, .. } => Some(name.clone()),
        _ => None,
    }
}

/// `T[number]` を解決する。
///
/// const 配列の要素型を取得。全要素が string literal なら string enum を生成。
fn resolve_number_index(
    obj_name: &str,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> anyhow::Result<RustType> {
    let typedef = match reg.get(obj_name) {
        Some(td) => td,
        None => return Ok(RustType::Any),
    };

    if let TypeDef::ConstValue { elements, .. } = typedef {
        if elements.is_empty() {
            return Ok(RustType::Any);
        }

        // 全要素が string literal → string enum 生成
        let string_values: Vec<String> = elements
            .iter()
            .filter_map(|e| e.string_literal_value.clone())
            .collect();
        if string_values.len() == elements.len() {
            let enum_name = synthetic.register_string_literal_enum(obj_name, &string_values);
            return Ok(RustType::Named {
                name: enum_name,
                type_args: vec![],
            });
        }

        // 要素型のユニーク集合
        let mut unique_types: Vec<RustType> = Vec::new();
        for elem in elements {
            if !unique_types.contains(&elem.ty) {
                unique_types.push(elem.ty.clone());
            }
        }

        return match unique_types.len() {
            1 => Ok(unique_types.into_iter().next().expect("len == 1")),
            _ => {
                let name = synthetic.register_union(&unique_types);
                Ok(RustType::Named {
                    name,
                    type_args: vec![],
                })
            }
        };
    }

    Ok(RustType::Any)
}

/// `T[keyof typeof X]` を解決する。
///
/// const オブジェクトの全値型を取得。全値が string literal なら string enum を生成。
fn resolve_keyof_typeof_index(
    obj_name: &str,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> anyhow::Result<RustType> {
    let typedef = match reg.get(obj_name) {
        Some(td) => td,
        None => {
            return Err(anyhow::anyhow!(
                "unsupported indexed access: [keyof typeof] requires a const object type"
            ))
        }
    };

    // 全フィールドが string literal → string enum
    if let Some(values) = typedef.all_string_literal_field_values() {
        let enum_name = synthetic.register_string_literal_enum(obj_name, &values);
        return Ok(RustType::Named {
            name: enum_name,
            type_args: vec![],
        });
    }

    // ユニークな値型を取得
    let unique_types = typedef.unique_field_types().unwrap_or_default();
    match unique_types.len() {
        0 => Ok(RustType::Any),
        1 => Ok(unique_types.into_iter().next().expect("len == 1")),
        _ => {
            let name = synthetic.register_union(&unique_types);
            Ok(RustType::Named {
                name,
                type_args: vec![],
            })
        }
    }
}

/// ジェネリクス消去による indexed access 解決。
///
/// `T[K]` where K is a type parameter → 全フィールド値型の union。
fn resolve_type_param_indexed_access(
    obj_name: &str,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    let typedef = match reg.get(obj_name) {
        Some(td) => td,
        None => return Ok(RustType::Any),
    };

    let unique_types = typedef.unique_field_types().unwrap_or_default();
    match unique_types.len() {
        0 => Ok(RustType::Any),
        1 => Ok(unique_types.into_iter().next().expect("len == 1")),
        _ => {
            let name = synthetic.register_union(&unique_types);
            Ok(RustType::Named {
                name,
                type_args: vec![],
            })
        }
    }
}

/// フィールド名による型参照解決。
///
/// TypeRegistry → SyntheticTypeRegistry の順に検索する。
fn lookup_field_type(
    type_name: &str,
    field_name: &str,
    reg: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) -> Option<RustType> {
    // TypeRegistry から検索
    if let Some(typedef) = reg.get(type_name) {
        match typedef {
            TypeDef::Struct { fields, .. } => {
                if let Some(f) = fields.iter().find(|f| f.name == field_name) {
                    return Some(f.ty.clone());
                }
            }
            TypeDef::ConstValue { fields, .. } => {
                if let Some(f) = fields.iter().find(|f| f.name == field_name) {
                    return Some(f.ty.clone());
                }
            }
            _ => {}
        }
    }

    // SyntheticTypeRegistry から検索
    if let Some(syn_def) = synthetic.get(type_name) {
        if let crate::ir::Item::Struct { fields, .. } = &syn_def.item {
            if let Some(f) = fields.iter().find(|f| f.name == field_name) {
                return Some(f.ty.clone());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ConstElement;

    #[test]
    fn string_literal_index() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "Foo".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![crate::registry::FieldDef {
                    name: "bar".to_string(),
                    ty: RustType::String,
                    optional: false,
                }],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );

        let result = resolve_indexed_access(
            &TsTypeInfo::TypeRef {
                name: "Foo".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::Literal(TsLiteralKind::String("bar".to_string())),
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::String);
    }

    #[test]
    fn number_index_const_array() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "Arr".to_string(),
            TypeDef::ConstValue {
                fields: vec![],
                elements: vec![
                    ConstElement {
                        ty: RustType::String,
                        string_literal_value: Some("a".to_string()),
                    },
                    ConstElement {
                        ty: RustType::String,
                        string_literal_value: Some("b".to_string()),
                    },
                ],
                type_ref_name: None,
            },
        );

        let result = resolve_indexed_access(
            &TsTypeInfo::TypeRef {
                name: "Arr".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::Number,
            &reg,
            &mut syn,
        )
        .unwrap();

        // string literal → string enum（name_hint = "Arr" → PascalCase "Arr"）
        match result {
            RustType::Named { name, .. } => assert!(name.contains("Arr")),
            _ => panic!("expected Named type for string literal enum"),
        }
    }

    #[test]
    fn unknown_object_fallback() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_indexed_access(
            &TsTypeInfo::TypeRef {
                name: "Unknown".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::Literal(TsLiteralKind::String("x".to_string())),
            &reg,
            &mut syn,
        )
        .unwrap();
        // Unknown type → associated type format fallback
        assert_eq!(
            result,
            RustType::Named {
                name: "Unknown::x".to_string(),
                type_args: vec![]
            }
        );
    }

    // ---- resolve_indexed_access with TypeLiteral ----

    #[test]
    fn type_literal_string_index_direct_field_lookup() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let lit = crate::ts_type_info::TsTypeLiteralInfo {
            fields: vec![
                crate::ts_type_info::TsFieldInfo {
                    name: "x".to_string(),
                    ty: TsTypeInfo::Number,
                    optional: false,
                },
                crate::ts_type_info::TsFieldInfo {
                    name: "y".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                },
            ],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        };
        // { x: number; y: string }["x"] → f64
        let result = resolve_indexed_access(
            &TsTypeInfo::TypeLiteral(lit),
            &TsTypeInfo::Literal(TsLiteralKind::String("x".to_string())),
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::F64);
    }

    #[test]
    fn type_literal_nonexistent_field_falls_back_to_synthetic() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let lit = crate::ts_type_info::TsTypeLiteralInfo {
            fields: vec![crate::ts_type_info::TsFieldInfo {
                name: "x".to_string(),
                ty: TsTypeInfo::Number,
                optional: false,
            }],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        };
        // { x: number }["z"] → synthetic struct name + "::z"
        let result = resolve_indexed_access(
            &TsTypeInfo::TypeLiteral(lit),
            &TsTypeInfo::Literal(TsLiteralKind::String("z".to_string())),
            &reg,
            &mut syn,
        )
        .unwrap();
        match result {
            RustType::Named { name, .. } => assert!(name.ends_with("::z")),
            _ => panic!("expected Named type with ::z suffix"),
        }
    }

    // ---- resolve_number_index ----

    #[test]
    fn number_index_registry_miss_returns_any() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_number_index("NoSuchType", &mut syn, &reg).unwrap();
        assert_eq!(result, RustType::Any);
    }

    #[test]
    fn number_index_mixed_element_types_returns_union() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "MixedArr".to_string(),
            TypeDef::ConstValue {
                fields: vec![],
                elements: vec![
                    ConstElement {
                        ty: RustType::String,
                        string_literal_value: None,
                    },
                    ConstElement {
                        ty: RustType::F64,
                        string_literal_value: None,
                    },
                ],
                type_ref_name: None,
            },
        );
        let result = resolve_number_index("MixedArr", &mut syn, &reg).unwrap();
        // Multiple unique types → synthetic union
        match result {
            RustType::Named { .. } => {} // union was registered
            _ => panic!("expected Named (synthetic union) for mixed element types"),
        }
    }

    #[test]
    fn number_index_empty_elements_returns_any() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "EmptyArr".to_string(),
            TypeDef::ConstValue {
                fields: vec![],
                elements: vec![],
                type_ref_name: None,
            },
        );
        let result = resolve_number_index("EmptyArr", &mut syn, &reg).unwrap();
        assert_eq!(result, RustType::Any);
    }

    #[test]
    fn number_index_single_non_string_type() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "NumArr".to_string(),
            TypeDef::ConstValue {
                fields: vec![],
                elements: vec![
                    ConstElement {
                        ty: RustType::F64,
                        string_literal_value: None,
                    },
                    ConstElement {
                        ty: RustType::F64,
                        string_literal_value: None,
                    },
                ],
                type_ref_name: None,
            },
        );
        let result = resolve_number_index("NumArr", &mut syn, &reg).unwrap();
        assert_eq!(result, RustType::F64);
    }

    // ---- resolve_keyof_typeof_index ----

    #[test]
    fn keyof_typeof_registry_miss_returns_error() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_keyof_typeof_index("NoSuchObj", &mut syn, &reg);
        assert!(result.is_err());
    }

    // ---- resolve_type_param_indexed_access ----

    #[test]
    fn type_param_indexed_access_registry_miss_returns_any() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_type_param_indexed_access("Missing", &reg, &mut syn).unwrap();
        assert_eq!(result, RustType::Any);
    }

    #[test]
    fn type_param_indexed_access_zero_unique_types_returns_any() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // Struct with no fields → unique_field_types returns None → default empty
        reg.register(
            "Empty".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let result = resolve_type_param_indexed_access("Empty", &reg, &mut syn).unwrap();
        assert_eq!(result, RustType::Any);
    }

    #[test]
    fn type_param_indexed_access_single_unique_type() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "AllString".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![
                    crate::registry::FieldDef {
                        name: "a".to_string(),
                        ty: RustType::String,
                        optional: false,
                    },
                    crate::registry::FieldDef {
                        name: "b".to_string(),
                        ty: RustType::String,
                        optional: false,
                    },
                ],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let result = resolve_type_param_indexed_access("AllString", &reg, &mut syn).unwrap();
        assert_eq!(result, RustType::String);
    }

    #[test]
    fn type_param_indexed_access_multiple_unique_types() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "Mixed".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![
                    crate::registry::FieldDef {
                        name: "a".to_string(),
                        ty: RustType::String,
                        optional: false,
                    },
                    crate::registry::FieldDef {
                        name: "b".to_string(),
                        ty: RustType::F64,
                        optional: false,
                    },
                ],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let result = resolve_type_param_indexed_access("Mixed", &reg, &mut syn).unwrap();
        // Multiple unique types → synthetic union
        match result {
            RustType::Named { .. } => {}
            _ => panic!("expected Named (synthetic union) for multiple unique types"),
        }
    }

    // ---- lookup_field_type ----

    #[test]
    fn lookup_field_type_struct_variant() {
        let mut reg = TypeRegistry::new();
        let syn = SyntheticTypeRegistry::new();
        reg.register(
            "S".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![crate::registry::FieldDef {
                    name: "f".to_string(),
                    ty: RustType::Bool,
                    optional: false,
                }],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let result = lookup_field_type("S", "f", &reg, &syn);
        assert_eq!(result, Some(RustType::Bool));
    }

    #[test]
    fn lookup_field_type_const_value_variant() {
        let mut reg = TypeRegistry::new();
        let syn = SyntheticTypeRegistry::new();
        reg.register(
            "C".to_string(),
            TypeDef::ConstValue {
                fields: vec![crate::registry::ConstField {
                    name: "key".to_string(),
                    ty: RustType::String,
                    string_literal_value: Some("val".to_string()),
                }],
                elements: vec![],
                type_ref_name: None,
            },
        );
        let result = lookup_field_type("C", "key", &reg, &syn);
        assert_eq!(result, Some(RustType::String));
    }

    #[test]
    fn lookup_field_type_missing_field_returns_none() {
        let mut reg = TypeRegistry::new();
        let syn = SyntheticTypeRegistry::new();
        reg.register(
            "S".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![crate::registry::FieldDef {
                    name: "a".to_string(),
                    ty: RustType::Bool,
                    optional: false,
                }],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let result = lookup_field_type("S", "nonexistent", &reg, &syn);
        assert_eq!(result, None);
    }

    #[test]
    fn lookup_field_type_unregistered_type_returns_none() {
        let reg = TypeRegistry::new();
        let syn = SyntheticTypeRegistry::new();
        let result = lookup_field_type("NoSuch", "f", &reg, &syn);
        assert_eq!(result, None);
    }

    // ---- resolve_indexed_access: non-TypeRef object fallback ----

    #[test]
    fn non_extractable_object_type_returns_any() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // TsTypeInfo::Boolean is not TypeRef, not TypeQuery, not IndexedAccess, not TypeLiteral
        let result = resolve_indexed_access(
            &TsTypeInfo::Boolean,
            &TsTypeInfo::Literal(TsLiteralKind::String("x".to_string())),
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::Any);
    }

    #[test]
    fn type_query_unregistered_object_returns_any() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_indexed_access(
            &TsTypeInfo::TypeQuery("unregistered".to_string()),
            &TsTypeInfo::Literal(TsLiteralKind::String("x".to_string())),
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::Any);
    }

    #[test]
    fn number_literal_index_returns_any() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_indexed_access(
            &TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::Literal(TsLiteralKind::Number(0.0)),
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::Any);
    }
}
