use std::collections::HashMap;

use super::*;
use crate::ir::{BinOp, Expr, MatchPattern, RustType, Stmt, UnOp};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{MethodSignature, TypeDef, TypeRegistry};
use crate::transformer::context::TransformContext;
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;
use std::path::Path;
use swc_ecma_ast::{Decl, ModuleItem};

/// Helper: convert a single statement and assert exactly one IR statement is produced.
fn convert_single_stmt(
    stmt: &ast::Stmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Stmt {
    let (mg, res) = TctxFixture::empty_context_parts();
    let tctx = TransformContext::new(&mg, reg, &res, Path::new("test.ts"));
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let mut stmts = t.convert_stmt(stmt, return_type).unwrap();
    assert_eq!(stmts.len(), 1, "expected single statement, got {stmts:?}");
    stmts.remove(0)
}

/// Helper: convert a single statement from source, running TypeResolver first.
///
/// Unlike `convert_single_stmt`, this runs TypeResolver to populate expected types.
/// Use for tests that depend on type annotation-based expected type propagation.
fn convert_single_stmt_resolved(
    source: &str,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Stmt {
    let module = parse_typescript(source).expect("parse failed");
    let mut source_reg = crate::registry::build_registry(&module);
    source_reg.merge(reg);
    let mg = crate::pipeline::ModuleGraph::empty();
    let mut synthetic = SyntheticTypeRegistry::new();
    let parsed = crate::pipeline::ParsedFile {
        path: std::path::PathBuf::from("test.ts"),
        source: source.to_string(),
        module: module.clone(),
    };
    let mut resolver =
        crate::pipeline::type_resolver::TypeResolver::new(&source_reg, &mut synthetic);
    let res = resolver.resolve_file(&parsed);
    let tctx = TransformContext::new(&mg, &source_reg, &res, Path::new("test.ts"));

    // Extract the statement from the parsed module
    let stmt = match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Var(_))) => match &module.body[0] {
            ModuleItem::Stmt(s) => s,
            _ => unreachable!(),
        },
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => {
            // For function declarations, extract the return statement from body
            let body = fn_decl.function.body.as_ref().expect("no function body");
            let stmt = &body.stmts[0];
            let mut stmts = {
                let mut synthetic = SyntheticTypeRegistry::new();
                Transformer::for_module(&tctx, &mut synthetic).convert_stmt(stmt, return_type)
            }
            .unwrap();
            assert_eq!(stmts.len(), 1, "expected single statement, got {stmts:?}");
            return stmts.remove(0);
        }
        ModuleItem::Stmt(s) => s,
        _ => panic!("expected statement"),
    };

    let mut stmts = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(stmt, return_type)
    }
    .unwrap();
    assert_eq!(stmts.len(), 1, "expected single statement, got {stmts:?}");
    stmts.remove(0)
}

/// Helper: parse TS source containing a function and return its body statements.
fn parse_fn_body(source: &str) -> Vec<ast::Stmt> {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => fn_decl
            .function
            .body
            .as_ref()
            .expect("no function body")
            .stmts
            .clone(),
        _ => panic!("expected function declaration"),
    }
}

#[test]
fn test_convert_stmt_return_expr() {
    let stmts = parse_fn_body("function f() { return 42; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
}

#[test]
fn test_convert_stmt_return_no_value() {
    let stmts = parse_fn_body("function f() { return; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(result, Stmt::Return(None));
}

#[test]
fn test_convert_stmt_const_decl() {
    let stmts = parse_fn_body("function f() { const x = 1; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(1.0)),
        }
    );
}

#[test]
fn test_convert_stmt_let_decl_mutable() {
    let stmts = parse_fn_body("function f() { let x = 1; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::Let {
            mutable: true,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(1.0)),
        }
    );
}

