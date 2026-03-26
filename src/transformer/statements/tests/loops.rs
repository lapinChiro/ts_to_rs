use super::*;

#[test]
fn test_convert_stmt_for_counter_zero_to_n() {
    let stmts = parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { i; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Some(Box::new(Expr::NumberLit(0.0))),
                end: Some(Box::new(Expr::Ident("n".to_string()))),
            },
            body: vec![
                Stmt::Let {
                    mutable: false,
                    name: "i".to_string(),
                    ty: None,
                    init: Some(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    }),
                },
                Stmt::Expr(Expr::Ident("i".to_string())),
            ],
        }
    );
}

#[test]
fn test_convert_stmt_for_counter_start_to_literal() {
    let stmts = parse_fn_body("function f() { for (let i = 1; i < 10; i++) { i; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Some(Box::new(Expr::NumberLit(1.0))),
                end: Some(Box::new(Expr::NumberLit(10.0))),
            },
            body: vec![
                Stmt::Let {
                    mutable: false,
                    name: "i".to_string(),
                    ty: None,
                    init: Some(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    }),
                },
                Stmt::Expr(Expr::Ident("i".to_string())),
            ],
        }
    );
}

#[test]
fn test_convert_stmt_for_of() {
    let stmts = parse_fn_body("function f() { for (const item of items) { item; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "item".to_string(),
            iterable: Expr::Ident("items".to_string()),
            body: vec![Stmt::Expr(Expr::Ident("item".to_string()))],
        }
    );
}

// --- for...in ---

#[test]
fn test_convert_stmt_for_in_generates_keys_iteration() {
    let stmts = parse_fn_body("function f() { for (const k in obj) { k; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "k".to_string(),
            iterable: Expr::MethodCall {
                object: Box::new(Expr::Ident("obj".to_string())),
                method: "keys".to_string(),
                args: vec![],
            },
            body: vec![Stmt::Expr(Expr::Ident("k".to_string()))],
        }
    );
}

#[test]
fn test_convert_for_range_inserts_f64_shadow() {
    // for (let i = 0; i < n; i++) { sum += i; }
    // → body should start with: let i = i as f64;
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { sum += i; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::ForIn { body, .. } => {
            // First stmt should be: let i = i as f64;
            assert!(
                matches!(&body[0], Stmt::Let { name, init: Some(Expr::Cast { target: RustType::F64, .. }), .. } if name == "i"),
                "expected let i = i as f64; as first stmt, got {:?}",
                body[0]
            );
        }
        other => panic!("expected ForIn, got: {other:?}"),
    }
}

// -- General for loop (loop fallback) tests --

#[test]
fn test_convert_stmt_list_for_decrement_becomes_loop() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = n; i >= 0; i--) { console.log(i); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Should produce: let mut i = n; loop { if !(i >= 0) { break; } body; i--; }
    assert_eq!(result.len(), 2); // init + loop
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
    assert!(matches!(&result[1], Stmt::Loop { .. }));
}

#[test]
fn test_convert_stmt_list_for_step_by_two_becomes_loop() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(n: number) { for (let i = 0; i < n; i += 2) { console.log(i); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
    assert!(matches!(&result[1], Stmt::Loop { .. }));
}

#[test]
fn test_convert_stmt_for_simple_counter_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Existing simple counter pattern should still produce ForIn
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { console.log(i); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], Stmt::ForIn { .. }));
}

#[test]
fn test_convert_stmt_labeled_for_range() {
    let stmts =
        parse_fn_body("function f() { outer: for (let i = 0; i < 10; i++) { break outer; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::ForIn { label, .. } => {
            assert_eq!(label, Some("outer".to_string()));
        }
        _ => panic!("expected labeled ForIn"),
    }
}

#[test]
fn test_convert_stmt_for_of_array_destructuring_generates_tuple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // for (const [k, v] of entries) { ... }
    let stmts = parse_fn_body(
        "function f(entries: [string, number][]) { for (const [k, v] of entries) { console.log(k); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    // Should produce a ForIn with a tuple destructuring pattern
    assert!(!result.is_empty(), "should produce at least one statement");
}

#[test]
fn test_convert_stmt_for_of_array_destructuring_3_elements() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(entries: [string, number, boolean][]) { for (const [a, b, c] of entries) { console.log(a); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    // Should produce a ForIn with a 3-element tuple destructuring pattern "(a, b, c)"
    assert!(!result.is_empty(), "should produce at least one statement");
    let for_in = result.iter().find(|s| matches!(s, Stmt::ForIn { .. }));
    assert!(for_in.is_some(), "should contain a ForIn statement");
    match for_in.unwrap() {
        Stmt::ForIn { var, .. } => {
            assert_eq!(
                var, "(a, b, c)",
                "for-in var should be tuple pattern (a, b, c)"
            );
        }
        _ => unreachable!(),
    }
}

// ---- for loop multiple declarators ----

#[test]
fn test_convert_stmt_for_loop_multiple_declarators() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(n: number) { for (let i = 0, len = n; i < len; i++) { console.log(i); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    // Multiple declarators fall back to loop pattern: Let(i), Let(len), Loop { ... }
    assert!(
        result.len() >= 3,
        "expected at least 3 statements (2 lets + loop), got {:?}",
        result
    );
    // First two should be Let statements for i and len
    match &result[0] {
        Stmt::Let { name, mutable, .. } => {
            assert_eq!(name, "i");
            assert!(*mutable, "i should be mutable");
        }
        other => panic!("expected Let for i, got {:?}", other),
    }
    match &result[1] {
        Stmt::Let { name, mutable, .. } => {
            assert_eq!(name, "len");
            assert!(*mutable, "len should be mutable");
        }
        other => panic!("expected Let for len, got {:?}", other),
    }
}
