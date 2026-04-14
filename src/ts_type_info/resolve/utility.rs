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
        .map(|f| StructField {
            name: f.name,
            ty: f.ty.wrap_optional(),
            vis: Some(Visibility::Public),
        })
        .collect();

    let name = format!("Partial{inner_name}");
    if synthetic.get(&name).is_none() {
        synthetic.push_item(
            name.clone(),
            SyntheticTypeKind::InlineStruct,
            Item::Struct {
                vis: Visibility::Public,
                name: name.clone(),
                fields: partial_fields,
                type_params: vec![],
                is_unit_struct: false,
            },
        );
    }

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
    if synthetic.get(&name).is_none() {
        synthetic.push_item(
            name.clone(),
            SyntheticTypeKind::InlineStruct,
            Item::Struct {
                vis: Visibility::Public,
                name: name.clone(),
                fields: required_fields,
                type_params: vec![],
                is_unit_struct: false,
            },
        );
    }

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

    let mut sorted_keys: Vec<String> = keys.iter().map(|k| capitalize_first(k)).collect();
    sorted_keys.sort();
    let keys_suffix: String = sorted_keys.join("");
    let name = format!("Pick{inner_name}{keys_suffix}");
    if synthetic.get(&name).is_none() {
        synthetic.push_item(
            name.clone(),
            SyntheticTypeKind::InlineStruct,
            Item::Struct {
                vis: Visibility::Public,
                name: name.clone(),
                fields: picked_fields,
                type_params: vec![],
                is_unit_struct: false,
            },
        );
    }

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

    let mut sorted_keys: Vec<String> = keys.iter().map(|k| capitalize_first(k)).collect();
    sorted_keys.sort();
    let keys_suffix: String = sorted_keys.join("");
    let name = format!("Omit{inner_name}{keys_suffix}");
    if synthetic.get(&name).is_none() {
        synthetic.push_item(
            name.clone(),
            SyntheticTypeKind::InlineStruct,
            Item::Struct {
                vis: Visibility::Public,
                name: name.clone(),
                fields: omitted_fields,
                type_params: vec![],
                is_unit_struct: false,
            },
        );
    }

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

    // --- resolve_partial: Option double-wrap avoidance ---

    #[test]
    fn test_resolve_partial_already_option_field_no_double_wrap() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Bar".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![
                    FieldDef {
                        name: "required_field".to_string(),
                        ty: RustType::String,
                        optional: false,
                    },
                    FieldDef {
                        name: "optional_field".to_string(),
                        ty: RustType::Option(Box::new(RustType::F64)),
                        optional: true,
                    },
                ],
                methods: HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_partial(
            &[TsTypeInfo::TypeRef {
                name: "Bar".to_string(),
                type_args: vec![],
            }],
            &reg,
            &mut syn,
        )
        .unwrap();

        assert_eq!(
            result,
            RustType::Named {
                name: "PartialBar".to_string(),
                type_args: vec![],
            }
        );

        let syn_def = syn
            .get("PartialBar")
            .expect("PartialBar should be registered");
        if let Item::Struct { fields, .. } = &syn_def.item {
            // required_field → Option<String>
            assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
            // optional_field → Option<f64> (NOT Option<Option<f64>>)
            assert_eq!(fields[1].ty, RustType::Option(Box::new(RustType::F64)));
        } else {
            panic!("expected Struct item");
        }
    }

    #[test]
    fn test_resolve_partial_nested_utility_partial_pick() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        // Partial<Pick<Foo, "name">>
        let result = resolve_partial(
            &[TsTypeInfo::TypeRef {
                name: "Pick".to_string(),
                type_args: vec![
                    TsTypeInfo::TypeRef {
                        name: "Foo".to_string(),
                        type_args: vec![],
                    },
                    TsTypeInfo::Literal(TsLiteralKind::String("name".to_string())),
                ],
            }],
            &reg,
            &mut syn,
        )
        .unwrap();

        // Pick is resolved first, then Partial wraps its fields
        match &result {
            RustType::Named { name, .. } => {
                assert!(
                    name.contains("Partial"),
                    "expected name containing 'Partial', got: {name}"
                );
            }
            _ => panic!("expected Named, got: {result:?}"),
        }
    }

    // --- resolve_required: Option unwrap ---

    #[test]
    fn test_resolve_required_unwraps_option_fields() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Baz".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![
                    FieldDef {
                        name: "opt".to_string(),
                        ty: RustType::Option(Box::new(RustType::String)),
                        optional: true,
                    },
                    FieldDef {
                        name: "req".to_string(),
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
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_required(
            &[TsTypeInfo::TypeRef {
                name: "Baz".to_string(),
                type_args: vec![],
            }],
            &reg,
            &mut syn,
        )
        .unwrap();

        assert_eq!(
            result,
            RustType::Named {
                name: "RequiredBaz".to_string(),
                type_args: vec![],
            }
        );

        let syn_def = syn
            .get("RequiredBaz")
            .expect("RequiredBaz should be registered");
        if let Item::Struct { fields, .. } = &syn_def.item {
            // opt: Option<String> → String
            assert_eq!(fields[0].ty, RustType::String);
            // req: f64 → f64 (unchanged)
            assert_eq!(fields[1].ty, RustType::F64);
        } else {
            panic!("expected Struct item");
        }
    }

    // --- resolve_pick / resolve_omit: union key, zero-result filter ---

    #[test]
    fn test_resolve_pick_union_key() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        // Pick<Foo, "name" | "age">
        let result = resolve_pick(
            &[
                TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                },
                TsTypeInfo::Union(vec![
                    TsTypeInfo::Literal(TsLiteralKind::String("name".to_string())),
                    TsTypeInfo::Literal(TsLiteralKind::String("age".to_string())),
                ]),
            ],
            &reg,
            &mut syn,
        )
        .unwrap();

        match &result {
            RustType::Named { name, .. } => {
                assert!(name.starts_with("PickFoo"), "got: {name}");
            }
            _ => panic!("expected Named"),
        }
        // Both fields should be present
        let syn_def = syn
            .get(&match &result {
                RustType::Named { name, .. } => name.clone(),
                _ => unreachable!(),
            })
            .unwrap();
        if let Item::Struct { fields, .. } = &syn_def.item {
            assert_eq!(fields.len(), 2);
            let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert!(names.contains(&"name"));
            assert!(names.contains(&"age"));
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_resolve_pick_nonexistent_key_yields_empty_struct() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        // Pick<Foo, "nonexistent">
        let result = resolve_pick(
            &[
                TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                },
                TsTypeInfo::Literal(TsLiteralKind::String("nonexistent".to_string())),
            ],
            &reg,
            &mut syn,
        )
        .unwrap();

        match &result {
            RustType::Named { name, .. } => {
                let syn_def = syn.get(name).unwrap();
                if let Item::Struct { fields, .. } = &syn_def.item {
                    assert_eq!(fields.len(), 0, "no fields should match");
                } else {
                    panic!("expected Struct");
                }
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_resolve_omit_nonexistent_key_keeps_all_fields() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        // Omit<Foo, "nonexistent"> — nothing to omit, all fields remain
        let result = resolve_omit(
            &[
                TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                },
                TsTypeInfo::Literal(TsLiteralKind::String("nonexistent".to_string())),
            ],
            &reg,
            &mut syn,
        )
        .unwrap();

        match &result {
            RustType::Named { name, .. } => {
                let syn_def = syn.get(name).unwrap();
                if let Item::Struct { fields, .. } = &syn_def.item {
                    assert_eq!(fields.len(), 2, "all fields should remain");
                } else {
                    panic!("expected Struct");
                }
            }
            _ => panic!("expected Named"),
        }
    }

    // --- resolve_inner_fields_with_conversion: direct search paths ---

    #[test]
    fn test_resolve_inner_fields_with_conversion_direct_success() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_inner_fields_with_conversion(
            &[TsTypeInfo::TypeRef {
                name: "Foo".to_string(),
                type_args: vec![],
            }],
            &reg,
            &mut syn,
        )
        .unwrap();

        let (name, fields) = result.expect("should find Foo directly");
        assert_eq!(name, "Foo");
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn test_resolve_inner_fields_with_conversion_not_found() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // Unknown type not in any registry
        let result = resolve_inner_fields_with_conversion(
            &[TsTypeInfo::TypeRef {
                name: "Unknown".to_string(),
                type_args: vec![],
            }],
            &reg,
            &mut syn,
        )
        .unwrap();

        assert!(result.is_none(), "unknown type should yield None");
    }

    #[test]
    fn test_resolve_inner_fields_with_conversion_empty_args() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_inner_fields_with_conversion(&[], &reg, &mut syn).unwrap();
        assert!(result.is_none(), "empty type_args should yield None");
    }

    // --- extract_string_keys ---

    #[test]
    fn test_extract_string_keys_single_literal() {
        let keys = extract_string_keys(&TsTypeInfo::Literal(TsLiteralKind::String(
            "hello".to_string(),
        )));
        assert_eq!(keys, vec!["hello".to_string()]);
    }

    #[test]
    fn test_extract_string_keys_union_recursion() {
        let keys = extract_string_keys(&TsTypeInfo::Union(vec![
            TsTypeInfo::Literal(TsLiteralKind::String("a".to_string())),
            TsTypeInfo::Literal(TsLiteralKind::String("b".to_string())),
            TsTypeInfo::Literal(TsLiteralKind::String("c".to_string())),
        ]));
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_extract_string_keys_non_string_returns_empty() {
        let keys = extract_string_keys(&TsTypeInfo::Number);
        assert!(keys.is_empty());
    }

    // --- resolve_non_nullable ---

    #[test]
    fn test_resolve_non_nullable_non_option_passthrough() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // NonNullable<string> → String (no Option to strip)
        let result = resolve_non_nullable(&[TsTypeInfo::String], &reg, &mut syn).unwrap();
        assert_eq!(result, RustType::String);
    }

    #[test]
    fn test_resolve_non_nullable_error_on_empty_args() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_non_nullable(&[], &reg, &mut syn);
        assert!(result.is_err(), "empty args should error");
    }

    #[test]
    fn pick_keys_suffix_is_sorted() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        // Pick<Foo, "name" | "age"> — keys in source order: name, age
        let result = resolve_pick(
            &[
                TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                },
                TsTypeInfo::Union(vec![
                    TsTypeInfo::Literal(TsLiteralKind::String("name".to_string())),
                    TsTypeInfo::Literal(TsLiteralKind::String("age".to_string())),
                ]),
            ],
            &reg,
            &mut syn,
        )
        .unwrap();
        // Keys should be sorted: Age before Name
        match result {
            RustType::Named { name, .. } => {
                assert_eq!(
                    name, "PickFooAgeName",
                    "keys_suffix should be sorted alphabetically"
                );
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn omit_keys_suffix_is_sorted() {
        let reg = make_registry_with_foo();
        let mut syn = SyntheticTypeRegistry::new();
        // Omit<Foo, "name" | "age"> — keys in source order: name, age
        let result = resolve_omit(
            &[
                TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                },
                TsTypeInfo::Union(vec![
                    TsTypeInfo::Literal(TsLiteralKind::String("name".to_string())),
                    TsTypeInfo::Literal(TsLiteralKind::String("age".to_string())),
                ]),
            ],
            &reg,
            &mut syn,
        )
        .unwrap();
        // Keys should be sorted: Age before Name
        match result {
            RustType::Named { name, .. } => {
                assert_eq!(
                    name, "OmitFooAgeName",
                    "keys_suffix should be sorted alphabetically"
                );
            }
            _ => panic!("expected Named"),
        }
    }
}
