//! Type conversion from SWC TypeScript AST to IR.
//!
//! Converts TypeScript type declarations (interfaces, type aliases) and type
//! annotations into the IR representation. Synthetic types (union enums,
//! inline structs) are registered in [`SyntheticTypeRegistry`].

mod indexed_access;
mod interfaces;
mod intersections;
mod type_aliases;
mod unions;
mod utilities;

// Re-export public/pub(crate) API for external callers
pub use interfaces::{convert_interface, convert_interface_items};
pub use type_aliases::{convert_type_alias, convert_type_alias_items};
pub(crate) use unions::string_to_pascal_case;
pub(crate) use utilities::convert_property_signature;
pub use utilities::extract_type_params;

// Import all pub(super) items from submodules into this module's namespace.
// Submodules use `use super::*;` to access these.
use indexed_access::convert_indexed_access_type;
use interfaces::convert_method_signature;
use intersections::{
    convert_fn_type, convert_intersection_in_annotation, convert_type_lit_in_annotation,
    try_convert_intersection_type, try_simplify_identity_mapped_type,
};
use type_aliases::convert_conditional_type;
use unions::{
    convert_union_type, try_convert_discriminated_union, try_convert_general_union,
    try_convert_single_string_literal, try_convert_string_literal_union,
};
use utilities::{
    convert_unsupported_union_member, convert_utility_non_nullable, convert_utility_omit,
    convert_utility_partial, convert_utility_pick, convert_utility_required,
};

use anyhow::{anyhow, Result};
use swc_ecma_ast::{
    Expr, TsInterfaceDecl, TsKeywordTypeKind, TsMethodSignature, TsPropertySignature, TsType,
    TsTypeAliasDecl, TsTypeElement,
};

use crate::ir::{
    sanitize_field_name, EnumValue, EnumVariant, Item, Method, Param, RustType, StructField,
    TraitRef, TypeParam, Visibility,
};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::type_position::{wrap_trait_for_position, TypePosition};

/// Returns true if the keyword type is a nullable sentinel (`null`, `undefined`, `void`).
///
/// These types are filtered from union members and cause the union to be wrapped in `Option`.
fn is_nullable_keyword(kind: TsKeywordTypeKind) -> bool {
    matches!(
        kind,
        TsKeywordTypeKind::TsNullKeyword
            | TsKeywordTypeKind::TsUndefinedKeyword
            | TsKeywordTypeKind::TsVoidKeyword
    )
}

/// Converts a SWC [`TsType`] into an IR [`RustType`] with position-aware trait wrapping.
///
/// Combines [`convert_ts_type`] and [`wrap_trait_for_position`]: converts the type annotation
/// and then wraps trait types according to the specified position.
///
/// Use this instead of calling `convert_ts_type` + `wrap_trait_for_position` separately.
pub fn convert_type_for_position(
    ts_type: &TsType,
    position: TypePosition,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let ty = convert_ts_type(ts_type, synthetic, reg)?;
    Ok(wrap_trait_for_position(ty, position, reg))
}

