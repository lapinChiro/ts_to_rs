use super::*;

/// Recursively unwraps `TsParenthesizedType` to get the inner type.
fn unwrap_parenthesized(ty: &TsType) -> &TsType {
    match ty {
        TsType::TsParenthesizedType(paren) => unwrap_parenthesized(&paren.type_ann),
        _ => ty,
    }
}

/// Checks if a type is an empty object literal (`{}`).
fn is_empty_type_lit(ty: &TsType) -> bool {
    matches!(ty, TsType::TsTypeLit(lit) if lit.members.is_empty())
}

/// Checks if a key remapping clause is a no-op symbol filter.
///
/// `K extends symbol ? never : K` removes only symbol keys, which don't exist in Rust.
/// This is equivalent to identity (no filtering).
fn is_symbol_filter_noop(name_type: &TsType, param_name: &str) -> bool {
    if let TsType::TsConditionalType(cond) = name_type {
        // check_type must be the param name (K)
        let check_is_param = match cond.check_type.as_ref() {
            TsType::TsTypeRef(r) => matches!(
                &r.type_name,
                swc_ecma_ast::TsEntityName::Ident(i) if i.sym.as_ref() == param_name
            ),
            _ => false,
        };
        // extends_type must be `symbol`
        let extends_is_symbol = matches!(
            cond.extends_type.as_ref(),
            TsType::TsKeywordType(k) if k.kind == swc_ecma_ast::TsKeywordTypeKind::TsSymbolKeyword
        );
        // true_type must be `never`
        let true_is_never = matches!(
            cond.true_type.as_ref(),
            TsType::TsKeywordType(k) if k.kind == swc_ecma_ast::TsKeywordTypeKind::TsNeverKeyword
        );
        // false_type must be the param name (K)
        let false_is_param = match cond.false_type.as_ref() {
            TsType::TsTypeRef(r) => matches!(
                &r.type_name,
                swc_ecma_ast::TsEntityName::Ident(i) if i.sym.as_ref() == param_name
            ),
            _ => false,
        };
        check_is_param && extends_is_symbol && true_is_never && false_is_param
    } else {
        false
    }
}

/// Detects identity mapped types: `{ [K in keyof T]: T[K] }`.
///
/// Returns `Some(RustType::Named { name: "T" })` if the mapped type is an identity mapping
/// (equivalent to `T` itself). An identity mapped type satisfies all of:
/// 1. No key remapping, or key remapping is a no-op symbol filter
///    (`K extends symbol ? never : K` — removes only symbol keys, which don't exist in Rust)
/// 2. Constraint is `keyof T` (`TsTypeOperator(KeyOf, TsTypeRef(T))`)
/// 3. Value type is `T[K]` (`TsIndexedAccessType` where obj = T, key = K)
/// 4. No readonly/optional modifiers
pub(super) fn try_simplify_identity_mapped_type(
    mapped: &swc_ecma_ast::TsMappedType,
) -> Option<RustType> {
    // 1. No key remapping, OR key remapping is a no-op symbol filter
    if let Some(name_type) = &mapped.name_type {
        let param_name = mapped.type_param.name.sym.as_ref();
        if !is_symbol_filter_noop(name_type, param_name) {
            return None;
        }
    }

    // 4. No modifiers
    if mapped.readonly.is_some() || mapped.optional.is_some() {
        return None;
    }

    // Get mapped type param name (K)
    let param_name = mapped.type_param.name.sym.to_string();

    // 2. Constraint is `keyof T`
    let constraint = mapped.type_param.constraint.as_ref()?;
    let base_type_name = match constraint.as_ref() {
        TsType::TsTypeOperator(op) if op.op == swc_ecma_ast::TsTypeOperatorOp::KeyOf => {
            match op.type_ann.as_ref() {
                TsType::TsTypeRef(type_ref) => match &type_ref.type_name {
                    swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                    _ => return None,
                },
                _ => return None,
            }
        }
        _ => return None,
    };

    // 3. Value type is `T[K]`
    let value_type = mapped.type_ann.as_ref()?;
    match value_type.as_ref() {
        TsType::TsIndexedAccessType(indexed) => {
            // obj must be T
            let obj_name = match indexed.obj_type.as_ref() {
                TsType::TsTypeRef(type_ref) => match &type_ref.type_name {
                    swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                    _ => return None,
                },
                _ => return None,
            };
            if obj_name != base_type_name {
                return None;
            }
            // key must be K (the mapped type parameter)
            let key_name = match indexed.index_type.as_ref() {
                TsType::TsTypeRef(type_ref) => match &type_ref.type_name {
                    swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                    _ => return None,
                },
                _ => return None,
            };
            if key_name != param_name {
                return None;
            }
            Some(RustType::Named {
                name: base_type_name,
                type_args: vec![],
            })
        }
        _ => None,
    }
}

