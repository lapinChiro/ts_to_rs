use super::*;

// --- contains_throw recursion tests ---

/// Helper: check if the function's return type is Result
fn fn_returns_result(source: &str) -> bool {
    let fn_decl = parse_fn_decl(source);
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap();
    matches!(
        item,
        Item::Fn {
            return_type: Some(RustType::Result { .. }),
            ..
        }
    )
}

#[test]
fn test_contains_throw_in_for_loop_wraps_result() {
    assert!(fn_returns_result(
        "function f(n: number) { for (let i = 0; i < n; i++) { throw new Error(\"x\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_while_loop_wraps_result() {
    assert!(fn_returns_result(
        "function f() { while (true) { throw new Error(\"x\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_do_while_wraps_result() {
    assert!(fn_returns_result(
        "function f() { do { throw new Error(\"x\"); } while (true); }"
    ));
}

#[test]
fn test_contains_throw_in_switch_detected() {
    // switch is not yet supported by convert_stmt, so test contains_throw directly
    let fn_decl =
        parse_fn_decl("function f(x: number) { switch(x) { case 1: throw new Error(\"x\"); } }");
    let block = fn_decl.function.body.as_ref().unwrap();
    assert!(
        contains_throw(&block.stmts),
        "should detect throw inside switch case"
    );
}

#[test]
fn test_contains_throw_in_for_of_wraps_result() {
    assert!(fn_returns_result(
        "function f(arr: string[]) { for (const x of arr) { throw new Error(\"x\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_try_block_excluded() {
    assert!(!fn_returns_result(
        "function f() { try { throw new Error(\"x\"); } catch(e) {} }"
    ));
}

#[test]
fn test_contains_throw_in_catch_block_wraps_result() {
    assert!(fn_returns_result(
        "function f() { try { } catch(e) { throw new Error(\"rethrow\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_labeled_wraps_result() {
    assert!(fn_returns_result(
        "function f() { outer: while(true) { throw new Error(\"x\"); } }"
    ));
}

// --- convert_last_return_to_tail tests ---

#[test]
fn test_convert_last_return_to_tail_converts_final_return() {
    let mut body = vec![
        Stmt::Expr(Expr::Ident("setup".to_string())),
        Stmt::Return(Some(Expr::Ident("x".to_string()))),
    ];
    convert_last_return_to_tail(&mut body);
    assert_eq!(body.len(), 2);
    assert_eq!(body[1], Stmt::TailExpr(Expr::Ident("x".to_string())));
}

#[test]
fn test_convert_last_return_to_tail_preserves_non_final_return() {
    let mut body = vec![
        Stmt::Return(Some(Expr::Ident("early".to_string()))),
        Stmt::Expr(Expr::Ident("x".to_string())),
    ];
    convert_last_return_to_tail(&mut body);
    // Non-final return should remain unchanged
    assert_eq!(
        body[0],
        Stmt::Return(Some(Expr::Ident("early".to_string())))
    );
}

#[test]
fn test_convert_last_return_to_tail_skips_return_none() {
    let mut body = vec![Stmt::Return(None)];
    convert_last_return_to_tail(&mut body);
    // Return(None) cannot be a tail expression — should remain unchanged
    assert_eq!(body[0], Stmt::Return(None));
}

#[test]
fn test_convert_last_return_to_tail_empty_body_noop() {
    let mut body: Vec<Stmt> = vec![];
    convert_last_return_to_tail(&mut body);
    assert!(body.is_empty());
}
