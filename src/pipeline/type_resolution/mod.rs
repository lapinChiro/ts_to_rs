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

#[cfg(test)]
mod tests;

/// Returns `true` iff `position` falls in the half-open byte range `[lo, hi)`.
///
/// Centralizes the "position membership in scope" knowledge consumed by every
/// scoped lookup ([`FileTypeResolution::narrowed_type`],
/// [`FileTypeResolution::any_enum_override`],
/// [`FileTypeResolution::is_du_field_binding`],
/// [`FileTypeResolution::is_var_closure_reassigned`]) so the half-open
/// invariant (boundary `hi` excluded) is enforced in one place rather than
/// re-stated at every call site.
fn position_in_range(position: u32, lo: u32, hi: u32) -> bool {
    lo <= position && position < hi
}

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
                n.var_name == var_name && position_in_range(position, n.scope_start, n.scope_end)
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
                o.var_name == var_name && position_in_range(position, o.scope_start, o.scope_end)
            })
            .map(|o| &o.enum_type)
    }

    /// Checks if a variable name at a given position is a DU field binding.
    ///
    /// Returns `true` if the variable was destructured from a discriminated union
    /// match arm at this position. The Transformer uses this to emit `.clone()`
    /// instead of generating a standalone inline match expression.
    pub fn is_du_field_binding(&self, var_name: &str, position: u32) -> bool {
        self.du_field_bindings.iter().any(|b| {
            b.var_name == var_name && position_in_range(position, b.scope_start, b.scope_end)
        })
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
                v == var_name
                    && position_in_range(position, enclosing_fn_body.lo, enclosing_fn_body.hi)
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
