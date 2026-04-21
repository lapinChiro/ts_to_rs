//! Non-literal case values (variable / expression) are rewritten to
//! `Wildcard + guard`. Covers single non-literal, combined non-literal
//! fallthrough (`||` guard) and mixed literal/non-literal (each gets its
//! own guard).

use super::*;

#[test]
fn test_switch_nonliteral_case_generates_guard() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // case A (variable reference) should generate a match guard, not a pattern binding
    let stmts = parse_fn_body(
        "function f(x: number) { const A: number = 1; switch(x) { case A: doA(); break; default: doB(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Find the Match statement (second stmt after the const)
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");
    match match_stmt {
        Stmt::Match { arms, .. } => {
            // First arm (case A) should have a guard
            assert!(
                arms[0].guard.is_some(),
                "non-literal case should have a guard, got {:?}",
                arms[0]
            );
            assert!(
                arms[0]
                    .patterns
                    .iter()
                    .any(|p| matches!(p, crate::ir::Pattern::Wildcard)),
                "non-literal case should use wildcard pattern"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_switch_nonliteral_fallthrough_cases_combined_guard() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // case A: case B: ... should combine into a single guard with ||
    let stmts = parse_fn_body(
        "function f(x: number) { const A: number = 1; const B: number = 2; switch(x) { case A: case B: doAB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");
    match match_stmt {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert!(
                arms[0].guard.is_some(),
                "combined non-literal cases should have a guard"
            );
            // Guard should be a LogicalOr of two equality checks
            match &arms[0].guard {
                Some(Expr::BinaryOp {
                    op: BinOp::LogicalOr,
                    ..
                }) => {} // OK
                other => panic!("expected LogicalOr guard, got {other:?}"),
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_switch_mixed_literal_nonliteral_separate_arms() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Literal cases should have no guard, non-literal cases should have guards
    let stmts = parse_fn_body(
        "function f(x: number) { const A: number = 10; switch(x) { case 1: doA(); break; case A: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");
    match match_stmt {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // First arm (case 1) - f64 numeric, uses guard (I-315)
            assert!(
                arms[0].guard.is_some(),
                "numeric case should have a guard, got {:?}",
                arms[0]
            );
            // Second arm (case A) - non-literal, also has guard
            assert!(
                arms[1].guard.is_some(),
                "non-literal case should have a guard, got {:?}",
                arms[1]
            );
        }
        _ => unreachable!(),
    }
}