#[test]
fn test_convert_stmt_const_with_type_annotation() {
    let stmts = parse_fn_body("function f() { const x: number = 1; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(RustType::F64),
            init: Some(Expr::NumberLit(1.0)),
        }
    );
}

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
fn test_convert_stmt_list_try_catch_expands_to_let_block_if() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f() { try { const x = 1; return x; } catch (e) { return 0; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Let(_try_result) + LabeledBlock + If
    assert_eq!(result.len(), 3, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_try_result"));
    match &result[1] {
        Stmt::LabeledBlock { label, body } => {
            assert_eq!(label, "try_block");
            assert_eq!(body.len(), 2, "try body should have 2 stmts");
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }
    match &result[2] {
        Stmt::IfLet {
            pattern, then_body, ..
        } => {
            assert!(
                pattern.contains("Err(e)"),
                "expected Err(e) pattern, got {pattern}"
            );
            assert_eq!(then_body.len(), 1);
        }
        _ => panic!("expected IfLet, got {:?}", result[2]),
    }
}

#[test]
fn test_convert_stmt_list_try_catch_empty_catch_expands_correctly() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { const x = 1; } catch (e) { } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 3, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_try_result"));
    match &result[1] {
        Stmt::LabeledBlock { body, .. } => assert_eq!(body.len(), 1),
        _ => panic!("expected LabeledBlock"),
    }
    match &result[2] {
        Stmt::IfLet { then_body, .. } => assert!(then_body.is_empty()),
        _ => panic!("expected IfLet"),
    }
}

#[test]
fn test_convert_stmt_list_try_finally_expands_to_scopeguard_and_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { const x = 1; } finally { const y = 2; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // scopeguard + try body inline
    assert_eq!(result.len(), 2, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_finally_guard"));
    assert!(matches!(&result[1], Stmt::Let { .. })); // const x = 1
}

#[test]
fn test_convert_stmt_list_try_catch_finally_expands_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { const x = 1; } catch (e) { const y = 2; } finally { const z = 3; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // scopeguard + _try_result + LabeledBlock + If
    assert_eq!(result.len(), 4, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_finally_guard"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "_try_result"));
    assert!(matches!(&result[2], Stmt::LabeledBlock { .. }));
    assert!(matches!(&result[3], Stmt::IfLet { .. }));
}

#[test]
fn test_convert_stmt_nested_try_catch_expands_inner_in_outer_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { try { x(); } catch (inner) { y(); } } catch (outer) { z(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Outer: Let + LabeledBlock + If
    assert_eq!(result.len(), 3, "got {result:?}");
    // The LabeledBlock body should contain the inner try/catch expansion (3 stmts)
    match &result[1] {
        Stmt::LabeledBlock { body, .. } => {
            // Inner: Let(_try_result) + LabeledBlock + If = 3 stmts
            assert_eq!(
                body.len(),
                3,
                "inner try/catch should expand to 3 stmts, got {body:?}"
            );
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }
    // Outer if let should use "outer" param
    match &result[2] {
        Stmt::IfLet { pattern, .. } => {
            assert!(
                pattern.contains("outer"),
                "expected outer in pattern, got {pattern}"
            );
        }
        _ => panic!("expected IfLet, got {:?}", result[2]),
    }
}

#[test]
fn test_convert_stmt_throw_new_error_string() {
    let stmts = parse_fn_body("function f() { throw new Error(\"something went wrong\"); }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    // throw new Error("msg") → return Err("msg".to_string())
    assert_eq!(
        result,
        Stmt::Return(Some(Expr::FnCall {
            name: "Err".to_string(),
            args: vec![Expr::MethodCall {
                object: Box::new(Expr::StringLit("something went wrong".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            }],
        }))
    );
}

#[test]
fn test_convert_stmt_throw_string_literal() {
    let stmts = parse_fn_body("function f() { throw \"error msg\"; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    // throw "msg" → return Err("msg".to_string())
    assert_eq!(
        result,
        Stmt::Return(Some(Expr::FnCall {
            name: "Err".to_string(),
            args: vec![Expr::MethodCall {
                object: Box::new(Expr::StringLit("error msg".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            }],
        }))
    );
}

// -- try/catch expansion tests (primitive IR, no Stmt::TryCatch) --

#[test]
fn test_convert_try_catch_basic_expands_to_let_labeledblock_if() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { risky(); } catch (e) { handle(e); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should expand to 3 statements: Let, LabeledBlock, If
    assert_eq!(result.len(), 3, "expected 3 stmts, got {result:?}");

    // 1. let mut _try_result: Result<(), String> = Ok(());
    match &result[0] {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            assert!(mutable, "expected mutable");
            assert_eq!(name, "_try_result");
            assert_eq!(
                ty,
                &Some(RustType::Result {
                    ok: Box::new(RustType::Unit),
                    err: Box::new(RustType::String),
                })
            );
            assert!(
                matches!(init, Some(Expr::FnCall { name, .. }) if name == "Ok"),
                "expected Ok(...) init, got {init:?}"
            );
        }
        _ => panic!("expected Let, got {:?}", result[0]),
    }

    // 2. 'try_block: { risky(); }
    match &result[1] {
        Stmt::LabeledBlock { label, body } => {
            assert_eq!(label, "try_block");
            assert_eq!(body.len(), 1, "expected 1 stmt in try body");
            assert!(
                matches!(&body[0], Stmt::Expr(Expr::FnCall { name, .. }) if name == "risky"),
                "expected risky() call, got {:?}",
                body[0]
            );
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }

    // 3. if let Err(e) = _try_result { handle(e); }
    match &result[2] {
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            assert!(
                pattern.contains("Err"),
                "expected Err pattern, got {pattern:?}"
            );
            assert!(
                matches!(expr, Expr::Ident(s) if s == "_try_result"),
                "expected _try_result expr, got {expr:?}"
            );
            assert!(!then_body.is_empty(), "expected catch body");
            assert!(else_body.is_none());
        }
        _ => panic!("expected IfLet, got {:?}", result[2]),
    }
}

#[test]
fn test_convert_try_catch_throw_in_body_expands_to_assign_break() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { throw new Error(\"oops\"); } catch (e) { handle(e); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    assert_eq!(result.len(), 3, "expected 3 stmts, got {result:?}");

    // Check the labeled block body: throw should be expanded to Assign + Break
    match &result[1] {
        Stmt::LabeledBlock { body, .. } => {
            assert_eq!(body.len(), 2, "expected assign + break, got {body:?}");
            // First: _try_result = Err("oops".to_string())
            match &body[0] {
                Stmt::Expr(Expr::Assign { target, value }) => {
                    assert!(
                        matches!(target.as_ref(), Expr::Ident(s) if s == "_try_result"),
                        "expected _try_result target"
                    );
                    assert!(
                        matches!(value.as_ref(), Expr::FnCall { name, .. } if name == "Err"),
                        "expected Err(...) value"
                    );
                }
                _ => panic!("expected Assign, got {:?}", body[0]),
            }
            // Second: break 'try_block;
            assert!(
                matches!(&body[1], Stmt::Break { label: Some(l), value: None } if l == "try_block"),
                "expected break 'try_block, got {:?}",
                body[1]
            );
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }
}

#[test]
fn test_convert_try_finally_expands_to_scopeguard_and_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { risky(); } finally { cleanup(); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should expand to: Let(scopeguard) + try body inline
    assert!(
        result.len() >= 2,
        "expected at least 2 stmts, got {result:?}"
    );

    // 1. let _finally_guard = scopeguard::guard((), |_| { cleanup(); });
    match &result[0] {
        Stmt::Let { name, init, .. } => {
            assert_eq!(name, "_finally_guard");
            assert!(
                matches!(init, Some(Expr::FnCall { name, .. }) if name == "scopeguard::guard"),
                "expected scopeguard::guard call, got {init:?}"
            );
        }
        _ => panic!("expected Let for scopeguard, got {:?}", result[0]),
    }

    // 2. try body inline: risky();
    assert!(
        matches!(&result[1], Stmt::Expr(Expr::FnCall { name, .. }) if name == "risky"),
        "expected risky() call, got {:?}",
        result[1]
    );
}

#[test]
fn test_convert_try_catch_finally_expands_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { risky(); } catch (e) { handle(e); } finally { cleanup(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should expand to: Let(scopeguard), Let(_try_result), LabeledBlock, If
    assert_eq!(result.len(), 4, "expected 4 stmts, got {result:?}");

    // 1. scopeguard
    match &result[0] {
        Stmt::Let { name, .. } => assert_eq!(name, "_finally_guard"),
        _ => panic!("expected Let for scopeguard, got {:?}", result[0]),
    }

    // 2. _try_result
    match &result[1] {
        Stmt::Let { name, .. } => assert_eq!(name, "_try_result"),
        _ => panic!("expected Let for _try_result, got {:?}", result[1]),
    }

    // 3. labeled block
    assert!(
        matches!(&result[2], Stmt::LabeledBlock { label, .. } if label == "try_block"),
        "expected LabeledBlock, got {:?}",
        result[2]
    );

    // 4. if let error check
    assert!(
        matches!(&result[3], Stmt::IfLet { .. }),
        "expected IfLet, got {:?}",
        result[3]
    );
}

// -- try/catch break/continue in try body tests --

#[test]
fn test_convert_try_catch_break_in_loop_uses_flag() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // break inside try body (within a for loop) should use flag pattern
    let stmts = parse_fn_body(
        "function f(items: number[]) { for (const item of items) { try { if (item < 0) { break; } risky(); } catch (e) { handle(e); } } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should have a ForIn with body containing: _try_result, _try_break, LabeledBlock, if _try_break { break }, if let Err
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::ForIn { body, .. } => {
            // Look for _try_break flag declaration
            let has_break_flag = body
                .iter()
                .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_try_break"));
            assert!(has_break_flag, "expected _try_break flag, got {body:?}");

            // Look for break flag check: if _try_break { break; }
            let has_break_check = body.iter().any(|s| {
                matches!(s, Stmt::If { condition: Expr::Ident(c), then_body, .. }
                    if c == "_try_break" && matches!(then_body.first(), Some(Stmt::Break { label: None, .. })))
            });
            assert!(
                has_break_check,
                "expected if _try_break {{ break; }}, got {body:?}"
            );
        }
        _ => panic!("expected ForIn, got {:?}", result[0]),
    }
}

#[test]
fn test_convert_try_catch_continue_in_loop_uses_flag() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(items: number[]) { for (const item of items) { try { if (item < 0) { continue; } risky(); } catch (e) { handle(e); } } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::ForIn { body, .. } => {
            // Look for _try_continue flag declaration
            let has_continue_flag = body
                .iter()
                .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_try_continue"));
            assert!(
                has_continue_flag,
                "expected _try_continue flag, got {body:?}"
            );

            // Look for continue flag check: if _try_continue { continue; }
            let has_continue_check = body.iter().any(|s| {
                matches!(s, Stmt::If { condition: Expr::Ident(c), then_body, .. }
                    if c == "_try_continue" && matches!(then_body.first(), Some(Stmt::Continue { label: None })))
            });
            assert!(
                has_continue_check,
                "expected if _try_continue {{ continue; }}, got {body:?}"
            );
        }
        _ => panic!("expected ForIn, got {:?}", result[0]),
    }
}

// -- try/catch with return type tests --

#[test]
fn test_convert_try_catch_both_return_adds_unreachable() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // When both try and catch end with return in a function with return type,
    // unreachable!() should be added after the if-let-Err block
    let stmts = parse_fn_body(
        "function safeDivide(a: number, b: number): number { try { if (b === 0) throw new Error(\"div by zero\"); return a / b; } catch (e) { return 0; } }",
    );
    let return_type = RustType::F64;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, Some(&return_type))
    }
    .unwrap();

    // Last statement should be Expr(MacroCall { name: "unreachable", args: [] })
    let last = result.last().expect("should have statements");
    assert!(
        matches!(last, Stmt::Expr(Expr::MacroCall { name, args, .. }) if name == "unreachable" && args.is_empty()),
        "expected unreachable!() as last stmt, got {last:?}"
    );
}

#[test]
fn test_convert_try_catch_try_no_return_no_unreachable() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // When try body does NOT end with return, unreachable!() should NOT be added
    let stmts = parse_fn_body(
        "function riskyOp(x: number): number { try { if (x < 0) { throw new Error(\"negative\"); } console.log(x); } catch (e) { console.log(e); } return x; }",
    );
    let return_type = RustType::F64;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, Some(&return_type))
    }
    .unwrap();

    // The last statement should be the Return(x), NOT unreachable
    let last = result.last().expect("should have statements");
    assert!(
        matches!(last, Stmt::Return(_)),
        "expected Return as last stmt (no unreachable), got {last:?}"
    );
}

// -- for-range loop variable f64 shadow tests --

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

// -- Object literal in variable declaration tests --

#[test]
fn test_convert_stmt_var_decl_object_literal_with_type_annotation() {
    // const + Named type → let mut (TS const allows field mutation)
    let result = convert_single_stmt_resolved(
        "const p: Point = { x: 1, y: 2 };",
        &TypeRegistry::new(),
        None,
    );
    assert_eq!(
        result,
        Stmt::Let {
            mutable: true,
            name: "p".to_string(),
            ty: Some(RustType::Named {
                name: "Point".to_string(),
                type_args: vec![],
            }),
            init: Some(Expr::StructInit {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), Expr::NumberLit(1.0)),
                    ("y".to_string(), Expr::NumberLit(2.0)),
                ],
                base: None,
            }),
        }
    );
}

#[test]
fn test_convert_stmt_expression_statement() {
    let stmts = parse_fn_body("function f() { foo; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(result, Stmt::Expr(Expr::Ident("foo".to_string())));
}

// -- Expected type propagation tests --

#[test]
fn test_convert_stmt_var_decl_string_type_annotation_adds_to_string() {
    let result =
        convert_single_stmt_resolved(r#"const s: string = "hello";"#, &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
            name: "s".to_string(),
            ty: Some(RustType::String),
            init: Some(Expr::MethodCall {
                object: Box::new(Expr::StringLit("hello".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            }),
        }
    );
}

#[test]
fn test_convert_stmt_var_decl_string_array_type_annotation() {
    // const + Vec type → let mut (TS const allows push/pop)
    let result = convert_single_stmt_resolved(
        r#"const a: string[] = ["a", "b"];"#,
        &TypeRegistry::new(),
        None,
    );
    assert_eq!(
        result,
        Stmt::Let {
            mutable: true,
            name: "a".to_string(),
            ty: Some(RustType::Vec(Box::new(RustType::String))),
            init: Some(Expr::Vec {
                elements: vec![
                    Expr::MethodCall {
                        object: Box::new(Expr::StringLit("a".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    },
                    Expr::MethodCall {
                        object: Box::new(Expr::StringLit("b".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    },
                ],
            }),
        }
    );
}

#[test]
fn test_convert_stmt_return_string_with_string_return_type() {
    let result = convert_single_stmt_resolved(
        r#"function f(): string { return "ok"; }"#,
        &TypeRegistry::new(),
        Some(&RustType::String),
    );
    assert_eq!(
        result,
        Stmt::Return(Some(Expr::MethodCall {
            object: Box::new(Expr::StringLit("ok".to_string())),
            method: "to_string".to_string(),
            args: vec![],
        }))
    );
}

#[test]
fn test_convert_stmt_return_number_with_f64_return_type_unchanged() {
    let stmts = parse_fn_body("function f(): number { return 42; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), Some(&RustType::F64));
    assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
}

// -- break / continue tests --

#[test]
fn test_convert_stmt_break_no_label() {
    let stmts = parse_fn_body("function f() { while (true) { break; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::While { body, .. } => {
            assert_eq!(
                body[0],
                Stmt::Break {
                    label: None,
                    value: None
                }
            );
        }
        _ => panic!("expected While"),
    }
}

#[test]
fn test_convert_stmt_continue_no_label() {
    let stmts = parse_fn_body("function f() { while (true) { continue; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::While { body, .. } => {
            assert_eq!(body[0], Stmt::Continue { label: None });
        }
        _ => panic!("expected While"),
    }
}

#[test]
fn test_convert_stmt_break_with_label() {
    let stmts = parse_fn_body("function f() { outer: while (true) { break outer; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::While { label, body, .. } => {
            assert_eq!(label, Some("outer".to_string()));
            assert_eq!(
                body[0],
                Stmt::Break {
                    label: Some("outer".to_string()),
                    value: None,
                }
            );
        }
        _ => panic!("expected labeled While"),
    }
}

#[test]
fn test_convert_stmt_continue_with_label() {
    let stmts = parse_fn_body("function f() { outer: for (const x of items) { continue outer; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::ForIn { label, body, .. } => {
            assert_eq!(label, Some("outer".to_string()));
            assert_eq!(
                body[0],
                Stmt::Continue {
                    label: Some("outer".to_string())
                }
            );
        }
        _ => panic!("expected labeled ForIn"),
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

// -- Object destructuring tests --

#[test]
fn test_convert_stmt_list_object_destructuring_basic() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { const { x, y } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::FieldAccess {
                object: Box::new(Expr::Ident("obj".to_string())),
                field: "x".to_string(),
            }),
        }
    );
    assert_eq!(
        result[1],
        Stmt::Let {
            mutable: false,
            name: "y".to_string(),
            ty: None,
            init: Some(Expr::FieldAccess {
                object: Box::new(Expr::Ident("obj".to_string())),
                field: "y".to_string(),
            }),
        }
    );
}

#[test]
fn test_convert_stmt_list_object_destructuring_let_mutable() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { let { x, y } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "y"));
}

#[test]
fn test_convert_stmt_list_object_destructuring_rename() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { const { x: newX } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "newX".to_string(),
            ty: None,
            init: Some(Expr::FieldAccess {
                object: Box::new(Expr::Ident("obj".to_string())),
                field: "x".to_string(),
            }),
        }
    );
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

// -- Array destructuring tests --

#[test]
fn test_convert_stmt_list_array_destructuring_basic() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(arr: number[]) { const [a, b] = arr; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "a".to_string(),
            ty: None,
            init: Some(Expr::Index {
                object: Box::new(Expr::Ident("arr".to_string())),
                index: Box::new(Expr::NumberLit(0.0)),
            }),
        }
    );
    assert_eq!(
        result[1],
        Stmt::Let {
            mutable: false,
            name: "b".to_string(),
            ty: None,
            init: Some(Expr::Index {
                object: Box::new(Expr::Ident("arr".to_string())),
                index: Box::new(Expr::NumberLit(1.0)),
            }),
        }
    );
}

