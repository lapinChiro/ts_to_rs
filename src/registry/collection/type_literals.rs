//! Type-literal → `TypeDef<TsTypeInfo>::Struct` construction plus the
//! TsMethodInfo / TsFnSigInfo → `MethodSignature<TsTypeInfo>` sig
//! converters shared with [`super::resolvers`].
//!
//! Used primarily from Pass 2 ([`super::decl::collect_decl`]) when a
//! `type X = { ... }` alias decomposes into a struct-shaped
//! `TypeDef<TsTypeInfo>`.

use std::collections::HashMap;

use crate::ir::TypeParam;
use crate::registry::{FieldDef, MethodSignature, ParamDef, TypeDef};
use crate::ts_type_info::{TsFnSigInfo, TsMethodInfo, TsTypeInfo, TsTypeLiteralInfo};

/// `TsTypeLiteralInfo` から `TypeDef<TsTypeInfo>::Struct` を構築する。
///
/// TsTypeInfo の各メンバーを `FieldDef<TsTypeInfo>` / `MethodSignature<TsTypeInfo>` に変換し、
/// `resolve_typedef` に渡せる形式を返す。index signature は TypeDef では表現できないため、
/// 呼び出し元で別途処理する。
pub(super) fn build_struct_from_type_literal(
    lit: &TsTypeLiteralInfo,
    type_params: Vec<TypeParam<TsTypeInfo>>,
) -> TypeDef<TsTypeInfo> {
    // TsFieldInfo → FieldDef<TsTypeInfo>
    let fields: Vec<FieldDef<TsTypeInfo>> = lit
        .fields
        .iter()
        .map(|f| FieldDef {
            name: f.name.clone(),
            ty: f.ty.clone(),
            optional: f.optional,
        })
        .collect();

    // TsMethodInfo → MethodSignature<TsTypeInfo> (grouped by name)
    let mut methods: HashMap<String, Vec<MethodSignature<TsTypeInfo>>> = HashMap::new();
    for m in &lit.methods {
        let sig = convert_method_info_to_sig(m);
        methods.entry(m.name.clone()).or_default().push(sig);
    }

    // TsFnSigInfo (call) → call_signatures
    let call_signatures: Vec<MethodSignature<TsTypeInfo>> = lit
        .call_signatures
        .iter()
        .map(convert_fn_sig_to_method_sig)
        .collect();

    // TsFnSigInfo (construct) → constructor
    let constructor = if lit.construct_signatures.is_empty() {
        None
    } else {
        Some(
            lit.construct_signatures
                .iter()
                .map(convert_fn_sig_to_method_sig)
                .collect(),
        )
    };

    TypeDef::Struct {
        type_params,
        fields,
        methods,
        constructor,
        call_signatures,
        extends: vec![],
        is_interface: false,
    }
}

/// `TsMethodInfo` → `MethodSignature<TsTypeInfo>` 変換。
pub(super) fn convert_method_info_to_sig(m: &TsMethodInfo) -> MethodSignature<TsTypeInfo> {
    let params = m
        .params
        .iter()
        .map(|p| ParamDef {
            name: p.name.clone(),
            ty: p.ty.clone(),
            optional: p.optional,
            has_default: false,
        })
        .collect();
    let type_params = m
        .type_params
        .iter()
        .map(|name| TypeParam {
            name: name.clone(),
            constraint: None,
            default: None,
        })
        .collect();
    MethodSignature {
        params,
        return_type: m.return_type.clone(),
        has_rest: m.has_rest,
        type_params,
    }
}

/// `TsFnSigInfo` → `MethodSignature<TsTypeInfo>` 変換。
pub(super) fn convert_fn_sig_to_method_sig(sig: &TsFnSigInfo) -> MethodSignature<TsTypeInfo> {
    let params = sig
        .params
        .iter()
        .map(|p| ParamDef {
            name: p.name.clone(),
            ty: p.ty.clone(),
            optional: p.optional,
            has_default: false,
        })
        .collect();
    MethodSignature {
        params,
        return_type: sig.return_type.clone(),
        has_rest: sig.has_rest,
        type_params: vec![],
    }
}

