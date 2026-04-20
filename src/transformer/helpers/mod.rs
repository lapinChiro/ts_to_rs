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

pub(crate) mod coerce_default;
