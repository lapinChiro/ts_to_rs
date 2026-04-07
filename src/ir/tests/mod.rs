//! IR のユニットテスト。カテゴリ別に分割している:
//!
//! - [`types_tests`][]: `RustType` / `Visibility` / `wrap_optional`
//! - [`naming_tests`][]: `sanitize_*` / `string_to_pascal_case` / `camel_to_snake`
//! - [`item_tests`][]: `Item` / `EnumVariant` / `canonical_name`
//! - [`stmt_tests`][]: `Stmt` 各 variant
//! - [`expr_tests`][]: `Expr` / `BinOp` / `is_trivially_pure` / `is_copy_literal` / `CallTarget`
//! - [`substitute_tests`][]: 型パラメータ置換

mod expr_tests;
mod item_tests;
mod naming_tests;
mod stmt_tests;
mod substitute_tests;
mod types_tests;
