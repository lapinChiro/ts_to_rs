//! Data structures for file-level type resolution results.
//!
//! `FileTypeResolution` is the output of [`TypeResolver`](super::type_resolver::TypeResolver),
//! containing pre-computed type information for every expression and variable in a file.
//! The Transformer reads this data to make conversion decisions without performing
//! type inference itself.

use std::collections::HashMap;

use crate::ir::RustType;
use crate::pipeline::narrowing_analyzer::{EmissionHint, NarrowEvent};

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

/// Records that a variable is a destructured field binding in a DU match arm.
///
/// When a discriminated union switch is converted to `match`, each case arm may
/// destructure fields (e.g., `Shape::Circle { radius, .. }`). Within the arm body,
/// `radius` is a local variable bound by reference. This record tracks such bindings
/// so the Transformer can emit `.clone()` on field access.
#[derive(Debug, Clone)]
pub struct DuFieldBinding {
    /// The field variable name (e.g., "radius").
    pub var_name: String,
    /// Start byte position of the match arm body scope.
    pub scope_start: u32,
    /// End byte position of the match arm body scope.
    pub scope_end: u32,
}

/// Records that an `any`-typed variable should use a synthesized enum type.
///
/// When `typeof`/`instanceof` narrowing is detected on an `any`-typed variable,
/// a synthetic enum type is generated. This override is scoped to the function
/// or arrow body where the variable resides.
#[derive(Debug, Clone)]
pub struct AnyEnumOverride {
    /// The variable name being overridden (parameter or local variable).
    pub var_name: String,
    /// Start byte position of the scope where the override is active.
    pub scope_start: u32,
    /// End byte position of the scope where the override is active.
    pub scope_end: u32,
    /// The synthesized enum type to use instead of `Any`.
    pub enum_type: RustType,
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

    /// Narrowing events: scoped type overrides, resets, and closure captures.
    ///
    /// `NarrowEvent::Narrow` entries override a variable's declared type
    /// within the event's scope range. `Reset` / `ClosureCapture` entries
    /// are consumed by the I-144 CFG analyzer for emission-hint decisions.
    pub narrow_events: Vec<NarrowEvent>,

    /// Variable mutability: whether each variable needs `let mut`.
    ///
    /// Determined by `const` vs `let` declaration and whether the variable
    /// is reassigned in the body.
    pub var_mutability: HashMap<VarId, bool>,

    /// DU field bindings: fields destructured in discriminated union match arms.
    ///
    /// Used by the Transformer to determine if a field name at a given position
    /// refers to a match arm binding (requiring `.clone()`) rather than a
    /// standalone field access (requiring inline match).
    pub du_field_bindings: Vec<DuFieldBinding>,

    /// Any-narrowing enum type overrides: position-scoped overrides for `any`-typed variables.
    ///
    /// When AnyTypeAnalyzer detects that an `any`-typed variable is narrowed via
    /// `typeof`/`instanceof`, it generates a synthetic enum type. These overrides
    /// are scoped to the function/arrow body where the variable is declared, so the
    /// Transformer can use the enum type instead of `Any` for variable declarations.
    pub any_enum_overrides: Vec<AnyEnumOverride>,

    /// Pre-resolved struct field lists for object literals with spread sources.
    ///
    /// When TypeResolver resolves spread source fields (via `resolve_spread_source_fields`),
    /// the result is stored here keyed by the object literal's span. The Transformer
    /// reads this to expand spreads into individual field accesses, avoiding the need
    /// to re-resolve type parameter constraints or Option unwrapping.
    pub spread_fields: HashMap<Span, Vec<(String, RustType)>>,

    /// Per-`??=` emission hints produced by
    /// [`crate::pipeline::narrowing_analyzer::analyze_function`].
    ///
    /// Keyed by the LHS identifier's start byte position
    /// (`ident.id.span.lo.0` where `ident` is the `??=` assignment's bare
    /// `Ident` LHS — same key the analyzer writes from
    /// `classifier::extract_nullish_assign_ident_stmt`). Consumed by
    /// `Transformer::try_convert_nullish_assign_stmt` to pick between
    /// E1 shadow-let and E2a `get_or_insert_with` emission depending on
    /// whether the narrow would be invalidated by a later reset, loop
    /// iteration, or closure reassign.
    pub emission_hints: HashMap<u32, EmissionHint>,
}

