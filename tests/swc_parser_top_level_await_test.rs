//! I-224 cells 6/7/8 (Axis A0 × Axis C1): SWC parser empirical lock-in test.
//!
//! **Spec stage NA justification (Rule 3 (3-2) hard-code、本 PRD scope)**:
//!
//! I-224 matrix cells 6/7/8 are claimed NA on the basis that Axis A0 (= declarations
//! only / library mode / no top-level execution statement) is **AST-structurally
//! mutually exclusive** with Axis C1 (= top-level await present). Per the precision-up
//! M-1 wording: any source containing a top-level `await` expression must produce an
//! AST shape that includes either `Stmt::Expr(Expr::Await(_))` (Axis A1 partition)
//! or `Decl::Var(VarDecl { decls[0].init: Expr::Await(_) })` (Axis A3 partition).
//! Therefore A0 + C1 cannot coexist as an AST shape.
//!
//! This file empirically locks the structural claim using `parse_typescript()`
//! (Rust unit test, no `tsx` runtime dependency — Rule 3 (3-2) requirement that
//! NA cells be empirically verified at the parser level even when the runtime would
//! reject the input).
//!
//! Lesson source: I-224 third-party `/check_job` review C-2 (2026-05-01) flagged
//! that deferring SWC parser empirical lock-ins to a sister PRD (I-226) is a
//! Rule 3 (3-2) hard violation: SWC parser empirical is orthogonal to test-harness
//! ESM-mode upgrades, so the empirical verification belongs in this PRD scope.
//! The Option β cohesive batch (I-224 + test-harness ESM upgrade) further removes
//! any rationale for deferring; the lock-in is now performed here as a Spec-stage
//! Rule 3 (3-2) artifact.

use swc_ecma_ast::{Decl, Expr, ModuleItem, Stmt};
use ts_to_rs::parser::parse_typescript;

#[test]
fn test_top_level_bare_await_parses_as_stmt_expr_await_axis_a1() {
    // Input: `await x;` — bare top-level await expression.
    // Empirical claim: SWC parser produces `Stmt::Expr(Expr::Await(_))`.
    // Therefore this source belongs to Axis A1 (top-level Stmt::Expr) regardless of C1
    // presence — it is NOT Axis A0 (declarations only).
    //
    // This proves the A0 + C1 combination cannot arise from this input form.
    let source = "declare const x: Promise<number>;\nawait x;\n";
    let module = parse_typescript(source).expect("SWC parser should accept top-level bare await");

    // Find the await stmt (skip the declare).
    let await_stmt = module
        .body
        .iter()
        .find_map(|item| match item {
            ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => match &*expr_stmt.expr {
                Expr::Await(_) => Some(expr_stmt),
                _ => None,
            },
            _ => None,
        })
        .expect(
            "Axis structural claim violated: top-level `await x;` should parse to \
             Stmt::Expr(Expr::Await), placing this source in Axis A1 (NOT A0). \
             If this fires, the I-224 NA justification for cells 6/7/8 is invalid — \
             reconsider whether A0 + C1 is structurally reachable.",
        );

    // Confirm the inner expression is Expr::Await (defensive — already filtered above).
    assert!(matches!(*await_stmt.expr, Expr::Await(_)));
}

#[test]
fn test_top_level_var_decl_with_await_init_parses_as_decl_var_axis_a3() {
    // Input: `const x = await fetch(...);` — Decl::Var with await initializer.
    // Empirical claim: SWC parser produces `Decl::Var` with `init: Expr::Await(_)`.
    // Therefore this source belongs to Axis A3 (Decl::Var with side-effect/non-const
    // init) regardless of C1 presence — it is NOT Axis A0.
    //
    // A2 (Decl::Var with literal init) is also distinguished: an await initializer
    // is by definition non-literal, so this never collapses into A2 either.
    let source = "declare function fetch(): Promise<number>;\nconst x = await fetch();\n";
    let module = parse_typescript(source).expect("SWC parser should accept top-level await init");

    let var_decl_with_await = module
        .body
        .iter()
        .find_map(|item| match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => {
                let first = var.decls.first()?;
                match first.init.as_deref()? {
                    Expr::Await(_) => Some(var),
                    _ => None,
                }
            }
            _ => None,
        })
        .expect(
            "Axis structural claim violated: top-level `const x = await fetch();` \
             should parse to Decl::Var with init=Expr::Await, placing this source \
             in Axis A3 (NOT A0). If this fires, the I-224 NA justification for \
             cells 6/7/8 is invalid — reconsider whether A0 + C1 is reachable.",
        );

    assert_eq!(
        var_decl_with_await.decls.len(),
        1,
        "expected single declarator in test fixture"
    );
}

#[test]
fn test_pure_axis_a0_source_contains_no_await_expression() {
    // Input: pure A0 fixture — only declarations, no execution statements, no await.
    // Empirical claim: every ModuleItem is either ModuleDecl (import/export) or a
    // declaration-only Stmt::Decl. No `Expr::Await(_)` appears anywhere in the
    // top-level body.
    //
    // This is the symmetric counterpart to the A1/A3 cases above: pure A0 is
    // observable by absence of any top-level await — confirming A0 + C1 is empty.
    let source = r#"
        function helper(): number { return 7; }
        function main(): void { console.log("user main:", helper()); }
        interface Box<T> { value: T; }
        type Id = number;
    "#;
    let module = parse_typescript(source).expect("pure A0 source should parse");

    for (idx, item) in module.body.iter().enumerate() {
        match item {
            ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => {
                assert!(
                    !matches!(*expr_stmt.expr, Expr::Await(_)),
                    "pure A0 source must not contain Expr::Await at top level (item {idx})",
                );
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => {
                for decl in &var.decls {
                    if let Some(init) = &decl.init {
                        assert!(
                            !matches!(**init, Expr::Await(_)),
                            "pure A0 source must not contain Decl::Var with Expr::Await init \
                             (item {idx})",
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

#[test]
fn test_axis_c1_implies_a1_or_a3_partition_synthesis() {
    // Synthesis test: any source carrying a top-level `await` falls into A1 or A3.
    // This is the structural inference behind cells 6/7/8 NA justification.
    //
    // We probe a small set of representative C1 forms and assert the partition.
    let cases = [
        ("await x;", "A1"),
        ("const x = await y;", "A3"),
        ("let x = await y;", "A3"),
        ("var x = await y;", "A3"),
    ];

    for (source, expected_axis) in cases {
        // Provide minimal declarations so the awaitable references are not unbound
        // at parse time (SWC is lenient on undeclared references but we keep the
        // input self-consistent for clarity).
        let full_source =
            format!("declare const x: number;\ndeclare const y: Promise<number>;\n{source}");
        let module = parse_typescript(&full_source)
            .unwrap_or_else(|e| panic!("SWC should accept `{source}`: {e}"));

        let has_top_level_await = module.body.iter().any(|item| match item {
            ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => matches!(*expr_stmt.expr, Expr::Await(_)),
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => var.decls.iter().any(|d| {
                d.init
                    .as_deref()
                    .is_some_and(|e| matches!(e, Expr::Await(_)))
            }),
            _ => false,
        });

        assert!(
            has_top_level_await,
            "case `{source}` (expected axis {expected_axis}) should produce a top-level \
             await AST node, confirming the C1 partition is materialized at AST shape level",
        );
    }
}