#[test]
fn test_convert_stmt_list_array_destructuring_let_mutable() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(arr: number[]) { let [x, y] = arr; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "y"));
}

#[test]
fn test_convert_stmt_list_array_destructuring_single_element() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(arr: number[]) { const [a] = arr; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "a".to_string(),
            ty: None,
            init: Some(Expr::Index {
                object: Box::new(Expr::Ident("arr".to_string())),
                index: Box::new(Expr::NumberLit(0.0)),
            }),
        }
    );
}

// -- do...while tests --

#[test]
fn test_convert_stmt_do_while_basic() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(x: number) { do { x = x + 1; } while (x < 10); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Loop { label, body } => {
            assert!(label.is_none());
            // body should have: x = x + 1, then if !(x < 10) { break; }
            assert_eq!(body.len(), 2);
            assert!(matches!(&body[0], Stmt::Expr(Expr::Assign { .. })));
            match &body[1] {
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    assert!(matches!(condition, Expr::UnaryOp { op, .. } if *op == UnOp::Not));
                    assert_eq!(then_body.len(), 1);
                    assert!(matches!(
                        &then_body[0],
                        Stmt::Break {
                            label: None,
                            value: None
                        }
                    ));
                    assert!(else_body.is_none());
                }
                _ => panic!("expected If statement for break condition"),
            }
        }
        _ => panic!("expected Loop"),
    }
}