impl FileTypeResolution {
    /// Creates an empty resolution (no types resolved).
    pub fn empty() -> Self {
        Self {
            expr_types: HashMap::new(),
            expected_types: HashMap::new(),
            narrow_events: Vec::new(),
            var_mutability: HashMap::new(),
            du_field_bindings: Vec::new(),
            any_enum_overrides: Vec::new(),
            spread_fields: HashMap::new(),
            emission_hints: HashMap::new(),
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
    ///
    /// Only consults [`NarrowEvent::Narrow`] variants; `Reset` /
    /// `ClosureCapture` events carry no type and are skipped.
    ///
    /// I-144 T6-2 closure-reassign suppression (I-169 follow-up: position-
    /// aware): when `var_name` is reassigned inside any closure body whose
    /// `enclosing_fn_body` contains `position`, the narrow is suppressed
    /// and `None` is returned. Callers fall back to the variable's declared
    /// `Option<T>` type, which matches the Transformer's narrow-guard
    /// suppression so reads see a consistent type and the `coerce_default`
    /// wrapper can be applied at arithmetic / string-concat sites.
    pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
        if self.is_var_closure_reassigned(var_name, position) {
            return None;
        }
        self.narrow_events
            .iter()
            .filter_map(NarrowEvent::as_narrow)
            .rfind(|n| {
                n.var_name == var_name && n.scope_start <= position && position < n.scope_end
            })
            .map(|n| n.narrowed_type)
    }

    /// Gets the mutability for a variable.
    pub fn is_mutable(&self, var_id: &VarId) -> Option<bool> {
        self.var_mutability.get(var_id).copied()
    }

    /// Gets the any-enum override type for a variable at a given position.
    ///
    /// Returns the synthesized enum type if the variable has been overridden
    /// from `any` within the current scope, or `None` otherwise.
    pub fn any_enum_override(&self, var_name: &str, position: u32) -> Option<&RustType> {
        self.any_enum_overrides
            .iter()
            .rfind(|o| {
                o.var_name == var_name && o.scope_start <= position && position < o.scope_end
            })
            .map(|o| &o.enum_type)
    }

    /// Checks if a variable name at a given position is a DU field binding.
    ///
    /// Returns `true` if the variable was destructured from a discriminated union
    /// match arm at this position. The Transformer uses this to emit `.clone()`
    /// instead of generating a standalone inline match expression.
    pub fn is_du_field_binding(&self, var_name: &str, position: u32) -> bool {
        self.du_field_bindings
            .iter()
            .any(|b| b.var_name == var_name && b.scope_start <= position && position < b.scope_end)
    }

    /// Returns the emission hint for a `??=` statement keyed by its start
    /// byte position (`stmt.span.lo.0`).
    ///
    /// `None` when no analyzer output exists for the site — the Transformer
    /// then falls back to the default E1 shadow-let strategy for backward
    /// compatibility with paths the analyzer does not yet cover.
    pub fn emission_hint(&self, stmt_lo: u32) -> Option<EmissionHint> {
        self.emission_hints.get(&stmt_lo).copied()
    }

    /// Returns `true` iff some closure body in the same function as `position`
    /// reassigns `var_name` (I-144 T6-2 `NarrowEvent::ClosureCapture`,
    /// I-169 follow-up: position-aware via `enclosing_fn_body`).
    ///
    /// Used by the Transformer to suppress narrow shadow-let emission for
    /// `var_name` so the variable stays `Option<T>` and the closure body can
    /// continue to assign `null` / `undefined` to it. Subsequent T-expected
    /// reads are wrapped via the `coerce_default` table to reproduce JS
    /// runtime semantics (`null + 1 = 1`, `"v=" + null = "v=null"`).
    ///
    /// `position` filters events by `enclosing_fn_body` membership so a
    /// closure-reassign in function `f` does not affect narrow queries in
    /// a sibling function `g` (multi-fn scope isolation, I-169 P1 fix).
    pub fn is_var_closure_reassigned(&self, var_name: &str, position: u32) -> bool {
        self.narrow_events.iter().any(|e| match e {
            NarrowEvent::ClosureCapture {
                var_name: v,
                enclosing_fn_body,
                ..
            } => {
                v == var_name && enclosing_fn_body.lo <= position && position < enclosing_fn_body.hi
            }
            _ => false,
        })
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
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        // Outer narrowing: x is StringOrF64 in range [10, 50)
        resolution.narrow_events.push(NarrowEvent::Narrow {
            scope_start: 10,
            scope_end: 50,
            var_name: "x".to_string(),
            narrowed_type: RustType::Named {
                name: "StringOrF64".to_string(),
                type_args: vec![],
            },
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        });
        // Inner narrowing: x is String in range [20, 40)
        resolution.narrow_events.push(NarrowEvent::Narrow {
            scope_start: 20,
            scope_end: 40,
            var_name: "x".to_string(),
            narrowed_type: RustType::String,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard("string".to_string())),
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
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            scope_start: 10,
            scope_end: 50,
            var_name: "x".to_string(),
            narrowed_type: RustType::String,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard("string".to_string())),
        });

