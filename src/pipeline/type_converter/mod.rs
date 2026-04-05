//! Type conversion from SWC TypeScript AST to IR.
//!
//! Converts TypeScript type declarations (interfaces, type aliases) and type
//! annotations into the IR representation. Synthetic types (union enums,
//! inline structs) are registered in [`SyntheticTypeRegistry`].

// 型解決は convert_ts_type → TsTypeInfo → resolve 経由に統一済み（Batch 4d-B, 4d-C）。
// declaration 変換（unions, intersections）の型解決も TsTypeInfo 経由に移行完了。
// interface 変換（interfaces）は SWC AST 依存が残存（別 PRD で対応予定）。
mod interfaces;
mod intersections;
mod type_aliases;
mod unions;
mod utilities;

// Re-export public/pub(crate) API for external callers
pub use interfaces::{convert_interface, convert_interface_items};
pub use type_aliases::{convert_type_alias, convert_type_alias_items};
pub(crate) use utilities::convert_property_signature;
pub use utilities::extract_type_params;

// Import all pub(super) items from submodules into this module's namespace.
// Submodules use `use super::*;` to access these.
use intersections::try_convert_intersection_type;
use unions::{
    try_convert_discriminated_union, try_convert_general_union, try_convert_single_string_literal,
    try_convert_string_literal_union,
};
use utilities::convert_unsupported_union_member;

use anyhow::{anyhow, Result};
use swc_ecma_ast::{
    Expr, TsInterfaceDecl, TsKeywordTypeKind, TsMethodSignature, TsPropertySignature, TsType,
    TsTypeAliasDecl, TsTypeElement,
};

use crate::ir::{
    sanitize_field_name, sanitize_rust_type_name, string_to_pascal_case, EnumValue, EnumVariant,
    Item, Method, Param, RustType, StructField, TraitRef, TypeParam, Visibility,
};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::type_position::{wrap_trait_for_position, TypePosition};

/// Applies monomorphization substitutions to IR items using `Item::substitute`.
pub(super) fn apply_mono_subs_to_items(
    items: Vec<Item>,
    subs: &std::collections::HashMap<String, RustType>,
) -> Vec<Item> {
    if subs.is_empty() {
        return items;
    }
    items.iter().map(|item| item.substitute(subs)).collect()
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

#[cfg(test)]
mod tests;