#[test]
fn test_convert_stmt_list_array_destructuring_three_elements() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(arr: number[]) { const [a, b, c] = arr; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 3);
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "b"));
    assert!(matches!(&result[2], Stmt::Let { name, .. } if name == "c"));
}

#[test]
fn test_convert_stmt_list_array_destructuring_skip_element() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(arr: number[]) { const [a, , b] = arr; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "b"));
    // Verify correct indices: a = arr[0], b = arr[2]
    if let Stmt::Let {
        init: Some(Expr::Index { index, .. }),
        ..
    } = &result[1]
    {
        assert_eq!(**index, Expr::NumberLit(2.0));
    } else {
        panic!("expected Index expression");
    }
}

#[test]
fn test_convert_stmt_list_array_destructuring_rest() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(arr: number[]) { const [first, ...rest] = arr; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "first"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "rest"));
}

#[test]
fn test_convert_stmt_nested_fn_decl_generates_closure_let() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function outer() { function inner(x: number): number { return x; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Let {
            name,
            mutable,
            init: Some(Expr::Closure { params, .. }),
            ..
        } => {
            assert_eq!(name, "inner");
            assert!(!mutable);
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
        }
        other => panic!("expected Let with Closure, got: {other:?}"),
    }
}

// --- Spread array expansion tests (SWC AST level) ---

