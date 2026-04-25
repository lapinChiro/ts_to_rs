//! Bang `!x` Layer 2 lowering tests for the consolidated-match emission
//! ([`Transformer::try_generate_option_truthy_complement_match`]).
//!
//! Covers all three [`OptionTruthyShape`] dispatch variants for primitive
//! and always-truthy inner types, peek-through wrappers, closure-reassign
//! suppression, and the cross-cutting Some-wrap-coerce-via-narrow-event
//! cohesion verification.
//!
//! Per-variant synthetic-union tests live in the sibling
//! [`super::synthetic_union`] module (separated for file-size budget).
//!
//! [`OptionTruthyShape`]: crate::transformer::statements::option_truthy_complement::OptionTruthyShape

use super::*;

// -----------------------------------------------------------------------------
// ElseBranch shape (no post-if narrow materialization)
// -----------------------------------------------------------------------------

/// I-171 T5 Matrix C-5a/c: `if (!x) A; else B` on `Option<F64>` lowers
/// to `match x { Some(x) if truthy => { else_body }, _ => { then_body } }`
/// — `Some(x)` shadow narrows `x` inside the else-arm only; the wildcard
/// arm keeps `x: Option<f64>`.
#[test]
fn option_f64_else_branch_lowers_to_match_with_shadow() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): string {
    if (!x) {
        return "falsy";
    } else {
        return `truthy:${x + 1}`;
    }
}
"#,
        "f",
    );
    assert_eq!(body.len(), 1, "expected single Stmt::Match, got {body:?}");
    let arms = extract_match_stmt_arms(&body, 0);
    assert_eq!(arms.len(), 2, "expected 2 arms, got {arms:?}");

    // Some(x) arm: pattern binding `x` shadows the outer Option<f64>.
    let Pattern::TupleStruct {
        ctor: crate::ir::PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
        fields,
    } = &arms[0].patterns[0]
    else {
        panic!("arm 0 must be `Some(...)`, got {:?}", arms[0].patterns[0]);
    };
    let Pattern::Binding { name, .. } = &fields[0] else {
        panic!("Some(...) must contain a binding, got {:?}", fields[0]);
    };
    assert_eq!(name, "x", "binding must shadow outer var name `x`");
    assert!(
        arms[0].guard.is_some(),
        "Some arm must carry a truthy guard"
    );

    // Wildcard arm: no binding, no guard.
    assert!(matches!(arms[1].patterns[0], Pattern::Wildcard));
    assert!(arms[1].guard.is_none());
}

/// I-171 T5: `else_body.is_some()` + `then_body` non-exit case also
/// routes through the consolidated-match path (the `ir_body_always_exits`
/// gate only applies when there is no else branch).
#[test]
fn else_branch_form_emits_match_even_with_non_exit_then() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): number {
    let result = 0;
    if (!x) {
        result = -1;
    } else {
        result = x + 1;
    }
    return result;
}
"#,
        "f",
    );
    // body[0] = `let result = 0`, body[1] = the `Stmt::Match`, body[2] = return.
    let arms = extract_match_stmt_arms(&body, 1);
    assert_eq!(arms.len(), 2);
    assert!(arms[0].guard.is_some(), "Some arm must carry truthy guard");
    assert!(matches!(arms[1].patterns[0], Pattern::Wildcard));
}

// -----------------------------------------------------------------------------
// Peek-through (Matrix C-12 / C-13)
// -----------------------------------------------------------------------------

/// I-171 T5 Matrix C-12: `peek_through_type_assertions` strips `TsAs`
/// in the if-test so `!(x as T)` routes through the same consolidated-
/// match narrow path as `!x`.
#[test]
fn early_return_form_peeks_through_ts_as_assertion() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): string {
    if (!(x as number | null)) return "none";
    return `ok:${x + 1}`;
}
"#,
        "f",
    );
    let arms = extract_let_match_arms(&body, 0, "x");
    assert_eq!(
        arms.len(),
        2,
        "expected 2 arms (Some(x) if truthy / _ => exit)"
    );
    assert!(
        arms[0].guard.is_some(),
        "Some arm must have truthy guard for primitive Option inner"
    );
    assert!(matches!(arms[1].patterns[0], Pattern::Wildcard));
}

/// I-171 T5 Matrix C-13: same path as C-12 but for `TsNonNull` (`!(x!)`).
#[test]
fn early_return_form_peeks_through_ts_non_null_assertion() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): string {
    if (!(x!)) return "none";
    return `ok:${x + 1}`;
}
"#,
        "f",
    );
    let arms = extract_let_match_arms(&body, 0, "x");
    assert_eq!(arms.len(), 2);
    assert!(arms[0].guard.is_some());
}

// -----------------------------------------------------------------------------
// EarlyReturnFromExitWithElse (post-/check_job deep-fix, Matrix C-5d)
// -----------------------------------------------------------------------------

