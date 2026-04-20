//! Integration tests for I-142 (`??=` NullishAssign rewrite).
//!
//! Each test matches a cell in the Problem Space matrix of
//! `backlog/I-142-nullish-assign-shadow-let.md`. Cells share the same emission
//! path (`try_convert_nullish_assign_stmt` for statement context,
//! `convert_assign_expr` for expression context) so we drive them through the
//! full TypeResolver → Transformer → Generator pipeline via `TctxFixture`.
//!
//! The generator output is asserted as substring matches rather than full
//! snapshots — the fixture-level snapshot at `tests/snapshots/` and the
//! compile_test suite own the whole-file regression lock.

use super::*;

/// Extract the body of a function-scoped fixture as generated Rust.
///
/// `fixture_source` must define exactly one function; the generated Rust is
/// returned verbatim so tests can assert on specific substrings.
fn generate(fixture_source: &str) -> String {
    let f = TctxFixture::from_source(fixture_source);
    let (_items, output) = f.transform(fixture_source);
    output
}

// -----------------------------------------------------------------------------
// Cell #1: Option<T> LHS, statement context, fusible (prior `let val = init`).
// -----------------------------------------------------------------------------

#[test]
fn cell1_fusible_option_number_lhs_emits_single_unwrap_or() {
    let src = r#"
        function ensureDefault(x: number | null): number {
            let val = x;
            val ??= 0;
            return val;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("let val = x.unwrap_or(0.0);"),
        "expected fused `let val = x.unwrap_or(0.0);`, got:\n{out}"
    );
    // Two-let shape must NOT survive after fusion.
    assert!(
        !out.contains("let val = x;\n    let val = val.unwrap_or"),
        "fusion must collapse the shadow-let pair"
    );
    // Broken legacy shape must not re-emerge.
    assert!(
        !out.contains("val.get_or_insert_with"),
        "statement-context ??= must not emit `get_or_insert_with`"
    );
}

// -----------------------------------------------------------------------------
// Cell #2: Option<T> LHS, statement context, non-fusible (no adjacent let).
// -----------------------------------------------------------------------------

#[test]
fn cell2_non_fusible_option_number_lhs_emits_shadow_let() {
    // Intervening statement between `let` and `??=` blocks fusion; shadow-let
    // must still narrow `x` for the subsequent `return x`.
    let src = r#"
        function nonFusibleOption(x: number | null, cond: boolean): number {
            let y = 1;
            if (cond) { y = 2; }
            x ??= y;
            return x;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("let x = x.unwrap_or_else(|| y);"),
        "expected shadow-let with unwrap_or_else (y is Ident, not Copy literal), got:\n{out}"
    );
    assert!(
        !out.contains("x.get_or_insert_with"),
        "statement-context ??= must not fall through to get_or_insert_with"
    );
}

// -----------------------------------------------------------------------------
// Cell #3: Optional parameter `x?: T` (IR Option<T>).
// -----------------------------------------------------------------------------

