use std::collections::HashMap;

use super::*;

/// Extracts type parameters (name + optional constraint) from an optional [`TsTypeParamDecl`],
/// then applies monomorphization to remove non-trait-bound constraints.
///
/// Returns `(type_params, mono_subs)` where `mono_subs` maps monomorphized type param names
/// to their concrete types. Callers must apply `mono_subs` to field/method types via `substitute`.
pub fn extract_type_params(
    type_params: Option<&swc_ecma_ast::TsTypeParamDecl>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> (Vec<TypeParam>, HashMap<String, RustType>) {
    let raw_params: Vec<TypeParam> = match type_params {
        Some(params) => params
            .params
            .iter()
            .map(|p| TypeParam {
                name: p.name.sym.to_string(),
                constraint: p
                    .constraint
                    .as_ref()
                    .and_then(|c| convert_ts_type(c, synthetic, reg).ok()),
            })
            .collect(),
        None => return (vec![], HashMap::new()),
    };
    crate::ts_type_info::resolve::typedef::monomorphize_type_params(raw_params, reg, synthetic)
}

/// Converts a property signature into an IR [`StructField`].
pub(crate) fn convert_property_signature(
    prop: &TsPropertySignature,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<StructField> {
    let field_name = match prop.key.as_ref() {
        Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported property key (only identifiers)")),
    };

    let type_ann = prop
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("property '{}' has no type annotation", field_name))?;

    let mut ty = convert_ts_type(&type_ann.type_ann, synthetic, reg)?;

    // Optional properties (`?`) become Option<T>
    if prop.optional {
        // Avoid double-wrapping if the type is already Option (e.g., `name?: string | null`)
        if !matches!(ty, RustType::Option(_)) {
            ty = RustType::Option(Box::new(ty));
        }
    }

    Ok(StructField {
        vis: None,
        name: sanitize_field_name(&field_name),
        ty,
    })
}

/// Converts an unsupported union member type into a typed enum variant.
///
/// - Function types → `Fn(Box<dyn Fn(T) -> U>)` variant
/// - Tuple types → `Tuple((T1, T2, ...))` variant
/// - Other unsupported types → `Other{N}(serde_json::Value)` variant with index
pub(super) fn convert_unsupported_union_member(
    ty: &TsType,
    variants: &mut Vec<EnumVariant>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) {
    // Unwrap parenthesized types: ((x: number) => string) → (x: number) => string
    let ty = match ty {
        TsType::TsParenthesizedType(paren) => paren.type_ann.as_ref(),
        other => other,
    };
    match ty {
        TsType::TsFnOrConstructorType(swc_ecma_ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            // Function type → Fn(Box<dyn Fn(params) -> ret>)
            if let Ok(rust_type) = convert_fn_type_to_rust(fn_type, synthetic, reg) {
                variants.push(EnumVariant {
                    name: "Fn".to_string(),
                    value: None,
                    data: Some(rust_type),
                    fields: vec![],
                });
                return;
            }
        }
        TsType::TsTupleType(tuple) => {
            // Tuple type → Tuple((T1, T2, ...))
            let elem_types: Vec<RustType> = tuple
                .elem_types
                .iter()
                .filter_map(|elem| convert_ts_type(&elem.ty, synthetic, reg).ok())
                .collect();
            if elem_types.len() == tuple.elem_types.len() {
                variants.push(EnumVariant {
                    name: "Tuple".to_string(),
                    value: None,
                    data: Some(RustType::Tuple(elem_types)),
                    fields: vec![],
                });
                return;
            }
        }
        _ => {}
    }
    // Fallback: unsupported types become Other{N}(serde_json::Value)
    variants.push(EnumVariant {
        name: format!("Other{}", variants.len()),
        value: None,
        data: Some(RustType::Any),
        fields: vec![],
    });
}

/// Converts a TS function type to a `RustType::Fn`.
fn convert_fn_type_to_rust(
    fn_type: &swc_ecma_ast::TsFnType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params: Vec<RustType> = fn_type
        .params
        .iter()
        .filter_map(|param| {
            if let swc_ecma_ast::TsFnParam::Ident(ident) = param {
                ident
                    .type_ann
                    .as_ref()
                    .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, reg).ok())
            } else {
                None
            }
        })
        .collect();
    let return_type =
        convert_ts_type(&fn_type.type_ann.type_ann, synthetic, reg).unwrap_or(RustType::Unit);
    Ok(RustType::Fn {
        params,
        return_type: Box::new(return_type),
    })
}
