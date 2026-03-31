use super::*;

#[test]
fn test_convert_expr_postfix_increment_returns_old_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // i++ → { let _old = i; i = i + 1.0; _old }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("i++");

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
    }
    .convert_expr(&expr)
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 3);
            assert!(matches!(&stmts[0], IrStmt::Let { name, .. } if name == "_old"));
            assert!(matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "_old"));
        }
        other => panic!("expected Block for postfix i++, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_prefix_increment_returns_new_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ++i → { i = i + 1.0; i }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("++i");

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
    }
    .convert_expr(&expr)
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2);
            assert!(matches!(&stmts[0], IrStmt::Expr(Expr::Assign { .. })));
            assert!(matches!(&stmts[1], IrStmt::TailExpr(Expr::Ident(n)) if n == "i"));
        }
        other => panic!("expected Block for prefix ++i, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_postfix_decrement_returns_old_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // i-- → { let _old = i; i = i - 1.0; _old }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("i--");

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
    }
    .convert_expr(&expr)
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 3);
            assert!(matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "_old"));
        }
        other => panic!("expected Block for postfix i--, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_prefix_decrement_returns_new_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // --i → { i = i - 1.0; i }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("--i");

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
    }
    .convert_expr(&expr)
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2);
            assert!(matches!(&stmts[1], IrStmt::TailExpr(Expr::Ident(n)) if n == "i"));
        }
        other => panic!("expected Block for prefix --i, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_update_non_ident_target_errors() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr[0]++ should error because the target is not an identifier
    let expr = parse_expr("arr[0]++");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&expr);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("unsupported update expression target"),
        "expected 'unsupported update expression target', got: {err_msg}"
    );
}
