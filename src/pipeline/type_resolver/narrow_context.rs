//! [`NarrowTypeContext`] implementation for [`super::TypeResolver`].
//!
//! Thin adapter that exposes the resolver's private scope stack and
//! synthetic registry to the narrowing-analyzer guard detection
//! routines. Guard detection itself lives in
//! `pipeline::narrowing_analyzer::guards`; this file bridges the two.

use crate::ir::{EnumVariant, Item, RustType};
use crate::pipeline::narrowing_analyzer::{NarrowEvent, NarrowTypeContext};
use crate::pipeline::ResolvedType;

use super::TypeResolver;

impl<'a> NarrowTypeContext for TypeResolver<'a> {
    fn lookup_var(&self, name: &str) -> ResolvedType {
        // Re-exports the resolver's private scope-stack lookup. Kept here
        // rather than making `TypeResolver::lookup_var` `pub` so the
        // scope-stack remains an internal concern of the resolver.
        TypeResolver::lookup_var(self, name)
    }

    fn synthetic_enum_variants(&self, enum_name: &str) -> Option<Vec<EnumVariant>> {
        self.synthetic
            .get(enum_name)
            .and_then(|def| match &def.item {
                Item::Enum { variants, .. } => Some(variants.clone()),
                _ => None,
            })
    }

    fn register_sub_union(&mut self, member_types: &[RustType]) -> String {
        self.synthetic.register_union(member_types)
    }

    fn push_narrow_event(&mut self, event: NarrowEvent) {
        self.result.narrow_events.push(event);
    }
}
