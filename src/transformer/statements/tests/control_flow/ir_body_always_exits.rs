//! Locks in [`ir_body_always_exits`] coverage across all control-flow
//! statement kinds. Previously the implementation omitted `Stmt::Match`,
//! silently treating a body whose tail was `match x { a => return, b =>
//! return }` as non-exit — this would cause the consolidated match in
//! `try_generate_option_truthy_complement_match` to be skipped for
//! future cells that emit a match-based exit. Round 2 review finding R2-I1.

use super::*;

#[test]
fn test_ir_body_always_exits_covers_all_exit_shapes() {
    use crate::ir::MatchArm;
    use crate::transformer::statements::control_flow::ir_body_always_exits;

    // Positive: return / break / continue are exits.
    assert!(ir_body_always_exits(&[Stmt::Return(Some(
        Expr::NumberLit(1.0)
    ))]));
    assert!(ir_body_always_exits(&[Stmt::Break {
        label: None,
        value: None,
    }]));
    assert!(ir_body_always_exits(&[Stmt::Continue { label: None }]));

    // Negative: empty body and non-exit tail.
    assert!(!ir_body_always_exits(&[]));
    assert!(!ir_body_always_exits(&[Stmt::Expr(Expr::NumberLit(1.0))]));

    // If: always-exit only when both branches exit.
    let if_both_exit = Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(None)],
        else_body: Some(vec![Stmt::Return(None)]),
    };
    assert!(ir_body_always_exits(&[if_both_exit]));
    let if_only_then_exit = Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(None)],
        else_body: Some(vec![Stmt::Expr(Expr::NumberLit(1.0))]),
    };
    assert!(!ir_body_always_exits(&[if_only_then_exit]));
    let if_no_else = Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(None)],
        else_body: None,
    };
    assert!(!ir_body_always_exits(&[if_no_else]));

    // Match: always-exit iff non-empty AND every arm exits.
    let match_all_exit = Stmt::Match {
        expr: Expr::Ident("x".to_string()),
        arms: vec![
            MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![Stmt::Return(None)],
            },
            MatchArm {
                patterns: vec![Pattern::Literal(Expr::NumberLit(1.0))],
                guard: None,
                body: vec![Stmt::Break {
                    label: None,
                    value: None,
                }],
            },
        ],
    };
    assert!(ir_body_always_exits(&[match_all_exit]));

    let match_one_non_exit = Stmt::Match {
        expr: Expr::Ident("x".to_string()),
        arms: vec![
            MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![Stmt::Return(None)],
            },
            MatchArm {
                patterns: vec![Pattern::Literal(Expr::NumberLit(1.0))],
                guard: None,
                body: vec![Stmt::Expr(Expr::NumberLit(1.0))],
            },
        ],
    };
    assert!(!ir_body_always_exits(&[match_one_non_exit]));

    let match_empty = Stmt::Match {
        expr: Expr::Ident("x".to_string()),
        arms: vec![],
    };
    assert!(
        !ir_body_always_exits(&[match_empty]),
        "empty match cannot be always-exit"
    );
}
