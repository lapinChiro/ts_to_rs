//! Integration tests for `build_registry` grouped by semantic concern.
//!
//! The original `build_registry.rs` reached 1123 LOC (exceeding the 1000
//! threshold). Split into 6 cohesive sub-modules by test subject:
//!
//! - [`basic_decls`] — interface / type alias / enum / export / optional /
//!   empty / forward-reference / intersection-merge
//! - [`trait_detection`] — `is_trait_type` classification + method return
//!   type storage
//! - [`const_values`] — `as const` / typed const / let / un-annotated var
//! - [`class`] — class constructor + field + method registration
//! - [`type_alias`] — `type X = Base` resolve paths (type ref / utility /
//!   pick / intersection variants / mapped / index sig / method-bearing)
//! - [`callable_interface`] — callable interface call-signature
//!   classification + forward-declaration resolution (Pass 2b)
//!
//! All sub-modules share the parent's `use super::*;` imports
//! (`HashMap`, `FieldDef`, `MethodSignature`, `TypeDef`, `TypeRegistry`,
//! `RustType`, `parse_typescript`, `build_registry*`) via their own
//! `use super::*;` re-import.

use super::*;

mod basic_decls;
mod callable_interface;
mod class;
mod const_values;
mod trait_detection;
mod type_alias;
