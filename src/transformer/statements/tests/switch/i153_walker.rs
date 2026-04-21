//! I-153 T2: `rewrite_nested_bare_break_in_stmts` walker tests.
//!
//! The walker rewrites nested bare `Stmt::Break { label: None, value:
//! None }` within switch case body into `Stmt::Break { label:
//! Some("__ts_switch"), value: None }`. Descent policy is exhaustive
//! over 14 IR Stmt variants — see the helper's docstring.
//!
//! This file uses its own local imports (not the parent's `super::*`)
//! since the tests only exercise the walker function, not the
//! fixture/parser helpers used elsewhere in the switch suite.

use crate::ir::{Expr, MatchArm, Pattern, Stmt};
use crate::transformer::statements::switch::rewrite_nested_bare_break_in_stmts;

fn bare_break() -> Stmt {
    Stmt::Break {
        label: None,
        value: None,
    }
}

fn labeled_break(label: &str) -> Stmt {
    Stmt::Break {
        label: Some(label.to_string()),
        value: None,
    }
}

// ---- LEAF: direct bare break (top-level in a vec) ----

#[test]
fn i153_rewriter_top_level_bare_break_rewrites() {
    let mut stmts = vec![bare_break()];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);
    assert_eq!(stmts, vec![labeled_break("__ts_switch")]);
}

#[test]
fn i153_rewriter_labeled_break_not_rewritten() {
    let mut stmts = vec![labeled_break("outer")];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
    assert_eq!(stmts, vec![labeled_break("outer")]);
}

// ---- DESCENT: If.then_body / If.else_body ----

#[test]
fn i153_rewriter_if_cons_bare_break_rewrites() {
    let mut stmts = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![bare_break()],
        else_body: None,
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);
    match &stmts[0] {
        Stmt::If { then_body, .. } => {
            assert_eq!(then_body, &vec![labeled_break("__ts_switch")]);
        }
        _ => panic!(),
    }
}

#[test]
fn i153_rewriter_if_alt_bare_break_rewrites() {
    let mut stmts = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![],
        else_body: Some(vec![bare_break()]),
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);
    match &stmts[0] {
        Stmt::If { else_body, .. } => {
            assert_eq!(
                else_body.as_ref().unwrap(),
                &vec![labeled_break("__ts_switch")]
            );
        }
        _ => panic!(),
    }
}

// ---- DESCENT: IfLet.then_body / IfLet.else_body ----

#[test]
fn i153_rewriter_if_let_then_body_rewrites() {
    let mut stmts = vec![Stmt::IfLet {
        pattern: Pattern::Wildcard,
        expr: Expr::BoolLit(true),
        then_body: vec![bare_break()],
        else_body: None,
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);
    match &stmts[0] {
        Stmt::IfLet { then_body, .. } => {
            assert_eq!(then_body, &vec![labeled_break("__ts_switch")]);
        }
        _ => panic!(),
    }
}

// G-S5: IfLet.else_body descent (parity with then_body).
#[test]
fn i153_rewriter_if_let_else_body_rewrites() {
    let mut stmts = vec![Stmt::IfLet {
        pattern: Pattern::Wildcard,
        expr: Expr::BoolLit(true),
        then_body: vec![],
        else_body: Some(vec![bare_break()]),
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);
    match &stmts[0] {
        Stmt::IfLet { else_body, .. } => {
            assert_eq!(
                else_body.as_ref().unwrap(),
                &vec![labeled_break("__ts_switch")]
            );
        }
        _ => panic!(),
    }
}

// G-S6: Match with multiple arms, rewrites in multiple arms simultaneously
// (no short-circuit — verifies the explicit loop in walker).
#[test]
fn i153_rewriter_match_multiple_arms_all_rewritten() {
    let mut stmts = vec![Stmt::Match {
        expr: Expr::BoolLit(true),
        arms: vec![
            MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![bare_break()],
            },
            MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![bare_break()],
            },
            MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![bare_break()],
            },
        ],
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);
    match &stmts[0] {
        Stmt::Match { arms, .. } => {
            // Every arm must be rewritten (no short-circuit).
            for (i, arm) in arms.iter().enumerate() {
                assert_eq!(
                    arm.body,
                    vec![labeled_break("__ts_switch")],
                    "arm {i} must be rewritten (walker must not short-circuit)"
                );
            }
        }
        _ => panic!(),
    }
}

// ---- DESCENT: Match.arms[*].body ----

#[test]
fn i153_rewriter_match_arm_body_rewrites() {
    let mut stmts = vec![Stmt::Match {
        expr: Expr::BoolLit(true),
        arms: vec![MatchArm {
            patterns: vec![Pattern::Wildcard],
            guard: None,
            body: vec![bare_break()],
        }],
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);
    match &stmts[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms[0].body, vec![labeled_break("__ts_switch")]);
        }
        _ => panic!(),
    }
}

// ---- SKIP: LabeledBlock (inner emission 所掌) ----

