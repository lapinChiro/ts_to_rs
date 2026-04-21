//! Control-flow statement conversion tests, grouped by feature area.
//!
//! The original `control_flow.rs` reached 1095 LOC (exceeding the 1000
//! threshold). Split into 7 cohesive sub-modules by concern:
//!
//! - [`block_flatten`] — I-153 T0 Block stmt flatten into parent scope
//! - [`truthy_complement_match`] — I-144 T6-3 H-3 consolidated match
//!   emission for `!x` on Option<Union>
//! - [`ir_body_always_exits`] — I-144 T6-5 structural exit-detection
//!   across all stmt shapes
//! - [`if_while`] — basic if / while conversion + labeled-for-in
//! - [`do_while`] — do-while body rewriting (continue/break label +
//!   nested loop scope + labeled variants) from I-153 / I-154
//! - [`cond_assign`] — RC-7 conditional assignment pattern (if / while
//!   with Option / F64 / comparison extraction)
//! - [`narrowing`] — I-144 T6-2 narrowing_match closure reassign
//!   suppression
//!
//! All sub-modules share the parent's imports via `use super::*;`.

use super::*;

mod block_flatten;
mod cond_assign;
mod do_while;
mod if_while;
mod ir_body_always_exits;
mod narrowing;
mod truthy_complement_match;
