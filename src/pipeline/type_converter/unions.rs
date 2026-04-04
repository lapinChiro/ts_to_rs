use super::*;

/// Tries to convert a discriminated union type alias.
///
/// A discriminated union is a union of object types that share a common field
/// with string literal types. Example:
///
/// ```typescript
/// type Event = { kind: "click", x: number } | { kind: "hover", y: number }
/// ```
///
/// Produces a `#[serde(tag = "kind")]` enum with struct variants.
pub(super) fn try_convert_discriminated_union(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Option<Item>> {
    // Convert union to TsTypeInfo
    let union_members = match crate::ts_type_info::convert_to_ts_type_info(decl.type_ann.as_ref())?
    {
        crate::ts_type_info::TsTypeInfo::Union(members) => members,
        _ => return Ok(None),
    };

    // All members must be TypeLiteral (2+ members)
    if union_members.len() < 2
        || !union_members
            .iter()
            .all(|m| matches!(m, crate::ts_type_info::TsTypeInfo::TypeLiteral(_)))
    {
        return Ok(None);
    }

    // Find a common field that has string literal types in all members
    let discriminant_field =
        crate::ts_type_info::resolve::intersection::find_discriminant_field(&union_members);
    let discriminant_field = match discriminant_field {
        Some(f) => f,
        None => return Ok(None),
    };

    // Build enum variants
    let mut variants = Vec::new();
    for member in &union_members {
        let (raw_value, pascal_name, other_fields) =
            crate::ts_type_info::resolve::intersection::extract_discriminated_variant(
                member,
                &discriminant_field,
                reg,
                synthetic,
            )?;
        variants.push(EnumVariant {
            name: pascal_name,
            value: Some(EnumValue::Str(raw_value)),
            data: None,
            fields: other_fields,
        });
    }

    Ok(Some(Item::Enum {
        vis,
        name: sanitize_rust_type_name(&decl.id.sym),
        serde_tag: Some(discriminant_field),
        variants,
    }))
}

/// Tries to convert a type alias with a union body where all members are string literals.
pub(super) fn try_convert_string_literal_union(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
) -> Result<Option<Item>> {
    let union = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return Ok(None),
    };

    let mut variants = Vec::new();
    for ty in &union.types {
        match ty.as_ref() {
            TsType::TsLitType(lit) => match &lit.lit {
                swc_ecma_ast::TsLit::Str(s) => {
                    let value = s.value.to_string_lossy().into_owned();
                    variants.push(EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                        fields: vec![],
                    });
                }
                _ => return Ok(None), // Non-string literal → not a string literal union
            },
            // Skip nullable members in string literal unions (they become Option wrapping)
            TsType::TsKeywordType(kw) if is_nullable_keyword(kw.kind) => {
                continue;
            }
            _ => return Ok(None), // Non-literal member → not a string literal union
        }
    }

    Ok(Some(Item::Enum {
        vis,
        name: sanitize_rust_type_name(&decl.id.sym),
        serde_tag: None,
        variants,
    }))
}