#[test]
fn i153_rewriter_labeled_block_skipped_even_with_inner_bare_break() {
    // A nested `__ts_try_block` (or other internal) may contain its own
    // rewritten breaks. Our walker must NOT descend or re-rewrite.
    let mut stmts = vec![Stmt::LabeledBlock {
        label: "__ts_try_block".to_string(),
        body: vec![bare_break()],
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten, "LabeledBlock is non-descent (inner-owned)");
    match &stmts[0] {
        Stmt::LabeledBlock { body, .. } => {
            assert_eq!(
                body,
                &vec![bare_break()],
                "inner body must remain untouched"
            );
        }
        _ => panic!(),
    }
}

// ---- NON-DESCENT: loop variants (inner bare break targets inner loop correctly) ----

#[test]
fn i153_rewriter_while_body_non_descent() {
    let mut stmts = vec![Stmt::While {
        label: None,
        condition: Expr::BoolLit(true),
        body: vec![bare_break()],
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
    match &stmts[0] {
        Stmt::While { body, .. } => {
            assert_eq!(body, &vec![bare_break()], "inner loop break preserved");
        }
        _ => panic!(),
    }
}

#[test]
fn i153_rewriter_for_in_body_non_descent() {
    let mut stmts = vec![Stmt::ForIn {
        label: None,
        var: "i".to_string(),
        iterable: Expr::Ident("xs".to_string()),
        body: vec![bare_break()],
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
}

#[test]
fn i153_rewriter_while_let_body_non_descent() {
    let mut stmts = vec![Stmt::WhileLet {
        label: None,
        pattern: Pattern::Wildcard,
        expr: Expr::BoolLit(true),
        body: vec![bare_break()],
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
}

#[test]
fn i153_rewriter_loop_body_non_descent() {
    let mut stmts = vec![Stmt::Loop {
        label: None,
        body: vec![bare_break()],
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
}

// ---- LEAF: defense-in-depth (Expr-with-body variants, currently no-descent) ----

#[test]
fn i153_rewriter_return_stmt_is_leaf() {
    // Stmt::Return(Some(Expr)) is a leaf: Expr does not embed Stmt::Break
    // in current emission paths (empirical grep).
    let mut stmts = vec![Stmt::Return(Some(Expr::NumberLit(1.0)))];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
}

#[test]
fn i153_rewriter_let_init_expr_is_leaf() {
    let mut stmts = vec![Stmt::Let {
        mutable: false,
        name: "x".to_string(),
        ty: None,
        init: Some(Expr::NumberLit(1.0)),
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
}

#[test]
fn i153_rewriter_tail_expr_is_leaf() {
    let mut stmts = vec![Stmt::TailExpr(Expr::NumberLit(1.0))];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
}

#[test]
fn i153_rewriter_continue_is_leaf() {
    let mut stmts = vec![Stmt::Continue { label: None }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(!rewritten);
}

// ---- Composite: deeply nested structure with mix of rewrite + skip ----

#[test]
fn i153_rewriter_deeply_nested_mixed_structure() {
    // Structure:
    //   if (cond) {
    //     if (cond2) { break; }     // rewrite (matrix cell #3 nested)
    //     while (cond3) { break; }   // non-descent (inner loop)
    //     match x { _ => { break; } }// rewrite (match arm body)
    //   } else {
    //     break;                     // rewrite (else body)
    //   }
    let mut stmts = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![
            Stmt::If {
                condition: Expr::BoolLit(true),
                then_body: vec![bare_break()],
                else_body: None,
            },
            Stmt::While {
                label: None,
                condition: Expr::BoolLit(true),
                body: vec![bare_break()],
            },
            Stmt::Match {
                expr: Expr::BoolLit(true),
                arms: vec![MatchArm {
                    patterns: vec![Pattern::Wildcard],
                    guard: None,
                    body: vec![bare_break()],
                }],
            },
        ],
        else_body: Some(vec![bare_break()]),
    }];
    let rewritten = rewrite_nested_bare_break_in_stmts(&mut stmts, "__ts_switch");
    assert!(rewritten);

    // Extract the outer if
    let Stmt::If {
        then_body,
        else_body,
        ..
    } = &stmts[0]
    else {
        panic!()
    };

    // Inner if.then_body rewritten
    let Stmt::If {
        then_body: inner_then,
        ..
    } = &then_body[0]
    else {
        panic!()
    };
    assert_eq!(inner_then, &vec![labeled_break("__ts_switch")]);

    // While body UNCHANGED (non-descent)
    let Stmt::While { body: wbody, .. } = &then_body[1] else {
        panic!()
    };
    assert_eq!(wbody, &vec![bare_break()]);

    // Match arm body rewritten
    let Stmt::Match { arms, .. } = &then_body[2] else {
        panic!()
    };
    assert_eq!(arms[0].body, vec![labeled_break("__ts_switch")]);

    // Else body rewritten
    assert_eq!(
        else_body.as_ref().unwrap(),
        &vec![labeled_break("__ts_switch")]
    );
}
