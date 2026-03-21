//! Data structures for file-level type resolution results.
//!
//! `FileTypeResolution` is the output of [`TypeResolver`](super::type_resolver::TypeResolver),
//! containing pre-computed type information for every expression and variable in a file.
//! The Transformer reads this data to make conversion decisions without performing
//! type inference itself.

use std::collections::HashMap;

use crate::ir::RustType;

use super::ResolvedType;

/// Span identifier for AST nodes. Uses SWC's byte positions.
///
/// Since SWC's `BytePos` can overlap between parent and child nodes,
/// we use the `(lo, hi)` pair which is practically unique for distinct nodes.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct Span {
    pub lo: u32,
    pub hi: u32,
}

impl Span {
    /// Creates a `Span` from a SWC `Span`.
    pub fn from_swc(span: swc_common::Span) -> Self {
        Self {
            lo: span.lo.0,
            hi: span.hi.0,
        }
    }
}

/// Unique identifier for a variable. Combines name and declaration position
/// to distinguish same-named variables in different scopes.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct VarId {
    pub name: String,
    pub declared_at: Span,
}

/// Records that a variable's type is narrowed within a specific scope.
///
/// For example, `if (typeof x === "string") { ... }` generates a `NarrowingEvent`
/// where `x` is narrowed to `String` within the then-block's scope.
#[derive(Debug, Clone)]
pub struct NarrowingEvent {
    /// Start byte position of the scope where narrowing is active.
    pub scope_start: u32,
    /// End byte position of the scope where narrowing is active.
    pub scope_end: u32,
    /// The variable being narrowed.
    pub var_name: String,
    /// The narrowed type (replaces the variable's original type in this scope).
    pub narrowed_type: RustType,
}

/// File-level type resolution results.
///
/// Produced by `TypeResolver::resolve_file()` and consumed by the Transformer.
/// All data is immutable after construction.
#[derive(Debug)]
pub struct FileTypeResolution {
    /// Expression types: Span → resolved type.
    ///
    /// Contains the type of every expression that TypeResolver could resolve.
    /// Expressions not in this map have type `Unknown`.
    pub expr_types: HashMap<Span, ResolvedType>,

    /// Expected types: Span → expected Rust type.
    ///
    /// The expected type at a position, derived from the parent context:
    /// - Variable declaration with annotation: the annotation type
    /// - Return statement: the function's return type
    /// - Function call argument: the parameter type
    pub expected_types: HashMap<Span, RustType>,

    /// Narrowing events: scoped type overrides for variables.
    ///
    /// When checking variable types, narrowing events override the variable's
    /// declared type within the event's scope range.
    pub narrowing_events: Vec<NarrowingEvent>,

    /// Variable mutability: whether each variable needs `let mut`.
    ///
    /// Determined by `const` vs `let` declaration and whether the variable
    /// is reassigned in the body.
    pub var_mutability: HashMap<VarId, bool>,
}

impl FileTypeResolution {
    /// Creates an empty resolution (no types resolved).
    pub fn empty() -> Self {
        Self {
            expr_types: HashMap::new(),
            expected_types: HashMap::new(),
            narrowing_events: Vec::new(),
            var_mutability: HashMap::new(),
        }
    }

    /// Gets the resolved type for an expression at the given span.
    /// Returns `Unknown` if not in the map.
    pub fn expr_type(&self, span: Span) -> &ResolvedType {
        static UNKNOWN: ResolvedType = ResolvedType::Unknown;
        self.expr_types.get(&span).unwrap_or(&UNKNOWN)
    }

    /// Gets the expected type for an expression at the given span, if any.
    pub fn expected_type(&self, span: Span) -> Option<&RustType> {
        self.expected_types.get(&span)
    }