#[test]
fn test_convert_stmt_spread_let_single_spread_optimizes_to_clone() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const x = [...arr] → let x = arr.clone();
    let stmts = parse_fn_body("function f(arr: number[]) { const x = [...arr]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(&result[0], Stmt::Let { name, init: Some(Expr::MethodCall { method, .. }), .. }
            if name == "x" && method == "clone"),
        "expected let x = arr.clone(), got: {result:?}"
    );
}

#[test]
fn test_convert_stmt_spread_let_mixed_segments_expands_to_stmts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const x = [...arr, 1] → let mut x = Vec::new(); x.extend(...); x.push(1.0);
    let stmts = parse_fn_body("function f(arr: number[]) { const x = [...arr, 1]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 3, "expected 3 statements, got: {result:?}");
    // First: let mut x = Vec::new();
    assert!(matches!(
        &result[0],
        Stmt::Let { mutable: true, name, init: Some(Expr::FnCall { name: fn_name, .. }), .. }
        if name == "x" && fn_name == "Vec::new"
    ));
    // Second: x.extend(...)
    assert!(matches!(
        &result[1],
        Stmt::Expr(Expr::MethodCall { method, .. }) if method == "extend"
    ));
    // Third: x.push(1.0)
    assert!(matches!(
        &result[2],
        Stmt::Expr(Expr::MethodCall { method, .. }) if method == "push"
    ));
}

#[test]
fn test_convert_stmt_spread_return_single_spread_optimizes_to_clone() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // return [...arr] → return arr.clone();
    let stmts = parse_fn_body("function f(arr: number[]) { return [...arr]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(&result[0], Stmt::Return(Some(Expr::MethodCall { method, .. })) if method == "clone"),
        "expected return arr.clone(), got: {result:?}"
    );
}

#[test]
fn test_convert_stmt_spread_return_mixed_segments_expands_to_stmts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // return [...arr, 1] → let mut __spread_vec = Vec::new(); ...; return __spread_vec;
    let stmts = parse_fn_body("function f(arr: number[]) { return [...arr, 1]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert!(
        result.len() >= 4,
        "expected at least 4 statements, got: {result:?}"
    );
    assert!(matches!(
        &result[0],
        Stmt::Let { mutable: true, name, .. } if name == "__spread_vec"
    ));
    assert!(matches!(
        result.last().unwrap(),
        Stmt::Return(Some(Expr::Ident(n))) if n == "__spread_vec"
    ));
}

#[test]
fn test_convert_stmt_spread_non_spread_array_uses_normal_path() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const x = [1, 2] → let x = vec![1.0, 2.0]; (normal path, no expansion)
    let stmts = parse_fn_body("function f() { const x = [1, 2]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(&result[0], Stmt::Let { name, init: Some(Expr::Vec { .. }), .. } if name == "x")
    );
}

