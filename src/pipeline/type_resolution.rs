//! Data structures for file-level type resolution results.
//!
//! `FileTypeResolution` is the output of [`TypeResolver`](super::type_resolver::TypeResolver),
//! containing pre-computed type information for every expression and variable in a file.
//! The Transformer reads this data to make conversion decisions without performing
//! type inference itself.

use std::collections::HashMap;

use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::RustType;
use crate::pipeline::narrowing_analyzer::{EmissionHint, NarrowEvent, NarrowTrigger};

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
    /// # Closure-reassign suppression (I-144 T6-2 + I-177-D 案 C)
    ///
    /// When a closure reassigns `var_name` (detected via
    /// [`NarrowEvent::ClosureCapture`] with matching `enclosing_fn_body`),
    /// suppression is dispatched on the matching narrow event's
    /// [`NarrowTrigger`]:
    ///
    /// - **`Primary` narrow** (e.g. `if (x !== null) { /* cons-span */ }`):
    ///   not suppressed. The Transformer emits an IR shadow form (typically
    ///   `if let Some(x) = x { ... }` or a typeof variant pattern) that
    ///   rebinds `x` to the narrow type within the cons-span. Preserving
    ///   the narrow keeps the TypeResolver view consistent with the IR
    ///   shadow so type-driven emission decisions (e.g. operator desugar,
    ///   `??` lowering) at narrow-typed sites do not produce E0599 / E0282
    ///   / E0308 mismatches against the shadow (I-161 T7 cohesion gap
    ///   resolved at compile-error level). 案 C does **not** address two
    ///   related concerns that surface in body-mutation + closure-call
    ///   patterns (e.g. T7-3 `if (x !== null) { x &&= 3; reset(); return x ?? -1; }`):
    ///
    ///   1. **Mutation propagation** (deferred to I-177 mutation propagation
    ///      本体): inside the IR shadow, `x &&= 3` mutates the inner
    ///      shadow binding only. After a closure call (`reset()`) that
    ///      mutates the outer `Option<T>` to `None`, the inner shadow
    ///      retains the pre-closure-call value. Reading `x ?? -1` returns
    ///      the inner shadow's value rather than the post-closure outer
    ///      `None`, producing a silent semantic change vs TS runtime.
    ///   2. **Closure-mutable-capture borrow conflict** (deferred to I-048):
    ///      `let mut reset = || { x = None; };` mutably borrows outer `x`,
    ///      conflicting with the subsequent `if let Some(x) = x` move /
    ///      borrow (E0503 / E0506).
    ///
    ///   T7-3 GREEN-ify therefore depends on I-177-D (this fix, completed) +
    ///   I-177 mutation propagation 本体 + I-048 closure ownership inference.
    /// - **`EarlyReturnComplement` narrow** (e.g.
    ///   `if (x === null) return; /* post-if */`): suppressed (returns
    ///   `None`). Post-if scope can be invalidated at runtime by a closure
    ///   call that reassigns `var_name`, so callers fall back to the
    ///   variable's declared `Option<T>` type and the
    ///   [`Transformer`](crate::transformer)'s `coerce_default` wrapper
    ///   reproduces JS runtime semantics (`null + 1 = 1`,
    ///   `"v=" + null = "v=null"`).
    ///
    /// `position` filters [`NarrowEvent::ClosureCapture`] events by
    /// `enclosing_fn_body` membership so a closure-reassign in function
    /// `f` does not affect narrow queries in a sibling function `g`
    /// (multi-fn scope isolation, I-169 P1 invariant).
    pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
        // Find innermost matching narrow event (rfind = rightmost in Vec).
        let narrow = self
            .narrow_events
            .iter()
            .filter_map(NarrowEvent::as_narrow)
            .rfind(|n| {
                n.var_name == var_name && n.scope_start <= position && position < n.scope_end
            })?;

        // Trigger-kind-based suppression dispatch (I-177-D 案 C).
        //
        // Primary trigger: keep narrow active even when closure-reassign exists.
        // EarlyReturnComplement: suppress on closure-reassign (preserve
        // coerce_default workaround in post-if scope).
        let should_suppress = matches!(narrow.trigger, NarrowTrigger::EarlyReturnComplement(_))
            && self.is_var_closure_reassigned(var_name, position);

        if should_suppress {
            return None;
        }

        Some(narrow.narrowed_type)
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

    /// Resolves the type of a variable at a given byte position.
    ///
    /// Canonical precedence for **leaf type lookup** (I-177-B):
    /// 1. If a [`NarrowEvent::Narrow`] applies at `span.lo.0` (and is not
    ///    suppressed by a sibling [`NarrowEvent::ClosureCapture`] under
    ///    [`NarrowTrigger::EarlyReturnComplement`]), returns the narrowed
    ///    type via [`narrowed_type`](Self::narrowed_type).
    /// 2. Otherwise, returns the resolved expression type from
    ///    [`expr_type`](Self::expr_type) at `Span::from_swc(span)`.
    /// 3. Returns `None` if neither is known.
    ///
    /// Suppression dispatch (closure-reassign × `EarlyReturnComplement`) lives
    /// inside [`narrowed_type`], so callers never compose `narrowed_type` and
    /// `expr_type` manually. Composing them in reverse order
    /// (e.g. `expr_type` first → `narrowed_type` only on `Unknown`) silently
    /// drops narrowing because `expr_type` returns `Known(declared)` for any
    /// `Ident` with a declared type — that inversion was the root cause of
    /// the I-177-B defect (`collect_expr_leaf_types` returning the declared
    /// union for narrowed variables in return-wrap contexts, producing
    /// `cannot determine return variant` hard errors and silently incorrect
    /// emissions in callable-interface forms).
    pub fn resolve_var_type(&self, var_name: &str, span: swc_common::Span) -> Option<&RustType> {
        if let Some(narrowed) = self.narrowed_type(var_name, span.lo.0) {
            return Some(narrowed);
        }
        match self.expr_type(Span::from_swc(span)) {
            ResolvedType::Known(ty) => Some(ty),
            ResolvedType::Unknown => None,
        }
    }

    /// Resolves the type of an arbitrary expression (I-177-B canonical
    /// primitive).
    ///
    /// For an [`ast::Expr::Ident`], delegates to
    /// [`resolve_var_type`](Self::resolve_var_type) so the canonical narrow
    /// precedence is preserved. For all other expressions, returns the
    /// resolved type from [`expr_type`](Self::expr_type) at the expression's
    /// span — non-`Ident` expressions are not subject to per-variable
    /// narrowing (narrowing keys on variable name + position).
    pub fn resolve_expr_type(&self, expr: &ast::Expr) -> Option<&RustType> {
        if let ast::Expr::Ident(ident) = expr {
            return self.resolve_var_type(ident.sym.as_ref(), ident.span);
        }
        match self.expr_type(Span::from_swc(expr.span())) {
            ResolvedType::Known(ty) => Some(ty),
            ResolvedType::Unknown => None,
        }
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
            enclosing_fn_body: Span { lo: 0, hi: 100 },
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
        // I-144 T6-2 + I-177-D: a `ClosureCapture` event for the same var
        // causes `narrowed_type` to return `None` for `EarlyReturnComplement`
        // narrows so the variable's declared `Option<T>` type is read at
        // narrow-stale sites and the Transformer can apply the
        // `coerce_default` wrapper. Primary narrows (e.g.
        // `if (x !== null) { /* cons-span */ }`) are NOT suppressed under
        // I-177-D 案 C, since the IR shadow form rebinds `x` to the narrow
        // type within the `if` body and the closure call only mutates the
        // outer `Option<T>` after cons-span exit.
        //
        // This test pins the EarlyReturnComplement suppression invariant
        // (matrix cell #16: `Truthy` × EarlyReturnComplement × closure-
        // reassign present → None) which I-177-D preserves.
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 30,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });

        assert!(resolution.narrowed_type("x", 20).is_none());
        // A different variable's narrow is unaffected (no closure-capture
        // event for `y`, so even an EarlyReturnComplement narrow stays alive).
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "y".to_string(),
            scope_start: 10,
            scope_end: 30,
            narrowed_type: RustType::String,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
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
            // Event is observable only for positions in [10, 50).
            enclosing_fn_body: Span { lo: 10, hi: 50 },
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
    fn test_narrowed_type_suppress_only_fires_inside_enclosing_fn_body() {
        // I-169 P1 (matrix cell #3) + I-177-D 案 C INV-2: when two
        // EarlyReturnComplement Narrow events for `x` exist in different
        // functions and only one has a ClosureCapture event, the other's
        // narrow must NOT be suppressed (multi-fn scope isolation).
        //
        // Uses EarlyReturnComplement trigger so the suppression dispatch
        // path (case-C: suppress only `EarlyReturnComplement` + closure-
        // reassign) actually fires inside f, allowing this test to verify
        // that the suppression boundary is the correct fn body span.
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        // Function f at [0, 100): has Narrow event + ClosureCapture event.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 90,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        // Function g at [200, 300): has Narrow event, NO ClosureCapture.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 210,
            scope_end: 290,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
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

    // ============================================================
    // I-177-D: trigger-kind-based suppression dispatch tests
    // ============================================================
    //
    // 案 C (PRD I-177-D): closure-reassign suppression は narrow event の
    // trigger 種別で dispatch する。
    //
    // - Primary narrow (`if (x !== null) { /* cons-span */ }` 内): suppression
    //   対象外。narrow を保持して IR shadow form (`if let Some(x) = x { ... }`)
    //   と TypeResolver narrow の cohesion を確立する。
    // - EarlyReturnComplement narrow (`if (x === null) return; /* post-if */`):
    //   suppression 維持。post-if scope は closure call で runtime に narrow が
    //   invalidate されうる構造のため、`coerce_default` workaround を発動させる。
    //
    // 以下の 10 test は matrix cells #2, 6, 10, 14, 18 (主 fix) +
    // #4, 8, 12, 16, 20 (suppress preserve) を direct 検証する。

    // ----- 主 fix cells: Primary trigger × closure-reassign × narrow scope内 query →
    //       case-C で Some(narrow) を返す (案 C 効果)。 -----

    #[test]
    fn test_narrowed_type_primary_typeof_with_closure_reassign_keeps_narrow() {
        // Matrix cell #2: Primary(TypeofGuard) + closure-reassign present →
        // case-C で narrow 維持 (Some(String))。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::String,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard("string".to_string())),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(matches!(
            resolution.narrowed_type("x", 25),
            Some(RustType::String)
        ));
    }

    #[test]
    fn test_narrowed_type_primary_instanceof_with_closure_reassign_keeps_narrow() {
        // Matrix cell #6: Primary(InstanceofGuard) + closure-reassign present →
        // case-C で narrow 維持 (Some(Named { name: "Foo" }))。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::Named {
                name: "Foo".to_string(),
                type_args: vec![],
            },
            trigger: NarrowTrigger::Primary(PrimaryTrigger::InstanceofGuard("Foo".to_string())),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(matches!(
            resolution.narrowed_type("x", 25),
            Some(RustType::Named { name, .. }) if name == "Foo"
        ));
    }

    #[test]
    fn test_narrowed_type_primary_nullcheck_with_closure_reassign_keeps_narrow() {
        // Matrix cell #10: Primary(NullCheck NotEqEqNull) + closure-reassign present →
        // case-C で narrow 維持 (Some(F64))。T7-3 と同型 pattern (body read-only)。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, NullCheckKind, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::NotEqEqNull)),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(matches!(
            resolution.narrowed_type("x", 25),
            Some(RustType::F64)
        ));
    }

    #[test]
    fn test_narrowed_type_primary_truthy_with_closure_reassign_keeps_narrow() {
        // Matrix cell #14: Primary(Truthy) + closure-reassign present →
        // case-C で narrow 維持 (Some(F64))。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(matches!(
            resolution.narrowed_type("x", 25),
            Some(RustType::F64)
        ));
    }

    #[test]
    fn test_narrowed_type_primary_optchain_with_closure_reassign_keeps_narrow() {
        // Matrix cell #18: Primary(OptChainInvariant) + closure-reassign present →
        // case-C で narrow 維持 (Some(Named { name: "Config" }))。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "c".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::Named {
                name: "Config".to_string(),
                type_args: vec![],
            },
            trigger: NarrowTrigger::Primary(PrimaryTrigger::OptChainInvariant),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "c".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(matches!(
            resolution.narrowed_type("c", 25),
            Some(RustType::Named { name, .. }) if name == "Config"
        ));
    }

    // ----- Suppress preserve cells: EarlyReturnComplement trigger × closure-reassign
    //       × narrow scope内 query → suppression 維持で None を返す (regression lock-in)。 -----

    #[test]
    fn test_narrowed_type_early_return_typeof_with_closure_reassign_suppresses() {
        // Matrix cell #4: EarlyReturnComplement(TypeofGuard) + closure-reassign →
        // suppression 維持 (None)。post-if scope の coerce_default 発動を保証。
        //
        // Twin assertion: sibling var `y` の同 trigger narrow を ClosureCapture
        // 不在で push し `Some(narrow)` を返すことを確認。これにより `x` の
        // None が「suppression 動作の結果」であって「narrow event 不在」では
        // ないことを構造的に証明する。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::String,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::TypeofGuard(
                "string".to_string(),
            )),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(resolution.narrowed_type("x", 25).is_none());
        // Sibling var without ClosureCapture: same EarlyReturnComplement trigger,
        // narrow stays alive → distinguishes suppression from absence of event.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "y".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::String,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::TypeofGuard(
                "string".to_string(),
            )),
        });
        assert!(matches!(
            resolution.narrowed_type("y", 25),
            Some(RustType::String)
        ));
    }

    #[test]
    fn test_narrowed_type_early_return_instanceof_with_closure_reassign_suppresses() {
        // Matrix cell #8: EarlyReturnComplement(InstanceofGuard) + closure-reassign →
        // suppression 維持 (None)。Twin assertion で suppression 由来の None を確証。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::Named {
                name: "Foo".to_string(),
                type_args: vec![],
            },
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::InstanceofGuard(
                "Foo".to_string(),
            )),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(resolution.narrowed_type("x", 25).is_none());
        // Sibling var: same trigger, no ClosureCapture → narrow alive.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "y".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::Named {
                name: "Foo".to_string(),
                type_args: vec![],
            },
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::InstanceofGuard(
                "Foo".to_string(),
            )),
        });
        assert!(matches!(
            resolution.narrowed_type("y", 25),
            Some(RustType::Named { name, .. }) if name == "Foo"
        ));
    }

    #[test]
    fn test_narrowed_type_early_return_nullcheck_with_closure_reassign_suppresses() {
        // Matrix cell #12: EarlyReturnComplement(NullCheck EqEqEqNull) + closure-reassign →
        // suppression 維持 (None)。c2b/c2c-like pattern で coerce_default 発動を保証。
        // Twin assertion で suppression 由来の None を確証。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, NullCheckKind, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::NullCheck(
                NullCheckKind::EqEqEqNull,
            )),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(resolution.narrowed_type("x", 25).is_none());
        // Sibling var: same trigger, no ClosureCapture → narrow alive.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "y".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::NullCheck(
                NullCheckKind::EqEqEqNull,
            )),
        });
        assert!(matches!(
            resolution.narrowed_type("y", 25),
            Some(RustType::F64)
        ));
    }

    #[test]
    fn test_narrowed_type_early_return_truthy_with_closure_reassign_suppresses() {
        // Matrix cell #16: EarlyReturnComplement(Truthy) + closure-reassign →
        // suppression 維持 (None)。Twin assertion で suppression 由来の None を確証。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(resolution.narrowed_type("x", 25).is_none());
        // Sibling var: same trigger, no ClosureCapture → narrow alive.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "y".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
        });
        assert!(matches!(
            resolution.narrowed_type("y", 25),
            Some(RustType::F64)
        ));
    }

    #[test]
    fn test_narrowed_type_early_return_optchain_with_closure_reassign_suppresses() {
        // Matrix cell #20: EarlyReturnComplement(OptChainInvariant) + closure-reassign →
        // suppression 維持 (None)。Twin assertion で suppression 由来の None を確証。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "c".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::Named {
                name: "Config".to_string(),
                type_args: vec![],
            },
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "c".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        assert!(resolution.narrowed_type("c", 25).is_none());
        // Sibling var: same trigger, no ClosureCapture → narrow alive.
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "d".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::Named {
                name: "Config".to_string(),
                type_args: vec![],
            },
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant),
        });
        assert!(matches!(
            resolution.narrowed_type("d", 25),
            Some(RustType::Named { name, .. }) if name == "Config"
        ));
    }

    // ============================================================
    // I-177-B: canonical leaf type resolution helpers
    // ============================================================
    //
    // PRD I-177-B (Plan η Step 2): `narrowed_type` 優先 → `expr_type` fallback の
    // 「leaf 位置における型 lookup」 knowledge を `FileTypeResolution` 1 箇所に集約する
    // canonical primitive (`resolve_var_type` / `resolve_expr_type`)。 Production code 内
    // 3 site (`Transformer::get_type_for_var` / `Transformer::get_expr_type` /
    // `transformer::return_wrap::collect_expr_leaf_types`) を本 helper 経由に統一し、
    // DRY violation を構造的に解消する。
    //
    // 以下の 5 test は matrix cells #1〜#7, #11 を direct 検証する
    // (PRD Problem Space matrix 参照)。

    fn dummy_swc_span(lo: u32, hi: u32) -> swc_common::Span {
        swc_common::Span::new(swc_common::BytePos(lo), swc_common::BytePos(hi))
    }

    #[test]
    fn test_resolve_var_type_returns_narrowed_when_active() {
        // Matrix cell #2: narrow active かつ expr_type も present → narrowed が優先される
        // (これが本 PRD 修正対象 cell #9 の正解 invariant の primitive lock-in)。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard("number".to_string())),
        });
        let span = Span { lo: 25, hi: 26 };
        resolution.expr_types.insert(
            span,
            ResolvedType::Known(RustType::Named {
                name: "F64OrString".to_string(),
                type_args: vec![],
            }),
        );

        let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
        assert!(matches!(resolved, Some(RustType::F64)));
    }

    #[test]
    fn test_resolve_var_type_returns_declared_when_outside_scope() {
        // Matrix cell #1: narrow none + expr_type present → expr_type を返す。
        let mut resolution = FileTypeResolution::empty();
        let span = Span { lo: 25, hi: 26 };
        resolution
            .expr_types
            .insert(span, ResolvedType::Known(RustType::F64));

        let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
        assert!(matches!(resolved, Some(RustType::F64)));
    }

    #[test]
    fn test_resolve_var_type_returns_none_when_neither_present() {
        // narrow none + expr_type Unknown → None。
        let resolution = FileTypeResolution::empty();
        let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_var_type_returns_declared_when_suppressed() {
        // Matrix cell #3: EarlyReturnComplement narrow + closure-reassign → suppression
        // で narrowed_type は None、expr_type fallback で declared を返す。
        // (I-177-D suppression dispatch が canonical primitive 経由でも正しく effect する
        // ことの lock-in。)
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
        });
        resolution.narrow_events.push(NarrowEvent::ClosureCapture {
            var_name: "x".to_string(),
            enclosing_fn_body: Span { lo: 0, hi: 100 },
        });
        let span = Span { lo: 25, hi: 26 };
        resolution.expr_types.insert(
            span,
            ResolvedType::Known(RustType::Option(Box::new(RustType::F64))),
        );

        let resolved = resolution.resolve_var_type("x", dummy_swc_span(25, 26));
        assert!(matches!(resolved, Some(RustType::Option(_))));
    }

    #[test]
    fn test_resolve_expr_type_delegates_to_var_type_for_ident() {
        // Matrix cell #5: Ident expr で narrow active → narrowed を返す
        // (resolve_expr_type が Ident path で resolve_var_type に delegate する invariant)。
        use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
        use swc_ecma_ast as ast;

        let mut resolution = FileTypeResolution::empty();
        resolution.narrow_events.push(NarrowEvent::Narrow {
            var_name: "x".to_string(),
            scope_start: 10,
            scope_end: 50,
            narrowed_type: RustType::F64,
            trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard("number".to_string())),
        });
        let span = Span { lo: 25, hi: 26 };
        resolution.expr_types.insert(
            span,
            ResolvedType::Known(RustType::Named {
                name: "F64OrString".to_string(),
                type_args: vec![],
            }),
        );

        let ident_expr = ast::Expr::Ident(ast::Ident {
            span: dummy_swc_span(25, 26),
            sym: "x".into(),
            optional: false,
            ctxt: Default::default(),
        });

        let resolved = resolution.resolve_expr_type(&ident_expr);
        assert!(matches!(resolved, Some(RustType::F64)));
    }

    #[test]
    fn test_resolve_expr_type_uses_expr_type_for_non_ident() {
        // Matrix cell #7 / #11: 非 Ident expr (NumLit) は narrow に subject されない
        // → expr_type のみを参照。
        use swc_ecma_ast as ast;

        let mut resolution = FileTypeResolution::empty();
        let span = Span { lo: 25, hi: 30 };
        resolution
            .expr_types
            .insert(span, ResolvedType::Known(RustType::F64));

        let lit_expr = ast::Expr::Lit(ast::Lit::Num(ast::Number {
            span: dummy_swc_span(25, 30),
            value: 42.0,
            raw: None,
        }));

        let resolved = resolution.resolve_expr_type(&lit_expr);
        assert!(matches!(resolved, Some(RustType::F64)));
    }
}