#[cfg(test)]
mod build_struct_from_type_literal_tests {
    use super::*;
    use crate::ts_type_info::*;

    fn empty_lit() -> TsTypeLiteralInfo {
        TsTypeLiteralInfo {
            fields: vec![],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_empty_returns_empty_struct() {
        let lit = empty_lit();
        let result = build_struct_from_type_literal(&lit, vec![]);
        match result {
            TypeDef::Struct {
                fields,
                methods,
                call_signatures,
                constructor,
                ..
            } => {
                assert!(fields.is_empty());
                assert!(methods.is_empty());
                assert!(call_signatures.is_empty());
                assert!(constructor.is_none());
            }
            other => panic!("expected Struct, got {other:?}"),
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_property_only() {
        let lit = TsTypeLiteralInfo {
            fields: vec![
                TsFieldInfo {
                    name: "x".to_string(),
                    ty: TsTypeInfo::Number,
                    optional: false,
                },
                TsFieldInfo {
                    name: "name".to_string(),
                    ty: TsTypeInfo::String,
                    optional: true,
                },
            ],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct { fields, .. } = result {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, TsTypeInfo::Number);
            assert!(!fields[0].optional);
            assert_eq!(fields[1].name, "name");
            assert_eq!(fields[1].ty, TsTypeInfo::String);
            assert!(fields[1].optional);
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_method_only() {
        let lit = TsTypeLiteralInfo {
            methods: vec![TsMethodInfo {
                name: "handle".to_string(),
                params: vec![TsParamInfo {
                    name: "x".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                }],
                return_type: Some(TsTypeInfo::Void),
                type_params: vec![],
                optional: false,
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct { methods, .. } = result {
            let sigs = methods.get("handle").expect("handle method");
            assert_eq!(sigs.len(), 1);
            assert_eq!(sigs[0].params.len(), 1);
            assert_eq!(sigs[0].params[0].name, "x");
            assert_eq!(sigs[0].params[0].ty, TsTypeInfo::String);
            assert_eq!(sigs[0].return_type, Some(TsTypeInfo::Void));
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_call_signature() {
        let lit = TsTypeLiteralInfo {
            call_signatures: vec![TsFnSigInfo {
                params: vec![TsParamInfo {
                    name: "input".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                }],
                return_type: Some(TsTypeInfo::Number),
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct {
            call_signatures, ..
        } = result
        {
            assert_eq!(call_signatures.len(), 1);
            assert_eq!(call_signatures[0].params.len(), 1);
            assert_eq!(call_signatures[0].params[0].name, "input");
            assert_eq!(call_signatures[0].return_type, Some(TsTypeInfo::Number));
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_construct_signature() {
        let lit = TsTypeLiteralInfo {
            construct_signatures: vec![TsFnSigInfo {
                params: vec![TsParamInfo {
                    name: "config".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                }],
                return_type: Some(TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                }),
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct { constructor, .. } = result {
            let ctors = constructor.expect("constructor should be Some");
            assert_eq!(ctors.len(), 1);
            assert_eq!(ctors[0].params[0].name, "config");
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_mixed_fields_and_methods() {
        let lit = TsTypeLiteralInfo {
            fields: vec![TsFieldInfo {
                name: "count".to_string(),
                ty: TsTypeInfo::Number,
                optional: false,
            }],
            methods: vec![TsMethodInfo {
                name: "increment".to_string(),
                params: vec![],
                return_type: Some(TsTypeInfo::Void),
                type_params: vec![],
                optional: false,
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct {
            fields, methods, ..
        } = result
        {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "count");
            assert!(methods.contains_key("increment"));
        } else {
            panic!("expected Struct");
        }
    }
}