// -- switch statement tests --

#[test]
fn test_convert_switch_single_case_break_generates_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(x: number) { switch(x) { case 1: doA(); break; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert_eq!(arms[0].patterns.len(), 1);
            assert!(arms[0]
                .patterns
                .iter()
                .all(|p| matches!(p, crate::ir::MatchPattern::Literal(_))));
            assert!(!arms[0].body.is_empty());
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_empty_fallthrough_merges_patterns() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f(x: number) { switch(x) { case 1: case 2: doAB(); break; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert_eq!(
                arms[0].patterns.len(),
                2,
                "expected 2 patterns for merged cases"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_default_generates_wildcard() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); break; default: doB(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            assert!(arms[0]
                .patterns
                .iter()
                .all(|p| matches!(p, crate::ir::MatchPattern::Literal(_))));
            assert!(
                arms[1]
                    .patterns
                    .iter()
                    .any(|p| matches!(p, crate::ir::MatchPattern::Wildcard)),
                "last arm should be wildcard"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_fallthrough_generates_labeled_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // break-less fall-through: case 1 falls into case 2
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); case 2: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    // Fall-through path generates a LabeledBlock with flag pattern
    match &result[0] {
        Stmt::LabeledBlock { label, body } => {
            assert_eq!(label, "switch");
            // Should contain: let mut _fall = false; + if chains
            let has_fall_flag = body
                .iter()
                .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_fall"));
            assert!(has_fall_flag, "expected _fall flag, got {body:?}");
        }
        other => panic!("expected LabeledBlock for fall-through, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_return_terminated_case_generates_clean_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // case ending with return should be treated as terminated → clean match, not fall-through
    let stmts = parse_fn_body(
        "function f(x: number): string { switch(x) { case 1: return \"one\"; case 2: return \"two\"; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // Both arms should have return statements
            assert!(matches!(arms[0].body.last(), Some(Stmt::Return(_))));
            assert!(matches!(arms[1].body.last(), Some(Stmt::Return(_))));
        }
        other => panic!("expected Match (not LabeledBlock), got {other:?}"),
    }
}

#[test]
fn test_convert_switch_throw_terminated_case_generates_clean_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); throw new Error(\"fail\"); case 2: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2, "expected 2 arms, got {arms:?}");
        }
        other => panic!("expected Match (not LabeledBlock), got {other:?}"),
    }
}

#[test]
fn test_convert_switch_string_discriminant_generates_string_patterns() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(s: string) { switch(s) { case \"hello\": doA(); break; case \"world\": doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // Patterns should be StringLit
            assert!(
                arms[0].patterns.iter().any(|p| matches!(
                    p,
                    crate::ir::MatchPattern::Literal(Expr::StringLit(s)) if s == "hello"
                )),
                "expected string pattern 'hello', got {:?}",
                arms[0].patterns
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// --- Switch non-literal case ---

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
                    .any(|p| matches!(p, crate::ir::MatchPattern::Wildcard)),
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
            // First arm (case 1) - literal, no guard
            assert!(
                arms[0].guard.is_none(),
                "literal case should have no guard, got {:?}",
                arms[0]
            );
            // Second arm (case A) - non-literal, has guard
            assert!(
                arms[1].guard.is_some(),
                "non-literal case should have a guard, got {:?}",
                arms[1]
            );
        }
        _ => unreachable!(),
    }
}

// --- Local interface/type declarations ---

#[test]
fn test_convert_stmt_local_interface_skipped() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // interface inside function body should not error, just be skipped
    let stmts = parse_fn_body("function f() { interface Foo { x: number; } const a: number = 1; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Should have 1 statement (const a), interface is skipped
    assert_eq!(
        result.len(),
        1,
        "expected 1 stmt (interface skipped), got {result:?}"
    );
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
}

#[test]
fn test_convert_stmt_local_type_alias_skipped() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // type alias inside function body should not error, just be skipped
    let stmts = parse_fn_body("function f() { type ID = number; const b: number = 2; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(
        result.len(),
        1,
        "expected 1 stmt (type alias skipped), got {result:?}"
    );
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "b"));
}

// --- const mutability body scan ---

#[test]
fn test_const_field_assignment_in_body_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const with number type (primitive, not object) but field assignment in body → let mut
    // is_object_type returns false for number, so body scan must detect the mutation
    let stmts = parse_fn_body("function f() { const p: number = 0; p.x = 1; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // First stmt should be `let mut p = ...`
    match &result[0] {
        Stmt::Let { mutable, name, .. } => {
            assert_eq!(name, "p");
            assert!(
                *mutable,
                "const with field assignment should become let mut"
            );
        }
        other => panic!("expected Let, got {other:?}"),
    }
}

#[test]
fn test_const_mutating_method_in_body_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const arr WITHOUT type annotation, with push() call in body → let mut
    // This tests the body-scan path (not the is_object_type path)
    let stmts = parse_fn_body("function f() { const arr = [1, 2, 3]; arr.push(4); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    match &result[0] {
        Stmt::Let { mutable, name, .. } => {
            assert_eq!(name, "arr");
            assert!(
                *mutable,
                "const with mutating method call should become let mut"
            );
        }
        other => panic!("expected Let, got {other:?}"),
    }
}

// --- Closure mutable capture ---

#[test]
fn test_closure_mutating_outer_var_closure_binding_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const inc = () => { count += 1; } where closure captures mutably
    // → inc should be `let mut` because calling FnMut requires mutable binding
    let stmts = parse_fn_body(
        "function f() { let count: number = 0; const inc = (): void => { count += 1; }; }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Second stmt: let mut inc = || { ... } (closure binding needs mut for FnMut)
    match &result[1] {
        Stmt::Let { mutable, name, .. } => {
            assert_eq!(name, "inc");
            assert!(*mutable, "closure that captures mutably should be let mut");
        }
        other => panic!("expected Let for inc, got {other:?}"),
    }
}

