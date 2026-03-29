use super::*;

use super::super::synthetic_registry::variant_name_for_type;

/// Converts a union type. Handles `T | null` and `T | undefined` as `Option<T>`.
pub(super) fn convert_union_type(
    union: &swc_ecma_ast::TsUnionType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let mut non_null_types: Vec<&TsType> = Vec::new();
    let mut has_null_or_undefined = false;

    for ty in &union.types {
        match ty.as_ref() {
            TsType::TsKeywordType(kw) if is_nullable_keyword(kw.kind) => {
                has_null_or_undefined = true;
            }
            // never is the bottom type — remove from unions (T | never = T)
            TsType::TsKeywordType(kw) if kw.kind == TsKeywordTypeKind::TsNeverKeyword => {}
            other => {
                non_null_types.push(other);
            }
        }
    }

    if has_null_or_undefined && non_null_types.len() == 1 {
        let inner = convert_ts_type(non_null_types[0], synthetic, reg)?;
        Ok(RustType::Option(Box::new(inner)))
    } else if has_null_or_undefined && non_null_types.is_empty() {
        // `null | undefined` — treat as Option of unit, but we don't have unit type
        Err(anyhow!("union of only null/undefined is not supported"))
    } else if !has_null_or_undefined {
        // Convert all members, with fallback for unsupported types
        let mut variants = Vec::new();
        let mut name_parts = Vec::new();
        for ty in &non_null_types {
            match convert_ts_type(ty, synthetic, reg) {
                Ok(rust_type) => {
                    let unwrapped = unwrap_promise(rust_type);
                    let variant_name = variant_name_for_type(&unwrapped);
                    // Deduplicate
                    if !name_parts.contains(&variant_name) {
                        name_parts.push(variant_name.clone());
                        variants.push(EnumVariant {
                            name: variant_name,
                            value: None,
                            data: Some(unwrapped),
                            fields: vec![],
                        });
                    }
                }
                Err(_) => {
                    convert_unsupported_union_member(ty, &mut variants, synthetic, reg);
                    if let Some(last) = variants.last() {
                        name_parts.push(last.name.clone());
                    }
                }
            }
        }

        // After dedup, if only one type remains, return it directly
        if variants.len() == 1 && variants[0].data.is_some() {
            return Ok(variants.into_iter().next().unwrap().data.unwrap());
        }

        let enum_name = name_parts.join("Or");
        synthetic.push_item(
            enum_name.clone(),
            crate::pipeline::SyntheticTypeKind::UnionEnum,
            Item::Enum {
                vis: Visibility::Public,
                name: enum_name.clone(),
                serde_tag: None,
                variants,
            },
        );
        Ok(RustType::Named {
            name: enum_name,
            type_args: vec![],
        })
    } else {
        // has_null_or_undefined && non_null_types.len() > 1
        // e.g., string | number | null → Option<StringOrF64>
        let mut variants = Vec::new();
        let mut name_parts = Vec::new();
        for ty in &non_null_types {
            match convert_ts_type(ty, synthetic, reg) {
                Ok(rust_type) => {
                    let unwrapped = unwrap_promise(rust_type);
                    let variant_name = variant_name_for_type(&unwrapped);
                    if !name_parts.contains(&variant_name) {
                        name_parts.push(variant_name.clone());
                        variants.push(EnumVariant {
                            name: variant_name,
                            value: None,
                            data: Some(unwrapped),
                            fields: vec![],
                        });
                    }
                }
                Err(_) => {
                    convert_unsupported_union_member(ty, &mut variants, synthetic, reg);
                    if let Some(last) = variants.last() {
                        name_parts.push(last.name.clone());
                    }
                }
            }
        }

        // After dedup, if only one type remains (e.g., null | undefined | T)
        if variants.len() == 1 && variants[0].data.is_some() {
            return Ok(RustType::Option(Box::new(
                variants.into_iter().next().unwrap().data.unwrap(),
            )));
        }

        let enum_name = name_parts.join("Or");
        synthetic.push_item(
            enum_name.clone(),
            crate::pipeline::SyntheticTypeKind::UnionEnum,
            Item::Enum {
                vis: Visibility::Public,
                name: enum_name.clone(),
                serde_tag: None,
                variants,
            },
        );
        Ok(RustType::Option(Box::new(RustType::Named {
            name: enum_name,
            type_args: vec![],
        })))
    }
}

/// Unwraps `Promise<T>` to `T`. Returns the type unchanged for non-Promise types.
fn unwrap_promise(ty: RustType) -> RustType {
    match &ty {
        RustType::Named { name, type_args } if name == "Promise" && type_args.len() == 1 => {
            type_args[0].clone()
        }
        _ => ty,
    }
}

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
    let union = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return Ok(None),
    };

    // All members must be object type literals
    let type_lits: Vec<&swc_ecma_ast::TsTypeLit> = union
        .types
        .iter()
        .filter_map(|ty| match ty.as_ref() {
            TsType::TsTypeLit(lit) => Some(lit),
            _ => None,
        })
        .collect();

    if type_lits.len() != union.types.len() || type_lits.len() < 2 {
        return Ok(None);
    }

    // Find a common field that has string literal types in all members
    let discriminant_field = find_discriminant_field(&type_lits);
    let discriminant_field = match discriminant_field {
        Some(f) => f,
        None => return Ok(None),
    };

    // Build enum variants
    let mut variants = Vec::new();
    for type_lit in &type_lits {
        let (discriminant_value, other_fields) =
            extract_variant_info(type_lit, &discriminant_field, synthetic, reg)?;
        variants.push(EnumVariant {
            name: string_to_pascal_case(&discriminant_value),
            value: Some(EnumValue::Str(discriminant_value)),
            data: None,
            fields: other_fields,
        });
    }

    Ok(Some(Item::Enum {
        vis,
        name: decl.id.sym.to_string(),
        serde_tag: Some(discriminant_field),
        variants,
    }))
}

