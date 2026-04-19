//! `NarrowTrigger` completeness tests: every push site must record the
//! correct [`PrimaryTrigger`] (with `NullCheckKind` where applicable),
//! and every early-return complement must wrap the original primary
//! trigger via [`NarrowTrigger::EarlyReturnComplement`].
//!
//! Also covers the DD-1 / DD-2 regression fixtures for Paren / TS wrapper
//! peeled null/undefined detection.

use super::*;
use crate::pipeline::narrowing_analyzer::{NarrowTrigger, NullCheckKind, PrimaryTrigger};

fn find_narrow_with<'a>(res: &'a FileTypeResolution, var: &str) -> Option<&'a NarrowTrigger> {
    res.narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .find(|n| n.var_name == var)
        .map(|n| n.trigger)
}

#[test]
fn typeof_guard_records_type_literal() {
    let res = resolve(
        r#"
            function foo(x: any) {
                if (typeof x === "string") { console.log(x); }
            }
            "#,
    );
    let trigger = find_narrow_with(&res, "x").expect("typeof narrow event");
    match trigger {
        NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard(s)) => assert_eq!(s, "string"),
        other => panic!("expected Primary(TypeofGuard(\"string\")), got {other:?}"),
    }
}

#[test]
fn loose_eq_null_records_eq_null_kind() {
    let res = resolve(
        r#"
            function foo(x: string | null) {
                if (x == null) { return; } else { console.log(x); }
            }
            "#,
    );
    let trigger = find_narrow_with(&res, "x").expect("null-check narrow");
    assert!(matches!(
        trigger,
        NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::EqNull))
    ));
}

#[test]
fn strict_eq_null_records_eq_eq_eq_null_kind() {
    let res = resolve(
        r#"
            function foo(x: string | null): string {
                if (x === null) { return "n"; } else { return x; }
            }
            "#,
    );
    let consequent_trigger = res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .find(|n| {
            n.var_name == "x"
                && matches!(
                    n.trigger,
                    NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::EqEqEqNull))
                )
        })
        .map(|n| n.trigger);
    assert!(
        consequent_trigger.is_some(),
        "expected a Primary(NullCheck(EqEqEqNull)) event; all triggers = {:?}",
        res.narrow_events
    );
}

#[test]
fn strict_not_eq_null_records_not_eq_eq_null_kind() {
    let res = resolve(
        r#"
            function foo(x: string | null) {
                if (x !== null) { console.log(x); }
            }
            "#,
    );
    assert!(res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| n.var_name == "x"
            && matches!(
                n.trigger,
                NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::NotEqEqNull))
            )));
}

#[test]
fn strict_eq_undefined_records_eq_eq_eq_undefined_kind() {
    let res = resolve(
        r#"
            function foo(x: string | undefined): string {
                if (x === undefined) { return "u"; } else { return x; }
            }
            "#,
    );
    assert!(res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| n.var_name == "x"
            && matches!(
                n.trigger,
                NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::EqEqEqUndefined))
            )));
}

#[test]
fn strict_not_eq_undefined_records_not_eq_eq_undefined_kind() {
    let res = resolve(
        r#"
            function foo(x: string | undefined) {
                if (x !== undefined) { console.log(x); }
            }
            "#,
    );
    assert!(res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| n.var_name == "x"
            && matches!(
                n.trigger,
                NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::NotEqEqUndefined))
            )));
}

#[test]
fn instanceof_records_class_name() {
    let res = resolve(
        r#"
            function foo(x: any) {
                if (x instanceof Error) { console.log(x); }
            }
            "#,
    );
    let trigger = find_narrow_with(&res, "x").expect("instanceof narrow");
    match trigger {
        NarrowTrigger::Primary(PrimaryTrigger::InstanceofGuard(class_name)) => {
            assert_eq!(class_name, "Error")
        }
        other => panic!("expected Primary(InstanceofGuard(\"Error\")), got {other:?}"),
    }
}

#[test]
fn truthy_records_truthy() {
    let res = resolve(
        r#"
            function foo(x: string | null) {
                if (x) { console.log(x); }
            }
            "#,
    );
    let trigger = find_narrow_with(&res, "x").expect("truthy narrow");
    assert!(matches!(
        trigger,
        NarrowTrigger::Primary(PrimaryTrigger::Truthy)
    ));
}

