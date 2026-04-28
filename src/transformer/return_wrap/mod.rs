//! Return value wrapping for divergent callable interface return types.
//!
//! When a callable interface has overloads with different return types,
//! the inner function returns a synthetic union enum. Each return expression
//! in the arrow body must be wrapped in the appropriate enum variant.
//!
//! Phase 7 (P7.0) で inner fn body の return wrap に使用。
//!
//! ## Submodules (Iteration 2026-04-29 file-line refactor)
//!
//! - [`context`]: `ReturnWrapContext` struct + builders + variant lookup helpers
//!   (`variant_for` / `unique_option_variant`)
//! - [`wrapping`]: leaf wrapping logic (`wrap_leaf`) + private inference / coercion helpers
//!   (`infer_variant_from_expr` / `wrap_in_variant` / `coerce_string_literal` / `is_none_expr`)
//! - [`collection`]: SWC AST walking (`collect_*_return_leaf_types`) + `ReturnLeafType` struct

mod collection;
mod context;
mod wrapping;

pub(crate) use collection::{
    collect_return_leaf_types, collect_stmts_return_leaf_types, ReturnLeafType,
};
pub(crate) use context::{
    build_return_wrap_context, build_return_wrap_context_from_enum, ReturnWrapContext,
};
pub(crate) use wrapping::wrap_leaf;
