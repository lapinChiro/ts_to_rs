//! Intermediate Representation (IR) for Rust code generation.
//!
//! The IR sits between the SWC TypeScript AST and Rust source code generation.
//! It models the subset of Rust constructs needed for Phase 1 of ts_to_rs.
//!
//! このモジュールは facade であり、実体は以下のサブモジュールに分割されている:
//!
//! - [`types`][]: `RustType`, `TypeParam`, `TraitRef`, `Visibility`（型の形）
//! - [`naming`][]: `sanitize_field_name`, `sanitize_rust_type_name`, `string_to_pascal_case`,
//!   `camel_to_snake`（識別子変換ルール）
//! - [`item`][]: `Item`, `EnumValue`, `EnumVariant`, `StructField`, `Param`, `AssocConst`,
//!   `Method`（トップレベル宣言）
//! - [`stmt`][]: `Stmt`, `MatchArm`（関数本体の文）
//! - [`expr`][]: `Expr`, `CallTarget`, `BinOp`, `UnOp`, `ClosureBody`（式）
//! - [`pattern`][]: `Pattern`（構造化パターン）
//! - [`visit`] / [`fold`][]: IR ウォーカー/変換 trait
//! - `substitute`: 型パラメータ置換

pub mod expr;
pub mod item;
pub mod naming;
pub mod stmt;
pub mod types;

mod substitute;

pub mod fold;
pub mod pattern;
pub mod visit;

pub use expr::{
    BinOp, BuiltinVariant, CallTarget, ClosureBody, Expr, PrimitiveType, StdConst, UnOp,
    UserTypeRef,
};
pub use item::{AssocConst, EnumValue, EnumVariant, Item, Method, Param, StructField};
pub use naming::{
    camel_to_snake, sanitize_field_name, sanitize_rust_type_name, string_to_pascal_case,
};
pub use pattern::Pattern;
pub use stmt::{MatchArm, Stmt};
pub use types::{RustType, TraitRef, TypeParam, Visibility};

#[cfg(test)]
mod test_fixtures;

#[cfg(test)]
mod tests;