/// I-171 T5 deep-fix Matrix C-5d sub-case: "then-always-exits +
/// else-non-exit" must materialise post-if narrow because TS narrows `x`
/// to `T` for any code reachable past the `if` (only the truthy else-
/// branch falls through; the then-branch always exits).
///
/// Bare `Stmt::Match` ElseBranch shape leaves `x: Option<T>` post-match,
/// so subsequent `x + 1` (or any `T`-typed use) fails to compile in
/// Rust. The `EarlyReturnFromExitWithElse` shape wraps the match in
/// `let x = ...;` and tail-emits the narrowed value so `x + 1` post-if
/// receives `x: f64`.
#[test]
fn then_exit_with_non_exit_else_lowers_to_let_match_with_narrow_tail() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): number {
    if (!x) {
        return -1;
    } else {
        // non-exit body — falls through to post-if so narrow must
        // materialise as `x: f64` for the `x + 1` line below.
    }
    return x + 1;
}
"#,
        "f",
    );
    assert!(
        body.len() >= 2,
        "expected at least Let + post-if return, got {body:?}"
    );
    let arms = extract_let_match_arms(&body, 0, "x");
    assert_eq!(arms.len(), 2, "expected 2 arms (Some(x) if truthy / _)");

    // Some arm: pattern `Some(x)` shadow + body ending in `TailExpr(Ident("x"))`.
    let Pattern::TupleStruct {
        ctor: crate::ir::PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
        fields,
    } = &arms[0].patterns[0]
    else {
        panic!("Some arm pattern mismatch: {:?}", arms[0].patterns[0]);
    };
    let Pattern::Binding { name, .. } = &fields[0] else {
        panic!("Some(...) pattern must bind, got {:?}", fields[0]);
    };
    assert_eq!(name, "x", "Some(...) pattern must shadow outer var name");
    assert!(arms[0].guard.is_some(), "Some arm must carry truthy guard");

    assert_arm_body_ends_with_tail_ident(&arms[0], "x");

    // Wildcard arm runs the source then_body (always-exits).
    assert!(matches!(arms[1].patterns[0], Pattern::Wildcard));
    assert!(arms[1].guard.is_none());
    assert!(
        matches!(arms[1].body.last(), Some(Stmt::Return(_))),
        "wildcard arm body must always-exit (e.g., Return), got {:?}",
        arms[1].body
    );
}

// -----------------------------------------------------------------------------
// Always-truthy path (post-/check_job deep-deep-deep-deep-fix, Matrix C-3 ext.)
// -----------------------------------------------------------------------------

/// I-171 T5 deep-deep-deep-deep-fix: Bang `!x` × `Option<Named other>`
/// (interface / class / non-synthetic enum) must materialise post-if
/// narrow via a single `Some(x) => <body>` arm WITHOUT a truthy guard
/// (JS always-truthy when `Some` for object references).
///
/// PRD Matrix C-3 enumerated this as "✓ T6-3" but `build_option_truthy_match_arms`
/// returned `None` for non-synthetic-union `Named` (since
/// `build_union_variant_truthy_arms` returns `None` for non-`UnionEnum`
/// kinds), falling back to Layer 1's `if x.is_none() { exit }` form
/// without IR shadow rebinding. Post-if access to `x.field` then failed
/// (E0609 on `Option<Named>`).
#[test]
fn bang_option_named_other_lowers_to_let_match_with_always_truthy_arm() {
    use crate::ir::PatternCtor;

    let body = convert_named_fn_body(
        r#"
interface Tag { label: string; }
function f(x: Tag | null): string {
    if (!x) return "no";
    return x.label;
}
"#,
        "f",
    );
    let arms = extract_let_match_arms(&body, 0, "x");
    assert_eq!(arms.len(), 2, "expected 2 arms (Some(x) / wildcard)");

    let Pattern::TupleStruct {
        ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
        ..
    } = &arms[0].patterns[0]
    else {
        panic!("Some arm pattern mismatch: {:?}", arms[0].patterns[0]);
    };
    assert!(
        arms[0].guard.is_none(),
        "Named other Some(x) arm must NOT carry a truthy guard (JS \
         always-truthy when Some); got {:?}",
        arms[0].guard
    );
    assert_arm_body_ends_with_tail_ident(&arms[0], "x");
}

/// I-171 T5 deep-deep-deep-deep-fix: same always-truthy path applies to
/// `Option<Vec<T>>`, `Option<Fn>`, `Option<Tuple>`, `Option<StdCollection>`,
/// `Option<DynTrait>`, `Option<Ref>`. Lock-in via Vec representative.
#[test]
fn bang_option_vec_lowers_to_let_match_with_always_truthy_arm() {
    let body = convert_named_fn_body(
        r#"
function f(x: number[] | null): number {
    if (!x) return -1;
    return x.length;
}
"#,
        "f",
    );
    let arms = extract_let_match_arms(&body, 0, "x");
    assert_eq!(arms.len(), 2);
    assert!(
        arms[0].guard.is_none(),
        "Option<Vec<T>> Some arm must be guard-less"
    );
}

