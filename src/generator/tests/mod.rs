//! Unit tests for the generator (IR → Rust source) split by `Item` kind.
//!
//! The original `tests.rs` reached 1068 LOC (exceeding the 1000-line
//! threshold). Grouped by the `Item` variant / feature under test:
//!
//! - [`use_items`] — `Item::Use` rendering (single/multiple, pub/private)
//! - [`struct_items`] — `Item::Struct` including unit-struct marker and
//!   reserved-word field escaping
//! - [`enum_items`] — `Item::Enum` numeric/string/data variants + Display
//!   impl synthesis
//! - [`const_items`] — `Item::Const` (primitive + unit struct init)
//! - [`fn_items`] — `Item::Fn` (with/without return, type params,
//!   reserved-word name, `#[attr]` lines)
//! - [`impl_items`] — inherent `Item::Impl` (including async method and
//!   I-218 type_params / constraint)
//! - [`trait_items`] — `Item::Trait` and trait impl (`Item::Impl` with
//!   `for_trait`), including async trait method and I-218 trait type args
//! - [`misc`] — multi-item separators and `Expr::Regex` rendering

use super::*;

mod const_items;
mod enum_items;
mod fn_items;
mod impl_items;
mod misc;
mod struct_items;
mod trait_items;
mod use_items;