    /// Gets the narrowed type for a variable at a given byte position.
    ///
    /// Returns the innermost (most specific) narrowing that applies,
    /// or `None` if no narrowing is active for this variable at this position.
    pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
        self.narrowing_events
            .iter()
            .rfind(|e| {
                e.var_name == var_name && e.scope_start <= position && position < e.scope_end
            })
            .map(|e| &e.narrowed_type)
    }

    /// Gets the mutability for a variable.
    pub fn is_mutable(&self, var_id: &VarId) -> Option<bool> {
        self.var_mutability.get(var_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_from_swc() {
        let swc_span = swc_common::Span::new(swc_common::BytePos(10), swc_common::BytePos(20));
        let span = Span::from_swc(swc_span);
        assert_eq!(span.lo, 10);
        assert_eq!(span.hi, 20);
    }

    #[test]
    fn test_expr_type_returns_unknown_for_missing() {
        let resolution = FileTypeResolution::empty();
        let span = Span { lo: 0, hi: 5 };
        assert!(matches!(resolution.expr_type(span), ResolvedType::Unknown));
    }

    #[test]
    fn test_expr_type_returns_known_when_present() {
        let mut resolution = FileTypeResolution::empty();
        let span = Span { lo: 0, hi: 5 };
        resolution
            .expr_types
            .insert(span, ResolvedType::Known(RustType::String));
        assert!(matches!(
            resolution.expr_type(span),
            ResolvedType::Known(RustType::String)
        ));
    }

    #[test]
    fn test_expected_type_returns_none_for_missing() {
        let resolution = FileTypeResolution::empty();
        let span = Span { lo: 0, hi: 5 };
        assert!(resolution.expected_type(span).is_none());
    }

    #[test]
    fn test_narrowed_type_returns_innermost_scope() {
        let mut resolution = FileTypeResolution::empty();
        // Outer narrowing: x is StringOrF64 in range [10, 50)
        resolution.narrowing_events.push(NarrowingEvent {
            scope_start: 10,
            scope_end: 50,
            var_name: "x".to_string(),
            narrowed_type: RustType::Named {
                name: "StringOrF64".to_string(),
                type_args: vec![],
            },
        });
        // Inner narrowing: x is String in range [20, 40)
        resolution.narrowing_events.push(NarrowingEvent {
            scope_start: 20,
            scope_end: 40,
            var_name: "x".to_string(),
            narrowed_type: RustType::String,
        });

        // At position 15 (outer only): StringOrF64
        let ty = resolution.narrowed_type("x", 15);
        assert!(matches!(ty, Some(RustType::Named { name, .. }) if name == "StringOrF64"));

        // At position 25 (both, inner wins): String
        let ty = resolution.narrowed_type("x", 25);
        assert!(matches!(ty, Some(RustType::String)));

        // At position 45 (outer only): StringOrF64
        let ty = resolution.narrowed_type("x", 45);
        assert!(matches!(ty, Some(RustType::Named { name, .. }) if name == "StringOrF64"));

        // At position 55 (none): None
        let ty = resolution.narrowed_type("x", 55);
        assert!(ty.is_none());
    }

    #[test]
    fn test_narrowed_type_different_variables() {
        let mut resolution = FileTypeResolution::empty();
        resolution.narrowing_events.push(NarrowingEvent {
            scope_start: 10,
            scope_end: 50,
            var_name: "x".to_string(),
            narrowed_type: RustType::String,
        });

        // x is narrowed, y is not
        assert!(resolution.narrowed_type("x", 25).is_some());
        assert!(resolution.narrowed_type("y", 25).is_none());
    }

    #[test]
    fn test_is_mutable() {
        let mut resolution = FileTypeResolution::empty();
        let var_id = VarId {
            name: "x".to_string(),
            declared_at: Span { lo: 0, hi: 5 },
        };
        resolution.var_mutability.insert(var_id.clone(), true);

        assert_eq!(resolution.is_mutable(&var_id), Some(true));

        let unknown_var = VarId {
            name: "y".to_string(),
            declared_at: Span { lo: 10, hi: 15 },
        };
        assert_eq!(resolution.is_mutable(&unknown_var), None);
    }
}