#[test]
fn cell3_optional_param_string_lhs_emits_shadow_let() {
    let src = r#"
        function paramOption(x?: string): string {
            x ??= "def";
            return x;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains(r#"let x = x.unwrap_or_else(|| "def".to_string());"#),
        "expected shadow-let over optional param, got:\n{out}"
    );
    // return must be naked `x` (now String), not `x.unwrap_or*`.
    assert!(out.contains("x\n}"), "expected tail `x`, got:\n{out}");
}

// -----------------------------------------------------------------------------
// Cell #4: Non-nullable `T` LHS — `??=` is dead code, emit no-op.
// -----------------------------------------------------------------------------

#[test]
fn cell4_non_nullable_number_emits_nothing_for_assign() {
    let src = r#"
        function nonNullableNoOp(x: number): number {
            x ??= 0;
            return x;
        }
    "#;
    let out = generate(src);
    // No output for the ??= statement itself.
    assert!(
        !out.contains("unwrap_or") && !out.contains("get_or_insert_with"),
        "Cell #4 must produce no Option-related method call, got:\n{out}"
    );
    // Naked `return x` must survive.
    assert!(out.contains("    x\n}"), "expected tail `x`, got:\n{out}");
}

// -----------------------------------------------------------------------------
// Cell #7: Option<T> LHS, expression context, T: Copy (`f64`).
// -----------------------------------------------------------------------------

#[test]
fn cell7_expression_context_copy_emits_deref_get_or_insert_with() {
    let src = r#"
        function exprCopy(x: number | null): number {
            return (x ??= 99);
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("*x.get_or_insert_with(|| 99.0)"),
        "expected `*x.get_or_insert_with(|| 99.0)` (Copy deref), got:\n{out}"
    );
    // Non-Copy clone path must NOT appear for a Copy inner.
    assert!(
        !out.contains("get_or_insert_with(|| 99.0).clone()"),
        "Copy inner must not emit .clone()"
    );
    // Param reassignment to `let mut x = x;` is the codebase's standard pattern
    // for upgrading an immutable param to mutable — without this, the
    // `get_or_insert_with` call would not compile.
    assert!(
        out.contains("let mut x = x;"),
        "mark_mutated_vars must emit `let mut x = x;` rebind, got:\n{out}"
    );
}

// -----------------------------------------------------------------------------
// Cell #8: Option<T> LHS, expression context, T: !Copy (`String`).
// -----------------------------------------------------------------------------

#[test]
fn cell8_expression_context_non_copy_emits_get_or_insert_with_clone() {
    let src = r#"
        function exprNonCopy(x: string | null): string {
            return (x ??= "hello");
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains(r#"x.get_or_insert_with(|| "hello".to_string()).clone()"#),
        "expected non-Copy `.clone()` suffix, got:\n{out}"
    );
    assert!(
        !out.contains(r#"*x.get_or_insert_with"#),
        "non-Copy inner must not be deref-copied"
    );
    assert!(
        out.contains("let mut x = x;"),
        "mark_mutated_vars must emit `let mut x = x;` rebind, got:\n{out}"
    );
}

// -----------------------------------------------------------------------------
// Cell #5: Any LHS, statement context — BLOCKED by I-050 (Any coercion umbrella).
//
// Lock-in test: surfaces as `UnsupportedSyntaxError` with the explicit I-050
// tag. When I-050 lands and implements the structural `if x.is_null() {
// Value::from(d) } else { x }` emission, this test MUST be updated to assert
// the new IR shape. Deleting this test without replacing it is prohibited —
// the test exists to prevent silent regression of the surface behavior.
// -----------------------------------------------------------------------------

#[test]
fn cell5_any_lhs_stmt_is_blocked_by_i050() {
    // `any` resolves to `RustType::Any` only after the TypeResolver runs.
    // D-7: use `TctxFixture::transform_collecting` — which drives the same
    // TypeResolver as the rest of the cell-test suite — rather than calling
    // `transpile_collecting` (full pipeline including generator). The
    // generator output is irrelevant here: the assertion is on the
    // `UnsupportedSyntaxError` list, so the faster transformer-only path is
    // appropriate.
    let src = r#"
        function f(x: any): any {
            x ??= "def";
            return x;
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported
            .iter()
            .any(|u| u.kind.contains("nullish-assign on Any LHS (I-050")),
        "Cell #5 must surface as blocked-by-I-050, got: {:?}",
        unsupported
    );
}

// -----------------------------------------------------------------------------
// Cell #6: non-nullable `T` LHS, expression context, T: Copy.
// -----------------------------------------------------------------------------

#[test]
fn cell6_non_nullable_copy_expr_emits_identity() {
    // TS `x ??= 99` on `x: number` is dead at runtime — the ideal Rust is
    // simply `x`. Previously the converter emitted `*x.get_or_insert_with(|| 99.0)`
    // on an `f64`, which fails to compile (E0599 — method doesn't exist).
    let src = r#"
        function f(x: number): number {
            return (x ??= 99);
        }
    "#;
    let out = generate(src);
    assert!(
        !out.contains("get_or_insert_with"),
        "non-nullable LHS must not emit Option method, got:\n{out}"
    );
    // Body should contain a bare `x` tail expression (not `x.clone()` since
    // f64 is Copy, and not `*x.get_or_insert_with(...)`).
    assert!(
        out.contains("    x\n}"),
        "expected bare identity `x` tail, got:\n{out}"
    );
}

// -----------------------------------------------------------------------------
// Cell #9: Any LHS, expression context — BLOCKED by I-050.
// -----------------------------------------------------------------------------

#[test]
fn cell9_any_lhs_expr_is_blocked_by_i050() {
    // D-7: unified on `TctxFixture::transform_collecting`, same rationale as
    // Cell #5.
    let src = r#"
        function f(x: any): any {
            return (x ??= "def");
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported
            .iter()
            .any(|u| u.kind.contains("nullish-assign on Any LHS (I-050")),
        "Cell #9 must surface as blocked-by-I-050, got: {:?}",
        unsupported
    );
}

// -----------------------------------------------------------------------------
// Cell #10: non-nullable `T` LHS, expression context, T: !Copy (e.g. `String`).
// -----------------------------------------------------------------------------

#[test]
fn cell10_non_nullable_non_copy_expr_emits_clone() {
    // For `!Copy` non-nullable LHS in expr context, the identity path must
    // clone to yield an owned value (rather than moving out of the Ident and
    // breaking later uses). Previously emitted
    // `x.get_or_insert_with(|| "d".to_string()).clone()` on a `String`,
    // which fails to compile (E0599).
    let src = r#"
        function f(x: string): string {
            return (x ??= "def");
        }
    "#;
    let out = generate(src);
    assert!(
        !out.contains("get_or_insert_with"),
        "non-nullable LHS must not emit Option method, got:\n{out}"
    );
    assert!(
        out.contains("x.clone()"),
        "expected `x.clone()` identity for !Copy non-nullable, got:\n{out}"
    );
}

// -----------------------------------------------------------------------------
// Cell #11: unresolved LHS type, statement context — `UnsupportedSyntaxError`.
// -----------------------------------------------------------------------------

#[test]
fn cell11_unresolved_stmt_surfaces_unsupported() {
    // `let encoded;` — no annotation, no initialiser — leaves
    // `TypeResolver::narrowed_type(encoded, ...)` returning `None` because
    // the resolver does not perform flow-sensitive inference from subsequent
    // assignments. (The resolver records types from *declarations* and
    // *initialisers*, not from mutations; see
    // `src/pipeline/type_resolver/visitors.rs::visit_var_decl`.)
    //
    // When `pick_strategy` runs for `encoded ??= url;` the LHS type lookup
    // returns `None`, so `try_convert_nullish_assign_stmt` surfaces
    // `"nullish-assign on unresolved type"` rather than silently picking an
    // Identity / ShadowLet branch.
    //
    // **D-6 fragility note**: if a future PRD (e.g., I-144 CFG narrowing)
    // extends TypeResolver to flow-infer `encoded: string | undefined` from
    // the `??= url` assignment, this test's fixture would no longer produce
    // the unresolved path — the test must then be replaced with a TS fixture
    // that TypeResolver *still* can't resolve (e.g., a `declare let encoded;`
    // import, a deeply-destructured rest pattern, or an explicit `any`
    // coercion chain not yet recognised), rather than being silently
    // reinterpreted.
    let src = r#"
        function f(url: string): string {
            let encoded;
            encoded ??= url;
            return encoded;
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported
            .iter()
            .any(|u| u.kind.contains("nullish-assign on unresolved type")),
        "Cell #11 must surface as unresolved-type unsupported, got: {:?}",
        unsupported
    );
}

// -----------------------------------------------------------------------------
// Cell #12: unresolved LHS type, expression context — symmetric `UnsupportedSyntaxError`.
// -----------------------------------------------------------------------------

#[test]
fn cell12_unresolved_expr_surfaces_unsupported() {
    // Same `let encoded;` unresolved-type scenario as Cell #11 (see D-6
    // fragility note there), but in expression context. Before Step 2 the
    // stmt path errored but the expr path silently fell through to
    // `get_or_insert_with` emission, producing broken Rust. Post-Step 2 the
    // two paths are symmetric.
    let src = r#"
        function f(url: string): string {
            let encoded;
            return (encoded ??= url);
        }
    "#;
    let f = TctxFixture::from_source(src);
    let (_items, unsupported) = f.transform_collecting(src);
    assert!(
        unsupported
            .iter()
            .any(|u| u.kind.contains("nullish-assign on unresolved type")),
        "Cell #12 must surface as unresolved-type unsupported, got: {:?}",
        unsupported
    );
}

// -----------------------------------------------------------------------------
// Cell #13: `x ??= y ?? def` — `Option<T>` LHS with `Option<T>` RHS chain.
// The RHS `?? def` collapses to `T` via `unwrap_or_else`, and the outer
// shadow-let consumes it as the default.
// -----------------------------------------------------------------------------

#[test]
fn cell13_option_rhs_chain_in_stmt_nests_unwrap_or_else() {
    let src = r#"
        function chainStmt(x: string | null, y: string | null, def: string): string {
            x ??= y ?? def;
            return x;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("let x = x.unwrap_or_else(|| y.unwrap_or_else(|| def));"),
        "expected nested unwrap_or_else chain, got:\n{out}"
    );
}

// -----------------------------------------------------------------------------
// Cell #14: narrowing-reset — structural emission via I-144 CFG analyzer.
//
// `x ??= 0;` followed by a narrow-invalidating mutation (direct reassign,
// null reassign, loop-body rebind, closure-body reassign) must emit
// `x.get_or_insert_with(|| 0.0);` (E2a) instead of the E1 shadow-let that
// would not typecheck once `x` is reassigned to `None` or to a value whose
// type differs from the narrowed inner T.
//
// The analyzer lives in `pipeline::narrowing_analyzer::analyze_function`
// (T3/T4/T5); `try_convert_nullish_assign_stmt` (T6-1) dispatches on
// `EmissionHint::ShadowLet` vs `EmissionHint::GetOrInsertWith` to pick the
// emission.
// -----------------------------------------------------------------------------

/// Asserts the `??=` site emits `.get_or_insert_with(|| ...)` (E2a, Option
/// preserved) instead of a shadow-let `let x = x.unwrap_or(...)` (E1).
fn assert_cell14_emits_get_or_insert_with(src: &str, scenario: &str) {
    let (rust, unsupported) = crate::transpile_collecting(src).unwrap();
    assert!(
        !unsupported
            .iter()
            .any(|u| u.kind.contains("narrowing-reset")),
        "{scenario}: narrowing-reset must NOT surface as unsupported (structural fix), got: {unsupported:?}"
    );
    assert!(
        rust.contains("x.get_or_insert_with(|| 0.0)"),
        "{scenario}: must emit E2a `x.get_or_insert_with(|| 0.0)`, got:\n{rust}"
    );
    assert!(
        !rust.contains("let x = x.unwrap_or("),
        "{scenario}: must NOT emit E1 shadow-let (the narrow-invalidating sibling \
         makes it ill-typed), got:\n{rust}"
    );
}

#[test]
fn cell14_linear_null_reassign_emits_get_or_insert_with() {
    // Base case: linear reset (`x ??= 0; x = null;`). Analyzer detects
    // `ResetCause::NullAssign` → `EmissionHint::GetOrInsertWith`.
    let src = r#"
        function narrowingReset(x: number | null): number | null {
            x ??= 0;
            x = null;
            return x;
        }
    "#;
    assert_cell14_emits_get_or_insert_with(src, "linear null reassign");
}

#[test]
fn cell14_inner_if_block_null_reassign_emits_get_or_insert_with() {
    // Conditional reset (`if (cond) { x = null; }`) — analyzer must descend
    // into the nested if-consequent and merge branches conservatively.
    let src = r#"
        function condReset(x: number | null, cond: boolean): number {
            x ??= 0;
            if (cond) { x = null; }
            return 0;
        }
    "#;
    assert_cell14_emits_get_or_insert_with(src, "inner-if null reassign");
}

#[test]
fn cell14_loop_body_reassign_emits_get_or_insert_with() {
    // for-of body reassigns shadowed ident — analyzer must descend into the
    // loop body (the reassign runs ≥ 0 times, conservatively invalidating).
    let src = r#"
        function loopReset(x: number | null, arr: (number | null)[]): number {
            x ??= 0;
            for (const v of arr) { x = v; }
            return 0;
        }
    "#;
    assert_cell14_emits_get_or_insert_with(src, "for-of body reassign");
}

#[test]
fn cell14_closure_body_reassign_emits_get_or_insert_with() {
    // Closure reassign is the C-2a case: the closure body is NOT a scan
    // boundary for the CFG analyzer (unlike the legacy T6-preceding scanner).
    // When a closure captures the outer narrowed var and reassigns it, the
    // ideal emission is E2a `get_or_insert_with` so the captured
    // `Option<T>` is preserved and the closure body `x = 1` / `x = null`
    // compiles.
    let src = r#"
        function closureReassign(x: number | null): number {
            x ??= 0;
            const reassign = () => { x = 1; };
            reassign();
            return 0;
        }
    "#;
    assert_cell14_emits_get_or_insert_with(src, "closure-body reassign");
}

// -----------------------------------------------------------------------------
// I-144 T6-1 entry-point regression lock-in.
//
// `TypeResolver::collect_emission_hints` is wired in at five function-like
// body visitors (fn decl / method / constructor / arrow-BlockStmt / fn expr).
// The cell14_* tests above exercise only the fn-decl entry point. Each of
// the remaining four entry points is guarded by a dedicated test below so
// that accidentally dropping the `collect_emission_hints(body)` call from
// any of them fails immediately rather than silently regressing to E1
// shadow-let + broken compile.
// -----------------------------------------------------------------------------

#[test]
fn collect_emission_hints_wired_into_class_method_body() {
    // `visit_method_function` entry point — class method body hosting `??=`
    // + narrow-invalidating reset must emit E2a.
    let src = r#"
        class C {
            run(x: number | null): number | null {
                x ??= 0;
                x = null;
                return x;
            }
        }
    "#;
    assert_cell14_emits_get_or_insert_with(src, "class method body");
}

#[test]
fn collect_emission_hints_wired_into_constructor_body() {
    // Constructor entry point — `visit_class_body`'s `Constructor` arm
    // invokes `collect_emission_hints` before walking body stmts. A `??=` +
    // reset inside the constructor must pick E2a just like a free function.
    let src = r#"
        class C {
            constructor(x: number | null) {
                x ??= 0;
                x = null;
            }
        }
    "#;
    assert_cell14_emits_get_or_insert_with(src, "constructor body");
}

#[test]
fn collect_emission_hints_wired_into_arrow_block_body() {
    // `resolve_arrow_expr` BlockStmt branch — the arrow's block body must be
    // analyzed independently from any enclosing function so its own `??=`
    // sites get hints.
    let src = r#"
        const run = (x: number | null): number | null => {
            x ??= 0;
            x = null;
            return x;
        };
    "#;
    assert_cell14_emits_get_or_insert_with(src, "arrow BlockStmt body");
}

#[test]
fn collect_emission_hints_wired_into_fn_expr_body() {
    // `resolve_fn_expr` entry point. Top-level `const run = function() {...}`
    // is not transpiled at all by the current pipeline (`transform_decl`
    // skips function-expression initializers for module-level vars), so the
    // fn-expr sits inside a wrapper function here. The outer `wrapper` has
    // no `??=` of its own, so any `get_or_insert_with` in the output can
    // only have come from the fn-expr body's analyzer hints. Outer
    // `analyze_function` stops at the var-decl boundary, so the fn-expr
    // body's `??=` hint is populated only if `resolve_fn_expr` calls
    // `collect_emission_hints` on the fn-expr body itself.
    let src = r#"
        function wrapper(): boolean {
            const run = function(x: number | null): number | null {
                x ??= 0;
                x = null;
                return x;
            };
            return run(5) === null;
        }
    "#;
    assert_cell14_emits_get_or_insert_with(src, "fn expr body");
}

// -----------------------------------------------------------------------------
// Out-of-scope rejections: FieldAccess LHS (I-142-b) and Index LHS (I-142-c).
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// D-2: Problem Space matrix — RHS shape dimension parameterised coverage.
//
// Per `report/i142-step3-inv2-rhs-shape.md`, the RHS of `x ??= <rhs>` normalises
// to four classes. Existing Cells #1..#3, #7, #8, #13 already lock in Class A
// (Copy literal) and Class B (non-Copy literal) for Option<T> LHS in stmt /
// expr context. The tests below fill in Class C (side-effect expression) and
// Class D (transparent TS wrapper) for the same LHS/context combinations, so
// the matrix is enumerated rather than implicit. Seq / yield / throw RHS
// surface as UnsupportedSyntaxError (lock-in for I-114 / I-143 follow-ups).
// -----------------------------------------------------------------------------

#[test]
fn d2_class_c_call_rhs_stmt_emits_unwrap_or_else() {
    // Class C (side-effect Call RHS) in stmt context — lazy eval required.
    let src = r#"
        function f(x: number | null): number {
            function fallback(): number { return 42; }
            x ??= fallback();
            return x;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("x.unwrap_or_else(|| fallback())"),
        "Class C (Call) stmt must emit lazy `unwrap_or_else(|| fallback())`, got:\n{out}"
    );
}

#[test]
fn d2_class_c_binop_rhs_stmt_emits_unwrap_or_else() {
    // Class C (BinOp RHS) in stmt context.
    let src = r#"
        function f(x: number | null, a: number, b: number): number {
            x ??= a + b;
            return x;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("x.unwrap_or_else(|| a + b)"),
        "Class C (BinOp) stmt must emit lazy closure, got:\n{out}"
    );
}

#[test]
fn d2_class_c_ternary_rhs_stmt_emits_unwrap_or_else() {
    // Class C (Cond / ternary RHS).
    let src = r#"
        function f(x: number | null, c: boolean): number {
            x ??= c ? 1 : 2;
            return x;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("x.unwrap_or_else(|| if c { 1.0 } else { 2.0 })"),
        "Class C (Cond) stmt must emit lazy closure over if-else, got:\n{out}"
    );
}

#[test]
fn d2_class_c_call_rhs_expr_emits_get_or_insert_with() {
    // Class C (Call RHS) in expr context — Copy inner (f64) deref.
    let src = r#"
        function f(x: number | null): number {
            function fallback(): number { return 42; }
            return (x ??= fallback());
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("*x.get_or_insert_with(|| fallback())"),
        "Class C (Call) expr must emit deref + get_or_insert_with, got:\n{out}"
    );
}

#[test]
fn d2_class_d_ts_as_rhs_stmt_peeks_through() {
    // Class D (TsAs transparent wrapper) — `x ??= d as string` must behave
    // identically to `x ??= d` because the `as` cast is a static type
    // assertion only (no runtime effect). If the transformer did NOT peek
    // through TsAs at the RHS, it might emit spurious `.to_string()` or wrap
    // the RHS in a block — either would be a cosmetic regression.
    let src = r#"
        function f(x: string | null, d: string): string {
            x ??= (d as string);
            return x;
        }
    "#;
    let out = generate(src);
    // Must emit a single `.unwrap_or_else(|| d)` — NOT a string-coerce wrapper.
    assert!(
        out.contains("x.unwrap_or_else(|| d)"),
        "Class D (TsAs) stmt must peek through and emit `unwrap_or_else(|| d)`, got:\n{out}"
    );
}

#[test]
fn d2_class_d_ts_non_null_rhs_stmt_peeks_through() {
    // Class D (TsNonNull `!` postfix) — `x ??= d!` must peek through.
    let src = r#"
        function f(x: string | null, d: string | null): string {
            x ??= d!;
            return x;
        }
    "#;
    let out = generate(src);
    // TsNonNull itself is a cast; the inner `d: string | null` is unwrapped
    // *by TS* to `string`. The emission should unwrap `d` via `.unwrap()` or
    // similar before consuming as the default.
    //
    // Minimum guarantee: the statement converts without error, and the
    // `??=` shape is preserved.
    assert!(
        out.contains("x.unwrap_or_else"),
        "Class D (TsNonNull) must produce some unwrap_or_else shape, got:\n{out}"
    );
}

#[test]
fn d2_class_d_paren_rhs_stmt_peeks_through() {
    // Class D (Paren) — `x ??= (d)` must be identical to `x ??= d`.
    let src = r#"
        function f(x: number | null, d: number): number {
            x ??= (d);
            return x;
        }
    "#;
    let out = generate(src);
    assert!(
        out.contains("x.unwrap_or_else(|| d)"),
        "Class D (Paren) must peek through and emit `unwrap_or_else(|| d)`, got:\n{out}"
    );
}

#[test]
fn d2_seq_rhs_surfaces_unsupported() {
    // The comma operator `(a(), b)` is currently unsupported as a plain
    // expression (I-114). When appearing as a `??=` RHS, the conversion must
    // still surface that limitation explicitly rather than emitting a
    // silently-broken shape.
    let src = r#"
        function f(x: number | null): number {
            let side: number = 0;
            function touch(): number { side = 1; return side; }
            x ??= (touch(), 5);
            return x;
        }
    "#;
    // The pipeline may return an `Err` (anyhow) because `Seq` is unsupported
    // at the expression level — either surface path is acceptable as a
    // lock-in that the combination is not silently translated.
    let result = crate::transpile_collecting(src);
    match result {
        Err(_) => { /* direct error path — acceptable */ }
        Ok((_rust, unsupported)) => {
            assert!(
                !unsupported.is_empty(),
                "Seq RHS must produce at least one UnsupportedSyntaxError (I-114 follow-up), got empty"
            );
        }
    }
}

#[test]
fn field_access_lhs_emits_get_or_insert() {
    // I-142-b: FieldAccess ??= emits get_or_insert_with (expression context)
    let src = r#"
        interface Cfg { v?: number; }
        function f(b: Cfg): number { return (b.v ??= 0); }
    "#;
    let (rust, unsupported) = crate::transpile_collecting(src).unwrap();
    assert!(
        !unsupported
            .iter()
            .any(|u| u.kind.contains("nullish-assign")),
        "FieldAccess ??= must not produce nullish-assign unsupported, got: {:?}",
        unsupported
    );
    assert!(
        rust.contains("get_or_insert_with"),
        "FieldAccess ??= must emit get_or_insert_with, got:\n{rust}",
    );
}

#[test]
fn index_lhs_emits_entry_or_insert() {
    // I-142-c: Index ??= on Record emits entry().or_insert_with()
    let src = r#"
        function f(cache: Record<string, string>, key: string): string {
            return (cache[key] ??= "default");
        }
    "#;
    let (rust, unsupported) = crate::transpile_collecting(src).unwrap();
    assert!(
        !unsupported
            .iter()
            .any(|u| u.kind.contains("nullish-assign")),
        "Index ??= must not produce nullish-assign unsupported, got: {:?}",
        unsupported
    );
    assert!(
        rust.contains("entry") && rust.contains("or_insert_with"),
        "Index ??= must emit entry().or_insert_with(), got:\n{rust}",
    );
}
