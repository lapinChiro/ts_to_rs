//! Type conversion from SWC TypeScript AST to IR.
//!
//! Converts TypeScript type declarations (interfaces, type aliases) and type
//! annotations into the IR representation. Synthetic types (union enums,
//! inline structs) are registered in [`SyntheticTypeRegistry`].

// convert_ts_type が TsTypeInfo 経由の 2 ステップに移行したため、
// 旧ディスパッチから呼ばれていた関数は未使用となった。
// Phase 4（Registry 移行）完了後にクリーンアップする。
#[allow(dead_code)]
mod indexed_access;
mod interfaces;
#[allow(dead_code)]
mod intersections;
mod type_aliases;
#[allow(dead_code)]
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
use interfaces::convert_method_signature;
use intersections::try_convert_intersection_type;
use unions::{
    try_convert_discriminated_union, try_convert_general_union, try_convert_single_string_literal,
    try_convert_string_literal_union,
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
use crate::registry::{FieldDef, TypeDef, TypeRegistry};
use crate::transformer::type_position::{wrap_trait_for_position, TypePosition};

/// Rust prelude type names that would cause shadowing if used as user-defined type names.
///
/// Includes types, enum variants, and common std types that are in the prelude or
/// automatically imported. Using these as enum/struct names would shadow the standard
/// library definitions, causing compile errors or silent semantic changes.
const RUST_PRELUDE_TYPE_NAMES: &[&str] = &[
    // Core prelude types
    "Option", "Result", "String", "Vec", "Box",
    // Core prelude enum variants (used as value constructors)
    "Some", "None", "Ok", "Err", // Special keyword
    "Self",
];

/// Sanitizes a type name to avoid shadowing Rust prelude types.
///
/// If `name` matches a Rust prelude type name, prefixes it with "Ts"
/// (e.g., `Result` → `TsResult`). Otherwise returns the name unchanged.
pub(crate) fn sanitize_rust_type_name(name: &str) -> String {
    if RUST_PRELUDE_TYPE_NAMES.contains(&name) {
        format!("Ts{name}")
    } else {
        name.to_string()
    }
}

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
    let info = crate::ts_type_info::convert_to_ts_type_info(ts_type)?;
    crate::ts_type_info::resolve::resolve_ts_type(&info, reg, synthetic)
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
                name: sanitize_rust_type_name(other),
                type_args,
            })
        }
    }
}

#[cfg(test)]
mod tests;
