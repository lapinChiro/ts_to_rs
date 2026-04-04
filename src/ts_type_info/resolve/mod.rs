//! TsTypeInfo → RustType 変換。
//!
//! TypeDef<TsTypeInfo> → TypeDef<RustType> の変換を担う。
//! registry フェーズで収集された TS レベルの型情報を Rust 型に変換する。
//!
//! ## モジュール構成
//!
//! - `mod.rs`: メインディスパッチャ + TypeDef/FieldDef/ParamDef 変換
//! - `union.rs`: union 型解決（nullable → Option、multi-type → synthetic enum）
//! - `intersection.rs`: intersection 型解決（フィールドマージ → synthetic struct）
//! - `utility.rs`: ユーティリティ型解決（Partial, Required, Pick, Omit, NonNullable）
//! - `indexed_access.rs`: indexed access 型解決（T[K] → フィールド型参照）
//! - `conditional.rs`: 条件型解決（infer パターン、型述語、フォールバック）

mod conditional;
mod indexed_access;
mod intersection;
mod union;
mod utility;

use crate::ir::sanitize_rust_type_name;
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
        TsTypeInfo::Symbol => Ok(RustType::Any), // symbol は Rust に直接対応なし

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

        TsTypeInfo::Union(members) => union::resolve_union(members, reg, synthetic),

        TsTypeInfo::Intersection(members) => {
            intersection::resolve_intersection(members, reg, synthetic)
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
        TsTypeInfo::TypeLiteral(lit) => intersection::resolve_type_literal(lit, reg, synthetic),

        TsTypeInfo::Mapped {
            type_param,
            constraint,
            value,
            has_readonly,
            has_optional,
            name_type,
        } => resolve_mapped(
            type_param,
            constraint,
            value.as_deref(),
            *has_readonly,
            *has_optional,
            name_type.as_deref(),
            reg,
            synthetic,
        ),

        // ── Advanced types ──
        TsTypeInfo::Conditional {
            check,
            extends,
            true_type,
            false_type,
        } => {
            conditional::resolve_conditional(check, extends, true_type, false_type, reg, synthetic)
        }

        TsTypeInfo::IndexedAccess { object, index } => {
            indexed_access::resolve_indexed_access(object, index, reg, synthetic)
        }

        TsTypeInfo::KeyOf(inner) => resolve_keyof(inner, reg, synthetic),

        TsTypeInfo::TypeQuery(name) => resolve_type_query(name, reg, synthetic),

        TsTypeInfo::Readonly(inner) => resolve_ts_type(inner, reg, synthetic),

        TsTypeInfo::Infer(_) => {
            // infer T は conditional type の文脈でのみ有効。
            // 単独では Any にフォールバック。
            Ok(RustType::Any)
        }

        TsTypeInfo::TypePredicate => Ok(RustType::Bool),
    }
}

/// keyof 型を解決する。
///
/// `keyof typeof X` → フィールド名の string enum を生成。
/// `keyof T` → String にフォールバック。
fn resolve_keyof(
    inner: &TsTypeInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // keyof typeof X → string enum of field names
    if let TsTypeInfo::TypeQuery(name) = inner {
        return match reg.get(name) {
            Some(def) => {
                if let Some(field_names) = def.field_names() {
                    let enum_name = synthetic
                        .register_string_literal_enum(&format!("{name}_key"), &field_names);
                    Ok(RustType::Named {
                        name: enum_name,
                        type_args: vec![],
                    })
                } else {
                    Err(anyhow::anyhow!(
                        "unsupported type: keyof typeof {name} (no fields)"
                    ))
                }
            }
            None => Err(anyhow::anyhow!(
                "unsupported type: keyof typeof {name} (not found in registry)"
            )),
        };
    }

    // keyof TypeRef → フィールド名の string enum
    if let TsTypeInfo::TypeRef { name, .. } = inner {
        if let Some(def) = reg.get(name) {
            if let Some(field_names) = def.field_names() {
                let enum_name =
                    synthetic.register_string_literal_enum(&format!("{name}_key"), &field_names);
                return Ok(RustType::Named {
                    name: enum_name,
                    type_args: vec![],
                });
            }
        }
    }

    Ok(RustType::String)
}

