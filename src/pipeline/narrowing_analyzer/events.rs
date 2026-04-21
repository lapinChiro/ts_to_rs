//! Event types produced by the [`narrowing_analyzer`](super) module.
//!
//! This module contains the **data-type backbone** of the analyzer — the
//! facts the analyzer records about a function's narrowing state and the
//! emission strategies it recommends. It has no behavior: all inference
//! logic lives in [`super::classifier`]; all orchestration in
//! [`super`] (`analyze_function` + guard detection).
//!
//! # Semantic layering
//!
//! - [`NarrowEvent`] captures per-variable facts (narrow active, reset,
//!   closure-captured) that consumers store and query by position.
//! - [`NarrowTrigger`] / [`PrimaryTrigger`] record **why** a narrow was
//!   introduced. The two-layer split enforces "nested
//!   [`NarrowTrigger::EarlyReturnComplement`] is unrepresentable" at the
//!   type level.
//! - [`NullCheckKind`] records the exact operator + RHS shape of a
//!   null / undefined check (`==` loose vs `===` strict; null vs undefined).
//! - [`ResetCause`] classifies the nature of a mutation that may or may
//!   not invalidate a narrow (arithmetic / update are preserving; direct
//!   or null assignments are invalidating).
//! - [`EmissionHint`] hints the Transformer toward the right Rust AST pattern
//!   at each narrow-related site.

use crate::ir::RustType;
use crate::pipeline::type_resolution::Span;

/// Structured record of a narrowing-related fact about a variable.
///
/// Produced by [guard detection](super::detect_narrowing_guard) and consumed by the Transformer
/// through `FileTypeResolution::narrow_events`.
///
/// # Variants
///
/// - [`Narrow`](Self::Narrow): scope-based narrow (populated by T5's
///   `detect_narrowing_guard`).
/// - [`Reset`](Self::Reset): an operation at `position` invalidates the
///   narrow on `var_name`. Populated by T6 when the Transformer wires in
///   classifier output.
/// - [`ClosureCapture`](Self::ClosureCapture): a closure captures the outer
///   narrow; used to drive Phase 3b closure reassign emission policy.
///   Populated by T6.
#[derive(Debug, Clone, PartialEq)]
pub enum NarrowEvent {
    /// Narrow is active for `var_name` across `[scope_start, scope_end)`.
    Narrow {
        /// Variable whose type is narrowed.
        var_name: String,
        /// Start byte position (inclusive) of the narrow scope.
        scope_start: u32,
        /// End byte position (exclusive) of the narrow scope.
        scope_end: u32,
        /// Type that replaces the variable's declared type in this scope.
        narrowed_type: RustType,
        /// Detection cause (typeof, instanceof, null check, truthy, ...).
        trigger: NarrowTrigger,
    },
    /// Narrow is invalidated at `position` due to `cause`.
    Reset {
        /// Variable whose narrow is reset.
        var_name: String,
        /// Byte position of the operation causing the reset.
        position: u32,
        /// Classification of the reset cause (see [`ResetCause`]).
        cause: ResetCause,
    },
    /// A closure captures the outer narrow for `var_name`.
    ///
    /// Emitted when the closure either reads or reassigns a variable that is
    /// narrowed in the enclosing scope. Consumers drive narrow suppression
    /// (`FileTypeResolution::is_var_closure_reassigned`) from this event,
    /// using [`enclosing_fn_body`](Self::ClosureCapture::enclosing_fn_body)
    /// for position-aware narrow suppression queries.
    ClosureCapture {
        /// Variable captured by the closure.
        var_name: String,
        /// Span of the enclosing function body where this capture event was
        /// detected.
        ///
        /// Defines the position range (`[lo, hi)`) within which this event is
        /// observable for narrow suppression queries
        /// (`FileTypeResolution::is_var_closure_reassigned`,
        /// `FileTypeResolution::narrowed_type`). The analyzer
        /// (`analyze_function(body, params)`) populates this with the function
        /// body's span passed to it. Multi-function scope isolation (I-169 P1)
        /// is structurally guaranteed by this field: a query at a position
        /// outside `enclosing_fn_body` does not match this event.
        enclosing_fn_body: Span,
    },
}

impl NarrowEvent {
    /// Variable name targeted by this event (common to all variants).
    #[must_use]
    pub fn var_name(&self) -> &str {
        match self {
            Self::Narrow { var_name, .. }
            | Self::Reset { var_name, .. }
            | Self::ClosureCapture { var_name, .. } => var_name,
        }
    }

