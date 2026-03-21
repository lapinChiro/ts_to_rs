//! Type conversion — re-exports from pipeline::type_converter.
//!
//! The type conversion logic has been moved to `crate::pipeline::type_converter`.
//! This module re-exports everything for backward compatibility.

#[cfg(test)]
mod tests;

pub use crate::pipeline::type_converter::*;

// Re-export types used by tests and downstream consumers that were previously
// available through this module's `use` statements.
pub use crate::ir::{
    EnumValue, EnumVariant, Item, Method, Param, RustType, StructField, TypeParam, Visibility,
};
pub use crate::pipeline::SyntheticTypeRegistry;
pub use crate::registry::{TypeDef, TypeRegistry};
pub use swc_ecma_ast::{
    Expr, TsInterfaceDecl, TsKeywordTypeKind, TsMethodSignature, TsPropertySignature, TsType,
    TsTypeAliasDecl, TsTypeElement,
};
