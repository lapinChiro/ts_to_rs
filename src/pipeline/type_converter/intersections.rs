use super::*;

/// Extracts fields and methods from an intersection type's member types.
///
/// Shared logic for both type alias intersections and annotation-position intersections.
/// Handles `TsTypeLit` (property sigs → fields, method sigs → methods),
/// `TsTypeRef` (resolved from registry or embedded), and `TsKeywordType` (skipped).
fn extract_intersection_members(
    intersection: &swc_ecma_ast::TsIntersectionType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<(Vec<StructField>, Vec<Method>)> {
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    for (i, ty) in intersection.types.iter().enumerate() {
        match ty.as_ref() {
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
                    for (name, ty) in resolved_fields {
                        let sanitized = sanitize_field_name(name);
                        if fields.iter().any(|f: &StructField| f.name == sanitized) {
                            return Err(anyhow!("duplicate field '{}' in intersection type", name));
                        }
                        fields.push(StructField {
                            vis: None,
                            name: sanitized,
                            ty: ty.clone(),
                        });
                    }
                } else {
                    // Unresolved type reference — embed as a named field
                    let rust_type = convert_type_ref(type_ref, synthetic, reg)?;
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
            _ => {
                return Err(anyhow!("unsupported intersection member type"));
            }
        }
    }
    Ok((fields, methods))
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

    let (fields, methods) = extract_intersection_members(intersection, synthetic, reg)?;

    let type_params = extract_type_params(decl.type_params.as_deref(), synthetic, reg);

    // If all intersection members are named type refs that resolve to method-only types
    // (traits), generate a supertrait composition instead of a struct.
    let trait_names: Vec<TraitRef> = intersection
        .types
        .iter()
        .filter_map(|ty| {
            if let TsType::TsTypeRef(type_ref) = ty.as_ref() {
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

    if trait_names.len() == intersection.types.len() && !trait_names.is_empty() {
        // All members are method-only (trait-like) → supertrait composition
        return Ok(Some(Item::Trait {
            vis,
            name: decl.id.sym.to_string(),
            type_params: vec![],
            supertraits: trait_names,
            methods: vec![],
            associated_types: vec![],
        }));
    }

    let struct_name = decl.id.sym.to_string();

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

/// Converts a type literal in annotation position into a synthetic merged struct.
pub(super) fn convert_type_lit_in_annotation(
    type_lit: &swc_ecma_ast::TsTypeLit,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let mut fields = Vec::new();
    for member in &type_lit.members {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                fields.push(convert_property_signature(prop, synthetic, reg)?);
            }
            TsTypeElement::TsIndexSignature(idx) => {
                // { [key: string]: T } → HashMap<String, T>
                if let Some(type_ann) = &idx.type_ann {
                    let value_type = convert_ts_type(&type_ann.type_ann, synthetic, reg)?;
                    return Ok(RustType::Named {
                        name: "HashMap".to_string(),
                        type_args: vec![RustType::String, value_type],
                    });
                }
                return Err(anyhow!(
                    "unsupported index signature without type annotation"
                ));
            }
            _ => return Err(anyhow!("unsupported type literal member")),
        }
    }
    // Use register_inline_struct for deduplication (same field structure → same name)
    let field_pairs: Vec<(String, RustType)> = fields
        .iter()
        .map(|f| (f.name.clone(), f.ty.clone()))
        .collect();
    let struct_name = synthetic.register_inline_struct(&field_pairs);
    Ok(RustType::Named {
        name: struct_name,
        type_args: vec![],
    })
}

/// Converts an intersection type in annotation position into a synthetic merged struct.
///
/// Reuses the same merging logic as `try_convert_intersection_type` (type alias position),
/// but generates a synthetic name since no explicit name is available.
pub(super) fn convert_intersection_in_annotation(
    intersection: &swc_ecma_ast::TsIntersectionType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let (fields, methods) = extract_intersection_members(intersection, synthetic, reg)?;

    let struct_name = synthetic.generate_name("Intersection");
    synthetic.push_item(
        struct_name.clone(),
        crate::pipeline::SyntheticTypeKind::InlineStruct,
        Item::Struct {
            vis: Visibility::Public,
            name: struct_name.clone(),
            type_params: vec![],
            fields,
        },
    );

    // If intersection contains method signatures, generate an impl block
    if !methods.is_empty() {
        synthetic.push_item(
            format!("{struct_name}__impl"),
            crate::pipeline::SyntheticTypeKind::ImplBlock,
            Item::Impl {
                struct_name: struct_name.clone(),
                type_params: vec![],
                for_trait: None,
                consts: vec![],
                methods,
            },
        );
    }

    Ok(RustType::Named {
        name: struct_name,
        type_args: vec![],
    })
}

/// Converts a TS function type (`(x: number) => string`) into `RustType::Fn`.
pub(super) fn convert_fn_type(
    fn_type: &swc_ecma_ast::TsFnType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params = fn_type
        .params
        .iter()
        .map(|p| {
            let type_ann = match p {
                swc_ecma_ast::TsFnParam::Ident(ident) => ident
                    .type_ann
                    .as_ref()
                    .ok_or_else(|| anyhow!("function type parameter has no type annotation"))?,
                _ => return Err(anyhow!("unsupported function type parameter pattern")),
            };
            convert_ts_type(&type_ann.type_ann, synthetic, reg)
        })
        .collect::<Result<Vec<_>>>()?;

    let return_type = convert_ts_type(&fn_type.type_ann.type_ann, synthetic, reg)?;

    Ok(RustType::Fn {
        params,
        return_type: Box::new(return_type),
    })
}

/// Converts a TS indexed access type (`T['Key']`) into a Rust type.
///
/// Resolution strategy:
/// 1. Resolve the base type name (supports TypeRef, parenthesized, typeof)
/// 2. For string literal keys: look up the actual field type in the registry if available,
///    otherwise produce `T::Key` (associated type syntax)
/// 3. For non-string keys or unresolvable base types: return error
pub(super) fn convert_indexed_access_type(
    indexed: &swc_ecma_ast::TsIndexedAccessType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let obj_name = extract_indexed_access_base_name(&indexed.obj_type, synthetic, reg)
        .ok_or_else(|| anyhow!("unsupported indexed access base type"))?;

    // [number] key: extract element types from const arrays
    if is_number_keyword_type(&indexed.index_type) {
        return resolve_number_index(&obj_name, synthetic, reg);
    }

    // [keyof typeof X] key: extract value type union from const objects
    if extract_keyof_typeof_name(&indexed.index_type, reg).is_some() {
        return resolve_keyof_typeof_index(&obj_name, synthetic, reg);
    }

    // String literal key
    let key = match indexed.index_type.as_ref() {
        TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(s) => s.value.to_string_lossy().into_owned(),
            _ => {
                return Err(anyhow!(
                    "unsupported indexed access key: only string literals are supported"
                ))
            }
        },
        _ => {
            return Err(anyhow!(
                "unsupported indexed access key: only string literals are supported"
            ))
        }
    };

    // Try registry lookup for the exact field type
    if let Some(field_ty) = lookup_field_type(&obj_name, &key, reg) {
        return Ok(field_ty);
    }

    Ok(RustType::Named {
        name: format!("{obj_name}::{key}"),
        type_args: vec![],
    })
}

/// Checks if a type is the `number` keyword type.
fn is_number_keyword_type(ts_type: &TsType) -> bool {
    matches!(
        ts_type,
        TsType::TsKeywordType(swc_ecma_ast::TsKeywordType {
            kind: swc_ecma_ast::TsKeywordTypeKind::TsNumberKeyword,
            ..
        })
    )
}

/// Extracts the name from a `keyof typeof X` type expression.
///
/// Returns `Some(name)` if the type is `TsTypeOperator(KeyOf, TsTypeQuery(Ident(name)))`.
fn extract_keyof_typeof_name(ts_type: &TsType, reg: &TypeRegistry) -> Option<String> {
    if let TsType::TsTypeOperator(op) = ts_type {
        if op.op == swc_ecma_ast::TsTypeOperatorOp::KeyOf {
            if let TsType::TsTypeQuery(query) = op.type_ann.as_ref() {
                if let swc_ecma_ast::TsTypeQueryExpr::TsEntityName(
                    swc_ecma_ast::TsEntityName::Ident(ident),
                ) = &query.expr_name
                {
                    let name = ident.sym.to_string();
                    // Verify the name exists in registry
                    if reg.get(&name).is_some() {
                        return Some(name);
                    }
                }
            }
        }
    }
    None
}

/// Resolves `(typeof X)[number]` — extracts element type from const arrays.
fn resolve_number_index(
    obj_name: &str,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    match reg.get(obj_name) {
        Some(crate::registry::TypeDef::ConstValue { elements, .. }) if !elements.is_empty() => {
            // Check if all elements have string values → generate string enum
            let string_values: Vec<String> = elements
                .iter()
                .filter_map(|e| e.string_literal_value.clone())
                .collect();
            if string_values.len() == elements.len() {
                let enum_name = synthetic.register_string_literal_enum(obj_name, &string_values);
                return Ok(RustType::Named {
                    name: enum_name,
                    type_args: vec![],
                });
            }
            // Non-string elements → collect unique element types
            let mut unique_types: Vec<RustType> = Vec::new();
            for elem in elements {
                if !unique_types.contains(&elem.ty) {
                    unique_types.push(elem.ty.clone());
                }
            }
            if let [single] = unique_types.as_slice() {
                return Ok(single.clone());
            }
            let name = synthetic.register_union(&unique_types);
            Ok(RustType::Named {
                name,
                type_args: vec![],
            })
        }
        _ => Err(anyhow!(
            "unsupported indexed access: [number] key requires a const array"
        )),
    }
}

/// Resolves `(typeof X)[keyof typeof Y]` — extracts value type union from const objects.
fn resolve_keyof_typeof_index(
    obj_name: &str,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let typedef = reg
        .get(obj_name)
        .ok_or_else(|| anyhow!("unsupported indexed access: base type '{obj_name}' not found"))?;

    // Check if all fields have string literal values → generate string enum
    if let Some(string_values) = typedef.all_string_literal_field_values() {
        let enum_name = synthetic.register_string_literal_enum(obj_name, &string_values);
        return Ok(RustType::Named {
            name: enum_name,
            type_args: vec![],
        });
    }

    // Collect unique value types
    if let Some(value_types) = typedef.unique_field_types() {
        if let [single] = value_types.as_slice() {
            return Ok(single.clone());
        }
        let name = synthetic.register_union(&value_types);
        return Ok(RustType::Named {
            name,
            type_args: vec![],
        });
    }

    Err(anyhow!(
        "unsupported indexed access: [keyof typeof] requires a const object type"
    ))
}

/// Looks up a field type from the registry by struct name and field name.
fn lookup_field_type(type_name: &str, field_name: &str, reg: &TypeRegistry) -> Option<RustType> {
    match reg.get(type_name)? {
        crate::registry::TypeDef::Struct { fields, .. } => fields
            .iter()
            .find(|(n, _)| n == field_name)
            .map(|(_, t)| t.clone()),
        crate::registry::TypeDef::ConstValue { fields, .. } => fields
            .iter()
            .find(|f| f.name == field_name)
            .map(|f| f.ty.clone()),
        _ => None,
    }
}

/// Extracts the base type name from an indexed access type's object type.
///
/// Handles `TsTypeRef(Ident)`, `TsParenthesizedType`, and `TsTypeQuery` (typeof).
/// Returns `None` if the base type cannot be resolved to a simple name.
fn extract_indexed_access_base_name(
    obj_type: &TsType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Option<String> {
    match obj_type {
        TsType::TsTypeRef(type_ref) => match &type_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => Some(ident.sym.to_string()),
            _ => None,
        },
        TsType::TsParenthesizedType(paren) => {
            extract_indexed_access_base_name(&paren.type_ann, synthetic, reg)
        }
        TsType::TsTypeQuery(_) => {
            let resolved = convert_ts_type(obj_type, synthetic, reg).ok()?;
            match resolved {
                RustType::Named { name, .. } => Some(name),
                _ => None,
            }
        }
        _ => None,
    }
}