    /// If this is a [`NarrowEvent::Narrow`] variant, returns a borrowed view
    /// of its fields; otherwise `None`.
    ///
    /// Convenience for consumers that filter scope-based narrow events
    /// (e.g., `FileTypeResolution::narrowed_type` and test assertions).
    #[must_use]
    pub fn as_narrow(&self) -> Option<NarrowEventRef<'_>> {
        match self {
            Self::Narrow {
                var_name,
                scope_start,
                scope_end,
                narrowed_type,
                trigger,
            } => Some(NarrowEventRef {
                var_name,
                scope_start: *scope_start,
                scope_end: *scope_end,
                narrowed_type,
                trigger,
            }),
            _ => None,
        }
    }
}

/// Borrowed view of a [`NarrowEvent::Narrow`] variant's fields.
///
/// Returned by [`NarrowEvent::as_narrow`]. All fields are read-only
/// references / copies so consumers can match against them without
/// destructuring the full enum.
#[derive(Debug, Clone, Copy)]
pub struct NarrowEventRef<'a> {
    /// Variable whose type is narrowed.
    pub var_name: &'a str,
    /// Start byte position (inclusive) of the narrow scope.
    pub scope_start: u32,
    /// End byte position (exclusive) of the narrow scope.
    pub scope_end: u32,
    /// Narrowed type active within this scope.
    pub narrowed_type: &'a RustType,
    /// Detection cause.
    pub trigger: &'a NarrowTrigger,
}

/// Primary (non-composite) narrow trigger.
///
/// Every narrow event originates from exactly one primary trigger —
/// typeof / instanceof / null-check / truthy / opt-chain invariant.
/// A *composite* trigger such as an early-return complement wraps a
/// [`PrimaryTrigger`] as its underlying cause. Splitting the primary
/// case out as its own enum makes this structure explicit at the type
/// level: nested [`NarrowTrigger::EarlyReturnComplement`] is unrepresentable.
///
/// # Scope
///
/// Maps to the subset of Problem Space dimension T that flows through
/// [`NarrowEvent::Narrow`]. Two T-dimension triggers are intentionally
/// handled by **separate architectures** and therefore have no variant
/// in this enum:
///
/// - **T6 `??=` narrowing** is driven by
///   [`EmissionHint`](crate::pipeline::narrowing_analyzer::EmissionHint)
///   dispatch at the Transformer (per `??=` site chooses ShadowLet vs
///   GetOrInsertWith emission), not by a narrow event. The analyzer's
///   [`ResetCause::NullishAssignOnNarrow`] covers the reset-side of
///   `??=` on an already-narrowed ident (no-op predicate elide).
/// - **T8 DU switch-case narrowing** is driven by the
///   `pipeline::type_resolver::du_analysis` module (internal to the resolver
///   pipeline — link omitted because the module is `pub(crate)`), which
///   emits tag-based match patterns directly in the Transformer.
///
/// If a future PRD migrates either to the narrowing_analyzer pipeline,
/// the corresponding variant should be added here with its populator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimaryTrigger {
    /// `typeof x === "string"` (T1). Payload is the typeof string literal.
    TypeofGuard(String),
    /// `x instanceof Foo` (T2). Payload is the class name.
    InstanceofGuard(String),
    /// `x == null` / `x != null` / `x === undefined` / ... (T3a-c). The
    /// [`NullCheckKind`] payload captures both the operator and the RHS
    /// (null vs undefined) so complement emission can reason precisely.
    NullCheck(NullCheckKind),
    /// `if (x)` / `if (!x)` truthy check (T4a-f, T9).
    Truthy,
    /// `x?.prop !== undefined` — OptChain non-null invariant (T7, T12).
    ///
    /// Narrows the **base** of the OptChain (`x`) from `Option<T>` to `T`.
    /// Populated by `guards::extract_optchain_null_check_narrowing` (T6-4).
    OptChainInvariant,
}

/// Why a narrow was introduced, including whether it came from an
/// early-return complement.
///
/// The two variants are **mutually exclusive**: a [`Primary`] is a direct
/// narrow from its guard, while an [`EarlyReturnComplement`] is a complement
/// derived from the inverse of a primary guard that exited (return / throw /
/// break / continue). Because `EarlyReturnComplement` wraps
/// [`PrimaryTrigger`] (not `NarrowTrigger`), nested complement is impossible
/// by construction — the single-level wrap matches the semantics of
/// TypeScript's early-return narrowing.
///
/// [`Primary`]: Self::Primary
/// [`EarlyReturnComplement`]: Self::EarlyReturnComplement
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarrowTrigger {
    /// Direct narrow from a primary guard in the consequent / alternate scope.
    Primary(PrimaryTrigger),
    /// Narrow in the fall-through scope after an early-exiting primary guard
    /// (e.g. `if (typeof x !== "string") return; /* here x is string */`, T11).
    /// Carries the original primary guard so downstream consumers can still
    /// reason about the specific shape that produced the complement.
    EarlyReturnComplement(PrimaryTrigger),
}

