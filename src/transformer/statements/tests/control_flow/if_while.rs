//! Basic `if` / `while` conversion + labeled `for-in` tests.
//!
//! Covers the straightforward single-form control-flow stmts without
//! the do-while rewrite / conditional-assign heuristics that require
//! their own dedicated test modules.

use super::*;

// Runtime-variable conditions are used so the new I-171 Layer 2 const-fold
// dead-code elimination (BoolLit(true)/false → inline live branch) does not
// short-circuit these tests. Const-fold behaviour is exercised by
// `const_fold_dead_code_elim` below.

#[test]
fn test_convert_stmt_if_no_else() {
    let stmts = parse_fn_body("function f() { let b: boolean = true; if (b) { return 1; } }");
    // First stmt is the `let b`; the if-stmt is the second.
    let result = convert_single_stmt(&stmts[1], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::If {
            condition: Expr::Ident("b".to_string()),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
            else_body: None,
        }
    );
}

#[test]
fn test_convert_stmt_if_else() {
    let stmts = parse_fn_body(
        "function f() { let b: boolean = true; if (b) { return 1; } else { return 2; } }",
    );
    let result = convert_single_stmt(&stmts[1], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::If {
            condition: Expr::Ident("b".to_string()),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
            else_body: Some(vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]),
        }
    );
}

/// I-171 Layer 2 const-fold dead-code elimination (Matrix C-7..C-10 / C-24).
///
/// `if (true)` / `if (false)` (and the post-Layer-1 `if (Expr::BoolLit(_))`
/// produced by `!<lit>` / `!<always-truthy>` const-fold) should reduce to
/// the live branch directly so the emission matches the PRD ideal column
/// (no redundant `if true {...}` wrapper, no leftover dead branch).
mod const_fold_dead_code_elim {
    use super::*;

    #[test]
    fn if_true_no_else_inlines_then() {
        let stmts = parse_fn_body("function f() { if (true) { return 1; } }");
        let body = super::super::convert_stmts(&stmts, &TypeRegistry::new(), None);
        assert_eq!(body, vec![Stmt::Return(Some(Expr::NumberLit(1.0)))]);
    }

    #[test]
    fn if_true_with_else_inlines_then_drops_else() {
        let stmts = parse_fn_body("function f() { if (true) { return 1; } else { return 2; } }");
        let body = super::super::convert_stmts(&stmts, &TypeRegistry::new(), None);
        assert_eq!(body, vec![Stmt::Return(Some(Expr::NumberLit(1.0)))]);
    }

    #[test]
    fn if_false_no_else_drops_then() {
        let stmts = parse_fn_body("function f() { if (false) { return 1; } }");
        let body = super::super::convert_stmts(&stmts, &TypeRegistry::new(), None);
        // Then dropped, no else → empty.
        assert_eq!(body, Vec::<Stmt>::new());
    }

    #[test]
    fn if_false_with_else_inlines_else_drops_then() {
        let stmts = parse_fn_body("function f() { if (false) { return 1; } else { return 2; } }");
        let body = super::super::convert_stmts(&stmts, &TypeRegistry::new(), None);
        assert_eq!(body, vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]);
    }

    #[test]
    fn bang_null_const_fold_dead_code_elim() {
        // `!null` const-folds to `BoolLit(true)` via Layer 1; convert_if_stmt
        // then dead-code-eliminates the `if true { ... }` wrapper. End-to-end
        // shape verifies Layer 1 + Layer 2 cooperate (Matrix C-7).
        let stmts = parse_fn_body("function f() { if (!null) { return 1; } }");
        let body = super::super::convert_stmts(&stmts, &TypeRegistry::new(), None);
        assert_eq!(body, vec![Stmt::Return(Some(Expr::NumberLit(1.0)))]);
    }

    #[test]
    fn bang_arrow_const_fold_dead_code_elim() {
        // `!<arrow>` const-folds to `BoolLit(false)` (function definitions
        // are always-truthy in JS, and the bare definition has no observable
        // side effect to preserve). `convert_if_stmt` then dead-code-eliminates
        // the `if false { return 1; } else { return 2; }` wrapper down to the
        // live (else) branch (Matrix C-24).
        let stmts =
            parse_fn_body("function f() { if (!(() => 1)) { return 1; } else { return 2; } }");
        let body = super::super::convert_stmts(&stmts, &TypeRegistry::new(), None);
        assert_eq!(body, vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]);
    }
}

#[test]
fn test_convert_stmt_while() {
    let stmts = parse_fn_body("function f() { while (x > 0) { x = x - 1; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::While {
            label: None,
            condition: Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            },
            body: vec![Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident("x".to_string())),
                value: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Sub,
                    right: Box::new(Expr::NumberLit(1.0)),
                }),
            })],
        }
    );
}

#[test]
fn test_labeled_for_in_stmt() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f() { outer: for (const key in obj) { console.log(key); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::ForIn { label, var, .. } => {
            assert_eq!(label, &Some("outer".to_string()));
            assert_eq!(var, "key");
        }
        other => panic!("expected ForIn, got: {other:?}"),
    }
}
