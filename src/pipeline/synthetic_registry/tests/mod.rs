//! Unit tests for `SyntheticTypeRegistry`.
//!
//! Tests are split by responsibility (Iteration 2026-04-29 file-line refactor):
//! - [`dedup`]: union / inline struct / intersection struct / intersection enum
//!   registration + dedup invariants (idempotency, order independence, signature
//!   normalization).
//! - [`naming`]: [`super::variant_name_for_type`] + [`super::to_pascal_case`] format
//!   tests covering Named (with/without type args + paths), DynTrait, Tuple, Result,
//!   Fn variants.
//! - [`scope`]: type param scope push / restore / `is_in_type_param_scope` query +
//!   walker-based type-param detection from field / member / variant types.
//! - [`ops`]: bare registry operations — `get`, `all_items`, `generate_name`,
//!   `merge`, `register_any_enum`, Item-shape verification (Enum / Struct).
//! - [`integration`]: cross-origin dedup, fork+merge round-trip, type inheritance
//!   from parent (I-177-E invariant) — verifies subsystem boundaries.
//! - [`helpers`]: DRY-extracted [`StructField`](crate::ir::StructField) builder
//!   ([`helpers::pub_field`]) used across all sub-modules.

mod dedup;
mod helpers;
mod integration;
mod naming;
mod ops;
mod scope;
