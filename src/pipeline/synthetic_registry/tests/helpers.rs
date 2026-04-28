//! Shared test builders to consolidate boilerplate (DRY refactor 2026-04-29).
//!
//! `StructField` literal construction (`StructField { vis: Some(Visibility::Public),
//! name: ..., ty: ... }`) is the most repeated 5-line block in the test suite (used in
//! intersection struct / intersection enum / type param walker tests). Centralized
//! here so the visibility / name conversion knowledge stays in one place.

use crate::ir::{RustType, StructField, Visibility};

/// Builds a public [`StructField`] from a string-literal name + type.
pub(super) fn pub_field(name: &str, ty: RustType) -> StructField {
    StructField {
        vis: Some(Visibility::Public),
        name: name.to_string(),
        ty,
    }
}