        // x is narrowed, y is not
        assert!(resolution.narrowed_type("x", 25).is_some());
        assert!(resolution.narrowed_type("y", 25).is_none());
    }

    #[test]
    fn test_narrowed_type_skips_reset_events() {
        use crate::pipeline::narrowing_analyzer::ResetCause;
        let mut resolution = FileTypeResolution::empty();
        // A Reset event in scope must NOT be returned by `narrowed_type`.
        resolution.narrow_events.push(NarrowEvent::Reset {
            var_name: "x".to_string(),
            position: 20,
            cause: ResetCause::NullAssign,
        });
        assert!(resolution.narrowed_type("x", 20).is_none());
    }

    #[test]
    fn test_narrowed_type_skips_closure_capture_events() {
        // M-4: `ClosureCapture` variant must also be excluded from
        // `narrowed_type()` lookups — it carries no Narrow scope, only
        // capture metadata. Also exercises the I-169 enclosing_fn_body
        // suppression: query at position 30 falls inside enclosing_fn_body
        // [0, 100), so suppression activates and `narrowed_type` returns
        // None (no Narrow event present anyway).
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            closure_span: Span { lo: 10, hi: 50 },
            enclosing_fn_body: Span { lo: 0, hi: 100 },
            outer_narrow: RustType::String,
        });
        assert!(resolution.narrowed_type("x", 30).is_none());
    }

    #[test]
    fn test_narrowed_type_returned_with_interleaved_reset_only() {
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger, ResetCause};
        let mut resolution = FileTypeResolution::empty();
        // Reset event preceding Narrow must NOT suppress the narrow lookup —
        // only ClosureCapture for the same var (T6-2 suppression rule)
        // suppresses; Reset events are skipped by `as_narrow().filter_map`.
        resolution.narrow_events.push(NarrowEvent::Reset {
            var_name: "x".to_string(),
            position: 5,
            cause: ResetCause::DirectAssign,
        });
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 30,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        });

        assert!(matches!(
            resolution.narrowed_type("x", 20),
            Some(RustType::F64)
        ));
    }

    #[test]
    fn test_narrowed_type_suppressed_when_closure_reassign_present() {
        // I-144 T6-2: a `ClosureCapture` event for the same var causes
        // `narrowed_type` to return `None` so the variable's declared
        // `Option<T>` type is read at narrow-stale sites and the
        // Transformer can apply the `coerce_default` wrapper.
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 30,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            closure_span: Span { lo: 15, hi: 25 },
            enclosing_fn_body: Span { lo: 0, hi: 100 },
            outer_narrow: RustType::F64,
        });

        assert!(resolution.narrowed_type("x", 20).is_none());
        // A different variable's narrow is unaffected.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "y".to_string(),
            scope_start: 10,
            scope_end: 30,
            narrowed_type: RustType::String,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        });
        assert!(resolution.narrowed_type("y", 20).is_some());
    }

    #[test]
    fn is_var_closure_reassigned_respects_enclosing_fn_body_scope() {
        // I-169 P1: position outside enclosing_fn_body span must not match
        // the event — even when var_name matches.
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            closure_span: Span { lo: 20, hi: 40 },
            // Event is observable only for positions in [10, 50).
            enclosing_fn_body: Span { lo: 10, hi: 50 },
            outer_narrow: RustType::F64,
        });
        // Inside scope → true
        assert!(resolution.is_var_closure_reassigned("x", 15));
        assert!(resolution.is_var_closure_reassigned("x", 30));
        assert!(resolution.is_var_closure_reassigned("x", 49));
        // Boundary `hi` is exclusive → 50 NOT matched
        assert!(!resolution.is_var_closure_reassigned("x", 50));
        // Below `lo` → false (sibling-fn case: position before this fn)
        assert!(!resolution.is_var_closure_reassigned("x", 9));
        // Above `hi` → false (sibling-fn case: position after this fn)
        assert!(!resolution.is_var_closure_reassigned("x", 100));
        // Different var_name → always false regardless of position
        assert!(!resolution.is_var_closure_reassigned("y", 30));
    }

    #[test]
    fn narrowed_type_suppress_only_fires_inside_enclosing_fn_body() {
        // I-169 P1 (matrix cell #3): when two Narrow events for `x` exist
        // in different functions and one has a ClosureCapture event, the
        // other's narrow must NOT be suppressed.
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        // Function f at [0, 100): has Narrow event + ClosureCapture event.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 90,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            closure_span: Span { lo: 30, hi: 50 },
            enclosing_fn_body: Span { lo: 0, hi: 100 },
            outer_narrow: RustType::F64,
        });
        // Function g at [200, 300): has Narrow event, NO ClosureCapture.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 210,
            scope_end: 290,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        });

        // Query inside f (position 60): suppress → None
        assert!(resolution.narrowed_type("x", 60).is_none());
        // Query inside g (position 250): narrow fires normally → Some
        assert!(matches!(
            resolution.narrowed_type("x", 250),
            Some(RustType::F64)
        ));
    }

    #[test]
    fn test_du_field_binding_detection() {
        let mut resolution = FileTypeResolution::empty();
        // "radius" is bound in match arm at [100, 200)
        resolution.du_field_bindings.push(DuFieldBinding {
            var_name: "radius".to_string(),
            scope_start: 100,
            scope_end: 200,
        });

        // Inside scope: true
        assert!(resolution.is_du_field_binding("radius", 100));
        assert!(resolution.is_du_field_binding("radius", 150));

        // Different variable: false
        assert!(!resolution.is_du_field_binding("height", 150));
    }

    #[test]
    fn test_du_field_binding_outside_scope() {
        let mut resolution = FileTypeResolution::empty();
        resolution.du_field_bindings.push(DuFieldBinding {
            var_name: "radius".to_string(),
            scope_start: 100,
            scope_end: 200,
        });

        // Before scope: false
        assert!(!resolution.is_du_field_binding("radius", 50));

        // At scope_end (exclusive): false
        assert!(!resolution.is_du_field_binding("radius", 200));

        // After scope: false
        assert!(!resolution.is_du_field_binding("radius", 250));
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

    #[test]
    fn test_emission_hint_returns_stored_hint() {
        // I-144 T6-1: the `??=` emission-hint lookup returns the value stored
        // by `TypeResolver::collect_emission_hints` when the key matches.
        let mut resolution = FileTypeResolution::empty();
        resolution
            .emission_hints
            .insert(100, EmissionHint::ShadowLet);
        resolution
            .emission_hints
            .insert(200, EmissionHint::GetOrInsertWith);

        assert_eq!(resolution.emission_hint(100), Some(EmissionHint::ShadowLet));
        assert_eq!(
            resolution.emission_hint(200),
            Some(EmissionHint::GetOrInsertWith)
        );
    }

    #[test]
    fn test_emission_hint_returns_none_for_missing_key() {
        // Absent key means the analyzer did not produce a hint for the site
        // (e.g., the `??=` sits in a context `analyze_function` does not
        // cover, or the key does not correspond to a `??=` site at all).
        // The Transformer falls back to the default E1 shadow-let path.
        let resolution = FileTypeResolution::empty();
        assert_eq!(resolution.emission_hint(42), None);
    }
}
