use super::*;

/// Extracts type parameters (name + optional constraint) from an optional [`TsTypeParamDecl`].
///
/// Returns an empty vec if there are no type parameters.
pub fn extract_type_params(
    type_params: Option<&swc_ecma_ast::TsTypeParamDecl>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Vec<TypeParam> {
    match type_params {
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
        None => vec![],
    }
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

// -- Utility type helpers --

/// Resolved struct info: (type_name, fields).
type ResolvedFields = (String, Vec<FieldDef>);

/// Extracts the inner type name and resolves its fields from the registry.
/// Returns `(type_name, fields)` or `None` if unregistered.
fn resolve_utility_inner_fields(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Option<ResolvedFields> {
    let params = type_ref.type_params.as_ref()?;
    if params.params.is_empty() {
        return None;
    }
    let inner = &params.params[0];
    // Inner type must be a type reference with an ident name
    let inner_name = match inner.as_ref() {
        swc_ecma_ast::TsType::TsTypeRef(inner_ref) => match &inner_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
            _ => return None,
        },
        _ => return None,
    };

    // Try registry first, then check synthetic for synthesized structs
    let fields = if let Some(TypeDef::Struct { fields, .. }) = reg.get(&inner_name) {
        fields.clone()
    } else if let Some(def) = synthetic.get(&inner_name) {
        // Look in synthetic for a previously synthesized struct
        match &def.item {
            Item::Struct { fields, .. } => fields
                .iter()
                .map(|f| FieldDef {
                    name: f.name.clone(),
                    ty: f.ty.clone(),
                    optional: false,
                })
                .collect(),
            _ => return None,
        }
    } else {
        return None;
    };

    Some((inner_name, fields))
}

/// Resolves the inner type of a utility type, converting it first if needed (for nesting).
/// Returns `(resolved_name, fields)` or None if no struct fields can be found.
fn resolve_utility_inner_with_conversion(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Option<ResolvedFields>> {
    // First try direct resolution from registry/synthetic
    if let Some(result) = resolve_utility_inner_fields(type_ref, synthetic, reg) {
        return Ok(Some(result));
    }

    // If not found, convert the inner type (handles nested utility types)
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("utility type requires a type parameter"))?;
    if params.params.is_empty() {
        return Ok(None);
    }
    let converted = convert_ts_type(&params.params[0], synthetic, reg)?;

    // If conversion produced a Named type, look for it in synthetic
    if let RustType::Named { ref name, .. } = converted {
        if let Some(def) = synthetic.get(name) {
            if let Item::Struct { fields, .. } = &def.item {
                let field_defs = fields
                    .iter()
                    .map(|f| FieldDef {
                        name: f.name.clone(),
                        ty: f.ty.clone(),
                        optional: false,
                    })
                    .collect();
                return Ok(Some((name.clone(), field_defs)));
            }
        }
    }

    Ok(None)
}

/// `Partial<T>` → all fields wrapped in `Option<T>`
pub(super) fn convert_utility_partial(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let Some((inner_name, fields)) =
        resolve_utility_inner_with_conversion(type_ref, synthetic, reg)?
    else {
        // Fallback: return inner type as-is
        let params = type_ref
            .type_params
            .as_ref()
            .ok_or_else(|| anyhow!("Partial requires a type parameter"))?;
        return convert_ts_type(&params.params[0], synthetic, reg);
    };

    let synth_name = format!("Partial{inner_name}");
    let synth_fields = fields
        .into_iter()
        .map(|field| StructField {
            vis: None,
            name: sanitize_field_name(&field.name),
            ty: if matches!(field.ty, RustType::Option(_)) {
                field.ty
            } else {
                RustType::Option(Box::new(field.ty))
            },
        })
        .collect();

    synthetic.push_item(
        synth_name.clone(),
        crate::pipeline::SyntheticTypeKind::InlineStruct,
        Item::Struct {
            name: synth_name.clone(),
            vis: Visibility::Public,
            fields: synth_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// `Required<T>` → all `Option` wrappers removed from fields
pub(super) fn convert_utility_required(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let Some((inner_name, fields)) =
        resolve_utility_inner_with_conversion(type_ref, synthetic, reg)?
    else {
        let params = type_ref
            .type_params
            .as_ref()
            .ok_or_else(|| anyhow!("Required requires a type parameter"))?;
        return convert_ts_type(&params.params[0], synthetic, reg);
    };

    let synth_name = format!("Required{inner_name}");
    let synth_fields = fields
        .into_iter()
        .map(|field| StructField {
            vis: None,
            name: sanitize_field_name(&field.name),
            ty: match field.ty {
                RustType::Option(inner) => *inner,
                other => other,
            },
        })
        .collect();

    synthetic.push_item(
        synth_name.clone(),
        crate::pipeline::SyntheticTypeKind::InlineStruct,
        Item::Struct {
            name: synth_name.clone(),
            vis: Visibility::Public,
            fields: synth_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// Extracts string literal keys from a union type parameter (e.g., `"x" | "y"`).
fn extract_string_keys(ts_type: &swc_ecma_ast::TsType) -> Vec<String> {
    match ts_type {
        swc_ecma_ast::TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(s) => vec![s.value.to_string_lossy().into_owned()],
            _ => vec![],
        },
        swc_ecma_ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(union),
        ) => union
            .types
            .iter()
            .flat_map(|t| extract_string_keys(t))
            .collect(),
        _ => vec![],
    }
}

/// `Pick<T, K>` → only fields whose names are in K
pub(super) fn convert_utility_pick(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("Pick requires type parameters"))?;
    if params.params.len() < 2 {
        return Err(anyhow!("Pick expects at least two type parameters"));
    }

    let Some((inner_name, fields)) = resolve_utility_inner_fields(type_ref, synthetic, reg) else {
        return convert_ts_type(&params.params[0], synthetic, reg);
    };

    let keys = extract_string_keys(&params.params[1]);
    let picked_fields: Vec<StructField> = fields
        .into_iter()
        .filter(|field| keys.contains(&field.name))
        .map(|field| StructField {
            vis: None,
            name: sanitize_field_name(&field.name),
            ty: field.ty,
        })
        .collect();

    let keys_suffix = keys.iter().map(|k| capitalize_first(k)).collect::<String>();
    let synth_name = format!("Pick{inner_name}{keys_suffix}");

    synthetic.push_item(
        synth_name.clone(),
        crate::pipeline::SyntheticTypeKind::InlineStruct,
        Item::Struct {
            name: synth_name.clone(),
            vis: Visibility::Public,
            fields: picked_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// `Omit<T, K>` → all fields except those in K
pub(super) fn convert_utility_omit(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("Omit requires type parameters"))?;
    if params.params.len() < 2 {
        return Err(anyhow!("Omit expects at least two type parameters"));
    }

    let Some((inner_name, fields)) = resolve_utility_inner_fields(type_ref, synthetic, reg) else {
        return convert_ts_type(&params.params[0], synthetic, reg);
    };

    let keys = extract_string_keys(&params.params[1]);
    let omitted_fields: Vec<StructField> = fields
        .into_iter()
        .filter(|field| !keys.contains(&field.name))
        .map(|field| StructField {
            vis: None,
            name: sanitize_field_name(&field.name),
            ty: field.ty,
        })
        .collect();

    let keys_suffix = keys.iter().map(|k| capitalize_first(k)).collect::<String>();
    let synth_name = format!("Omit{inner_name}{keys_suffix}");

    synthetic.push_item(
        synth_name.clone(),
        crate::pipeline::SyntheticTypeKind::InlineStruct,
        Item::Struct {
            name: synth_name.clone(),
            vis: Visibility::Public,
            fields: omitted_fields,
            type_params: vec![],
        },
    );

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// `NonNullable<T>` → strip `Option` wrapper or remove null/undefined from union
pub(super) fn convert_utility_non_nullable(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("NonNullable requires a type parameter"))?;
    if params.params.len() != 1 {
        return Err(anyhow!("NonNullable expects exactly one type parameter"));
    }

    let inner = convert_ts_type(&params.params[0], synthetic, reg)?;
    // Strip Option wrapper
    Ok(match inner {
        RustType::Option(inner_ty) => *inner_ty,
        other => other,
    })
}

/// Capitalizes the first character of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
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