/// Pre-processes intersection members: unwraps parenthesized types and removes empty object literals.
///
/// Returns the filtered list of inner `TsType` references. If all members are empty,
/// returns an empty vec.
fn preprocess_intersection_members(
    intersection: &swc_ecma_ast::TsIntersectionType,
) -> Vec<&TsType> {
    intersection
        .types
        .iter()
        .map(|ty| unwrap_parenthesized(ty.as_ref()))
        .filter(|ty| !is_empty_type_lit(ty))
        .collect()
}

/// Extracts fields and methods from an intersection type's member types.
///
/// Shared logic for both type alias intersections and annotation-position intersections.
/// Handles `TsTypeLit` (property sigs → fields, method sigs → methods),
/// `TsTypeRef` (resolved from registry or embedded), and `TsKeywordType` (skipped).
/// Falls back to `convert_ts_type` for other member types.
fn extract_intersection_members(
    members: &[&TsType],
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(Vec<StructField>, Vec<Method>)> {
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    for (i, ty) in members.iter().enumerate() {
        match *ty {
            TsType::TsTypeLit(lit) => {
                for member in &lit.members {
                    match member {
                        TsTypeElement::TsPropertySignature(prop) => {
                            let field = convert_property_signature(prop, synthetic, reg)?;
                            if fields.iter().any(|f: &StructField| f.name == field.name) {
                                return Err(anyhow!(
                                    "duplicate field '{}' in intersection type",
                                    field.name
                                ));
                            }
                            fields.push(field);
                        }
                        TsTypeElement::TsMethodSignature(sig) => {
                            methods.push(convert_method_signature(sig, synthetic, reg)?);
                        }
                        _ => continue,
                    }
                }
            }
            TsType::TsTypeRef(type_ref) => {
                let type_name = match &type_ref.type_name {
                    swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported qualified type name in intersection")),
                };
                // Try to resolve and merge fields from TypeRegistry
                if let Some(crate::registry::TypeDef::Struct {
                    fields: resolved_fields,
                    ..
                }) = reg.get(&type_name)
                {
                    for field in resolved_fields {
                        let sanitized = sanitize_field_name(&field.name);
                        if fields.iter().any(|f: &StructField| f.name == sanitized) {
                            return Err(anyhow!(
                                "duplicate field '{}' in intersection type",
                                field.name
                            ));
                        }
                        fields.push(StructField {
                            vis: None,
                            name: sanitized,
                            ty: field.ty.clone(),
                        });
                    }
                } else {
                    // Unresolved type reference — embed as a named field
                    let rust_type = convert_ts_type(ty, synthetic, reg)?;
                    fields.push(StructField {
                        vis: None,
                        name: format!("_{i}"),
                        ty: rust_type,
                    });
                }
            }
            // Skip keyword types in intersections (e.g., `string & {}` → use object fields only).
            // This is safe for TypeScript branding patterns where the keyword is nominal.
            TsType::TsKeywordType(_) => continue,
            // Fallback: try convert_ts_type for any other member type (mapped, conditional, etc.)
            other => {
                let rust_type = convert_ts_type(other, synthetic, reg).unwrap_or(RustType::Any);
                fields.push(StructField {
                    vis: None,
                    name: format!("_{i}"),
                    ty: rust_type,
                });
            }
        }
    }
    Ok((fields, methods))
}

/// Merges base fields with variant-specific fields, deduplicating by name.
///
/// When a variant field has the same name as a base field, the variant field takes precedence.
/// In strict TypeScript semantics, `{ name: string } & ({ name: number } | ...)` would yield
/// `name: string & number` (= `never`) per variant. This implementation uses the variant's type
/// as a pragmatic simplification — since intersecting field types is not yet supported, keeping
/// the variant's type (the more specific union arm) prevents duplicate fields in generated Rust.
fn merge_fields(base: &[StructField], variant: Vec<StructField>) -> Vec<StructField> {
    let variant_names: std::collections::HashSet<&str> =
        variant.iter().map(|f| f.name.as_str()).collect();
    let mut merged: Vec<StructField> = base
        .iter()
        .filter(|f| !variant_names.contains(f.name.as_str()))
        .cloned()
        .collect();
    merged.extend(variant);
    merged
}

/// Extracts fields from a single union variant type for use in intersection distribution.
///
/// Handles TsTypeLit (extract property signatures), TsTypeRef (resolve from registry or embed),
/// and other types (convert_ts_type fallback as embedded `_data` field).
fn extract_variant_fields(
    ty: &TsType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Vec<StructField>> {
    let mut fields = Vec::new();
    match ty {
        TsType::TsTypeLit(lit) => {
            for member in &lit.members {
                if let TsTypeElement::TsPropertySignature(prop) = member {
                    fields.push(convert_property_signature(prop, synthetic, reg)?);
                }
            }
        }
        TsType::TsTypeRef(type_ref) => {
            let type_name = match &type_ref.type_name {
                swc_ecma_ast::TsEntityName::Ident(ident) => Some(ident.sym.to_string()),
                _ => None,
            };
            if let Some(ref name) = type_name {
                if let Some(crate::registry::TypeDef::Struct {
                    fields: resolved, ..
                }) = reg.get(name)
                {
                    for field in resolved {
                        fields.push(StructField {
                            vis: None,
                            name: sanitize_field_name(&field.name),
                            ty: field.ty.clone(),
                        });
                    }
                    return Ok(fields);
                }
            }
            // Unresolved type ref — embed as _data field
            let rust_type = convert_ts_type(ty, synthetic, reg)?;
            fields.push(StructField {
                vis: None,
                name: "_data".to_string(),
                ty: rust_type,
            });
        }
        _ => {
            // Other type — convert and embed as _data field
            let rust_type = convert_ts_type(ty, synthetic, reg).unwrap_or(RustType::Any);
            fields.push(StructField {
                vis: None,
                name: "_data".to_string(),
                ty: rust_type,
            });
        }
    }
    Ok(fields)
}

/// Distributes an intersection with a union member: `A & (B | C)` → enum with A's fields in each variant.
///
/// Returns `(variants, serde_tag)` where `serde_tag` is set if a discriminant field is detected.
fn distribute_intersection_with_union(
    base_fields: Vec<StructField>,
    union: &swc_ecma_ast::TsUnionType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(Vec<EnumVariant>, Option<String>)> {
    use super::unions::{extract_variant_info, find_discriminant_field};

    // Collect type literals for discriminant detection
    let type_lits: Vec<&swc_ecma_ast::TsTypeLit> = union
        .types
        .iter()
        .filter_map(|ty| {
            let inner = unwrap_parenthesized(ty.as_ref());
            match inner {
                TsType::TsTypeLit(lit) => Some(lit),
                _ => None,
            }
        })
        .collect();

    // Try discriminant detection (only if all variants are TsTypeLit)
    let serde_tag = if type_lits.len() == union.types.len() && type_lits.len() >= 2 {
        find_discriminant_field(&type_lits)
    } else {
        None
    };

    let mut variants = Vec::new();

    if let Some(ref discriminant_field) = serde_tag {
        // Discriminated union: extract variant info using the discriminant
        for type_lit in &type_lits {
            let (disc_value, variant_fields) =
                extract_variant_info(type_lit, discriminant_field, synthetic, reg)?;
            let merged = merge_fields(&base_fields, variant_fields);
            variants.push(EnumVariant {
                name: super::string_to_pascal_case(&disc_value),
                value: Some(EnumValue::Str(disc_value)),
                data: None,
                fields: merged,
            });
        }
    } else {
        // Non-discriminated: generate numbered variants
        for (idx, ty) in union.types.iter().enumerate() {
            let inner = unwrap_parenthesized(ty.as_ref());
            let variant_fields = extract_variant_fields(inner, synthetic, reg)?;
            let merged = merge_fields(&base_fields, variant_fields);
            variants.push(EnumVariant {
                name: format!("Variant{idx}"),
                value: None,
                data: None,
                fields: merged,
            });
        }
    }

    Ok((variants, serde_tag))
}

/// Tries to convert a type alias with an intersection type body into a struct.
///
/// Handles intersections of object type literals (`{ a: T } & { b: U }`) by merging
/// all fields into a single struct. Returns `None` if the type alias body is not
/// an intersection type.
pub(super) fn try_convert_intersection_type(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Option<Item>> {
    let intersection = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(i),
        ) => i,
        _ => return Ok(None),
    };

    let type_params = extract_type_params(decl.type_params.as_deref(), synthetic, reg);

    // Pre-process: unwrap parenthesized, remove empty type literals
    let members = preprocess_intersection_members(intersection);

    // Single member after filtering → simplify
    if members.is_empty() {
        return Ok(Some(Item::Struct {
            vis,
            name: sanitize_rust_type_name(&decl.id.sym),
            type_params,
            fields: vec![],
        }));
    }
    if members.len() == 1 {
        let single = members[0];
        // Check for identity mapped type: { [K in keyof T]: T[K] } → T
        if let TsType::TsMappedType(mapped) = single {
            if let Some(rust_type) = try_simplify_identity_mapped_type(mapped) {
                return Ok(Some(Item::TypeAlias {
                    vis,
                    name: sanitize_rust_type_name(&decl.id.sym),
                    type_params,
                    ty: rust_type,
                }));
            }
        }
        // TsTypeLit and TsTypeRef can be extracted into a struct — fall through to normal path.
        // Other types (mapped, conditional) → type alias via convert_ts_type.
        if !matches!(
            single,
            TsType::TsTypeLit(_) | TsType::TsTypeRef(_) | TsType::TsKeywordType(_)
        ) {
            let rust_type = convert_ts_type(single, synthetic, reg).unwrap_or(RustType::Any);
            return Ok(Some(Item::TypeAlias {
                vis,
                name: sanitize_rust_type_name(&decl.id.sym),
                type_params,
                ty: rust_type,
            }));
        }
    }

    // Check for union members → distribute intersection with union
    let (union_members, non_union_members): (Vec<&TsType>, Vec<&TsType>) =
        members.iter().partition(|ty| {
            matches!(
                ty,
                TsType::TsUnionOrIntersectionType(
                    swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(_)
                )
            )
        });

    if !union_members.is_empty() {
        // Extract base fields and methods from non-union members
        let (base_fields, methods) =
            extract_intersection_members(&non_union_members, synthetic, reg)?;

        // Distribute with the first union
        let first_union = match union_members[0] {
            TsType::TsUnionOrIntersectionType(
                swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
            ) => u,
            _ => unreachable!(),
        };

        // If there are additional unions, embed them as extra base fields
        let mut extra_base = base_fields;
        for &extra_union in &union_members[1..] {
            let rust_type = convert_ts_type(extra_union, synthetic, reg).unwrap_or(RustType::Any);
            extra_base.push(StructField {
                vis: None,
                name: format!("_{}", extra_base.len()),
                ty: rust_type,
            });
        }

        let (variants, serde_tag) =
            distribute_intersection_with_union(extra_base, first_union, synthetic, reg)?;

        let enum_name = sanitize_rust_type_name(&decl.id.sym);

        // If base members had method signatures, generate an impl block for the enum
        if !methods.is_empty() {
            synthetic.push_item(
                format!("{enum_name}__impl"),
                crate::pipeline::SyntheticTypeKind::ImplBlock,
                Item::Impl {
                    struct_name: enum_name.clone(),
                    type_params: type_params.clone(),
                    for_trait: None,
                    consts: vec![],
                    methods,
                },
            );
        }

        return Ok(Some(Item::Enum {
            vis,
            name: enum_name,
            serde_tag,
            variants,
        }));
    }

    let (fields, methods) = extract_intersection_members(&members, synthetic, reg)?;

    // If all intersection members are named type refs that resolve to method-only types
    // (traits), generate a supertrait composition instead of a struct.
    let trait_names: Vec<TraitRef> = members
        .iter()
        .filter_map(|ty| {
            if let TsType::TsTypeRef(type_ref) = *ty {
                if let swc_ecma_ast::TsEntityName::Ident(ident) = &type_ref.type_name {
                    let name = ident.sym.to_string();
                    if let Some(crate::registry::TypeDef::Struct {
                        fields: f,
                        methods: m,
                        ..
                    }) = reg.get(&name)
                    {
                        if f.is_empty() && !m.is_empty() {
                            let type_args = type_ref
                                .type_params
                                .as_ref()
                                .map(|ta| {
                                    ta.params
                                        .iter()
                                        .filter_map(|t| convert_ts_type(t, synthetic, reg).ok())
                                        .collect()
                                })
                                .unwrap_or_default();
                            return Some(TraitRef { name, type_args });
                        }
                    }
                }
            }
            None
        })
        .collect();

    if trait_names.len() == members.len() && !trait_names.is_empty() {
        // All members are method-only (trait-like) → supertrait composition
        return Ok(Some(Item::Trait {
            vis,
            name: sanitize_rust_type_name(&decl.id.sym),
            type_params: vec![],
            supertraits: trait_names,
            methods: vec![],
            associated_types: vec![],
        }));
    }

    let struct_name = sanitize_rust_type_name(&decl.id.sym);

    // If intersection contains method signatures, generate an impl block
    if !methods.is_empty() {
        synthetic.push_item(
            format!("{struct_name}__impl"),
            crate::pipeline::SyntheticTypeKind::ImplBlock,
            Item::Impl {
                struct_name: struct_name.clone(),
                type_params: type_params.clone(),
                for_trait: None,
                consts: vec![],
                methods,
            },
        );
    }

    Ok(Some(Item::Struct {
        vis,
        name: struct_name,
        type_params,
        fields,
    }))
}
