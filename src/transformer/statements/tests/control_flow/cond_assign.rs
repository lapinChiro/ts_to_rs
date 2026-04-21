//! RC-7 conditional assignment pattern tests.
//!
//! TS idiom `if (x = expr)` / `while (x = expr)` assigns then tests the new
//! value. The transformer rewrites by extracting the assignment into a `let`
//! binding and converting the test to a type-appropriate condition:
//!
//! - `Option<T>` result → `IfLet Some(x) = expr` / `WhileLet Some(x) = expr`
//! - `f64` result → `let x = expr; if x != 0.0 { ... }` /
//!   `loop { let x = expr; if x == 0.0 { break; } ... }`
//! - Comparison form `if ((x = compute()) > 0)` → extract assignment then
//!   compare the bound `x`
//! - Normal `if (x > 0)` (no assignment) → unchanged

use super::*;

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
    let expected_pat = crate::ir::Pattern::some_binding("x");
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::IfLet { pattern, .. } if *pattern == expected_pat)),
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

    let expected_pat = crate::ir::Pattern::some_binding("x");
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::WhileLet { pattern, .. } if *pattern == expected_pat)),
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
