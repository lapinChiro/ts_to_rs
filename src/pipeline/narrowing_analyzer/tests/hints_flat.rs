//! Flat-flow `??=` emission-hint tests.
//!
//! Exercises the hint-finder at the **top-level stmt list** of a
//! function body — sequential `??=` sites with following mutations /
//! preserving ops in the same scope, plus structural smoke tests
//! (non-ident LHS skipped, empty body, etc.) and the span-keying
//! invariant.

use super::*;

mod nullish_assign_flat {
    use super::*;

    #[test]
    fn clean_narrow_emits_shadow_let() {
        // No following mutation → shadow-let is safe.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn compound_arith_preserves_shadow_let() {
        // Sub-matrix 2 cell L1 × R2a (C-1): `x += 1` is narrow-preserving.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                x += 1;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn compound_bitwise_preserves_shadow_let() {
        // R2b: bitwise compound on numeric narrow is preserving.
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                x &= 1;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn update_expr_preserves_shadow_let() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                x++;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn predecrement_preserves_shadow_let() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                --x;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn direct_assign_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number {
                x ??= 10;
                x = 42;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn null_assign_forces_get_or_insert_with() {
        // I-142 Cell #14: linear `x = null` reset.
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn undefined_assign_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                x = undefined;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn ts_as_null_rhs_classified_as_null_assign() {
        // C-15: `x = null as any` must peel TS wrappers; otherwise the
        // null-RHS distinction is lost and we'd classify as DirectAssign.
        //
        // GetOrInsertWith is asserted here (both NullAssign and DirectAssign
        // invalidate). Shape-level classification is verified via direct
        // classifier tests elsewhere.
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                x = null as any;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn logical_compound_and_forces_get_or_insert_with() {
        // R4: `x &&= y` re-evaluates narrow.
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                x &&= 3;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn logical_compound_or_forces_get_or_insert_with() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                x ||= 7;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn second_nullish_assign_on_narrow_is_preserving() {
        let r = analyze_first_fn(
            r"
            function f(x: number | null): number {
                x ??= 10;
                x ??= 20;
                return x;
            }
            ",
        );
        assert_eq!(r.emission_hints.len(), 2);
        assert!(r
            .emission_hints
            .values()
            .all(|h| *h == EmissionHint::ShadowLet));
    }

    #[test]
    fn non_target_mutation_does_not_affect_hint() {
        assert_hint(
            r"
            function f(x: number | null): number {
                let y = 0;
                x ??= 10;
                y = 99;
                y += 1;
                return x;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn assigning_to_member_does_not_reset_ident() {
        // `x.v = 1` is a field write, not a reassign of `x` itself.
        assert_hint(
            r"
            function f(x: { v: number } | null): number {
                x ??= { v: 5 };
                x.v = 99;
                return x.v;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }
}

mod single_stmt_body {
    use super::*;

    #[test]
    fn braceless_if_body_with_reset_is_detected() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn braceless_if_body_with_nullish_assign_generates_hint() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number {
                if (flag) x ??= 10;
                return x ?? -1;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn braceless_else_body_with_reset_is_detected() {
        assert_hint(
            r"
            function f(x: number | null, flag: boolean): number | null {
                x ??= 10;
                if (flag) {} else x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn braceless_while_body_with_reset_is_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                while (Math.random() > 0.5) x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn braceless_while_body_with_nullish_assign_generates_hint() {
        assert_hint(
            r"
            function f(x: number | null): number {
                while (x === null) x ??= 10;
                return x ?? -1;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn braceless_for_body_with_reset_is_detected() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                for (let i = 0; i < 3; i++) x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }

    #[test]
    fn braceless_for_of_body_with_nullish_assign_generates_hint() {
        assert_hint(
            r"
            function f(arr: number[]): number {
                let x: number | null = null;
                for (const i of arr) x ??= i;
                return x ?? -1;
            }
            ",
            EmissionHint::ShadowLet,
        );
    }

    #[test]
    fn braceless_labeled_stmt_body_hint_generation() {
        assert_hint(
            r"
            function f(x: number | null): number | null {
                x ??= 10;
                outer: if (true) x = null;
                return x;
            }
            ",
            EmissionHint::GetOrInsertWith,
        );
    }
}

mod non_target_stmts {
    use super::*;

    #[test]
    fn member_nullish_assign_skipped() {
        assert_no_hint(
            r"
            function f(obj: { v: number | null }): number {
                obj.v ??= 10;
                return obj.v;
            }
            ",
        );
    }

    #[test]
    fn plain_assign_skipped() {
        assert_no_hint(
            r"
            function f(x: number | null): number | null {
                x = 10;
                return x;
            }
            ",
        );
    }

    #[test]
    fn logical_assign_skipped() {
        assert_no_hint(
            r"
            function f(x: number | null): number | null {
                x &&= 10;
                return x;
            }
            ",
        );
    }

    #[test]
    fn empty_body_produces_no_hints() {
        // Covers both `emission_hints` and `events` being empty — distinct
        // from `assert_no_hint` which asserts only the former.
        let r = analyze_first_fn(r"function f(): void {}");
        assert!(r.emission_hints.is_empty());
        assert!(r.events.is_empty());
    }
}

mod hint_keying {
    use super::*;

    #[test]
    fn hint_keyed_at_stmt_span_start() {
        let src = r"
function f(x: number | null): number {
    x ??= 10;
    return x;
}
";
        let module = parse_typescript(src).expect("parse");
        let body = find_first_fn_body(&module).expect("fn body");

        // Locate the `??=` stmt span manually for cross-check.
        let expected_start = body
            .stmts
            .iter()
            .find_map(|s| match s {
                ast::Stmt::Expr(e)
                    if matches!(
                        e.expr.as_ref(),
                        ast::Expr::Assign(a) if a.op == ast::AssignOp::NullishAssign
                    ) =>
                {
                    Some(e.span.lo.0)
                }
                _ => None,
            })
            .expect("`??=` stmt must exist");

        let r = NarrowingAnalyzer::new().analyze_function(body);
        assert!(
            r.emission_hints.contains_key(&expected_start),
            "hint should be keyed at `??=` stmt span start {expected_start}, got {:?}",
            r.emission_hints
        );
    }

    #[test]
    fn two_sibling_nullish_assigns_each_get_own_hint() {
        let r = analyze_first_fn(
            r"
            function f(x: number | null, y: number | null): number {
                x ??= 10;
                y ??= 20;
                y = 99;
                return x + y;
            }
            ",
        );
        assert_eq!(r.emission_hints.len(), 2);
        let mut values: Vec<_> = r.emission_hints.values().copied().collect();
        values.sort_by_key(|h| format!("{h:?}"));
        assert_eq!(
            values,
            vec![EmissionHint::GetOrInsertWith, EmissionHint::ShadowLet]
        );
    }
}
