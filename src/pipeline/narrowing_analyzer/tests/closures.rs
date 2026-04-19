//! Closure / callable-capture boundary tests.
//!
//! Every callable boundary (arrow / fn expr / nested fn decl / class
//! method / ctor / prop init / static block / object method / getter /
//! setter) that captures the outer narrow must escalate any inner
//! mutation to `ClosureReassign`. Also verifies the per-function scope
//! contract (inner closures' `??=` sites are NOT detected from the outer
//! `analyze_function`) and closure-param-default classification.

use super::*;

mod closure_reassign {
    use super::*;

    #[test]
    fn arrow_closure_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const reset = () => { x = null; };
                reset();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn fn_expression_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const reset = function () { x = null; };
                reset();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn closure_arith_on_outer_forces_get_or_insert_with() {
        // Even narrow-preserving arith inside closure escapes to outer as
        // ClosureReassign because shadow-let cannot be re-mutated across
        // the closure boundary.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const inc = () => { x += 1; };
                inc();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn arrow_expr_body_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const reset = () => (x = null);
                reset();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn closure_not_reassigning_outer_keeps_shadow_let() {
        // Regression lock-in: a closure that merely reads the outer ident
        // must NOT force E2a.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const read = () => x;
                return read();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn closure_with_shadowing_param_does_not_escape() {
        // Closure param shadows the outer `x` — `x = 99` inside the closure
        // refers to the local parameter; outer narrow unaffected.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const mutate = (x: number) => { x = 99; return x; };
                return mutate(5);
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn fn_expr_with_shadowing_param_does_not_escape() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const mutate = function (x: number) { x = 99; return x; };
                return mutate(5);
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn fn_expr_with_self_name_shadowing_does_not_escape() {
        // `const inner = function x() { x = null; };` — the fn's self-name
        // `x` shadows the outer `x` inside the body.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                // @ts-ignore — TS warns on reassigning fn self-name, but the
                // shadow logic must still skip classification.
                const inner = function x(): number { return 1; };
                return inner();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn closure_let_decl_shadowing_does_not_escape() {
        // Top-level `let x = ...` inside closure shadows outer.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const inner = () => { let x = 99; x = 42; return x; };
                return inner();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn closure_var_decl_shadowing_does_not_escape() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const inner = () => { var x = 99; x = 42; return x; };
                return inner();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn nested_closure_reassign_detected() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const outer = () => { const inner = () => { x = null; }; inner(); };
                outer();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn nested_closure_shadow_in_inner_does_not_block_detection_of_outer_scope_access() {
        // Outer closure body references outer x. Inner closure shadows with
        // a param. The outer closure's body (before/after inner closure) can
        // still read/reassign outer x.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const outer = () => {
                    x = null;
                    const inner = (x: number) => { x = 99; };
                    inner(5);
                };
                outer();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }
}

mod class_descent {
    use super::*;

    #[test]
    fn class_method_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const C = class { m(): void { x = null; } };
                new C().m();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn class_constructor_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const C = class { constructor() { x = null; } };
                new C();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn class_prop_init_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const C = class { field = (x = null, 0); };
                new C();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn class_static_block_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const C = class { static { x = null; } };
                void C;
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn class_method_with_param_shadow_does_not_escape() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const C = class { m(x: number): number { x = 99; return x; } };
                return new C().m(5);
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn nested_class_decl_reassigning_outer_forces_get_or_insert_with() {
        // Class decl (stmt-level) with method reassigning outer.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                class C { m(): void { x = null; } }
                new C().m();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }
}

mod object_literal_descent {
    use super::*;

    #[test]
    fn object_method_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const obj = { mutate() { x = null; } };
                obj.mutate();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn object_getter_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const obj = { get v(): number { x = null; return 0; } };
                void obj.v;
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn object_setter_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const obj = { set v(_: number) { x = null; } };
                obj.v = 1;
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn object_method_with_param_shadow_does_not_escape() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const obj = { mutate(x: number) { x = 99; return x; } };
                return obj.mutate(5);
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn object_setter_with_param_shadow_does_not_escape() {
        // Setter param is named `x` — shadows outer inside body.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const obj = { set v(x: number) { x = 99; } };
                obj.v = 1;
                return x ?? -99;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }
}

mod closure_param_default {
    use super::*;

    #[test]
    fn arrow_param_default_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const h = (p: number = ((x = null), 5)) => p;
                h();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn fn_expr_param_default_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const h = function (p: number = ((x = null), 5)) { return p; };
                h();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn nested_fn_decl_param_default_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                function inner(p: number = ((x = null), 5)): number { return p; }
                inner();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn array_destructure_default_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const h = ([a = ((x = null), 7)]: number[]): number => a;
                h([]);
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn object_destructure_default_reassigning_outer_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 0;
                const h = ({ k = ((x = null), 7) }: { k?: number }): number => k;
                h({});
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn param_default_reading_outer_preserves_shadow_let() {
        // Lock-in: param default that only READS outer x (no reassign) must
        // NOT escalate to ClosureReassign.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 5;
                const h = (p: number = x): number => p;
                return h();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }
}

mod scope_contract {
    use super::*;

    #[test]
    fn arrow_body_nullish_assign_is_not_detected_from_outer_scope() {
        // Design contract: `analyze_function` is **per-function**. A `??=`
        // inside an arrow defined in the outer body belongs to the arrow's
        // own scope; the caller must invoke `analyze_function` on the
        // arrow's body separately. Lock this in so a regression (e.g. an
        // accidental closure-body descent in `recurse_into_nested_stmts`)
        // fails this test.
        let r = analyze_first_fn(
            r"
            function outer(): void {
                const f = (x: number | null): number => {
                    x ??= 10;
                    return x;
                };
                f(null);
            }
            ",
        );
        assert!(
            r.emission_hints.is_empty(),
            "arrow body's ??= belongs to arrow's own scope; outer analyze_function must skip"
        );
    }

    #[test]
    fn fn_expression_body_nullish_assign_is_not_detected_from_outer_scope() {
        assert_no_hint(
            r"
            function outer(): void {
                const f = function (x: number | null): number {
                    x ??= 10;
                    return x;
                };
                f(null);
            }
            ",
        );
    }

    #[test]
    fn nested_fn_decl_body_nullish_assign_is_not_detected_from_outer_scope() {
        assert_no_hint(
            r"
            function outer(): void {
                function inner(x: number | null): number {
                    x ??= 10;
                    return x;
                }
                inner(null);
            }
            ",
        );
    }

    #[test]
    fn class_method_body_nullish_assign_is_not_detected_from_outer_scope() {
        assert_no_hint(
            r"
            function outer(): void {
                class C {
                    m(x: number | null): number {
                        x ??= 10;
                        return x;
                    }
                }
                new C().m(null);
            }
            ",
        );
    }
}

mod submatrix_r9_nested_fn_decl {
    use super::*;

    #[test]
    fn nested_fn_decl_reassigning_outer_forces_get_or_insert_with() {
        // R9: nested fn decl body reassigns outer → ClosureReassign → E2a.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                function inner() { x = null; }
                inner();
                return x ?? -99;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn nested_fn_decl_reading_outer_preserves_shadow_let() {
        // Lock-in: read-only closure body does NOT invalidate.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                function inner(): number { return x; }
                return inner();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn nested_fn_decl_with_param_shadowing_does_not_escape() {
        // Nested fn has param that shadows target ident; body mutations
        // target the local param.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                function inner(x: number) { x = 99; return x; }
                return inner(5);
            }
            ",
            EmissionHint::ShadowLet,
        );
    }
}