/// Finds a field name that is present in all type literals with a string literal type.
pub(super) fn find_discriminant_field(type_lits: &[&swc_ecma_ast::TsTypeLit]) -> Option<String> {
    // Collect field names from the first member
    let first = type_lits[0];
    for member in &first.members {
        if let TsTypeElement::TsPropertySignature(prop) = member {
            let field_name = match prop.key.as_ref() {
                Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };

            // Check if this field has a string literal type
            let has_str_lit = prop
                .type_ann
                .as_ref()
                .is_some_and(|ann| is_string_literal_type(&ann.type_ann));

            if !has_str_lit {
                continue;
            }

            // Check if all other members have this field with a string literal type
            let all_have = type_lits[1..].iter().all(|lit| {
                lit.members.iter().any(|m| {
                    if let TsTypeElement::TsPropertySignature(p) = m {
                        let name = match p.key.as_ref() {
                            Expr::Ident(id) => id.sym.to_string(),
                            _ => return false,
                        };
                        name == field_name
                            && p.type_ann
                                .as_ref()
                                .is_some_and(|ann| is_string_literal_type(&ann.type_ann))
                    } else {
                        false
                    }
                })
            });

            if all_have {
                return Some(field_name);
            }
        }
    }
    None
}

/// Checks if a type is a string literal type (e.g., `"click"`).
fn is_string_literal_type(ty: &TsType) -> bool {
    matches!(
        ty,
        TsType::TsLitType(lit) if matches!(&lit.lit, swc_ecma_ast::TsLit::Str(_))
    )
}

/// Extracts the discriminant value and non-discriminant fields from a type literal.
pub(super) fn extract_variant_info(
    type_lit: &swc_ecma_ast::TsTypeLit,
    discriminant_field: &str,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(String, Vec<StructField>)> {
    let mut discriminant_value = None;
    let mut fields = Vec::new();

    for member in &type_lit.members {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                let field_name = match prop.key.as_ref() {
                    Expr::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported property key in discriminated union")),
                };

                if field_name == discriminant_field {
                    // Extract discriminant value
                    let ann = prop
                        .type_ann
                        .as_ref()
                        .ok_or_else(|| anyhow!("discriminant field has no type annotation"))?;
                    match ann.type_ann.as_ref() {
                        TsType::TsLitType(lit) => match &lit.lit {
                            swc_ecma_ast::TsLit::Str(s) => {
                                discriminant_value = Some(s.value.to_string_lossy().into_owned());
                            }
                            _ => return Err(anyhow!("discriminant must be a string literal")),
                        },
                        _ => return Err(anyhow!("discriminant must be a string literal type")),
                    }
                } else {
                    // Regular field
                    let field = convert_property_signature(prop, synthetic, reg)?;
                    fields.push(field);
                }
            }
            _ => return Err(anyhow!("unsupported member in discriminated union variant")),
        }
    }

    let value = discriminant_value.ok_or_else(|| anyhow!("discriminant value not found"))?;
    Ok((value, fields))
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
        name: decl.id.sym.to_string(),
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
                    name: decl.id.sym.to_string(),
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

/// Converts a string value to PascalCase for use as an enum variant name.
///
/// Examples: `"up"` → `"Up"`, `"foo-bar"` → `"FooBar"`, `"UPPER_CASE"` → `"UpperCase"`
pub(crate) fn string_to_pascal_case(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

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
            name: decl.id.sym.to_string(),
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
                let rust_type = convert_type_ref(type_ref, synthetic, reg)?;
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
            TsType::TsTypeLit(lit) => {
                let mut fields = Vec::new();
                for member in &lit.members {
                    if let TsTypeElement::TsPropertySignature(prop) = member {
                        fields.push(convert_property_signature(prop, synthetic, reg)?);
                    }
                }
                variants.push(EnumVariant {
                    name: format!("Variant{}", variants.len()),
                    value: None,
                    data: None,
                    fields,
                });
            }
            TsType::TsUnionOrIntersectionType(
                swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
            ) => {
                let mut fields = Vec::new();
                for member_ty in &intersection.types {
                    if let TsType::TsTypeLit(lit) = member_ty.as_ref() {
                        for member in &lit.members {
                            if let TsTypeElement::TsPropertySignature(prop) = member {
                                fields.push(convert_property_signature(prop, synthetic, reg)?);
                            }
                        }
                    }
                }
                variants.push(EnumVariant {
                    name: format!("Variant{}", variants.len()),
                    value: None,
                    data: None,
                    fields,
                });
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
        name: decl.id.sym.to_string(),
        serde_tag: None,
        variants,
    };

    // For multi-type nullable unions (`type X = string | number | null`), we emit
    // the enum as-is. Single-type nullable (`T | null`) is handled above as Option<T>.
    Ok(Some(enum_item))
}