/// Converts a SWC [`TsType`] into an IR [`RustType`].
///
/// # Supported conversions
///
/// - `string` -> `String`
/// - `number` -> `f64`
/// - `boolean` -> `bool`
/// - `T[]` -> `Vec<T>`
/// - `Array<T>` -> `Vec<T>`
/// - `T | null` / `T | undefined` -> `Option<T>`
/// - `[T, U, ...]` -> `(T, U, ...)`
///
/// # Errors
///
/// Returns an error for unsupported type constructs.
pub fn convert_ts_type(
    ts_type: &TsType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    match ts_type {
        TsType::TsKeywordType(kw) => match kw.kind {
            TsKeywordTypeKind::TsStringKeyword => Ok(RustType::String),
            TsKeywordTypeKind::TsNumberKeyword => Ok(RustType::F64),
            TsKeywordTypeKind::TsBooleanKeyword => Ok(RustType::Bool),
            TsKeywordTypeKind::TsVoidKeyword => Ok(RustType::Unit),
            TsKeywordTypeKind::TsAnyKeyword | TsKeywordTypeKind::TsUnknownKeyword => {
                Ok(RustType::Any)
            }
            TsKeywordTypeKind::TsNeverKeyword => Ok(RustType::Never),
            TsKeywordTypeKind::TsObjectKeyword => Ok(RustType::Named {
                name: "serde_json::Value".to_string(),
                type_args: vec![],
            }),
            TsKeywordTypeKind::TsUndefinedKeyword | TsKeywordTypeKind::TsNullKeyword => {
                Ok(RustType::Unit)
            }
            TsKeywordTypeKind::TsBigIntKeyword => Ok(RustType::Named {
                name: "i128".to_string(),
                type_args: vec![],
            }),
            other => Err(anyhow!("unsupported keyword type: {:?}", other)),
        },
        TsType::TsArrayType(arr) => {
            let inner = convert_ts_type(&arr.elem_type, synthetic, reg)?;
            Ok(RustType::Vec(Box::new(inner)))
        }
        TsType::TsTypeRef(type_ref) => convert_type_ref(type_ref, synthetic, reg),
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(union),
        ) => convert_union_type(union, synthetic, reg),
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
        ) => convert_intersection_in_annotation(intersection, synthetic, reg),
        TsType::TsParenthesizedType(paren) => convert_ts_type(&paren.type_ann, synthetic, reg),
        TsType::TsFnOrConstructorType(swc_ecma_ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            convert_fn_type(fn_type, synthetic, reg)
        }
        TsType::TsTupleType(tuple) => {
            let elems = tuple
                .elem_types
                .iter()
                .map(|elem| convert_ts_type(&elem.ty, synthetic, reg))
                .collect::<Result<Vec<_>>>()?;
            Ok(RustType::Tuple(elems))
        }
        TsType::TsIndexedAccessType(indexed) => {
            convert_indexed_access_type(indexed, synthetic, reg)
        }
        TsType::TsTypeLit(type_lit) => convert_type_lit_in_annotation(type_lit, synthetic, reg),
        TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(_) | swc_ecma_ast::TsLit::Tpl(_) => Ok(RustType::String),
            swc_ecma_ast::TsLit::Bool(_) => Ok(RustType::Bool),
            swc_ecma_ast::TsLit::Number(_) => Ok(RustType::F64),
            swc_ecma_ast::TsLit::BigInt(_) => Ok(RustType::Named {
                name: "i128".to_string(),
                type_args: vec![],
            }),
        },
        TsType::TsConditionalType(cond) => convert_conditional_type(cond, synthetic, reg),
        TsType::TsMappedType(mapped) => {
            // Try identity simplification: { [K in keyof T]: T[K] } → T
            if let Some(simplified) = try_simplify_identity_mapped_type(mapped) {
                return Ok(simplified);
            }
            // Fallback: treat mapped types as HashMap<String, V>
            let value_type = mapped
                .type_ann
                .as_ref()
                .map(|ann| convert_ts_type(ann, synthetic, reg))
                .transpose()?
                .unwrap_or(RustType::Any);
            Ok(RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, value_type],
            })
        }
        TsType::TsTypePredicate(_) => {
            // `x is Type` → bool (type guard predicates are booleans at runtime)
            Ok(RustType::Bool)
        }
        TsType::TsTypeOperator(op) => {
            use swc_ecma_ast::TsTypeOperatorOp;
            match op.op {
                // `readonly T[]` → strip readonly, convert inner type
                // Rust enforces immutability through variable bindings, not types
                TsTypeOperatorOp::ReadOnly => convert_ts_type(&op.type_ann, synthetic, reg),
                // `keyof typeof X` → string enum of const object's field names
                TsTypeOperatorOp::KeyOf => resolve_keyof_type(&op.type_ann, synthetic, reg),
                _ => Err(anyhow!("unsupported type operator: {:?}", op.op)),
            }
        }
        TsType::TsTypeQuery(query) => {
            // `typeof X` → look up X in registry; if found, use that type
            let name = match &query.expr_name {
                swc_ecma_ast::TsTypeQueryExpr::TsEntityName(swc_ecma_ast::TsEntityName::Ident(
                    ident,
                )) => ident.sym.to_string(),
                _ => return Err(anyhow!("unsupported typeof expression")),
            };
            match reg.get(&name) {
                Some(crate::registry::TypeDef::Function {
                    params,
                    return_type,
                    ..
                }) => {
                    let param_types: Vec<RustType> =
                        params.iter().map(|(_, t)| t.clone()).collect();
                    let ret = return_type.clone().unwrap_or(RustType::Unit);
                    Ok(RustType::Fn {
                        params: param_types,
                        return_type: Box::new(ret),
                    })
                }
                Some(crate::registry::TypeDef::Struct {
                    constructor: Some(ctors),
                    ..
                }) if !ctors.is_empty() => {
                    // typeof ClassName with constructor → constructor function type
                    // Select the constructor with the most parameters (most specific overload).
                    // Safety: ctors is non-empty due to the guard above, so max_by_key
                    // always returns Some. Use if-let to satisfy the no-unwrap rule.
                    if let Some(ctor) = ctors.iter().max_by_key(|c| c.params.len()) {
                        let param_types: Vec<RustType> =
                            ctor.params.iter().map(|(_, t)| t.clone()).collect();
                        Ok(RustType::Fn {
                            params: param_types,
                            return_type: Box::new(RustType::Named {
                                name: name.clone(),
                                type_args: vec![],
                            }),
                        })
                    } else {
                        Ok(RustType::Named {
                            name,
                            type_args: vec![],
                        })
                    }
                }
                Some(
                    crate::registry::TypeDef::Struct { .. } | crate::registry::TypeDef::Enum { .. },
                ) => {
                    // typeof StructName/EnumName → the type itself
                    Ok(RustType::Named {
                        name,
                        type_args: vec![],
                    })
                }
                Some(crate::registry::TypeDef::ConstValue { type_ref_name, .. }) => {
                    // typeof ConstVariable → redirect to referenced type if available
                    let resolved_name = type_ref_name.as_deref().unwrap_or(&name);
                    Ok(RustType::Named {
                        name: resolved_name.to_string(),
                        type_args: vec![],
                    })
                }
                _ => Err(anyhow!(
                    "unsupported type: TsTypeQuery for unknown identifier '{name}'"
                )),
            }
        }
        _ => Err(anyhow!("unsupported type: {:?}", ts_type)),
    }
}

