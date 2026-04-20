//! Registry-access trait consumed by [`super::guards`].
//!
//! Narrow guard detection (typeof / instanceof / null check / truthy +
//! early-return complement) reads the declared type of the target
//! identifier, inspects synthetic union enums for complement computation,
//! and occasionally registers a new sub-union when a complement spans
//! three or more variants. All of these touch state owned by the
//! [`TypeResolver`](crate::pipeline::type_resolver::TypeResolver); the
//! analyzer itself is stateless.
//!
//! Abstracting these operations behind a trait keeps the analyzer free
//! of a hard dependency on `TypeResolver`'s private fields, and lets the
//! guard tests substitute a minimal mock without booting the full
//! resolver pipeline.

use crate::ir::{EnumVariant, RustType};
use crate::pipeline::ResolvedType;

use super::events::NarrowEvent;

/// Registry-access + event-sink operations required by narrow guard
/// detection.
///
/// Implemented by
/// [`TypeResolver`](crate::pipeline::type_resolver::TypeResolver) so the
/// pipeline walks via [`super::guards::detect_narrowing_guard`] /
/// [`super::guards::detect_early_return_narrowing`].
///
/// The methods are intentionally small and side-effect-local: lookups
/// return owned / copy types so callers may interleave immutable reads
/// with mutable calls (`register_sub_union`, `push_narrow_event`)
/// without borrow conflicts.
pub trait NarrowTypeContext {
    /// Resolves the declared type of `name` in the current scope stack.
    ///
    /// Returns [`ResolvedType::Unknown`] if the identifier is not in
    /// scope — guards must tolerate this (unresolved variables never
    /// narrow).
    fn lookup_var(&self, name: &str) -> ResolvedType;

    /// Returns the variants of a registered synthetic enum named
    /// `enum_name`, or `None` if no such enum exists or the registered
    /// item is not an enum.
    ///
    /// Returns owned [`EnumVariant`]s so the caller may mutably
    /// [`register_sub_union`](Self::register_sub_union) during
    /// complement computation without conflicting with the borrow of
    /// this result.
    fn synthetic_enum_variants(&self, enum_name: &str) -> Option<Vec<EnumVariant>>;

    /// Registers (or returns the dedup'd name of) a synthetic union
    /// enum over `member_types`.
    ///
    /// Called during complement narrowing when the remaining variants
    /// after exclusion number three or more, in which case a fresh
    /// sub-union enum is materialized to carry the remainder.
    fn register_sub_union(&mut self, member_types: &[RustType]) -> String;

    /// Records a detected [`NarrowEvent`] in the resolver's per-file
    /// event store.
    ///
    /// Implementations append to `FileTypeResolution::narrow_events`.
    fn push_narrow_event(&mut self, event: NarrowEvent);
}
