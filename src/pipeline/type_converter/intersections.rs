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
/// Each SWC member is converted to TsTypeInfo, then resolved via resolve functions.
/// Handles TypeLiteral (fields + methods), TypeRef (registry lookup), keyword (skip),
/// and fallback (resolve_ts_type).
fn extract_intersection_members(
    members: &[&TsType],
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(Vec<StructField>, Vec<Method>)> {
    use crate::ts_type_info::resolve::intersection::{
        resolve_method_info, resolve_type_literal_fields,
    };

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    for (i, ty) in members.iter().enumerate() {
        let info = crate::ts_type_info::convert_to_ts_type_info(ty)?;
        match &info {
            crate::ts_type_info::TsTypeInfo::TypeLiteral(lit) => {
                let lit_fields = resolve_type_literal_fields(lit, reg, synthetic)?;
                for field in lit_fields {
                    if fields.iter().any(|f: &StructField| f.name == field.name) {
                        return Err(anyhow!(
                            "duplicate field '{}' in intersection type",
                            field.name
                        ));
                    }
                    fields.push(field);
                }
                for method_info in &lit.methods {
                    methods.push(resolve_method_info(method_info, reg, synthetic)?);
                }
            }
            crate::ts_type_info::TsTypeInfo::TypeRef { name, .. } => {
                if let Some(crate::registry::TypeDef::Struct {
                    fields: resolved_fields,
                    ..
                }) = reg.get(name)
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
                    let rust_type =
                        crate::ts_type_info::resolve::resolve_ts_type(&info, reg, synthetic)?;
                    fields.push(StructField {
                        vis: None,
                        name: format!("_{i}"),
                        ty: rust_type,
                    });
                }
            }
            // Skip keyword types in intersections (e.g., `string & {}` → use object fields only).
            crate::ts_type_info::TsTypeInfo::String
            | crate::ts_type_info::TsTypeInfo::Number
            | crate::ts_type_info::TsTypeInfo::Boolean
            | crate::ts_type_info::TsTypeInfo::Any
            | crate::ts_type_info::TsTypeInfo::Unknown
            | crate::ts_type_info::TsTypeInfo::Object
            | crate::ts_type_info::TsTypeInfo::Void
            | crate::ts_type_info::TsTypeInfo::Null
            | crate::ts_type_info::TsTypeInfo::Undefined
            | crate::ts_type_info::TsTypeInfo::Never
            | crate::ts_type_info::TsTypeInfo::BigInt
            | crate::ts_type_info::TsTypeInfo::Symbol => continue,
            _ => {
                let rust_type =
                    crate::ts_type_info::resolve::resolve_ts_type(&info, reg, synthetic)
                        .unwrap_or(RustType::Any);
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

/// Distributes an intersection with a union member: `A & (B | C)` → enum with A's fields in each variant.
///
/// Returns `(variants, serde_tag)` where `serde_tag` is set if a discriminant field is detected.
fn distribute_intersection_with_union(
    base_fields: Vec<StructField>,
    union_info: &[crate::ts_type_info::TsTypeInfo],
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(Vec<EnumVariant>, Option<String>)> {
    let serde_tag = crate::ts_type_info::resolve::intersection::find_discriminant_field(union_info);

    let mut variants = Vec::new();

    if let Some(ref discriminant_field) = serde_tag {
        for variant in union_info {
            let (raw_value, pascal_name, variant_fields) =
                crate::ts_type_info::resolve::intersection::extract_discriminated_variant(
                    variant,
                    discriminant_field,
                    reg,
                    synthetic,
                )?;
            let merged = merge_fields(&base_fields, variant_fields);
            variants.push(EnumVariant {
                name: pascal_name,
                value: Some(EnumValue::Str(raw_value)),
                data: None,
                fields: merged,
            });
        }
    } else {
        for (idx, variant) in union_info.iter().enumerate() {
            let variant_fields =
                crate::ts_type_info::resolve::intersection::extract_variant_fields(
                    variant, reg, synthetic,
                )?;
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

        // Distribute with the first union — convert SWC union to TsTypeInfo
        let first_union_info = match crate::ts_type_info::convert_to_ts_type_info(union_members[0])?
        {
            crate::ts_type_info::TsTypeInfo::Union(members) => members,
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
            distribute_intersection_with_union(extra_base, &first_union_info, synthetic, reg)?;

        let enum_name = sanitize_rust_type_name(&decl.id.sym);

        // If base members had method signatures, generate an impl block for the enum
        if !methods.is_empty() {
            synthetic.push_item(
                format!("{enum_name}Impl"),
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
            format!("{struct_name}Impl"),
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
