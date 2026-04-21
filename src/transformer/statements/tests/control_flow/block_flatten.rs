//! I-153 T0 Block stmt flatten tests.
//!
//! TS allows block statements at any statement position (e.g., `case 1: { stmts }`,
//! `function f() { { stmts } }`). The fix flattens the block contents into the
//! parent via `convert_stmt_list`, preserving semantics for valid TS (Rust's
//! enclosing match arm / function body provides block scope).

use super::*;

#[test]
fn test_convert_block_stmt_flattens_into_parent() {
    // Bare block at function body level.
    let stmts = parse_fn_body("function f() { { const x = 1; return x; } }");
    let result = {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Expect 2 flattened statements (Let + Return), not a single wrapper.
    assert_eq!(
        result.len(),
        2,
        "block should flatten to parent, got {result:?}"
    );
    assert!(matches!(result[0], Stmt::Let { .. }));
    assert!(matches!(result[1], Stmt::Return(_)));
}

#[test]
fn test_convert_block_stmt_in_switch_case_flattens() {
    // Block inside switch case body (the common `no-case-declarations` pattern).
    let stmts = parse_fn_body(
        r#"function f(x: number): string {
            switch (x) {
                case 1: {
                    const y = x;
                    return "one";
                }
                default:
                    return "other";
            }
        }"#,
    );
    let result = {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Expect a Match (switch) with the block contents flattened into its arm body.
    assert_eq!(result.len(), 1, "expected single Match, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2, "expected case + default arms, got {arms:?}");
            // First arm body should have >= 2 stmts (Let + Return), flattened from block.
            assert!(
                arms[0].body.len() >= 2,
                "case body should contain flattened block contents, got {:?}",
                arms[0].body
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_block_stmt_nested_break_preserved_for_i153_walker() {
    // Block containing a nested bare break: verifies the block flatten
    // preserves the break stmt so I-153's walker can rewrite it later.
    let stmts = parse_fn_body(
        r#"function f(x: number, cond: boolean) {
            for (;;) {
                switch (x) {
                    case 1: {
                        if (cond) break;
                        return;
                    }
                    default:
                        return;
                }
            }
        }"#,
    );
    let result = {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Ensure conversion succeeded (no unsupported error) and produced a Loop wrapping Match.
    // `for (;;)` without init/test/update converts to Stmt::Loop (not ForIn).
    // The nested bare `break` inside block inside if inside case body is preserved
    // for I-153 walker (T2/T3) to rewrite later — T0 only verifies conversion succeeds.
    assert_eq!(result.len(), 1);
    let loop_body = match &result[0] {
        Stmt::Loop { body, .. } => body,
        other => panic!("expected Loop, got {other:?}"),
    };
    assert!(
        loop_body
            .iter()
            .any(|s| matches!(s, Stmt::Match { .. } | Stmt::LabeledBlock { .. })),
        "loop-body should contain Match or LabeledBlock(Match), got {loop_body:?}"
    );
    // The block inside `case 1: { if (cond) break; return; }` should flatten such that
    // the `if`/`return` become direct match arm body stmts.
    if let Some(Stmt::Match { arms, .. }) =
        loop_body.iter().find(|s| matches!(s, Stmt::Match { .. }))
    {
        let case1_arm = &arms[0];
        assert!(
            case1_arm.body.len() >= 2,
            "case 1 arm body should have if + return (flattened from block), got {:?}",
            case1_arm.body
        );
        assert!(
            matches!(case1_arm.body[0], Stmt::If { .. }),
            "case 1 arm body[0] should be If, got {:?}",
            case1_arm.body[0]
        );
    }
}
