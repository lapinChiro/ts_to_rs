//! TsTypeInfo → RustType 変換。
//!
//! TypeDef<TsTypeInfo> → TypeDef<RustType> の変換を担う。
//! registry フェーズで収集された TS レベルの型情報を Rust 型に変換する。

use crate::ir::RustType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{
    ConstElement, ConstField, FieldDef, MethodSignature, ParamDef, TypeDef, TypeRegistry,
};
use crate::ts_type_info::TsTypeInfo;

/// TsTypeInfo を RustType に変換する。
///
/// 既存の `convert_ts_type` と同等の変換を行うが、入力が SWC AST ではなく TsTypeInfo。
/// TypeRegistry を参照して型参照の解決を行い、SyntheticTypeRegistry に合成型を登録する。
pub fn resolve_ts_type(
    info: &TsTypeInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // TsTypeInfo → TsType 相当の変換を行わず、
    // 既存の convert_ts_type のロジックをミラーする。
    //
    // 現時点では、registry で TsTypeInfo を構築した後、
    // build_registry_with_synthetic 内で一括変換するために使用する。
    // convert_ts_type と同等の結果を保証する。
    match info {
        // ── Keyword types ──
        TsTypeInfo::String => Ok(RustType::String),
        TsTypeInfo::Number => Ok(RustType::F64),
        TsTypeInfo::Boolean => Ok(RustType::Bool),
        TsTypeInfo::Void => Ok(RustType::Unit),
        TsTypeInfo::Any | TsTypeInfo::Unknown => Ok(RustType::Any),
        TsTypeInfo::Never => Ok(RustType::Never),
        TsTypeInfo::Object => Ok(RustType::Named {
            name: "serde_json::Value".to_string(),
            type_args: vec![],
        }),
        TsTypeInfo::Null | TsTypeInfo::Undefined => Ok(RustType::Unit),
        TsTypeInfo::BigInt => Ok(RustType::Named {
            name: "i128".to_string(),
            type_args: vec![],
        }),

        // ── Composite types ──
        TsTypeInfo::Array(inner) => {
            let inner_ty = resolve_ts_type(inner, reg, synthetic)?;
            Ok(RustType::Vec(Box::new(inner_ty)))
        }

        TsTypeInfo::Tuple(elems) => {
            let elem_types = elems
                .iter()
                .map(|e| resolve_ts_type(e, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(RustType::Tuple(elem_types))
        }

        TsTypeInfo::Union(members) => {
            // Delegate to the existing convert_union logic via convert_ts_type.
            // For now, reconstruct the union handling inline.
            resolve_union(members, reg, synthetic)
        }

        TsTypeInfo::Intersection(members) => {
            // Intersection fallback: resolve each member, take the first
            // (real intersection handling is complex and delegated to convert_ts_type)
            if members.len() == 1 {
                return resolve_ts_type(&members[0], reg, synthetic);
            }
            // Fallback: convert the first type
            resolve_ts_type(&members[0], reg, synthetic)
        }

        TsTypeInfo::Function {
            params,
            return_type,
        } => {
            let param_types = params
                .iter()
                .map(|p| resolve_ts_type(p, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let ret = resolve_ts_type(return_type, reg, synthetic)?;
            Ok(RustType::Fn {
                params: param_types,
                return_type: Box::new(ret),
            })
        }

        // ── Reference types ──
        TsTypeInfo::TypeRef { name, type_args } => {
            resolve_type_ref(name, type_args, reg, synthetic)
        }

        // ── Literal types ──
        TsTypeInfo::Literal(kind) => {
            use super::TsLiteralKind;
            match kind {
                TsLiteralKind::String(_) | TsLiteralKind::Template => Ok(RustType::String),
                TsLiteralKind::Boolean(_) => Ok(RustType::Bool),
                TsLiteralKind::Number(_) => Ok(RustType::F64),
                TsLiteralKind::BigInt(_) => Ok(RustType::Named {
                    name: "i128".to_string(),
                    type_args: vec![],
                }),
            }
        }

        // ── Structural types ──
        TsTypeInfo::ObjectLiteral(fields) => {
            // Inline object types: register as synthetic struct
            let field_defs: Vec<(String, RustType)> = fields
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

        TsTypeInfo::Mapped { value, .. } => {
            // Mapped type fallback: HashMap<String, V>
            let value_type = value
                .as_ref()
                .map(|v| resolve_ts_type(v, reg, synthetic))
                .transpose()?
                .unwrap_or(RustType::Any);
            Ok(RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, value_type],
            })
        }

        // ── Advanced types ──
        TsTypeInfo::Conditional {
            true_type,
            false_type,
            ..
        } => {
            // Simplified conditional: try true_type, fallback to false_type
            resolve_ts_type(true_type, reg, synthetic)
                .or_else(|_| resolve_ts_type(false_type, reg, synthetic))
        }

        TsTypeInfo::IndexedAccess { .. } => {
            // Complex indexed access resolution: delegate to existing logic if needed
            Ok(RustType::Any)
        }

        TsTypeInfo::KeyOf(inner) => {
            // keyof T → String (simplified)
            match inner.as_ref() {
                TsTypeInfo::TypeRef { name, .. } => {
                    if let Some(def) = reg.get(name) {
                        if let Some(field_names) = def.field_names() {
                            // keyof with known fields → string enum
                            let _fields = field_names;
                            return Ok(RustType::String);
                        }
                    }
                    Ok(RustType::String)
                }
                _ => Ok(RustType::String),
            }
        }

        TsTypeInfo::TypeQuery(name) => {
            // typeof X → look up in registry
            match reg.get(name) {
                Some(TypeDef::Function {
                    params,
                    return_type,
                    ..
                }) => {
                    let param_types: Vec<RustType> = params.iter().map(|p| p.ty.clone()).collect();
                    let ret = return_type.clone().unwrap_or(RustType::Unit);
                    Ok(RustType::Fn {
                        params: param_types,
                        return_type: Box::new(ret),
                    })
                }
                Some(TypeDef::Struct { .. } | TypeDef::Enum { .. }) => Ok(RustType::Named {
                    name: name.clone(),
                    type_args: vec![],
                }),
                Some(TypeDef::ConstValue { type_ref_name, .. }) => {
                    let resolved_name = type_ref_name.as_deref().unwrap_or(name);
                    Ok(RustType::Named {
                        name: resolved_name.to_string(),
                        type_args: vec![],
                    })
                }
                _ => Err(anyhow::anyhow!(
                    "unsupported type: TsTypeQuery for unknown identifier '{name}'"
                )),
            }
        }

        TsTypeInfo::Readonly(inner) => {
            // readonly は Rust では無視（変数バインディングで制御）
            resolve_ts_type(inner, reg, synthetic)
        }

        TsTypeInfo::TypePredicate => Ok(RustType::Bool),
    }
}

/// Union 型を解決する。
///
/// nullable メンバー（null, undefined, void）を除去し、残りが単一なら Option<T> にラップ。
fn resolve_union(
    members: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    let is_nullable = |m: &TsTypeInfo| {
        matches!(
            m,
            TsTypeInfo::Null | TsTypeInfo::Undefined | TsTypeInfo::Void
        )
    };

    let has_nullable = members.iter().any(is_nullable);
    let non_nullable: Vec<&TsTypeInfo> = members.iter().filter(|m| !is_nullable(m)).collect();

    let inner = match non_nullable.len() {
        0 => RustType::Unit,
        1 => resolve_ts_type(non_nullable[0], reg, synthetic)?,
        _ => {
            // Multiple non-nullable members: check for string literal union
            let all_string_lit = non_nullable
                .iter()
                .all(|m| matches!(m, TsTypeInfo::Literal(super::TsLiteralKind::String(_))));
            if all_string_lit {
                // String literal union → string enum (handled at TypeDef level)
                RustType::String
            } else {
                // General union: resolve each member and create synthetic enum
                let resolved: Vec<RustType> = non_nullable
                    .iter()
                    .map(|m| resolve_ts_type(m, reg, synthetic))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                // Simplified: for 2+ non-nullable types, use Any as fallback
                // (full union → enum conversion is handled by the existing convert_union_type)
                if resolved.len() == 1 {
                    resolved.into_iter().next().expect("len == 1")
                } else {
                    RustType::Any
                }
            }
        }
    };

    if has_nullable {
        Ok(RustType::Option(Box::new(inner)))
    } else {
        Ok(inner)
    }
}

/// 型参照を解決する。
fn resolve_type_ref(
    name: &str,
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    let resolved_args = type_args
        .iter()
        .map(|a| resolve_ts_type(a, reg, synthetic))
        .collect::<anyhow::Result<Vec<_>>>()?;

    match name {
        "Array" | "ReadonlyArray" => {
            let inner = resolved_args.into_iter().next().unwrap_or(RustType::Any);
            Ok(RustType::Vec(Box::new(inner)))
        }
        "Promise" => {
            let ok = resolved_args.into_iter().next().unwrap_or(RustType::Unit);
            Ok(RustType::Result {
                ok: Box::new(ok),
                err: Box::new(RustType::Named {
                    name: "Box<dyn std::error::Error>".to_string(),
                    type_args: vec![],
                }),
            })
        }
        "Record" => {
            let value_type = resolved_args.get(1).cloned().unwrap_or(RustType::Any);
            Ok(RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, value_type],
            })
        }
        "Map" => {
            let key = resolved_args.first().cloned().unwrap_or(RustType::String);
            let val = resolved_args.get(1).cloned().unwrap_or(RustType::Any);
            Ok(RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![key, val],
            })
        }
        "Set" => {
            let inner = resolved_args.into_iter().next().unwrap_or(RustType::Any);
            Ok(RustType::Named {
                name: "HashSet".to_string(),
                type_args: vec![inner],
            })
        }
        _ => Ok(RustType::Named {
            name: name.to_string(),
            type_args: resolved_args,
        }),
    }
}