/// Tries to convert a type alias with a single string literal body.
///
/// Handles `type X = "only"` as a single-variant enum.
pub(super) fn try_convert_single_string_literal(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
) -> Result<Option<Item>> {
    match decl.type_ann.as_ref() {
        TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(s) => {
                let value = s.value.to_string_lossy().into_owned();
                Ok(Some(Item::Enum {
                    vis,
                    name: sanitize_rust_type_name(&decl.id.sym),
                    serde_tag: None,
                    variants: vec![EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                        fields: vec![],
                    }],
                }))
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

// string_to_pascal_case は super (type_converter/mod.rs) に定義。
// use super::*; 経由でアクセス可能。

/// Tries to convert a type alias with a union type body into an enum.
///
/// Handles numeric literal unions (`type Code = 200 | 404`),
/// primitive type unions (`type Value = string | number`), and
/// type reference unions (`type R = Success | Failure`).
/// Returns `None` if the type alias body is not a union type.
pub(super) fn try_convert_general_union(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Option<Item>> {
    let union = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return Ok(None),
    };

    // Filter out null/undefined members
    let mut non_null_types: Vec<&TsType> = Vec::new();
    let mut has_null_or_undefined = false;
    for ty in &union.types {
        match ty.as_ref() {
            TsType::TsKeywordType(kw) if is_nullable_keyword(kw.kind) => {
                has_null_or_undefined = true;
            }
            other => non_null_types.push(other),
        }
    }

    // Nullable union with single non-null type: `type X = T | null` → `type X = Option<T>`
    if has_null_or_undefined && non_null_types.len() == 1 {
        let inner_type = convert_ts_type(non_null_types[0], synthetic, reg)?;
        let type_params = extract_type_params(decl.type_params.as_deref(), synthetic, reg);
        return Ok(Some(Item::TypeAlias {
            vis,
            name: sanitize_rust_type_name(&decl.id.sym),
            type_params,
            ty: RustType::Option(Box::new(inner_type)),
        }));
    }

    // If all members are string literals, `try_convert_string_literal_union` handles it
    if non_null_types.iter().all(|t| {
        matches!(
            t,
            TsType::TsLitType(lit) if matches!(lit.lit, swc_ecma_ast::TsLit::Str(_))
        )
    }) {
        return Ok(None);
    }

    let mut variants = Vec::new();
    for ty in &non_null_types {
        match *ty {
            TsType::TsLitType(lit) => match &lit.lit {
                swc_ecma_ast::TsLit::Number(n) => {
                    let value = n.value as i64;
                    variants.push(EnumVariant {
                        name: format!(
                            "V{}",
                            if value < 0 {
                                format!("Neg{}", -value)
                            } else {
                                value.to_string()
                            }
                        ),
                        value: Some(EnumValue::Number(value)),
                        data: None,
                        fields: vec![],
                    });
                }
                swc_ecma_ast::TsLit::Str(s) => {
                    let value = s.value.to_string_lossy().into_owned();
                    variants.push(EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                        fields: vec![],
                    });
                }
                _ => return Err(anyhow!("unsupported literal type in union")),
            },
            TsType::TsKeywordType(kw) => {
                let (variant_name, rust_type) = match kw.kind {
                    TsKeywordTypeKind::TsStringKeyword => ("String".to_string(), RustType::String),
                    TsKeywordTypeKind::TsNumberKeyword => ("F64".to_string(), RustType::F64),
                    TsKeywordTypeKind::TsBooleanKeyword => ("Bool".to_string(), RustType::Bool),
                    TsKeywordTypeKind::TsBigIntKeyword => (
                        "I128".to_string(),
                        RustType::Named {
                            name: "i128".to_string(),
                            type_args: vec![],
                        },
                    ),
                    TsKeywordTypeKind::TsSymbolKeyword
                    | TsKeywordTypeKind::TsAnyKeyword
                    | TsKeywordTypeKind::TsUnknownKeyword
                    | TsKeywordTypeKind::TsObjectKeyword => ("Any".to_string(), RustType::Any),
                    TsKeywordTypeKind::TsNeverKeyword | TsKeywordTypeKind::TsVoidKeyword => {
                        continue
                    }
                    // undefined/null are typically handled via Option wrapping,
                    // not as union variants. Intrinsic is a TS compiler-internal type.
                    TsKeywordTypeKind::TsUndefinedKeyword
                    | TsKeywordTypeKind::TsNullKeyword
                    | TsKeywordTypeKind::TsIntrinsicKeyword => continue,
                };
                variants.push(EnumVariant {
                    name: variant_name,
                    value: None,
                    data: Some(rust_type),
                    fields: vec![],
                });
            }
            TsType::TsTypeRef(type_ref) => {
                let rust_type = convert_ts_type(ty, synthetic, reg)?;
                let variant_name = match &type_ref.type_name {
                    swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported qualified type name in union")),
                };
                variants.push(EnumVariant {
                    name: variant_name,
                    value: None,
                    data: Some(rust_type),
                    fields: vec![],
                });
            }
            TsType::TsTypeLit(_) => {
                let info = crate::ts_type_info::convert_to_ts_type_info(ty)?;
                if let crate::ts_type_info::TsTypeInfo::TypeLiteral(lit) = &info {
                    let fields =
                        crate::ts_type_info::resolve::intersection::resolve_type_literal_fields(
                            lit, reg, synthetic,
                        )?;
                    variants.push(EnumVariant {
                        name: format!("Variant{}", variants.len()),
                        value: None,
                        data: None,
                        fields,
                    });
                }
            }
            TsType::TsUnionOrIntersectionType(
                swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(_),
            ) => {
                let info = crate::ts_type_info::convert_to_ts_type_info(ty)?;
                if let crate::ts_type_info::TsTypeInfo::Intersection(members) = &info {
                    let mut fields = Vec::new();
                    for member in members {
                        if let crate::ts_type_info::TsTypeInfo::TypeLiteral(lit) = member {
                            let lit_fields = crate::ts_type_info::resolve::intersection::resolve_type_literal_fields(lit, reg, synthetic)?;
                            fields.extend(lit_fields);
                        }
                    }
                    variants.push(EnumVariant {
                        name: format!("Variant{}", variants.len()),
                        value: None,
                        data: None,
                        fields,
                    });
                }
            }
            _ => {
                convert_unsupported_union_member(ty, &mut variants, synthetic, reg);
            }
        }
    }

    if variants.is_empty() {
        return Err(anyhow!("empty union type"));
    }

    let enum_item = Item::Enum {
        vis: vis.clone(),
        name: sanitize_rust_type_name(&decl.id.sym),
        serde_tag: None,
        variants,
    };

    // For multi-type nullable unions (`type X = string | number | null`), we emit
    // the enum as-is. Single-type nullable (`T | null`) is handled above as Option<T>.
    Ok(Some(enum_item))
}