/// I-171 T5 deep-deep-deep-deep-fix: the always-truthy path also applies
/// to the `EarlyReturnFromExitWithElse` sub-case (then_exit + else_non_exit)
/// for `Option<Named other>`. Post-if narrow materialises so `x.field`
/// access compiles.
#[test]
fn then_exit_else_non_exit_option_named_other_threads_narrow_through_outer_let() {
    let body = convert_named_fn_body(
        r#"
interface Tag { label: string; }
function g(x: Tag | null): string {
    if (!x) {
        return "no";
    } else {
        // non-exit
    }
    return x.label;
}
"#,
        "g",
    );
    let arms = extract_let_match_arms(&body, 0, "x");
    assert_eq!(arms.len(), 2);
    assert!(arms[0].guard.is_none());
    assert_arm_body_ends_with_tail_ident(&arms[0], "x");
}

// -----------------------------------------------------------------------------
// Cross-cutting cohesion: Some-wrap-coerce via narrow event (Deep-Deep-Fix-1)
// -----------------------------------------------------------------------------

/// I-171 T5 deep-deep-fix: the `!x` Layer 2 EarlyReturnFromExitWithElse
/// path needs TypeResolver to also record the narrow event in post-if
/// scope so that *type-driven* coercions (like `Option<T>` return-type
/// re-wrapping with `Some(x)`) observe the narrow `T` and fire correctly.
///
/// Without the visitors.rs `visit_if_stmt` extension (then-exits &&
/// !else-exits → push narrow event), TypeResolver leaves `x: Option<T>`
/// in post-if scope, causing `return x;` against an `Option<T>` return
/// type to emit raw `return x` (no Some-wrap) which fails because the
/// IR-shadow makes `x: T`.
///
/// Lock-in: the emission must include `Some(x)` after the let-match.
#[test]
fn then_exit_else_non_exit_with_option_return_emits_some_wrap_via_narrow_event() {
    let body = convert_named_fn_body(
        r#"
function h(x: number | null): number | null {
    if (!x) {
        return -1;
    } else {
        // non-exit
    }
    return x;
}
"#,
        "h",
    );
    assert!(
        body.len() >= 2,
        "expected at least Let + return, got {body:?}"
    );
    // Function-tail return may be elided to `Stmt::TailExpr(...)`. Accept
    // either form — the lock-in target is the `Some(x)` wrapper.
    let last_stmt = &body[body.len() - 1];
    let ret_expr = match last_stmt {
        Stmt::Return(Some(e)) | Stmt::TailExpr(e) => e,
        other => panic!("expected last stmt to be `return <expr>` or `<expr>` tail, got {other:?}"),
    };
    let lowered = format!("{ret_expr:?}");
    assert!(
        lowered.contains("Some") && (lowered.contains("Ident(\"x\")") || lowered.contains("\"x\"")),
        "return expression must wrap `x` in `Some(...)` (TypeResolver narrow + IR shadow + \
         Some-wrap coerce cohesion check). Got: {ret_expr:?}"
    );
}

// -----------------------------------------------------------------------------
// Closure-reassign suppression (post-/check_problem audit P1)
// -----------------------------------------------------------------------------

/// I-171 T5 P1 lock-in: Layer 2 must suppress the let-match emission
/// when the variable has a closure-reassign in scope, falling through
/// to Layer 1's predicate form (`if !x.is_some_and(...) { ... }`) so
/// the outer `Option<T>` binding stays alive for the closure to reassign.
///
/// Pre-fix: Layer 2 unconditionally emitted `let x = match x { Some(x) if
/// truthy => x, _ => exit };` which shadowed `x` to immutable inner `T`,
/// breaking subsequent closure-reassigned `x = null`.
#[test]
fn bang_with_closure_reassign_falls_through_to_predicate_form() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): number {
    if (!x) return -1;
    const reset = () => { x = null; };
    reset();
    return x ?? 99;
}
"#,
        "f",
    );
    // The function body has a closure-capture `let mut x = x;` rebinding
    // at body[0] (auto-inserted for closure-reassigned vars). The
    // narrow-emission target is the FIRST `Stmt::If` after that rebinding
    // — it must be the Layer 1 predicate form (closure-reassign
    // suppression), NOT a `Stmt::Let { init: Some(Match) }` (Layer 2
    // narrow materialisation).
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
        "expected `Stmt::If` (Layer 1 predicate fall-through, closure-reassign \
         suppression), got {first_if_or_let_match:?}"
    );
}

// -----------------------------------------------------------------------------
// Layer 2 fall-through (Matrix C-4)
// -----------------------------------------------------------------------------

/// I-171 T5: when neither `else_body` is present nor `then_body always-exits`,
/// the helper returns `None` so Layer 1's `falsy_predicate_for_expr`
/// emits the predicate-form `if <falsy(x)> { body }` — the Matrix C-4
/// ideal shape.
#[test]
fn non_exit_no_else_falls_through_to_predicate_form() {
    let body = convert_named_fn_body(
        r#"
function f(x: number | null): string {
    if (!x) {
        console.log("falsy_side_effect");
    }
    return x === null ? "null" : `${x}`;
}
"#,
        "f",
    );
    assert!(
        matches!(body[0], Stmt::If { .. }),
        "expected Stmt::If predicate form, got {:?}",
        body[0]
    );
}
