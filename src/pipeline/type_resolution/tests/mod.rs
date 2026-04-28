//! Unit tests for `FileTypeResolution`.
//!
//! Tests are split by responsibility (Iteration 2026-04-29 file-line refactor):
//! - [`basic_queries`]: leaf queries (`expr_type` / `expected_type` / `is_mutable` /
//!   `is_du_field_binding` / `emission_hint`) + foundational `narrowed_type` /
//!   `is_var_closure_reassigned` invariants (innermost scope / variant filtering /
//!   enclosing_fn_body boundary).
//! - [`narrowing_suppression`]: I-177-D trigger-kind-based suppression dispatch matrix
//!   (5 Primary cells × 5 EarlyReturnComplement cells = 10 cells × twin-assertion
//!   structural lock-in).
//! - [`canonical_primitives`]: I-177-B canonical leaf type resolution helpers
//!   (`resolve_var_type` / `resolve_expr_type` precedence: narrow ≻ expr_type ≻ None).

mod basic_queries;
mod canonical_primitives;
mod narrowing_suppression;
