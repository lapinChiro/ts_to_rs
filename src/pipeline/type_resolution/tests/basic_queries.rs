//! Basic leaf-query tests for `FileTypeResolution`.
//!
//! Covers `expr_type`, `expected_type`, `is_mutable`, `is_du_field_binding`,
//! `emission_hint`, and foundational `narrowed_type` / `is_var_closure_reassigned`
//! invariants (innermost scope, variant filtering, `enclosing_fn_body` boundary).
//! Trigger-kind-based suppression dispatch (I-177-D) lives in
//! [`super::narrowing_suppression`]; canonical leaf type lookup (I-177-B) lives
//! in [`super::canonical_primitives`].

use super::super::position_in_range;
use super::super::*;

// ── position_in_range helper (boundary value analysis) ──
//
// Direct lock-in for the half-open `[lo, hi)` invariant the helper centralizes
// for `narrowed_type`, `any_enum_override`, `is_du_field_binding`, and
// `is_var_closure_reassigned`. Boundary tests document the interval semantics
// explicitly so a future change to the helper (e.g., switch to closed `[lo, hi]`)
// surfaces here before silently shifting all 4 consumer query semantics.

#[test]
fn position_in_range_inside_returns_true() {
    // Interior position — must match.
    assert!(position_in_range(15, 10, 20));
}

#[test]
fn position_in_range_at_lo_returns_true() {
    // Boundary `lo` inclusive — half-open interval matches `lo`.
    assert!(position_in_range(10, 10, 20));
}

#[test]
fn position_in_range_at_hi_returns_false() {
    // Boundary `hi` exclusive — `position == hi` is OUTSIDE the range.
    // This is the load-bearing invariant: narrow scopes / DU bindings /
    // closure-capture spans are all stored as `[start, end)` and the
    // exclusive `end` is what allows a same-line stmt boundary (e.g.,
    // `if-stmt.span.hi` == next-stmt.span.lo) to NOT belong to the if-stmt.
    assert!(!position_in_range(20, 10, 20));
}

#[test]
fn position_in_range_below_lo_returns_false() {
    // Position before scope start (sibling-fn case).
    assert!(!position_in_range(5, 10, 20));
}

#[test]
fn position_in_range_above_hi_returns_false() {
    // Position after scope end (sibling-fn case).
    assert!(!position_in_range(25, 10, 20));
}

#[test]
fn position_in_range_empty_range_always_false() {
    // Degenerate case `lo == hi`: empty interval, no position belongs.
    // Callers should not construct empty scopes, but the half-open
    // semantics guarantee safe `false` if one is somehow encountered
    // (rather than a special-case bug like `lo == position == hi → true`).
    assert!(!position_in_range(10, 10, 10));
    assert!(!position_in_range(0, 10, 10));
    assert!(!position_in_range(20, 10, 10));
}

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