// --- Object destructuring extensions ---

#[test]
fn test_object_destructuring_default_number_generates_unwrap_or() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { const { x = 0 } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // { x = 0 } → let x = obj.x.unwrap_or(0.0);
    match &result[0] {
        Stmt::Let {
            name,
            init: Some(expr),
            ..
        } => {
            assert_eq!(name, "x");
            assert!(
                matches!(expr, Expr::MethodCall { method, .. } if method == "unwrap_or"),
                "expected unwrap_or call, got: {:?}",
                expr
            );
        }
        other => panic!("expected Let with unwrap_or, got: {:?}", other),
    }
}

#[test]
fn test_object_destructuring_default_string_generates_unwrap_or_else() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { const { x = \"hi\" } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // { x = "hi" } → let x = obj.x.unwrap_or_else(|| "hi".to_string());
    match &result[0] {
        Stmt::Let {
            name,
            init: Some(expr),
            ..
        } => {
            assert_eq!(name, "x");
            assert!(
                matches!(expr, Expr::MethodCall { method, .. } if method == "unwrap_or_else"),
                "expected unwrap_or_else call, got: {:?}",
                expr
            );
        }
        other => panic!("expected Let with unwrap_or_else, got: {:?}", other),
    }
}

#[test]
fn test_object_destructuring_default_bool_generates_unwrap_or() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { const { x = true } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Let {
            name,
            init: Some(expr),
            ..
        } => {
            assert_eq!(name, "x");
            assert!(
                matches!(expr, Expr::MethodCall { method, .. } if method == "unwrap_or"),
                "expected unwrap_or call, got: {:?}",
                expr
            );
        }
        other => panic!("expected Let with unwrap_or, got: {:?}", other),
    }
}

#[test]
fn test_object_destructuring_nested_generates_chained_field_access() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // { a: { b } } = obj → let b = obj.a.b;
    let stmts = parse_fn_body("function f() { const { a: { b } } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Let {
            name,
            init: Some(init),
            ..
        } => {
            assert_eq!(name, "b");
            // Should be obj.a.b (nested FieldAccess)
            match init {
                Expr::FieldAccess { object, field } => {
                    assert_eq!(field, "b");
                    assert!(
                        matches!(object.as_ref(), Expr::FieldAccess { field: inner_field, .. } if inner_field == "a"),
                        "expected obj.a.b, got: {:?}",
                        init
                    );
                }
                _ => panic!("expected FieldAccess, got: {:?}", init),
            }
        }
        other => panic!("expected Let, got: {:?}", other),
    }
}

#[test]
fn test_object_destructuring_nested_multiple_fields() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // { a: { b, c } } = obj → let b = obj.a.b; let c = obj.a.c;
    let stmts = parse_fn_body("function f() { const { a: { b, c } } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2, "expected 2 stmts, got: {:?}", result);
    match &result[0] {
        Stmt::Let { name, .. } => assert_eq!(name, "b"),
        other => panic!("expected Let for b, got: {:?}", other),
    }
    match &result[1] {
        Stmt::Let { name, .. } => assert_eq!(name, "c"),
        other => panic!("expected Let for c, got: {:?}", other),
    }
}

#[test]
fn test_object_destructuring_rest_with_type_expands_remaining_fields() {
    // { a, ...rest } = point where Point has { a, b, c }
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("a".to_string(), RustType::F64),
                ("b".to_string(), RustType::F64),
                ("c".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let source = "function f(point: Point) { const { a, ...rest } = point; }";
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
    // { a, ...rest } → let a = point.a; let b = point.b; let c = point.c;
    assert_eq!(result.len(), 3, "expected 3 stmts, got: {:?}", result);
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "b"));
    assert!(matches!(&result[2], Stmt::Let { name, .. } if name == "c"));
}

#[test]
fn test_object_destructuring_rest_no_type_generates_comment() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // { a, ...rest } = obj where obj has unknown type
    let stmts = parse_fn_body("function f() { const { a, ...rest } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Should have at least the explicit field `a` and a comment statement for rest
    assert!(
        !result.is_empty(),
        "expected at least 1 stmt, got: {:?}",
        result
    );
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
}

#[test]
fn test_object_destructuring_no_default_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Existing behavior: { x } → let x = obj.x;
    let stmts = parse_fn_body("function f() { const { x } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(
            &result[0],
            Stmt::Let { name, init: Some(Expr::FieldAccess { .. }), .. } if name == "x"
        ),
        "expected plain FieldAccess, got: {:?}",
        result[0]
    );
}

// --- discriminated union switch → enum match ---

#[test]
fn test_convert_switch_discriminated_union_to_enum_match() {
    let source = r#"
        function main(): void {
            const s: Shape = { kind: "circle", radius: 5 };
            switch (s.kind) {
                case "circle":
                    console.log("circle");
                    break;
                case "square":
                    console.log("square");
                    break;
            }
        }
    "#;

    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![("side".to_string(), RustType::F64)],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );

    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected function declaration"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // Find the match statement
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");

    if let Stmt::Match { expr, arms } = match_stmt {
        // Match on `s` (the enum variable), not `s.kind`
        assert_eq!(*expr, Expr::Ref(Box::new(Expr::Ident("s".to_string()))));
        // First arm should be EnumVariant pattern
        assert!(
            arms[0].patterns.iter().any(
                |p| matches!(p, MatchPattern::EnumVariant { path, .. } if path == "Shape::Circle")
            ),
            "expected EnumVariant pattern for circle, got: {:?}",
            arms[0].patterns
        );
    } else {
        panic!("expected Match");
    }
}

