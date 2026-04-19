//! `TypeResolver::detect_narrowing_guard` + `detect_early_return_narrowing`
//! test groupings. Split for per-file cohesion and line-count compliance.

use super::*;
use crate::pipeline::narrowing_analyzer::{NarrowEvent, NarrowEventRef};

/// Iterates the [`NarrowEvent::Narrow`] variants of a file resolution as
/// borrowed views. Hides the enum-variant destructuring so legacy field
/// assertions read naturally after the T4 migration.
fn narrow_views(res: &FileTypeResolution) -> impl Iterator<Item = NarrowEventRef<'_>> {
    res.narrow_events.iter().filter_map(NarrowEvent::as_narrow)
}

mod legacy_events;
mod trigger_completeness;
