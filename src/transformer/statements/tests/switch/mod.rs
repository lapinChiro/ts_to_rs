//! Switch → `Stmt::Match` conversion tests grouped by semantic concern.
//!
//! The original `switch.rs` reached 1028 LOC (exceeding the 1000-line
//! threshold). Split into 6 cohesive sub-modules:
//!
//! - [`basic`] — single case / fallthrough / default / return / throw
//!   terminator shapes (numeric discriminant)
//! - [`string_discriminant`] — string discriminant → string literal
//!   patterns
//! - [`nonliteral`] — non-literal case values → guard rewrite (with
//!   mixed literal/non-literal + fallthrough variants)
//! - [`discriminated_union`] — DU (`switch(s.kind)`) → enum match on
//!   `&s` with field-access bindings (single / multi-field)
//! - [`misc`] — discriminant type propagation (string-enum case literal
//!   → enum variant pattern) and `default` source position → last arm
//! - [`i153_walker`] — `rewrite_nested_bare_break_in_stmts` walker unit
//!   tests (descent / non-descent exhaustive policy over IR Stmt
//!   variants)

use super::*;

mod basic;
mod discriminated_union;
mod i153_walker;
mod misc;
mod nonliteral;
mod string_discriminant;
