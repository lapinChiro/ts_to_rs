//! I-177-D trigger-kind-based suppression dispatch tests.
//!
//! 案 C (PRD I-177-D): closure-reassign suppression は narrow event の trigger 種別で
//! dispatch する。
//!
//! - **Primary narrow** (`if (x !== null) { /* cons-span */ }` 内): suppression 対象外。
//!   narrow を保持して IR shadow form (`if let Some(x) = x { ... }`) と TypeResolver
//!   narrow の cohesion を確立する。
//! - **EarlyReturnComplement narrow** (`if (x === null) return; /* post-if */`):
//!   suppression 維持。post-if scope は closure call で runtime に narrow が
//!   invalidate されうる構造のため、`coerce_default` workaround を発動させる。
//!
//! 以下の 10 test は matrix cells #2, 6, 10, 14, 18 (主 fix) +
//! #4, 8, 12, 16, 20 (suppress preserve) を direct 検証する。

use super::super::*;

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
