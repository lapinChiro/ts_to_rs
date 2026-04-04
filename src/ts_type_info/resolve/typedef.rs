//! TypeDef<TsTypeInfo> → TypeDef<RustType> 変換。
//!
//! registry フェーズで構築された TS 型ベースの TypeDef を、
//! Rust 型ベースに変換する。Optional ラップ、PascalCase 命名もここで行う。

use crate::ir::RustType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{
    ConstElement, ConstField, FieldDef, MethodSignature, ParamDef, TypeDef, TypeRegistry,
};
use crate::ts_type_info::TsTypeInfo;

use super::resolve_ts_type;

/// Vec<TypeParam<TsTypeInfo>> → Vec<TypeParam<RustType>> 変換。
///
/// 制約の型解決に失敗した場合はエラーを伝播する。
pub(crate) fn resolve_type_params(
    params: Vec<crate::ir::TypeParam<TsTypeInfo>>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<Vec<crate::ir::TypeParam<RustType>>> {
    params
        .into_iter()
        .map(|tp| {
            let constraint = tp
                .constraint
                .map(|c| resolve_ts_type(&c, reg, synthetic))
                .transpose()?;
            Ok(crate::ir::TypeParam {
                name: tp.name,
                constraint,
            })
        })
        .collect()
}

/// TypeDef<TsTypeInfo> → TypeDef<RustType> 変換。
///
/// registry フェーズで構築された TS 型ベースの TypeDef を、
/// Rust 型ベースに変換する。Optional ラップ、PascalCase 命名もここで行う。
///
/// # エラーハンドリング
///
/// 各要素（フィールド、パラメータ、メソッドシグネチャ等）の型解決失敗は
/// TypeDef 全体のエラーとして伝播する。部分的に成功した TypeDef は生成しない。
/// 呼び出し元（`collection.rs`）は `if let Ok(resolved)` でハンドリングし、
/// TypeDef 全体の失敗は「未解決の型」として安全に処理される。
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
                .map(|f| resolve_field_def(f, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let resolved_methods = methods
                .into_iter()
                .map(|(name, sigs)| {
                    let resolved_sigs = sigs
                        .into_iter()
                        .map(|sig| resolve_method_sig(sig, reg, synthetic))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    Ok((name, resolved_sigs))
                })
                .collect::<anyhow::Result<Vec<_>>>()?
                .into_iter()
                .collect();
            let resolved_ctor = constructor
                .map(|ctors| {
                    ctors
                        .into_iter()
                        .map(|sig| resolve_method_sig(sig, reg, synthetic))
                        .collect::<anyhow::Result<Vec<_>>>()
                })
                .transpose()?;
            let resolved_call_sigs = call_signatures
                .into_iter()
                .map(|sig| resolve_method_sig(sig, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()?;

            Ok(TypeDef::Struct {
                type_params: resolve_type_params(type_params, reg, synthetic)?,
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
                        .map(|f| resolve_field_def(f, reg, synthetic))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    Ok((variant, resolved))
                })
                .collect::<anyhow::Result<Vec<_>>>()?
                .into_iter()
                .collect();

            Ok(TypeDef::Enum {
                type_params: resolve_type_params(type_params, reg, synthetic)?,
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
                .map(|p| resolve_param_def(p, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let resolved_return = return_type
                .map(|rt| resolve_ts_type(&rt, reg, synthetic))
                .transpose()?;

            Ok(TypeDef::Function {
                type_params: resolve_type_params(type_params, reg, synthetic)?,
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
                .map(|f| {
                    let ty = resolve_ts_type(&f.ty, reg, synthetic)?;
                    Ok(ConstField {
                        name: f.name,
                        ty,
                        string_literal_value: f.string_literal_value,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let resolved_elements = elements
                .into_iter()
                .map(|e| {
                    let ty = resolve_ts_type(&e.ty, reg, synthetic)?;
                    Ok(ConstElement {
                        ty,
                        string_literal_value: e.string_literal_value,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

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
        .map(|p| resolve_param_def(p, reg, synthetic))
        .collect::<anyhow::Result<Vec<_>>>()?;
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