/// Resolves `keyof T` type operator.
///
/// Currently supports `keyof typeof X` where X is a ConstValue with fields.
/// Returns a synthetic string enum of the field names.
fn resolve_keyof_type(
    type_ann: &TsType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    // keyof typeof X → get field names from ConstValue/Struct
    if let TsType::TsTypeQuery(query) = type_ann {
        if let swc_ecma_ast::TsTypeQueryExpr::TsEntityName(swc_ecma_ast::TsEntityName::Ident(
            ident,
        )) = &query.expr_name
        {
            let name = ident.sym.to_string();
            if let Some(typedef) = reg.get(&name) {
                if let Some(field_names) = typedef.field_names() {
                    let enum_name = synthetic
                        .register_string_literal_enum(&format!("{name}_key"), &field_names);
                    return Ok(RustType::Named {
                        name: enum_name,
                        type_args: vec![],
                    });
                }
            }
        }
    }

    Err(anyhow!("unsupported type operator: KeyOf"))
}

/// Converts a type reference like `Array<T>`.
fn convert_type_ref(
    type_ref: &swc_ecma_ast::TsTypeRef,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let name = match &type_ref.type_name {
        swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported qualified type name")),
    };

    match name.as_str() {
        "Array" => {
            let params = type_ref
                .type_params
                .as_ref()
                .ok_or_else(|| anyhow!("Array requires a type parameter"))?;
            if params.params.len() != 1 {
                return Err(anyhow!("Array expects exactly one type parameter"));
            }
            let inner = convert_ts_type(&params.params[0], synthetic, reg)?;
            Ok(RustType::Vec(Box::new(inner)))
        }
        "Record" => {
            let params = type_ref
                .type_params
                .as_ref()
                .ok_or_else(|| anyhow!("Record requires type parameters"))?;
            if params.params.len() != 2 {
                return Err(anyhow!("Record expects exactly two type parameters"));
            }
            let key = convert_ts_type(&params.params[0], synthetic, reg)?;
            let val = convert_ts_type(&params.params[1], synthetic, reg)?;
            Ok(RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![key, val],
            })
        }
        "Readonly" => {
            // Rust is immutable by default — Readonly<T> is just T
            let params = type_ref
                .type_params
                .as_ref()
                .ok_or_else(|| anyhow!("Readonly requires a type parameter"))?;
            if params.params.len() != 1 {
                return Err(anyhow!("Readonly expects exactly one type parameter"));
            }
            convert_ts_type(&params.params[0], synthetic, reg)
        }
        "Partial" => convert_utility_partial(type_ref, synthetic, reg),
        "Required" => convert_utility_required(type_ref, synthetic, reg),
        "Pick" => convert_utility_pick(type_ref, synthetic, reg),
        "Omit" => convert_utility_omit(type_ref, synthetic, reg),
        "NonNullable" => convert_utility_non_nullable(type_ref, synthetic, reg),
        // User-defined types: pass through as Named, with any generic type arguments
        other => {
            let type_args = match &type_ref.type_params {
                Some(params) => params
                    .params
                    .iter()
                    .map(|p| convert_ts_type(p, synthetic, reg))
                    .collect::<Result<Vec<_>>>()?,
                None => vec![],
            };
            Ok(RustType::Named {
                name: other.to_string(),
                type_args,
            })
        }
    }
}

#[cfg(test)]
mod tests;
