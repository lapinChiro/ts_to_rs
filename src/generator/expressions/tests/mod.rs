//! Unit tests for `generate_expr` split by `Expr` variant kind.
//!
//! The original `tests.rs` reached 1019 LOC (exceeding the 1000-line
//! threshold). Grouped by the expression-rendering concern under test:
//!
//! - [`literals`] — pure literal rendering (`NumberLit`, `BoolLit`,
//!   `StringLit`, `Ident`, `Tuple`, `Unit`, `IntLit`, `Deref` / `Ref`)
//!   plus `escape_rust_string` unit tests
//! - [`binary`] — `BinaryOp` rendering including bitwise `i64`/`u32`
//!   cast rules and nested mixing with arithmetic
//! - [`access`] — `FieldAccess` / `MethodCall` / `Index` / `Await`
//!   paren-adding rules driven by receiver shape, method-call receiver
//!   classification, `StructInit` update syntax, and `escape_ident`
//!   reserved-word handling
//! - [`format_fncall`] — `FormatMacro` + `FnCall` per `CallTarget`
//!   variant (Free / UserAssocFn / ExternalPath / Super / UserTupleCtor
//!   / UserEnumVariantCtor / BuiltinVariant), plus the I-378 structured
//!   value Expr variants (`EnumVariant`, `PrimitiveAssocConst`,
//!   `StdConst`)
//! - [`closure`] — `Expr::Closure` (expr vs block body, return type
//!   brace rule, param annotation, no-params)
//! - [`control_expr`] — `If` / `Match` / `Vec` / `Block` /
//!   `RuntimeTypeof`
//! - [`macro_call`] — `MacroCall` rendering (`println!`, `eprintln!`,
//!   `{}` vs `{:?}` based on `use_debug`)

use super::*;

mod access;
mod binary;
mod closure;
mod control_expr;
mod format_fncall;
mod literals;
mod macro_call;
