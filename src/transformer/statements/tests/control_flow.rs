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

#[test]
fn test_convert_stmt_do_while_basic() {
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
            assert_eq!(*label, None);
            // Body should have: the original statement + break check
            assert!(body.len() >= 2, "loop body should have at least 2 stmts");
            // Last statement should be `if !(condition) { break; }`
            let last = body.last().unwrap();
            match last {
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    // Condition should be !(...) wrapping the while condition
                    assert!(
                        matches!(condition, Expr::UnaryOp { op: UnOp::Not, .. }),
                        "condition should be negated"
                    );
                    assert_eq!(
                        then_body,
                        &vec![Stmt::Break {
                            label: None,
                            value: None
                        }]
                    );
                    assert!(else_body.is_none());
                }
                _ => panic!("expected If as last stmt, got: {last:?}"),
            }
        }
        other => panic!("expected Loop, got: {other:?}"),
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