/// typeof クエリを解決する。
fn resolve_type_query(
    name: &str,
    reg: &TypeRegistry,
    _synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
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
        Some(TypeDef::Struct {
            constructor: Some(ctors),
            ..
        }) if !ctors.is_empty() => {
            // コンストラクタオーバーロード: パラメータ数最大のものを選択
            let best = ctors
                .iter()
                .max_by_key(|c| c.params.len())
                .expect("non-empty");
            let param_types: Vec<RustType> = best.params.iter().map(|p| p.ty.clone()).collect();
            let ret = best.return_type.clone().unwrap_or_else(|| RustType::Named {
                name: name.to_string(),
                type_args: vec![],
            });
            Ok(RustType::Fn {
                params: param_types,
                return_type: Box::new(ret),
            })
        }
        Some(TypeDef::Struct { .. } | TypeDef::Enum { .. }) => Ok(RustType::Named {
            name: name.to_string(),
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

/// Mapped 型を解決する。
///
/// identity mapped type `{ [K in keyof T]: T[K] }` → `T` に簡約。
/// それ以外は `HashMap<String, V>` にフォールバック。
fn resolve_mapped(
    type_param: &str,
    constraint: &TsTypeInfo,
    value: Option<&TsTypeInfo>,
    has_readonly: bool,
    has_optional: bool,
    name_type: Option<&TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // readonly/optional 修飾子がある場合は identity 簡約を行わない
    // name_type (as clause) は noop symbol filter の場合のみ identity 簡約を許可
    let name_type_is_noop = match name_type {
        None => true,
        Some(nt) => is_symbol_filter_noop(nt, type_param),
    };
    if !has_readonly && !has_optional && name_type_is_noop {
        if let Some(ty) = try_simplify_identity_mapped(type_param, constraint, value) {
            return Ok(ty);
        }
    }

    // HashMap フォールバック
    let value_type = value
        .map(|v| resolve_ts_type(v, reg, synthetic))
        .transpose()?
        .unwrap_or(RustType::Any);
    Ok(RustType::Named {
        name: "HashMap".to_string(),
        type_args: vec![RustType::String, value_type],
    })
}

/// name_type が noop symbol filter `K extends symbol ? never : K` かどうかを判定する。
///
/// このパターンはキーのリマッピングを行わない（symbol キーを除外するだけ）ため、
/// identity mapped type の簡約を妨げない。
fn is_symbol_filter_noop(name_type: &TsTypeInfo, param_name: &str) -> bool {
    match name_type {
        TsTypeInfo::Conditional {
            check,
            extends,
            true_type,
            false_type,
        } => {
            // check == param_name (K)
            let check_ok =
                matches!(check.as_ref(), TsTypeInfo::TypeRef { name, .. } if name == param_name);
            // extends == symbol keyword
            let extends_ok = matches!(extends.as_ref(), TsTypeInfo::Symbol);
            // true_type == never
            let true_ok = matches!(true_type.as_ref(), TsTypeInfo::Never);
            // false_type == param_name (K)
            let false_ok = matches!(false_type.as_ref(), TsTypeInfo::TypeRef { name, .. } if name == param_name);
            check_ok && extends_ok && true_ok && false_ok
        }
        _ => false,
    }
}

/// identity mapped type `{ [K in keyof T]: T[K] }` → `T` の簡約を試みる。
pub(super) fn try_simplify_identity_mapped(
    _type_param: &str,
    constraint: &TsTypeInfo,
    value: Option<&TsTypeInfo>,
) -> Option<RustType> {
    let base_name = match constraint {
        TsTypeInfo::KeyOf(inner) => match inner.as_ref() {
            TsTypeInfo::TypeRef { name, .. } => name.clone(),
            _ => return None,
        },
        _ => return None,
    };

    let value = value?;
    match value {
        TsTypeInfo::IndexedAccess { object, index } => match (object.as_ref(), index.as_ref()) {
            (TsTypeInfo::TypeRef { name: obj_name, .. }, TsTypeInfo::TypeRef { .. }) => {
                if obj_name == &base_name {
                    Some(RustType::Named {
                        name: base_name,
                        type_args: vec![],
                    })
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    }
}

/// 型参照を解決する。
///
/// 組み込みジェネリック型（Array, Promise, Record 等）およびユーティリティ型
/// （Partial, Required, Pick, Omit, NonNullable）を特殊処理し、
/// ユーザー定義型はそのまま RustType::Named に変換する。
fn resolve_type_ref(
    name: &str,
    type_args: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // ユーティリティ型: 型引数を事前解決せず TsTypeInfo のまま渡す
    match name {
        "Partial" => return utility::resolve_partial(type_args, reg, synthetic),
        "Required" => return utility::resolve_required(type_args, reg, synthetic),
        "Pick" => return utility::resolve_pick(type_args, reg, synthetic),
        "Omit" => return utility::resolve_omit(type_args, reg, synthetic),
        "NonNullable" => return utility::resolve_non_nullable(type_args, reg, synthetic),
        "Readonly" => {
            // Readonly<T> → T（Rust では immutability は変数バインディングで制御）
            if let Some(arg) = type_args.first() {
                return resolve_ts_type(arg, reg, synthetic);
            }
            return Ok(RustType::Any);
        }
        _ => {}
    }

    // 組み込みジェネリック型: 型引数を事前解決
    let resolved_args = type_args
        .iter()
        .map(|a| resolve_ts_type(a, reg, synthetic))
        .collect::<anyhow::Result<Vec<_>>>()?;

    match name {
        "Array" | "ReadonlyArray" => {
            let inner = resolved_args.into_iter().next().unwrap_or(RustType::Any);
            Ok(RustType::Vec(Box::new(inner)))
        }
        // Promise<T> は Named("Promise", [T]) のまま返す。
        // async 関数の戻り値型 unwrap は transformer 側の責務。
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
            name: sanitize_rust_type_name(name),
            type_args: resolved_args,
        }),
    }
}

/// Vec<TypeParam<TsTypeInfo>> → Vec<TypeParam<RustType>> 変換。
pub(crate) fn resolve_type_params(
    params: Vec<crate::ir::TypeParam<TsTypeInfo>>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Vec<crate::ir::TypeParam<RustType>> {
    params
        .into_iter()
        .map(|tp| crate::ir::TypeParam {
            name: tp.name,
            constraint: tp
                .constraint
                .and_then(|c| resolve_ts_type(&c, reg, synthetic).ok()),
        })
        .collect()
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
                type_params: resolve_type_params(type_params, reg, synthetic),
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
            // string literal union / DU: raw 文字列を PascalCase に変換
            let (pascal_variants, pascal_string_values, pascal_variant_fields) =
                if !string_values.is_empty() {
                    use crate::ir::string_to_pascal_case;
                    let pv: Vec<String> = variants
                        .iter()
                        .map(|v| {
                            if string_values.contains_key(v) {
                                string_to_pascal_case(v)
                            } else {
                                v.clone()
                            }
                        })
                        .collect();
                    let psv: std::collections::HashMap<String, String> = string_values
                        .into_keys()
                        .map(|raw| {
                            let pascal = string_to_pascal_case(&raw);
                            (raw, pascal)
                        })
                        .collect();
                    let pvf: std::collections::HashMap<String, Vec<FieldDef<TsTypeInfo>>> =
                        variant_fields
                            .into_iter()
                            .map(|(raw_key, fields)| {
                                let pascal_key = string_to_pascal_case(&raw_key);
                                (pascal_key, fields)
                            })
                            .collect();
                    (pv, psv, pvf)
                } else {
                    (variants, string_values, variant_fields)
                };

            let resolved_variant_fields = pascal_variant_fields
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
                type_params: resolve_type_params(type_params, reg, synthetic),
                variants: pascal_variants,
                string_values: pascal_string_values,
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
                type_params: resolve_type_params(type_params, reg, synthetic),
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
pub(crate) fn resolve_field_def(
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
pub(crate) fn resolve_param_def(
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
pub(crate) fn resolve_method_sig(
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
        // Promise<T> は Named("Promise", [T]) のまま返る（unwrap は transformer の責務）
        match result {
            RustType::Named { name, type_args } => {
                assert_eq!(name, "Promise");
                assert_eq!(type_args, vec![RustType::String]);
            }
            _ => panic!("expected Named(Promise)"),
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
            has_readonly: false,
            has_optional: false,
            name_type: None,
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

    // ── resolve_type_params ──

    #[test]
    fn resolve_type_params_empty() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_type_params(vec![], &reg, &mut syn);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_type_params_with_constraint() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let params = vec![crate::ir::TypeParam {
            name: "T".to_string(),
            constraint: Some(TsTypeInfo::String),
        }];
        let resolved = resolve_type_params(params, &reg, &mut syn);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "T");
        assert_eq!(resolved[0].constraint, Some(RustType::String));
    }

    #[test]
    fn resolve_type_params_without_constraint() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let params = vec![crate::ir::TypeParam {
            name: "T".to_string(),
            constraint: None,
        }];
        let resolved = resolve_type_params(params, &reg, &mut syn);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "T");
        assert_eq!(resolved[0].constraint, None);
    }

    // ── resolve_typedef PascalCase ──

    #[test]
    fn resolve_typedef_string_literal_union_applies_pascal_case() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let def: TypeDef<TsTypeInfo> = TypeDef::Enum {
            type_params: vec![],
            variants: vec!["up".to_string(), "down".to_string()],
            string_values: [
                ("up".to_string(), "up".to_string()),
                ("down".to_string(), "down".to_string()),
            ]
            .into_iter()
            .collect(),
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        };
        let resolved = resolve_typedef(def, &reg, &mut syn).unwrap();
        if let TypeDef::Enum {
            variants,
            string_values,
            ..
        } = resolved
        {
            assert_eq!(variants, vec!["Up".to_string(), "Down".to_string()]);
            assert_eq!(string_values.get("up"), Some(&"Up".to_string()));
            assert_eq!(string_values.get("down"), Some(&"Down".to_string()));
        } else {
            panic!("expected Enum");
        }
    }

    #[test]
    fn resolve_typedef_regular_enum_no_pascal_case() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let def: TypeDef<TsTypeInfo> = TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string(), "Green".to_string()],
            string_values: std::collections::HashMap::new(),
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        };
        let resolved = resolve_typedef(def, &reg, &mut syn).unwrap();
        if let TypeDef::Enum { variants, .. } = resolved {
            assert_eq!(variants, vec!["Red".to_string(), "Green".to_string()]);
        } else {
            panic!("expected Enum");
        }
    }

    // ── resolve_keyof ──

    #[test]
    fn resolve_keyof_type_ref_not_in_registry_falls_back_to_string() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // keyof UnknownType → String フォールバック
        let info = TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
            name: "UnknownType".to_string(),
            type_args: vec![],
        }));
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::String
        );
    }

    // ── resolve_type_query ──

    #[test]
    fn resolve_type_query_function_variant() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "myFunc".to_string(),
            TypeDef::Function {
                type_params: vec![],
                params: vec![ParamDef {
                    name: "x".to_string(),
                    ty: RustType::String,
                    optional: false,
                    has_default: false,
                }],
                return_type: Some(RustType::F64),
                has_rest: false,
            },
        );
        let info = TsTypeInfo::TypeQuery("myFunc".to_string());
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Fn {
                params: vec![RustType::String],
                return_type: Box::new(RustType::F64),
            }
        );
    }

    #[test]
    fn resolve_type_query_enum_variant() {
        let mut reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        reg.register(
            "Color".to_string(),
            TypeDef::Enum {
                type_params: vec![],
                variants: vec!["Red".to_string()],
                string_values: std::collections::HashMap::new(),
                tag_field: None,
                variant_fields: std::collections::HashMap::new(),
            },
        );
        let info = TsTypeInfo::TypeQuery("Color".to_string());
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "Color".to_string(),
                type_args: vec![],
            }
        );
    }

    // ── is_symbol_filter_noop (tested indirectly via resolve_ts_type + Mapped) ──

    #[test]
    fn is_symbol_filter_noop_valid_pattern_allows_identity() {
        // { [K in keyof T as K extends symbol ? never : K]: T[K] } → T
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
                name: "Foo".to_string(),
                type_args: vec![],
            }))),
            value: Some(Box::new(TsTypeInfo::IndexedAccess {
                object: Box::new(TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                }),
                index: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
            has_readonly: false,
            has_optional: false,
            name_type: Some(Box::new(TsTypeInfo::Conditional {
                check: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
                extends: Box::new(TsTypeInfo::Symbol),
                true_type: Box::new(TsTypeInfo::Never),
                false_type: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
        };
        // noop symbol filter → identity 簡約成功 → Named("Foo")
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "Foo".to_string(),
                type_args: vec![],
            }
        );
    }

    #[test]
    fn is_symbol_filter_noop_check_type_mismatch_blocks_identity() {
        // check が K ではなく X → noop ではない → identity 簡約されず HashMap フォールバック
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::String),
            value: Some(Box::new(TsTypeInfo::Number)),
            has_readonly: false,
            has_optional: false,
            name_type: Some(Box::new(TsTypeInfo::Conditional {
                check: Box::new(TsTypeInfo::TypeRef {
                    name: "X".to_string(), // K ではない
                    type_args: vec![],
                }),
                extends: Box::new(TsTypeInfo::Symbol),
                true_type: Box::new(TsTypeInfo::Never),
                false_type: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
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
    fn is_symbol_filter_noop_extends_not_symbol_blocks_identity() {
        // extends が Symbol ではなく String → noop ではない
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::String),
            value: Some(Box::new(TsTypeInfo::Number)),
            has_readonly: false,
            has_optional: false,
            name_type: Some(Box::new(TsTypeInfo::Conditional {
                check: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
                extends: Box::new(TsTypeInfo::String), // Symbol ではない
                true_type: Box::new(TsTypeInfo::Never),
                false_type: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
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
    fn is_symbol_filter_noop_true_type_not_never_blocks_identity() {
        // true_type が Never ではなく String → noop ではない
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::String),
            value: Some(Box::new(TsTypeInfo::Number)),
            has_readonly: false,
            has_optional: false,
            name_type: Some(Box::new(TsTypeInfo::Conditional {
                check: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
                extends: Box::new(TsTypeInfo::Symbol),
                true_type: Box::new(TsTypeInfo::String), // Never ではない
                false_type: Box::new(TsTypeInfo::TypeRef {
                    name: "K".to_string(),
                    type_args: vec![],
                }),
            })),
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, RustType::F64],
            }
        );
    }

    // ── resolve_mapped with value None ──

    #[test]
    fn resolve_mapped_value_none_falls_back_to_hashmap_any() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let info = TsTypeInfo::Mapped {
            type_param: "K".to_string(),
            constraint: Box::new(TsTypeInfo::String),
            value: None,
            has_readonly: false,
            has_optional: false,
            name_type: None,
        };
        assert_eq!(
            resolve_ts_type(&info, &reg, &mut syn).unwrap(),
            RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, RustType::Any],
            }
        );
    }

    #[test]
    fn resolve_typedef_discriminated_union_pascal_case_with_fields() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let mut variant_fields = std::collections::HashMap::new();
        variant_fields.insert(
            "circle".to_string(),
            vec![FieldDef {
                name: "radius".to_string(),
                ty: TsTypeInfo::Number,
                optional: false,
            }],
        );
        variant_fields.insert(
            "square".to_string(),
            vec![FieldDef {
                name: "side".to_string(),
                ty: TsTypeInfo::Number,
                optional: false,
            }],
        );
        let def: TypeDef<TsTypeInfo> = TypeDef::Enum {
            type_params: vec![],
            variants: vec!["circle".to_string(), "square".to_string()],
            string_values: [
                ("circle".to_string(), "circle".to_string()),
                ("square".to_string(), "square".to_string()),
            ]
            .into_iter()
            .collect(),
            tag_field: Some("kind".to_string()),
            variant_fields,
        };
        let resolved = resolve_typedef(def, &reg, &mut syn).unwrap();
        if let TypeDef::Enum {
            variants,
            string_values,
            variant_fields,
            tag_field,
            ..
        } = resolved
        {
            assert_eq!(variants, vec!["Circle".to_string(), "Square".to_string()]);
            assert_eq!(string_values.get("circle"), Some(&"Circle".to_string()));
            assert_eq!(tag_field, Some("kind".to_string()));
            // variant_fields keys should be PascalCase
            assert!(variant_fields.contains_key("Circle"));
            assert!(variant_fields.contains_key("Square"));
            assert!(!variant_fields.contains_key("circle"));
            // Field types should be resolved
            let circle_fields = &variant_fields["Circle"];
            assert_eq!(circle_fields.len(), 1);
            assert_eq!(circle_fields[0].name, "radius");
            assert_eq!(circle_fields[0].ty, RustType::F64);
        } else {
            panic!("expected Enum");
        }
    }
}
