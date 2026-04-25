//! `=== null` symmetric Let-wrap branch tests for
//! [`Transformer::try_generate_narrowing_match`] (deep-deep-deep-fix
//! retroactive fill of the `complement_is_none + is_swap + then_exit +
//! else_non_exit` branch + closure-reassign suppression).
//!
//! Counterpart to the Bang `!x` Layer 2 path tested in
//! [`super::bang_layer_2`]: both materialise post-if narrow when the
//! source has `then-exit + else-non-exit + Option<T> use post-if`, but
//! through different code paths (Bang via `OptionTruthyShape::
//! EarlyReturnFromExitWithElse` in
//! [`Transformer::try_generate_option_truthy_complement_match`];
//! `=== null` via the new branch in
//! [`Transformer::try_generate_narrowing_match`]).

use super::*;

/// I-171 T5 deep-deep-deep-fix: symmetric extension for `=== null`
/// early-return + non-exit-else + `Option<T>` return. The
/// `try_generate_narrowing_match` 4th branch
/// (`complement_is_none && is_swap && then_exits && !else_exits &&
/// else_body.is_some()`) emits a Let-wrap with tail expr — analogous to
/// `OptionTruthyShape::EarlyReturnFromExitWithElse` but for the
/// `=== null` if-let path.
///
/// Without this branch, the `=== null + else_non_exit` emission would
/// fall through to the bare `if let Some(x) = x { ... } else { return; }`
/// shape which scopes the narrow inside the `if let` block — leaving
/// post-if `x: Option<T>` and breaking the Some-wrap coerce that
/// `visitors.rs::visit_if_stmt` Deep-Deep-Fix-1 enables for the
/// `then-exits && !else-exits` case.
#[test]
fn null_check_then_exit_else_non_exit_lowers_to_let_match_with_narrow_tail() {
    let body = convert_named_fn_body(
        r#"
function h(x: number | null): number | null {
    if (x === null) {
        return -1;
    } else {
        // non-exit
    }
    return x;
}
"#,
        "h",
    );
    let arms = extract_let_match_arms(&body, 0, "x");
    assert_eq!(
        arms.len(),
        2,
        "expected 2 arms (None / Some(x)), got {arms:?}"
    );

    // Find the Some(x) arm (order is implementation-defined: my impl emits
    // None first, but the assertion is on the binding pattern).
    let some_arm = arms
        .iter()
        .find(|a| {
            matches!(
                &a.patterns[0],
                Pattern::TupleStruct {
                    ctor: crate::ir::PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
                    ..
                }
            )
        })
        .expect("expected a `Some(x)` arm");
    assert_arm_body_ends_with_tail_ident(some_arm, "x");

    // The None arm runs the source then_body (always-exits). Use
    // `is_none_unit` to identify the None pattern — `Pattern::none()`
    // constructs a tuple-struct Some-less pattern that `is_none_unit`
    // recognises.
    let none_arm = arms
        .iter()
        .find(|a| a.patterns[0].is_none_unit())
        .expect("expected a `None` arm");
    assert!(
        matches!(none_arm.body.last(), Some(Stmt::Return(_))),
        "None arm body must always-exit (e.g., Return), got {:?}",
        none_arm.body
    );
}

/// I-171 T5 P2 lock-in: the `=== null + then_exit + else_non_exit`
/// Let-wrap branch (Deep-Deep-Deep-Fix-1) must also honour
/// closure-reassign suppression, matching the pre-existing early-return
/// swap branch's behaviour and Bang Layer 2's P1 fix.
#[test]
fn null_check_then_exit_else_non_exit_with_closure_reassign_falls_through() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): number {
    if (x === null) {
        return -1;
    } else {
        // non-exit
    }
    const reset = () => { x = null; };
    reset();
    return x ?? 99;
}
"#,
        "f",
    );
    let first_if_or_let_match = body
        .iter()
        .find(|s| {
            matches!(s, Stmt::If { .. })
                || matches!(
                    s,
                    Stmt::Let {
                        init: Some(Expr::Match { .. }),
                        ..
                    }
                )
        })
        .expect("expected an If or Let-Match in the function body");
    assert!(
        matches!(first_if_or_let_match, Stmt::If { .. }),
        "expected `Stmt::If` (closure-reassign suppression in `=== null` \
         then-exit + else-non-exit Let-wrap branch), got {first_if_or_let_match:?}"
    );
}
