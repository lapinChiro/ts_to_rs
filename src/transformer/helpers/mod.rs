//! Cross-cutting helpers shared by `Transformer` modules.
//!
//! Submodules contain pure IR-construction helpers that don't fit inside
//! a single `expressions/` / `statements/` / `classes/` / `functions/`
//! module because they're consumed from multiple sites.
//!
//! - **`coerce_default`** — JS coerce_default table (I-144 T6-2): wraps
//!   `Option<T>` expressions read in a T-expected context with the
//!   appropriate `.unwrap_or(...)` / `.map(...).unwrap_or_else(...)`
//!   default so post-narrow-stale reads (e.g., closure-reassign aftermath)
//!   reproduce JS runtime semantics (`null + 1 = 1`, `"v=" + null = "v=null"`).
//! - **`truthy`** — JS truthy / falsy predicate table (I-144 T6-3 E10):
//!   builds per-`RustType` predicate expressions (`F64` → `x != 0.0 &&
//!   !x.is_nan()` etc.) used by `convert_if_stmt` fallback when the
//!   test is a bare identifier on a primitive, and by
//!   `try_generate_narrowing_match` when generating composite
//!   Option<Union> truthy guards.
//! - **`option_builders`** — Option-shape IR builders (I-022 / I-144
//!   T6-1): `build_option_unwrap_with_default` (`unwrap_or` / `unwrap_or_else`
//!   eager/lazy dispatch), `build_option_get_or_insert_with` (`??=` with
//!   preserved Option layer), `build_option_or_option` (Option-typed `??`
//!   chain). Three cohesive builders consolidated into `helpers/` at T6-6
//!   (previously inlined in `transformer/mod.rs` pre-existing broken window).

pub(crate) mod coerce_default;
pub(crate) mod option_builders;
pub(crate) mod truthy;