#[test]
fn early_return_typeof_wraps_original_trigger() {
    let res = resolve(
        r#"
            function foo(x: any): string {
                if (typeof x !== "string") { return ""; }
                return x;
            }
            "#,
    );
    let ok = res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| {
            n.var_name == "x"
                && matches!(
                    n.trigger,
                    NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::TypeofGuard(s))
                        if s == "string"
                )
        });
    assert!(
        ok,
        "expected EarlyReturnComplement(TypeofGuard(\"string\")); got {:?}",
        res.narrow_events
    );
}

#[test]
fn early_return_null_check_wraps_original_trigger() {
    let res = resolve(
        r#"
            function foo(x: string | null): string {
                if (x === null) { return ""; }
                return x;
            }
            "#,
    );
    let ok = res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| {
            n.var_name == "x"
                && matches!(
                    n.trigger,
                    NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::NullCheck(
                        NullCheckKind::EqEqEqNull
                    ))
                )
        });
    assert!(
        ok,
        "expected EarlyReturnComplement(NullCheck(EqEqEqNull)); got {:?}",
        res.narrow_events
    );
}

#[test]
fn early_return_instanceof_wraps_original_trigger() {
    // `compute_complement_type` only succeeds when the variable's type
    // is a synthetic union enum with multiple class variants. Use a
    // `Foo | Bar` union so the classifier can compute the complement.
    let res = resolve(
        r#"
            class Foo {}
            class Bar {}
            function foo(x: Foo | Bar): Bar {
                if (x instanceof Foo) { throw new Error("no"); }
                return x as Bar;
            }
            "#,
    );
    let ok = res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| {
            n.var_name == "x"
                && matches!(
                    n.trigger,
                    NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::InstanceofGuard(cls))
                        if cls == "Foo"
                )
        });
    assert!(
        ok,
        "expected EarlyReturnComplement(InstanceofGuard(\"Foo\")); got {:?}",
        res.narrow_events
    );
}

#[test]
fn paren_wrapped_null_in_not_eq_narrows_correctly() {
    // DD-1 regression lock-in: `x !== (null)` must detect the null RHS
    // even when Paren-wrapped, so a Narrow event is emitted for x in
    // the consequent.
    let res = resolve(
        r"
            function foo(x: string | null) {
                if (x !== (null)) { console.log(x); }
            }
            ",
    );
    assert!(
        narrow_views(&res)
            .any(|n| n.var_name == "x" && matches!(n.narrowed_type, RustType::String)),
        "expected Paren-wrapped null to be peeled and narrow x to String; events: {:?}",
        res.narrow_events
    );
}

#[test]
fn ts_as_null_in_strict_eq_narrows_in_else_branch() {
    // DD-1 regression lock-in: `x === null as any` must detect null
    // through the TS wrapper.
    let res = resolve(
        r"
            function foo(x: string | null): string {
                if (x === null as any) {
                    return 'n';
                } else {
                    return x;
                }
            }
            ",
    );
    assert!(
        narrow_views(&res)
            .any(|n| n.var_name == "x" && matches!(n.narrowed_type, RustType::String)),
        "expected `null as any` to be peeled and narrow x to String in else; events: {:?}",
        res.narrow_events
    );
}

#[test]
fn paren_wrapped_undefined_records_eq_eq_eq_undefined_kind() {
    // DD-2 regression lock-in: `x === (undefined)` must recognize
    // undefined (not null) and emit `EqEqEqUndefined`, not the
    // default `EqEqEqNull`.
    let res = resolve(
        r"
            function foo(x: string | undefined): string {
                if (x === (undefined)) {
                    return 'u';
                } else {
                    return x;
                }
            }
            ",
    );
    let has_undefined_kind = res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| {
            n.var_name == "x"
                && matches!(
                    n.trigger,
                    NarrowTrigger::Primary(PrimaryTrigger::NullCheck(
                        NullCheckKind::EqEqEqUndefined
                    ))
                )
        });
    assert!(
        has_undefined_kind,
        "expected Paren-wrapped undefined to populate EqEqEqUndefined kind; events: {:?}",
        res.narrow_events
    );
}

#[test]
fn early_return_negated_truthy_wraps_truthy() {
    let res = resolve(
        r#"
            function foo(x: string | null): string {
                if (!x) { return ""; }
                return x;
            }
            "#,
    );
    let ok = res
        .narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .any(|n| {
            n.var_name == "x"
                && matches!(
                    n.trigger,
                    NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy)
                )
        });
    assert!(
        ok,
        "expected EarlyReturnComplement(Truthy); got {:?}",
        res.narrow_events
    );
}
