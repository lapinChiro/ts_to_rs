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
//! - [`EmissionHint`] / [`RcContext`] hint the Transformer toward the
//!   right Rust AST pattern at each narrow-related site.

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
    /// narrowed in the enclosing scope. Consumers drive the Phase 3b emission
    /// policy (Policy A FnMut vs Policy B `Rc<RefCell<_>>`) from this event,
    /// and use [`enclosing_fn_body`](Self::ClosureCapture::enclosing_fn_body)
    /// for position-aware narrow suppression queries.
    ClosureCapture {
        /// Variable captured by the closure.
        var_name: String,
        /// Span of the closure expression.
        closure_span: Span,
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
        /// Narrowed type of the variable in the outer scope at capture time.
        ///
        /// Currently a `RustType::Any` placeholder populated by I-169 T6-2
        /// follow-up. Phase 3b (closure reassign emission policy) may resolve
        /// it to the actual outer narrow type later.
        outer_narrow: RustType,
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
/// typeof / instanceof / null-check / truthy / ??= / opt-chain invariant /
/// DU switch. A *composite* trigger such as an early-return complement
/// wraps a [`PrimaryTrigger`] as its underlying cause. Splitting the
/// primary case out as its own enum makes this structure explicit at the
/// type level: nested [`NarrowTrigger::EarlyReturnComplement`] is
/// unrepresentable.
///
/// Maps to Problem Space dimension T (T1-T10 in the PRD).
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
    /// `x ??= d` at a statement where `x` was nullable before the op (T6).
    ///
    /// Currently unused: populated by T6 when the Transformer wires in
    /// analyzer-sourced emission hints at `??=` sites.
    NullishAssign,
    /// `x?.prop !== undefined` — OptChain non-null invariant (T7, T12).
    ///
    /// Narrows the **base** of the OptChain (`x`) from `Option<T>` to `T`.
    /// Populated by `guards::extract_optchain_null_check_narrowing` (T6-4).
    OptChainInvariant,
    /// `switch (s.kind) { case "...": }` (T8). Payload is the discriminant tag.
    ///
    /// Currently unused: populated by T5 once DU switch-case narrowing is
    /// migrated from `du_analysis`.
    DiscriminatedUnion(String),
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

impl NarrowTrigger {
    /// Returns the underlying [`PrimaryTrigger`], regardless of whether the
    /// narrow was direct or a complement.
    #[must_use]
    pub fn primary(&self) -> &PrimaryTrigger {
        match self {
            Self::Primary(p) | Self::EarlyReturnComplement(p) => p,
        }
    }

    /// Returns `true` iff this trigger is an early-return complement.
    #[must_use]
    pub fn is_early_return_complement(&self) -> bool {
        matches!(self, Self::EarlyReturnComplement(_))
    }
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

/// Rust AST pattern selected for emission at a given narrow-related site.
///
/// Maps to Problem Space dimension E (E1-E10 in the PRD). See Sub-matrix 3/5
/// for the state → strategy mapping.
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
    /// E2b: `let v = x.unwrap_or(coerce_default(T));`.
    ///
    /// Currently unused: populated by T6 at narrow-stale reads (RC1 Expect-T
    /// after a closure reassign) using the JS coerce_default table.
    UnwrapOrCoerced,
    /// E2c: `x.as_ref().map(|v| ...)` — keep Option for direct manipulation.
    /// Currently unused (populated by T6).
    OptionPassthrough,
    /// E3: `if let Some(x) = x { ... }`. Currently unused (populated by T6).
    IfLetSome,
    /// E4: `match x { Some(v) => ..., None => ... }`. Currently unused
    /// (populated by T6).
    MatchExhaustive,
    /// E5: implicit `None` at reachable fall-off (I-025 structural fix).
    /// Currently unused (populated by T6).
    ImplicitNone,
    /// E6: any-narrowing enum variant match (I-030). Currently unused
    /// (populated by T6).
    AnyNarrowEnum,
    /// E7 / E8: DU struct pattern / union variant binding. Currently unused
    /// (populated by T6).
    VariantBinding,
    /// E9: passthrough — emission unchanged. Currently unused
    /// (populated by T6).
    Passthrough,
    /// E10: type-specific truthy predicate (`!x.is_empty()`, `matches!`).
    /// Currently unused (populated by T6).
    TruthyPredicate,
    /// `??=` on non-`Option` T — statement is dead code, emit nothing.
    /// Currently unused (populated by T6 alongside `pick_strategy`).
    Identity,
    /// `??=` on `Any` — blocked by I-050 Any coercion umbrella. Currently
    /// unused (populated by T6).
    BlockedByI050,
}

/// Context in which a narrow variable is read.
///
/// Mirrors Sub-matrix 5 (`emission-contexts.md` cluster). Currently
/// unused in production code — populated by T6 when the Transformer wires
/// in analyzer-sourced read context classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RcContext {
    /// Direct inner-T read (arithmetic, comparison, return expression, ...).
    ExpectT,
    /// `Option<T>` read (nullish coalescing LHS/RHS, OptChain receiver).
    ExpectOption,
    /// Mutation target (`=`, `??=`, `+=`, ...).
    Mutation,
    /// Boolean / truthy read (`if (x)`, logical operand).
    Boolean,
    /// Match discriminant (`switch (x)` scrutinee).
    MatchDiscriminant,
    /// String interpolation / concat.
    StringInterp,
    /// Callback body — narrow visibility governed by F8 closure rules.
    CallbackCapture,
    /// Passthrough (paren, expression statement).
    Passthrough,
}
