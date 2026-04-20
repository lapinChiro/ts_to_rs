//! I-144 T6-2 / I-169 T6-2 follow-up: closure-capture event population.
//!
//! Verifies that `analyze_function` returns `NarrowEvent::ClosureCapture`
//! entries for every outer ident reassigned inside an arrow / fn / class
//! member / object method body, while excluding shadowed-by-param idents
//! and purely read-only captures. Also covers the I-169 follow-up scope
//! isolation: multi-function same-name (P1), inner-fn local var leak (P2),
//! nested-fn shadow (R-2 / cell #25), and the shadow-regression lockins
//! (cells #26 / #27).

use super::*;
use crate::pipeline::narrowing_analyzer::NarrowEvent;

fn captured_var_names(result: &AnalysisResult) -> Vec<String> {
    let mut names: Vec<String> = result
        .closure_captures
        .iter()
        .map(|e| match e {
            NarrowEvent::ClosureCapture { var_name, .. } => var_name.clone(),
            _ => unreachable!("closure_captures should only contain ClosureCapture"),
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn arrow_reassigning_outer_emits_closure_capture_event() {
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            if (x === null) return -1;
            const reset = () => { x = null; };
            reset();
            return x + 1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn fn_expression_reassigning_outer_emits_closure_capture_event() {
    let r = analyze_first_fn(
        r"
        function f(x: number | null): number {
            const reset = function () { x = null; };
            reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn nested_fn_decl_reassigning_outer_emits_closure_capture_event() {
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            function reset() { x = null; }
            reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn closure_with_param_shadow_does_not_emit_event() {
    // I-169 matrix cell #26 (B15 closure param shadow regression lockin):
    // arrow's own param `x` shadows outer `let x`. The arrow body's
    // `x = 99` refers to the param, not outer. Walker shadow tracking
    // must remove `x` from active before classifying the arrow body.
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            const inner = (x: number) => { x = 99; return x; };
            return inner(5);
        }
        ",
    );
    assert!(
        captured_var_names(&r).is_empty(),
        "arrow param x shadows outer x — no event expected; got {:?}",
        captured_var_names(&r)
    );
}

#[test]
fn read_only_closure_does_not_emit_event() {
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            const reader = () => x;
            return reader() ?? -1;
        }
        ",
    );
    assert!(captured_var_names(&r).is_empty());
}

#[test]
fn nested_closures_each_emit_their_own_event_for_distinct_idents() {
    let r = analyze_first_fn(
        r#"
        function f(): number {
            let x: number | null = 5;
            let y: string | null = "init";
            const outer = () => {
                x = null;
                const inner = () => { y = null; };
                inner();
            };
            outer();
            return x ?? -1;
        }
        "#,
    );
    assert_eq!(
        captured_var_names(&r),
        vec!["x".to_string(), "y".to_string()]
    );
}

#[test]
fn class_method_reassigning_outer_emits_closure_capture_event() {
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            class C {
                reset() { x = null; }
            }
            new C().reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn object_method_reassigning_outer_emits_closure_capture_event() {
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            const obj = { reset() { x = null; } };
            obj.reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

// ------------------------------------------------------------------
// I-169 T6-2 follow-up: multi-fn isolation + inner-fn leak + nested-fn
// shadow (matrix cells #3 / #4 / #25 GREEN lockin)
// ------------------------------------------------------------------

/// Helper: parse source, analyze **both** top-level function decls, and
/// return their independent `AnalysisResult`s.
fn analyze_two_fns(source: &str) -> (AnalysisResult, AnalysisResult) {
    let module = crate::parser::parse_typescript(source).expect("fixture must parse");
    let mut fns = Vec::new();
    for item in &module.body {
        if let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = item {
            if let Some(body) = fn_decl.function.body.as_ref() {
                let params: Vec<&ast::Pat> =
                    fn_decl.function.params.iter().map(|p| &p.pat).collect();
                fns.push(crate::pipeline::narrowing_analyzer::analyze_function(
                    body, &params,
                ));
            }
        }
    }
    assert_eq!(fns.len(), 2, "fixture must declare exactly two functions");
    (fns.remove(0), fns.remove(0))
}

#[test]
fn two_functions_same_var_name_isolated() {
    // I-169 P1 (matrix cell #3): `f` has closure-reassign on `x`,
    // `g` has the same-named `let x` but NO closure-reassign. The two
    // analyses must be independent — f emits 1 capture event, g emits
    // none.
    let (f_result, g_result) = analyze_two_fns(
        r"
        function f(): number {
            let x: number | null = 5;
            if (x === null) return -1;
            const reset = () => { x = null; };
            reset();
            return x + 1;
        }
        function g(): number {
            let x: number | null = 10;
            if (x === null) return -2;
            return x + 1;
        }
        ",
    );
    assert_eq!(captured_var_names(&f_result), vec!["x".to_string()]);
    assert!(
        captured_var_names(&g_result).is_empty(),
        "g has no closure-reassign → 0 events; got {:?}",
        captured_var_names(&g_result)
    );
}

#[test]
fn inner_fn_local_var_does_not_leak_to_outer_events() {
    // I-169 P2 (matrix cell #4): outer has `let x`, inner fn has its own
    // `let z` (not in outer). The arrow inside inner reassigns z. Outer's
    // `analyze_function` candidate set is {x} only, so z is not emitted.
    let r = analyze_first_fn(
        r"
        function outer(): number {
            let x: number | null = 100;
            if (x === null) return -1;
            function inner() {
                let z: number | null = 5;
                if (z === null) return;
                const reset = () => { z = null; };
                reset();
            }
            inner();
            return x + 1;
        }
        ",
    );
    assert!(
        captured_var_names(&r).is_empty(),
        "outer's result must not include inner-local `z`; got {:?}",
        captured_var_names(&r)
    );
}

#[test]
fn nested_fn_shadow_does_not_emit_outer_event() {
    // I-169 R-2 (matrix cell #25): outer has `let x`, inner fn declares
    // its own `let x` (shadow), arrow inside inner reassigns the inner x.
    // The walker's active-candidate shadow tracking must remove x from
    // active when entering inner's body, so the arrow's reassign of x
    // does not get attributed to outer.
    let r = analyze_first_fn(
        r"
        function outer(): number {
            let x: number | null = 5;
            if (x === null) return -1;
            function inner() {
                let x: number | null = 10;
                const reset = () => { x = null; };
                reset();
            }
            inner();
            return x + 1;
        }
        ",
    );
    assert!(
        captured_var_names(&r).is_empty(),
        "outer's x must not be emitted — inner shadows x; got {:?}",
        captured_var_names(&r)
    );
}

// ------------------------------------------------------------------
// I-169 T7: 欠損 closure variant coverage (matrix cells #10 / #12 /
// #13 / #14 / #15) + shadow regression (cells #26 / #27)
// ------------------------------------------------------------------

#[test]
fn arrow_expr_body_reassigning_outer_emits_event() {
    // Matrix cell #15 / A2: arrow with single-expression body
    // (parenthesized assign). `() => (x = null)` still reassigns outer x.
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            const reset = () => (x = null);
            reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn object_getter_reassigning_outer_emits_event() {
    // Matrix cell #12 / A11: `{ get foo() { x = null; return 0; } }`
    // getter body reassigns outer x.
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            const obj = { get foo() { x = null; return 0; } };
            return obj.foo;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn object_setter_reassigning_outer_emits_event() {
    // Matrix cell #13 / A12: `{ set foo(v) { x = null; } }` setter body
    // reassigns outer x. Setter param `v` does NOT shadow `x`.
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            const obj = { set foo(v: number) { x = null; } };
            obj.foo = 1;
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn static_block_reassigning_outer_emits_event() {
    // Matrix cell #10 / A8: `static { x = null; }` reassigns outer x.
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            class C {
                static { x = null; }
            }
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn class_prop_init_reassigning_outer_emits_event() {
    // Matrix cell #14 / A9: `field = (x = null, 0)` — class prop init
    // expression reassigns outer x. The comma expression ensures the
    // init value is a number while the assign to x happens as a side
    // effect.
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            class C {
                field = (x = null, 0);
            }
            new C();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn fn_expr_self_name_shadow_does_not_emit_event() {
    // Matrix cell #27: `const fx = function x() { x = null; }` — the
    // named fn expression's self-name `x` shadows outer `let x` inside
    // the body. x = null targets the fn expression's own name (a
    // read-only binding in strict mode, but syntactically the
    // assignment LHS).
    let r = analyze_first_fn(
        r"
        function f(): number {
            let x: number | null = 5;
            const fx = function x() { x = null; };
            return x ?? -1;
        }
        ",
    );
    assert!(
        captured_var_names(&r).is_empty(),
        "fn expr self-name x shadows outer x — no event expected; got {:?}",
        captured_var_names(&r)
    );
}

// ------------------------------------------------------------------
// I-169 P3 regression lockin (matrix cells #16 / #17 / #18 candidate
// collection for destructured / default / rest param patterns).
// `collect_pat_idents` recurses through every supported `Pat` variant;
// these tests ensure each variant correctly seeds the outer candidate
// set so `collect_outer_candidates` emits events when the param's ident
// is reassigned inside a closure body.
// ------------------------------------------------------------------

#[test]
fn destructured_obj_param_emits_event() {
    // Matrix cell #16 / B2: `function f({ x }: { x: T | null })` —
    // destructured object param binds `x` as the candidate.
    let r = analyze_first_fn(
        r"
        function f({ x }: { x: number | null }): number {
            const reset = () => { x = null; };
            reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn destructured_array_param_emits_event() {
    // Matrix cell #16 / B3: `function f([x]: [T | null])` —
    // destructured array param binds `x` as the candidate.
    let r = analyze_first_fn(
        r"
        function f([x]: [number | null]): number {
            const reset = () => { x = null; };
            reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn param_with_default_emits_event() {
    // Matrix cell #17 / B4: `function f(x: T | null = null)` — default-
    // value wrapped param still binds `x` as the candidate (the
    // `Pat::Assign` wrapper is recursed through in `collect_pat_idents`).
    let r = analyze_first_fn(
        r"
        function f(x: number | null = null): number {
            const reset = () => { x = null; };
            reset();
            return x ?? -1;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["x".to_string()]);
}

#[test]
fn rest_param_emits_event_for_candidate_collection() {
    // Matrix cell #18 / B5: `function f(...xs: T[])` — rest param binds
    // `xs` as the candidate. The analyzer emits an event; downstream
    // coerce does NOT fire because `xs` is `Vec<T>`, not `Option<T>`
    // (the `matches!(ty, RustType::Option(_))` guard in
    // `maybe_coerce_for_arith` / `maybe_coerce_for_string_concat`
    // filters it out). This test locks in the analyzer-level candidate
    // collection for `Pat::Rest`.
    let r = analyze_first_fn(
        r"
        function f(...xs: number[]): number {
            const reset = () => { xs = []; };
            reset();
            return xs.length;
        }
        ",
    );
    assert_eq!(captured_var_names(&r), vec!["xs".to_string()]);
}
