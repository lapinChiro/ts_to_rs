use super::*;

#[test]
fn test_convert_stmt_if_no_else() {
    let stmts = parse_fn_body("function f() { if (true) { return 1; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
            else_body: None,
        }
    );
}

#[test]
fn test_convert_stmt_if_else() {
    let stmts = parse_fn_body("function f() { if (true) { return 1; } else { return 2; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
            else_body: Some(vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]),
        }
    );
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

/// Helper: extract (LabeledBlock body, break_check) from a do-while Loop with continue.
/// Asserts: Loop { label, body: [LabeledBlock("do_while", body), If { break 'label }] }
fn assert_do_while_with_continue(stmt: &Stmt) -> (&Vec<Stmt>, &Stmt) {
    match stmt {
        Stmt::Loop { label, body } => {
            assert!(
                label.is_some(),
                "do-while with continue should have a loop label"
            );
            assert!(
                body.len() >= 2,
                "do-while loop body should have LabeledBlock + break check, got {body:?}"
            );
            let labeled_block = &body[0];
            let break_check = body.last().unwrap();
            match labeled_block {
                Stmt::LabeledBlock {
                    label: block_label,
                    body,
                } => {
                    assert_eq!(block_label, "do_while", "block label should be 'do_while'");
                    // Verify break check targets the loop label
                    match break_check {
                        Stmt::If {
                            condition,
                            then_body,
                            else_body,
                        } => {
                            assert!(
                                matches!(condition, Expr::UnaryOp { op: UnOp::Not, .. }),
                                "condition should be negated"
                            );
                            assert_eq!(
                                then_body,
                                &vec![Stmt::Break {
                                    label: label.clone(),
                                    value: None
                                }]
                            );
                            assert!(else_body.is_none());
                        }
                        _ => panic!("expected break check If, got: {break_check:?}"),
                    }
                    (body, break_check)
                }
                _ => panic!("expected LabeledBlock as first stmt, got: {labeled_block:?}"),
            }
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
}

#[test]
fn test_do_while_basic_no_continue_no_labeled_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { do { x = x - 1; } while (x > 0); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Loop { label, body } => {
            assert_eq!(*label, None, "no loop label when no continue");
            // No LabeledBlock — body is [assignment, break_check]
            assert_eq!(body.len(), 2, "body should have stmt + break check");
            assert!(
                matches!(&body[0], Stmt::Expr(Expr::Assign { .. })),
                "expected assignment, got: {:?}",
                body[0]
            );
            match &body[1] {
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    assert!(matches!(condition, Expr::UnaryOp { op: UnOp::Not, .. }));
                    assert_eq!(
                        then_body,
                        &vec![Stmt::Break {
                            label: None,
                            value: None
                        }]
                    );
                    assert!(else_body.is_none());
                }
                _ => panic!("expected break check If"),
            }
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
}

#[test]
fn test_do_while_continue_rewritten_to_break_label() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // continue inside an if inside do-while body
    let stmts =
        parse_fn_body("function f() { do { if (skip) { continue; } x = x + 1; } while (x < 10); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    let (body, _) = assert_do_while_with_continue(&result[0]);
    // First stmt should be If with break 'do_while in then_body
    match &body[0] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("do_while".to_string()),
                    value: None,
                }],
                "continue should be rewritten to break 'do_while"
            );
        }
        other => panic!("expected If stmt, got: {other:?}"),
    }
}

#[test]
fn test_do_while_break_rewritten_when_continue_present() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Both continue and break inside do-while body
    // continue → break 'do_while, break → break 'do_while_loop
    let stmts = parse_fn_body(
        "function f() { do { if (skip) { continue; } if (done) { break; } x += 1; } while (x < 10); }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // Loop should have label "do_while_loop" (auto-generated)
    match &result[0] {
        Stmt::Loop { label, .. } => {
            assert_eq!(label, &Some("do_while_loop".to_string()));
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
    let (body, _) = assert_do_while_with_continue(&result[0]);
    // body[0] = if (skip) { break 'do_while } (rewritten continue)
    match &body[0] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("do_while".to_string()),
                    value: None,
                }],
                "continue should be rewritten to break 'do_while"
            );
        }
        other => panic!("expected If stmt for continue, got: {other:?}"),
    }
    // body[1] = if (done) { break 'do_while_loop } (rewritten break)
    match &body[1] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("do_while_loop".to_string()),
                    value: None,
                }],
                "break should be rewritten to break 'do_while_loop"
            );
        }
        other => panic!("expected If stmt for break, got: {other:?}"),
    }
}

#[test]
fn test_do_while_nested_loop_continue_not_rewritten() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // continue inside a nested for-of loop — targets the for-of, not do-while
    // So the do-while should NOT have a LabeledBlock
    let stmts =
        parse_fn_body("function f() { do { for (const x of items) { continue; } } while (cond); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Loop { label, body } => {
            assert_eq!(
                *label, None,
                "no loop label when no continue targets do-while"
            );
            // Body should be [for-in, break_check] (no LabeledBlock)
            assert_eq!(body.len(), 2);
            match &body[0] {
                Stmt::ForIn { body: for_body, .. } => {
                    assert_eq!(
                        *for_body,
                        vec![Stmt::Continue { label: None }],
                        "continue inside nested loop should remain unchanged"
                    );
                }
                other => panic!("expected ForIn stmt, got: {other:?}"),
            }
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
}