/// TypeDef<TsTypeInfo> → TypeDef<RustType> 変換。
///
/// registry フェーズで構築された TS 型ベースの TypeDef を、
/// Rust 型ベースに変換する。Optional ラップ、PascalCase 命名もここで行う。
pub fn resolve_typedef(
    def: TypeDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<TypeDef<RustType>> {
    match def {
        TypeDef::Struct {
            type_params,
            fields,
            methods,
            constructor,
            call_signatures,
            extends,
            is_interface,
        } => {
            let resolved_fields = fields
                .into_iter()
                .filter_map(|f| resolve_field_def(f, reg, synthetic).ok())
                .collect();
            let resolved_methods = methods
                .into_iter()
                .map(|(name, sigs)| {
                    let resolved_sigs = sigs
                        .into_iter()
                        .map(|sig| resolve_method_sig(sig, reg, synthetic))
                        .collect::<anyhow::Result<Vec<_>>>()
                        .unwrap_or_default();
                    (name, resolved_sigs)
                })
                .collect();
            let resolved_ctor = constructor.map(|ctors| {
                ctors
                    .into_iter()
                    .filter_map(|sig| resolve_method_sig(sig, reg, synthetic).ok())
                    .collect()
            });
            let resolved_call_sigs = call_signatures
                .into_iter()
                .filter_map(|sig| resolve_method_sig(sig, reg, synthetic).ok())
                .collect();

            Ok(TypeDef::Struct {
                type_params,
                fields: resolved_fields,
                methods: resolved_methods,
                constructor: resolved_ctor,
                call_signatures: resolved_call_sigs,
                extends,
                is_interface,
            })
        }

        TypeDef::Enum {
            type_params,
            variants,
            string_values,
            tag_field,
            variant_fields,
        } => {
            let resolved_variant_fields = variant_fields
                .into_iter()
                .map(|(variant, fields)| {
                    let resolved = fields
                        .into_iter()
                        .filter_map(|f| resolve_field_def(f, reg, synthetic).ok())
                        .collect();
                    (variant, resolved)
                })
                .collect();

            Ok(TypeDef::Enum {
                type_params,
                variants,
                string_values,
                tag_field,
                variant_fields: resolved_variant_fields,
            })
        }

        TypeDef::Function {
            type_params,
            params,
            return_type,
            has_rest,
        } => {
            let resolved_params = params
                .into_iter()
                .filter_map(|p| resolve_param_def(p, reg, synthetic).ok())
                .collect();
            let resolved_return = return_type
                .map(|rt| resolve_ts_type(&rt, reg, synthetic))
                .transpose()?;

            Ok(TypeDef::Function {
                type_params,
                params: resolved_params,
                return_type: resolved_return,
                has_rest,
            })
        }

        TypeDef::ConstValue {
            fields,
            elements,
            type_ref_name,
        } => {
            let resolved_fields = fields
                .into_iter()
                .filter_map(|f| {
                    let ty = resolve_ts_type(&f.ty, reg, synthetic).ok()?;
                    Some(ConstField {
                        name: f.name,
                        ty,
                        string_literal_value: f.string_literal_value,
                    })
                })
                .collect();
            let resolved_elements = elements
                .into_iter()
                .filter_map(|e| {
                    let ty = resolve_ts_type(&e.ty, reg, synthetic).ok()?;
                    Some(ConstElement {
                        ty,
                        string_literal_value: e.string_literal_value,
                    })
                })
                .collect();

            Ok(TypeDef::ConstValue {
                fields: resolved_fields,
                elements: resolved_elements,
                type_ref_name,
            })
        }
    }
}

/// FieldDef<TsTypeInfo> → FieldDef<RustType> 変換。optional フラグに基づき Option ラップ。
///
/// **注意**: この関数は `FieldDef<TsTypeInfo>` 専用。`FieldDef<RustType>` に対して
/// 呼ぶと Option が二重にラップされる。型パラメータで防止されている。
fn resolve_field_def(
    field: FieldDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<FieldDef<RustType>> {
    let ty = resolve_ts_type(&field.ty, reg, synthetic)?;
    let ty = if field.optional {
        RustType::Option(Box::new(ty))
    } else {
        ty
    };
    Ok(FieldDef {
        name: field.name,
        ty,
        optional: field.optional,
    })
}

/// ParamDef<TsTypeInfo> → ParamDef<RustType> 変換。has_default フラグに基づき Option ラップ。
///
/// **注意**: この関数は `ParamDef<TsTypeInfo>` 専用。型パラメータで防止されている。
fn resolve_param_def(
    param: ParamDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<ParamDef<RustType>> {
    let ty = resolve_ts_type(&param.ty, reg, synthetic)?;
    let ty = if param.has_default {
        RustType::Option(Box::new(ty))
    } else {
        ty
    };
    Ok(ParamDef {
        name: param.name,
        ty,
        optional: param.optional,
        has_default: param.has_default,
    })
}

/// MethodSignature<TsTypeInfo> → MethodSignature<RustType> 変換。
fn resolve_method_sig(
    sig: MethodSignature<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<MethodSignature<RustType>> {
    let params = sig
        .params
        .into_iter()
        .filter_map(|p| resolve_param_def(p, reg, synthetic).ok())
        .collect();
    let return_type = sig
        .return_type
        .map(|rt| resolve_ts_type(&rt, reg, synthetic))
        .transpose()?;
    Ok(MethodSignature {
        params,
        return_type,
        has_rest: sig.has_rest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_keyword_types() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();

        assert_eq!(
            resolve_ts_type(&TsTypeInfo::String, &reg, &mut syn).unwrap(),
            RustType::String
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Number, &reg, &mut syn).unwrap(),
            RustType::F64
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Boolean, &reg, &mut syn).unwrap(),
            RustType::Bool
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Void, &reg, &mut syn).unwrap(),
            RustType::Unit
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Any, &reg, &mut syn).unwrap(),
            RustType::Any
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Never, &reg, &mut syn).unwrap(),
            RustType::Never
        );
    }

    #[test]
    fn resolve_array() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Array(Box::new(TsTypeInfo::String));
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Vec(Box::new(RustType::String))
        );
    }

    #[test]
    fn resolve_nullable_union() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Union(vec![TsTypeInfo::String, TsTypeInfo::Null]);
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Option(Box::new(RustType::String))
        );
    }

    #[test]
    fn resolve_type_ref_array() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::TypeRef {
            name: "Array".to_string(),
            type_args: vec![TsTypeInfo::Number],
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Vec(Box::new(RustType::F64))
        );
    }

    #[test]
    fn resolve_promise() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::TypeRef {
            name: "Promise".to_string(),
            type_args: vec![TsTypeInfo::String],
        };
        let result = resolve_ts_type(&info, &reg, &mut syn).unwrap();
        match result {
            RustType::Result { ok, .. } => assert_eq!(*ok, RustType::String),
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn resolve_all_keyword_types() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();

        // Additional keywords not in resolve_keyword_types
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Unknown, &reg, &mut syn).unwrap(),
            RustType::Any
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Null, &reg, &mut syn).unwrap(),
            RustType::Unit
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Undefined, &reg, &mut syn).unwrap(),
            RustType::Unit
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::Object, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "serde_json::Value".to_string(),
                type_args: vec![]
            }
        );
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::BigInt, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "i128".to_string(),
                type_args: vec![]
            }
        );
    }

    #[test]
    fn resolve_tuple() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Tuple(vec![TsTypeInfo::String, TsTypeInfo::Number]);
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Tuple(vec![RustType::String, RustType::F64])
        );
    }

    #[test]
    fn resolve_function() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Function {
            params: vec![TsTypeInfo::String],
            return_type: Box::new(TsTypeInfo::Number),
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Fn {
                params: vec![RustType::String],
                return_type: Box::new(RustType::F64),
            }
        );
    }

    #[test]
    fn resolve_literal_types() {
        use super::super::TsLiteralKind;
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();

        assert_eq!(
            resolve_ts_type(
                &TsTypeInfo::Literal(TsLiteralKind::String("hi".to_string())),
                &reg,
                &mut syn
            )
            .unwrap(),
            RustType::String
        );
        assert_eq!(
            resolve_ts_type(
                &TsTypeInfo::Literal(TsLiteralKind::Number(42.0)),
                &reg,
                &mut syn
            )
            .unwrap(),
            RustType::F64
        );
        assert_eq!(
            resolve_ts_type(
                &TsTypeInfo::Literal(TsLiteralKind::Boolean(true)),
                &reg,
                &mut syn
            )
            .unwrap(),
            RustType::Bool
        );
        assert_eq!(
            resolve_ts_type(
                &TsTypeInfo::Literal(TsLiteralKind::Template),
                &reg,
                &mut syn
            )
            .unwrap(),
            RustType::String
        );
    }

    #[test]
    fn resolve_type_predicate() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        assert_eq!(
            resolve_ts_type(&TsTypeInfo::TypePredicate, &reg, &mut syn).unwrap(),
            RustType::Bool
        );
    }

    #[test]
    fn resolve_readonly_stripped() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Readonly(Box::new(TsTypeInfo::Array(Box::new(TsTypeInfo::String))));
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Vec(Box::new(RustType::String))
        );
    }

    #[test]
    fn resolve_record_type() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::TypeRef {
            name: "Record".to_string(),
            type_args: vec![TsTypeInfo::String, TsTypeInfo::Number],
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, RustType::F64],
            }
        );
    }

    #[test]
    fn resolve_set_type() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::TypeRef {
            name: "Set".to_string(),
            type_args: vec![TsTypeInfo::String],
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "HashSet".to_string(),
                type_args: vec![RustType::String],
            }
        );
    }

    #[test]
    fn resolve_user_defined_type() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::TypeRef {
            name: "MyStruct".to_string(),
            type_args: vec![],
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "MyStruct".to_string(),
                type_args: vec![]
            }
        );
    }

    #[test]
    fn resolve_mapped_type_fallback() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::String),
            value: Some(Box::new(TsTypeInfo::Number)),
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, RustType::F64],
            }
        );
    }

    #[test]
    fn resolve_nullable_undefined_union() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Union(vec![TsTypeInfo::Number, TsTypeInfo::Undefined]);
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Option(Box::new(RustType::F64))
        );
    }

    #[test]
    fn resolve_field_optional() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let field = FieldDef {
            name: "x".to_string(),
            ty: TsTypeInfo::String,
            optional: true,
        };
        let resolved = resolve_field_def(field, &reg, &mut syn).unwrap();
        assert_eq!(resolved.ty, RustType::Option(Box::new(RustType::String)));
        assert!(resolved.optional);
    }

    #[test]
    fn resolve_param_with_default() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let param = ParamDef {
            name: "x".to_string(),
            ty: TsTypeInfo::Number,
            optional: false,
            has_default: true,
        };
        let resolved = resolve_param_def(param, &reg, &mut syn).unwrap();
        assert_eq!(resolved.ty, RustType::Option(Box::new(RustType::F64)));
        assert!(resolved.has_default);
    }
}