/// Shape of a null / undefined check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullCheckKind {
    /// `x == null` — covers both `null` and `undefined` (JS coercion).
    EqNull,
    /// `x != null` — covers both `null` and `undefined`.
    NotEqNull,
    /// `x === null` — strict `null` only.
    EqEqEqNull,
    /// `x !== null` — strict non-`null`.
    NotEqEqNull,
    /// `x === undefined` — strict `undefined` only.
    EqEqEqUndefined,
    /// `x !== undefined` — strict non-`undefined`.
    NotEqEqUndefined,
}

/// Classified reason a narrow may be reset.
///
/// Maps to Problem Space dimension R (R1a-R10 in the PRD).
///
/// Not every cause invalidates the narrow: see [`Self::invalidates_narrow`].
/// Compound arithmetic (`x += 1`), update expressions (`x++`), and
/// `??=`-on-narrow are **preserving** and do not trigger E2 emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetCause {
    /// `x = value` — direct reassign with a non-null RHS.
    DirectAssign,
    /// `x = null` / `x = undefined` — narrow re-widened to `Option`.
    NullAssign,
    /// `x += n` / `-=` / `*=` / `/=` / `%=` / `&=` / `|=` / `^=` / `<<=` /
    /// `>>=` / `>>>=` / `**=`. **Narrow-preserving**: arithmetic / bitwise
    /// operators on a numeric narrow keep the numeric type.
    CompoundArith,
    /// `x++` / `x--` / `++x` / `--x`. **Narrow-preserving** (numeric only).
    UpdateExpr,
    /// `x &&= y` / `x ||= y`. Narrow re-evaluated from RHS type.
    CompoundLogical,
    /// `x ??= y` when `x` is already narrowed to non-`null`.
    /// **Narrow-preserving** (runtime no-op; predicate elides).
    NullishAssignOnNarrow,
    /// The ident is reassigned inside a closure / nested fn / class member
    /// that captures it from an outer scope (R7 / C-2). Invalidates the
    /// outer shadow-let.
    ClosureReassign,
    /// `for-of` / `for-in` / `for (x = 0; ...; ...)` head rebinds the outer
    /// ident at each iteration (R8). Invalidating.
    LoopBoundary,
}

impl ResetCause {
    /// Returns `true` iff this cause makes the existing narrow state invalid
    /// (requires E2 emission instead of E1 shadow-let).
    pub const fn invalidates_narrow(&self) -> bool {
        matches!(
            self,
            Self::DirectAssign
                | Self::NullAssign
                | Self::CompoundLogical
                | Self::ClosureReassign
                | Self::LoopBoundary
        )
    }
}

/// Rust AST pattern selected for emission at a `??=` site.
///
/// Scope-limited to the two strategies the analyzer currently dispatches for
/// the T6 `??=` emission pipeline (`try_convert_nullish_assign_stmt`):
///
/// - [`ShadowLet`](Self::ShadowLet) — E1 `let x = x.unwrap_or(d);`, narrow
///   remains alive for the rest of the enclosing block.
/// - [`GetOrInsertWith`](Self::GetOrInsertWith) — E2a
///   `x.get_or_insert_with(|| d);`, keeps `x: Option<T>` so subsequent
///   resets / closure reassigns remain valid.
///
/// Problem Space dimension E covers 12 emission patterns (E1-E10 +
/// variants); the remaining ten (E2b `unwrap_or(coerce_default)`, E3
/// `if let Some`, E4 `match` exhaustive, E5 implicit `None`, E6 any-narrow
/// enum, E7 DU struct pattern, E8 union variant, E9 passthrough, E10 truthy
/// predicate, plus the `NullishAssignStrategy::{Identity,BlockedByI050}`
/// dispatch arms) are emitted directly by the Transformer (helpers /
/// pattern visitors) without flowing through this hint, so they have no
/// variant here (enumerating them as dead `Currently unused` variants
/// violates YAGNI). If a future PRD migrates an emission path to analyzer
/// dispatch, the corresponding variant is added here with its populator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionHint {
    /// E1: `let x = x.unwrap_or(d);`.
    ///
    /// Selected when the narrow remains alive for the rest of the enclosing
    /// block: no true reset, no closure reassign, LHS type is `Option<T>`.
    ShadowLet,
    /// E2a: `x.get_or_insert_with(|| d);`.
    ///
    /// Selected when a subsequent operation (direct reassign, null assign,
    /// closure reassign, logical compound, loop boundary) would invalidate a
    /// shadow-let. `x` stays `Option<T>` to preserve TS runtime semantics.
    GetOrInsertWith,
}