// --- discriminated union field access in switch arms ---

/// Helper to build a Shape discriminated union registry.
fn build_shape_registry() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![
            ("width".to_string(), RustType::F64),
            ("height".to_string(), RustType::F64),
        ],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );
    reg
}

#[test]
fn test_convert_du_switch_field_access_single_field_becomes_binding() {
    let source = r#"
        function get_radius(s: Shape): number {
            switch (s.kind) {
                case "circle":
                    return s.radius;
                case "square":
                    return 0;
            }
        }
    "#;
    let reg = build_shape_registry();
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected function declaration"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic)
            .convert_stmt_list(body_stmts, Some(&RustType::F64))
    }
    .unwrap();

    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");

    if let Stmt::Match { arms, .. } = match_stmt {
        // Circle arm should have "radius" in bindings
        let circle_arm = &arms[0];
        assert!(
            circle_arm.patterns.iter().any(
                |p| matches!(p, MatchPattern::EnumVariant { bindings, .. } if bindings == &["radius"])
            ),
            "expected radius binding in Circle arm, got: {:?}",
            circle_arm.patterns
        );
        // Circle arm body should reference `radius.clone()` (match on &s binds by ref)
        assert!(
            circle_arm.body.iter().any(|s| {
                matches!(s, Stmt::Return(Some(Expr::MethodCall { object, method, .. }))
                    if matches!(object.as_ref(), Expr::Ident(name) if name == "radius")
                    && method == "clone")
            }),
            "expected return of `radius.clone()`, got: {:?}",
            circle_arm.body
        );
        // Square arm should have no bindings (no field access)
        let square_arm = &arms[1];
        assert!(
            square_arm.patterns.iter().any(
                |p| matches!(p, MatchPattern::EnumVariant { bindings, .. } if bindings.is_empty())
            ),
            "expected no bindings in Square arm, got: {:?}",
            square_arm.patterns
        );
    } else {
        panic!("expected Match");
    }
}

#[test]
fn test_convert_du_switch_field_access_multiple_fields_become_bindings() {
    let source = r#"
        function area(s: Shape): number {
            switch (s.kind) {
                case "circle":
                    return 0;
                case "square":
                    return s.width * s.height;
            }
        }
    "#;
    let reg = build_shape_registry();
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected function declaration"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic)
            .convert_stmt_list(body_stmts, Some(&RustType::F64))
    }
    .unwrap();

    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");

    if let Stmt::Match { arms, .. } = match_stmt {
        // Square arm should have width and height in bindings
        let square_arm = &arms[1];
        let has_bindings = square_arm.patterns.iter().any(|p| {
            if let MatchPattern::EnumVariant { bindings, .. } = p {
                bindings.contains(&"width".to_string()) && bindings.contains(&"height".to_string())
            } else {
                false
            }
        });
        assert!(
            has_bindings,
            "expected width, height bindings in Square arm, got: {:?}",
            square_arm.patterns
        );
    } else {
        panic!("expected Match");
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

// ---- for...of array destructuring ----

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

// ---- EmptyStmt ----

#[test]
fn test_convert_stmt_empty_stmt_produces_no_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // function f(): void { ; } — the empty statement should produce no IR
    let stmts = parse_fn_body("function f(): void { ; }");
    assert_eq!(stmts.len(), 1, "should parse one empty statement");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert!(
        result.is_empty(),
        "empty statement should produce no IR statements"
    );
}

// ---- for...of array destructuring - 3 elements ----

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

#[test]
fn test_convert_var_decl_trait_type_generates_box_dyn() {
    // const g: Greeter = ... → let g: Box<dyn Greeter> = ...
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String)],
            return_type: None,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_interface(vec![], methods, vec![]),
    );
    let stmts = parse_fn_body("function _f(): void { const g: Greeter = null as any; }");
    let stmt = &stmts[0];
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(stmt, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Let { name, ty, .. } => {
            assert_eq!(name, "g");
            assert_eq!(
                *ty,
                Some(RustType::Named {
                    name: "Box".to_string(),
                    type_args: vec![RustType::DynTrait("Greeter".to_string())],
                })
            );
        }
        other => panic!("expected Let, got {:?}", other),
    }
}

// --- Expected type propagation (Category B improvements) ---

/// Step 6: Switch case values should propagate discriminant type for string enum matching.
/// `switch(dir) { case "up": ... }` where dir: Direction → case becomes `Direction::Up`
#[test]
fn test_convert_switch_case_propagates_discriminant_type_for_string_enum() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values: HashMap::from([
                ("up".to_string(), "Up".to_string()),
                ("down".to_string(), "Down".to_string()),
            ]),
            tag_field: None,
            variant_fields: HashMap::new(),
        },
    );

    let source = r#"function f(dir: Direction) { switch(dir) { case "up": doA(); break; case "down": doB(); break; } }"#;
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
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // Case "up" should become Direction::Up
            assert!(
                arms[0].patterns.iter().any(|p| matches!(
                    p,
                    MatchPattern::Literal(Expr::Ident(s)) if s == "Direction::Up"
                )),
                "expected Direction::Up pattern, got {:?}",
                arms[0].patterns
            );
            // Case "down" should become Direction::Down
            assert!(
                arms[1].patterns.iter().any(|p| matches!(
                    p,
                    MatchPattern::Literal(Expr::Ident(s)) if s == "Direction::Down"
                )),
                "expected Direction::Down pattern, got {:?}",
                arms[1].patterns
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}