#[test]
fn test_do_while_nested_do_while_continues_independent() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Nested do-while: each continue targets its own do-while
    let stmts =
        parse_fn_body("function f() { do { do { continue; } while (a); continue; } while (b); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    let (outer_body, _) = assert_do_while_with_continue(&result[0]);
    // Outer body: [inner_do_while_loop, break 'do_while (rewritten continue)]
    assert_eq!(
        outer_body.len(),
        2,
        "outer body should have inner loop + rewritten continue"
    );

    // Inner do-while should also have LabeledBlock structure
    let (inner_body, _) = assert_do_while_with_continue(&outer_body[0]);
    // Inner continue should be rewritten to break 'do_while (inner's block label)
    assert_eq!(
        inner_body,
        &vec![Stmt::Break {
            label: Some("do_while".to_string()),
            value: None,
        }],
        "inner continue should be rewritten to break 'do_while"
    );

    // Outer continue should also be rewritten
    assert_eq!(
        outer_body[1],
        Stmt::Break {
            label: Some("do_while".to_string()),
            value: None,
        },
        "outer continue should be rewritten to break 'do_while"
    );
}

#[test]
fn test_do_while_labeled_in_labeled_stmt() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // labeled do-while: continue outer → break 'do_while
    let stmts = parse_fn_body(
        "function f() { outer: do { if (skip) { continue outer; } x += 1; } while (x < 10); }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // Should be Loop with label "outer"
    match &result[0] {
        Stmt::Loop { label, .. } => {
            assert_eq!(label, &Some("outer".to_string()));
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
    let (body, _) = assert_do_while_with_continue(&result[0]);
    // continue outer should be rewritten to break 'do_while
    match &body[0] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("do_while".to_string()),
                    value: None,
                }],
                "continue outer should be rewritten to break 'do_while"
            );
        }
        other => panic!("expected If stmt, got: {other:?}"),
    }
}

#[test]
fn test_do_while_labeled_continue_targeting_outer_loop_not_rewritten() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // continue outer targets the while loop, not the do-while
    // So the do-while should NOT have a LabeledBlock
    let stmts = parse_fn_body(
        "function f() { outer: while (true) { do { continue outer; } while (x > 0); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // Outer is While with label "outer"
    match &result[0] {
        Stmt::While { label, body, .. } => {
            assert_eq!(label, &Some("outer".to_string()));
            // Inner is do-while Loop without LabeledBlock
            match &body[0] {
                Stmt::Loop {
                    label: inner_label,
                    body: inner_body,
                } => {
                    assert_eq!(*inner_label, None, "do-while should have no label");
                    // continue outer should remain as Continue { label: Some("outer") }
                    assert_eq!(
                        inner_body[0],
                        Stmt::Continue {
                            label: Some("outer".to_string()),
                        },
                        "continue outer targeting while loop should not be rewritten"
                    );
                }
                other => panic!("expected Loop, got: {other:?}"),
            }
        }
        other => panic!("expected While, got: {other:?}"),
    }
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

// --- Conditional assignment tests ---

#[test]
fn test_cond_assign_if_option_type_generates_if_let_some() {
    // if (x = getOpt()) { use(x); }
    // When getOpt returns Option<f64>, should generate: if let Some(x) = get_opt() { ... }
    let source =
        "function f(): void { let x: number | null = null; if (x = getOpt()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getOpt".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::Option(Box::new(RustType::F64))),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // Should produce IfLet with Some(x) pattern
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::IfLet { pattern, .. } if pattern == "Some(x)")),
        "expected IfLet with Some(x), got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_if_f64_type_generates_let_and_if_neq_zero() {
    // if (x = getNum()) { use(x); }
    // When getNum returns f64, should generate: let x = get_num(); if x != 0.0 { ... }
    let source = "function f(): void { let x: number = 0; if (x = getNum()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getNum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // Should contain: Let + If with condition x != 0.0
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::Let { name, .. } if name == "x")),
        "expected Let binding for x, got: {:?}",
        result
    );
    assert!(
        result.iter().any(|s| matches!(s, Stmt::If { .. })),
        "expected If statement, got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_while_option_type_generates_while_let_some() {
    // while (x = getOpt()) { use(x); }
    // When getOpt returns Option<f64>, should generate: while let Some(x) = get_opt() { ... }
    let source = "function f(): void { let x: number | null = null; while (x = getOpt()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getOpt".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::Option(Box::new(RustType::F64))),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::WhileLet { pattern, .. } if pattern == "Some(x)")),
        "expected WhileLet with Some(x), got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_while_f64_type_generates_loop_with_break() {
    // while (x = getNum()) { use(x); }
    // When getNum returns f64, should generate: loop { let x = ...; if x == 0.0 { break; } ... }
    let source =
        "function f(): void { let x: number = 0; while (x = getNum()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getNum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    assert!(
        result.iter().any(|s| matches!(s, Stmt::Loop { .. })),
        "expected Loop statement, got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_if_comparison_extracts_assignment() {
    // if ((x = compute()) > 0) { use(x); }
    // Should generate: let x = compute(); if x > 0.0 { ... }
    let source =
        "function f(): void { let x: number = 0; if ((x = compute()) > 0) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "compute".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::Let { name, .. } if name == "x")),
        "expected Let binding for x, got: {:?}",
        result
    );
    assert!(
        result.iter().any(|s| matches!(s, Stmt::If { .. })),
        "expected If with comparison, got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_normal_if_unchanged() {
    // if (x > 0) { ... } — no assignment, should pass through unchanged
    let source = "function f(): void { let x: number = 1; if (x > 0) { console.log(x); } }";
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // The result should contain an If statement (not a conditional assignment)
    assert!(
        result.iter().any(|s| matches!(s, Stmt::If { .. })),
        "expected If statement, got: {:?}",
        result
    );
}
