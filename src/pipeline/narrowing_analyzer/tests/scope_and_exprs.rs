//! Block-level scope + expression-boundary tests.
//!
//! Exercises block-level shadowing (`let`/`const`/`var`/`function`/`class`
//! decl), VarDecl multi-decl left-to-right evaluation, expression-level
//! classification boundaries (TS wrappers / `Paren` / `Seq` / `Cond`
//! / tagged templates / call args / computed member keys), and the
//! read-only Sub-matrix 2 cells (pass-by-value, method call).

use super::*;

mod block_level_shadowing {
    use super::*;

    #[test]
    fn block_let_shadowing_skips_subsequent_stmts() {
        // After `let x = ...`, subsequent `x = null` refers to the local.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                let x = 99;
                x = 42;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn block_const_shadowing_skips_subsequent_stmts() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                const x = 99;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn block_let_init_reading_outer_is_classified_before_shadow() {
        // `let x = (x = null, 99);` — the comma expression reassigns outer
        // x BEFORE the new binding is created. We must detect it.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                let x: number = ((x = null), 99);
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn fn_decl_shadowing_skips_subsequent_stmts() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                function x(): number { return 1; }
                return x();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn class_decl_shadowing_skips_subsequent_stmts() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                class x { m(): number { return 1; } }
                return new x().m();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn nested_block_shadowing_does_not_leak_to_outer_block() {
        // Inner block's `let x = ...` is scoped — outer continues unshadowed.
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                { let x = 99; x = 42; }
                x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }
}

mod var_decl_multi_decl {
    use super::*;

    #[test]
    fn earlier_decl_init_reassigning_outer_before_shadow_is_detected() {
        // D-1 regression: `let a = ((x = null), 0), x = 5;`
        // decls[0] init runs first and reassigns outer x; decls[1] shadows.
        // The earlier init's invalidation must be captured.
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                let a = ((x = null), 0), x = 5;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn earlier_decl_reading_outer_does_not_invalidate() {
        // Read-only access to outer x in decls[0] — no cause. Shadow by
        // decls[1] means subsequent stmts are local.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                let a = x + 1, x = 5;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn shadow_then_later_decl_init_references_local_not_outer() {
        // `let x = 5, y = (x = null);` — decls[0] shadows outer x with 5.
        // decls[1] init `x = null` reassigns the LOCAL x, not outer. Outer
        // x is still narrow after `??=`.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                let x = 5, y = (x = null);
                return x ?? -1;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn single_decl_shadow_still_classifies_init() {
        // Single-decl shadow whose init reassigns outer (via comma expr):
        // `let x = ((x = null), 5);` — init side effect invalidates outer.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                let x: number = ((x = null), 5);
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn multi_decl_none_shadowing_continues_to_next_stmt() {
        // Neither decl shadows target. The outer `x = null` in decls[0]
        // init invalidates; subsequent stmts are classified normally.
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                let a = 1, b = 2;
                x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn multi_decl_no_shadow_no_reset_keeps_shadow_let() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                let a = 1, b = 2;
                return x + a + b;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }
}

mod expr_classification_boundary {
    use super::*;

    #[test]
    fn ts_as_wrapper_peeked_through() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                (x = null) as any;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn paren_expr_peeked_through() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                ((x = null));
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn seq_expr_detects_assign_in_any_slot() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                (0, x = null, 2);
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn tagged_template_peeked_through() {
        assert_hint(
            r#"
            function f(x: number | null): number | null {
                x ??= 10;
                String.raw`${x = null}`;
                return x;
            }
            "#,
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn call_arg_assign_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                console.log(x = null);
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn new_args_assign_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                new Error(String(x = null));
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn member_access_computed_key_detected() {
        assert_hint(
            r#"
            function f(x: number | null, obj: Record<string, number>): number {
                x ??= 10;
                const v = obj[x = null as any];
                return v;
            }
            "#,
            EmissionHint::GetOrInsertWith,
        );
    }
}

mod submatrix_read_only {
    use super::*;

    #[test]
    fn r6_pass_by_value_call_preserves_shadow_let() {
        // R6: `doSomething(x)` — argument is a read, narrow maintained.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                console.log(x);
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn r6_arbitrary_call_pass_preserves_shadow_let() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                g(x);
                return x;
            }
            function g(v: number): number { return v + 1; }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn r10_method_call_on_narrowed_preserves_shadow_let() {
        // R10: `x.method()` — read-through call, narrow maintained.
        assert_hint(
            r"
            function f(x: number | null): string {
                x ??= 10;
                return x.toString();
            }
            ",
            EmissionHint::ShadowLet,
        );
    }
}
