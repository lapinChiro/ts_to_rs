use super::*;

/// Converts a TS indexed access type (`T['Key']`) into a Rust type.
///
/// Resolution strategy (in priority order):
/// 1. Resolve the base type name (supports TypeRef, parenthesized, typeof, nested indexed access)
/// 2. `[number]` key → extract element types from const arrays, or fallback to `Any`
/// 3. `[keyof typeof X]` key → extract value type union from const objects
/// 4. `[keyof T]` key → union of all field value types (generics erasure)
/// 5. String literal key → look up the exact field type in the registry
/// 6. Numeric literal key (e.g., `[2]`) → `Any`
/// 7. Type parameter key (e.g., `T[K]`) → union of all field types (generics erasure)
/// 8. Unresolvable base types → `Any`
pub(super) fn convert_indexed_access_type(
    indexed: &swc_ecma_ast::TsIndexedAccessType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    // Resolve base type name: try simple extraction first, then recursive conversion
    let obj_name = match extract_indexed_access_base_name(&indexed.obj_type) {
        Some(name) => name,
        None => {
            // Complex obj_type (typeof, nested indexed access, mapped type, etc.)
            match convert_ts_type(&indexed.obj_type, synthetic, reg) {
                Ok(RustType::Named { name, .. }) => name,
                Ok(_) => return Ok(RustType::Any),
                Err(_) => return Ok(RustType::Any),
            }
        }
    };

    // [number] key: extract element types from const arrays, fallback to Any for non-const
    if is_number_keyword_type(&indexed.index_type) {
        return resolve_number_index(&obj_name, synthetic, reg).or_else(|_| Ok(RustType::Any));
    }

    // [keyof typeof X] key: extract value type union from const objects
    if extract_keyof_typeof_name(&indexed.index_type, reg).is_some() {
        return resolve_keyof_typeof_index(&obj_name, synthetic, reg);
    }

    // [keyof T] key (without typeof): resolve to union of all field value types
    if is_keyof_type(&indexed.index_type) {
        return resolve_type_param_indexed_access(&obj_name, reg, synthetic);
    }

    // Literal key (string or numeric)
    if let TsType::TsLitType(lit) = indexed.index_type.as_ref() {
        match &lit.lit {
            swc_ecma_ast::TsLit::Str(s) => {
                let key = s.value.to_string_lossy().into_owned();
                if let Some(field_ty) = lookup_field_type(&obj_name, &key, reg) {
                    return Ok(field_ty);
                }
                return Ok(RustType::Named {
                    name: format!("{obj_name}::{key}"),
                    type_args: vec![],
                });
            }
            // Numeric literal key (e.g., [2]) → position-based access, fallback to Any
            swc_ecma_ast::TsLit::Number(_) => return Ok(RustType::Any),
            _ => {}
        }
    }

    // Type parameter key: index_type is a TsTypeRef not found in registry
    // (e.g., T[K] where K extends keyof T)
    if let TsType::TsTypeRef(type_ref) = indexed.index_type.as_ref() {
        if let swc_ecma_ast::TsEntityName::Ident(ident) = &type_ref.type_name {
            let name = ident.sym.to_string();
            if reg.get(&name).is_none() {
                // Type parameter key → generics erasure: return union of all field types
                return resolve_type_param_indexed_access(&obj_name, reg, synthetic);
            }
        }
    }

    Err(anyhow!("unsupported indexed access key type"))
}

/// Checks if a type is a `keyof T` type operator (without typeof).
fn is_keyof_type(ts_type: &TsType) -> bool {
    matches!(
        ts_type,
        TsType::TsTypeOperator(op) if op.op == swc_ecma_ast::TsTypeOperatorOp::KeyOf
    )
}

/// Resolves a type parameter indexed access via generics erasure.
///
/// When `T[K]` where K is a type parameter with `keyof T` constraint,
/// returns the union of all field value types of T. If T is not in the
/// registry (also a type parameter), returns `Any`.
fn resolve_type_param_indexed_access(
    obj_name: &str,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<RustType> {
    if let Some(typedef) = reg.get(obj_name) {
        if let Some(field_types) = typedef.unique_field_types() {
            return match field_types.len() {
                0 => Ok(RustType::Any),
                1 => Ok(field_types[0].clone()),
                _ => {
                    let name = synthetic.register_union(&field_types);
                    Ok(RustType::Named {
                        name,
                        type_args: vec![],
                    })
                }
            };
        }
    }
    // Base type not in registry (type parameter itself) → Any
    Ok(RustType::Any)
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
            .find(|f| f.name == field_name)
            .map(|f| f.ty.clone()),
        crate::registry::TypeDef::ConstValue { fields, .. } => fields
            .iter()
            .find(|f| f.name == field_name)
            .map(|f| f.ty.clone()),
        _ => None,
    }
}

/// Extracts the base type name from simple indexed access object types.
///
/// Handles only simple patterns: `TsTypeRef(Ident)` and `TsParenthesizedType`.
/// Complex patterns (typeof, nested indexed access, mapped types, etc.) are handled
/// by the fallback in `convert_indexed_access_type` to avoid double evaluation.
fn extract_indexed_access_base_name(obj_type: &TsType) -> Option<String> {
    match obj_type {
        TsType::TsTypeRef(type_ref) => match &type_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => Some(ident.sym.to_string()),
            _ => None,
        },
        TsType::TsParenthesizedType(paren) => extract_indexed_access_base_name(&paren.type_ann),
        _ => None,
    }
}
