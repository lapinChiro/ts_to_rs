//! Nested control-flow `??=` emission-hint tests.
//!
//! Exercises `??=` sites under branch-merging constructs (`if` cons/alt,
//! `switch` cases, `try`/catch/finally, loops) and the
//! unreachable-code pruning after always-exiting statements.

use super::*;

mod nullish_assign_nested {
    use super::*;

    #[test]
    fn reset_in_if_then_branch_detected() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) {
                    x = null;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn reset_in_if_else_branch_detected() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) {
                    // nothing
                } else {
                    x = null;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn reset_in_nested_block_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                {
                    x = 42;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn reset_in_switch_case_body_detected() {
        assert_hint(
            r"
            function f(x: number | null, k: number): number | null {
                x ??= 10;
                switch (k) {
                    case 0: x = null; break;
                    default: break;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn reset_in_try_body_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                try {
                    x = null;
                } catch (_e) {}
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn reset_in_catch_body_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                try {
                    // nothing
                } catch (_e) {
                    x = null;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn reset_in_finally_block_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                try {
                    // nothing
                } finally {
                    x = null;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn preserving_arith_in_while_body_keeps_shadow_let() {
        // C-2 fix: `x += 1` inside loop body is narrow-preserving; previous
        // loop_escape policy incorrectly escalated this to LoopBoundary.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                while (x < 100) {
                    x += 1;
                }
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn preserving_arith_in_do_while_body_keeps_shadow_let() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                do {
                    x += 1;
                } while (x < 100);
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn invalidating_assign_in_while_body_forces_get_or_insert_with() {
        // Regression lock-in: invalidating body ops stay invalidating even
        // inside a loop, regardless of the loop_escape fix.
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                while (Math.random() > 0.5) {
                    x = null;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn for_of_rebinding_outer_classified_as_loop_boundary() {
        assert_hint(
            r"
            function f(x: number | null, arr: number[]): number | null {
                x ??= 10;
                for (x of arr) {}
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn for_of_with_const_does_not_reset_outer() {
        // `for (const x of arr)` introduces a fresh block binding — does NOT
        // reset the outer narrow.
        assert_hint(
            r"
            function f(x: number | null, arr: number[]): number {
                x ??= 10;
                for (const x of arr) { console.log(x); }
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn for_of_with_let_does_not_reset_outer() {
        assert_hint(
            r"
            function f(x: number | null, arr: number[]): number {
                x ??= 10;
                for (let x of arr) { console.log(x); }
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn for_init_let_shadowing_skips_body_classification() {
        // `for (let x = 0; ...) { x = null; }` — the body's x is local,
        // outer narrow unaffected.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                for (let x = 0; x < 10; x++) { x = 99; }
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn hint_produced_for_nullish_assign_inside_nested_block() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number {
                if (flag) {
                    x ??= 10;
                    return x;
                }
                return 0;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }
}

mod branch_merge {
    use super::*;

    #[test]
    fn if_cons_preserving_alt_invalidating_detects_invalidation() {
        // Critical bug fix: old `.or_else` pattern short-circuited on cons.
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) {
                    x += 1;
                } else {
                    x = null;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn if_cons_invalidating_alt_preserving_detects_invalidation() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) {
                    x = null;
                } else {
                    x += 1;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn if_both_branches_preserving_keeps_shadow_let() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number {
                x ??= 10;
                if (flag) {
                    x += 1;
                } else {
                    x *= 2;
                }
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn if_both_branches_invalidating_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) {
                    x = null;
                } else {
                    x = undefined;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn switch_case_0_preserving_case_1_invalidating_detects_invalidation() {
        assert_hint(
            r"
            function f(x: number | null, k: number): number | null {
                x ??= 10;
                switch (k) {
                    case 0: x += 1; break;
                    case 1: x = null; break;
                    default: break;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn switch_all_cases_preserving_keeps_shadow_let() {
        assert_hint(
            r"
            function f(x: number | null, k: number): number {
                x ??= 10;
                switch (k) {
                    case 0: x += 1; break;
                    case 1: x -= 2; break;
                    default: x *= 3; break;
                }
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn try_body_preserving_handler_invalidating_detects_invalidation() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                try {
                    x += 1;
                } catch (_e) {
                    x = null;
                }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn cond_expr_branch_invalidation_detected() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                flag ? (x = null) : 0;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn cond_expr_both_branches_invalidating_detected() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                flag ? (x = null) : (x = undefined);
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }
}

mod unreachable_code {
    use super::*;

    #[test]
    fn throw_makes_subsequent_reset_unreachable() {
        assert_hint(
            r#"
            function f(x: number | null): number {
                x ??= 10;
                throw new Error("stop");
                x = null;
                return x ?? -1;
            }
            "#,
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn return_makes_subsequent_reset_unreachable() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                return x;
                x = null;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn break_in_loop_makes_subsequent_reset_unreachable() {
        assert_hint(
            r"
            function f(x: number | null): number {
                while (true) {
                    x ??= 10;
                    break;
                    x = null;
                }
                return x ?? -1;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn continue_in_loop_makes_subsequent_reset_unreachable() {
        assert_hint(
            r"
            function f(x: number | null): number {
                while (true) {
                    x ??= 10;
                    continue;
                    x = null;
                }
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn exhaustive_if_else_exit_prunes_subsequent_reset() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number {
                x ??= 10;
                if (flag) { return 1; } else { return 2; }
                x = null;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn non_exhaustive_if_does_not_prune_subsequent_reset() {
        // Only then-branch exits; alt is absent → if may fall through.
        // Subsequent `x = null` IS reachable.
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) return 1;
                x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn if_one_branch_exits_other_does_not_does_not_prune() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) { return 1; } else { /* no exit */ }
                x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn reassign_within_always_exit_branch_still_classified() {
        // Even though `return` makes the `x = null` unreachable AFTER the
        // if, the reassign INSIDE the if branch DOES execute on the taken
        // path. So `if (flag) { x = null; return; }` still invalidates.
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) { x = null; return x; }
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }
}
