use super::*;
use crate::ir::{BinOp, Expr, RustType, Stmt, UnOp};
use crate::parser::parse_typescript;
use crate::registry::TypeRegistry;
use crate::transformer::TypeEnv;
use swc_ecma_ast::{Decl, ModuleItem};

/// Helper: convert a single statement and assert exactly one IR statement is produced.
fn convert_single_stmt(
    stmt: &ast::Stmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Stmt {
    let mut stmts = convert_stmt(stmt, reg, return_type, &mut TypeEnv::new()).unwrap();
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
            body: vec![Stmt::Expr(Expr::Ident("i".to_string()))],
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
            body: vec![Stmt::Expr(Expr::Ident("i".to_string()))],
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
fn test_convert_stmt_list_try_catch_generates_try_catch_ir() {
    let stmts =
        parse_fn_body("function f() { try { const x = 1; return x; } catch (e) { return 0; } }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::TryCatch {
            try_body,
            catch_param,
            catch_body,
            finally_body,
        } => {
            assert_eq!(try_body.len(), 2);
            assert_eq!(catch_param, &Some("e".to_string()));
            assert!(catch_body.is_some());
            assert_eq!(catch_body.as_ref().unwrap().len(), 1);
            assert!(finally_body.is_none());
        }
        _ => panic!("expected Stmt::TryCatch, got {:?}", result[0]),
    }
}

#[test]
fn test_convert_stmt_list_try_catch_empty_catch_generates_try_catch_ir() {
    let stmts = parse_fn_body("function f() { try { const x = 1; } catch (e) { } }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::TryCatch {
            try_body,
            catch_param,
            catch_body,
            finally_body,
        } => {
            assert_eq!(try_body.len(), 1);
            assert_eq!(catch_param, &Some("e".to_string()));
            assert!(catch_body.is_some());
            assert_eq!(catch_body.as_ref().unwrap().len(), 0);
            assert!(finally_body.is_none());
        }
        _ => panic!("expected Stmt::TryCatch, got {:?}", result[0]),
    }
}

#[test]
fn test_convert_stmt_list_try_finally_generates_try_catch_ir() {
    let stmts = parse_fn_body("function f() { try { const x = 1; } finally { const y = 2; } }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::TryCatch {
            try_body,
            catch_param,
            catch_body,
            finally_body,
        } => {
            assert_eq!(try_body.len(), 1);
            assert!(catch_param.is_none());
            assert!(catch_body.is_none());
            assert!(finally_body.is_some());
            assert_eq!(finally_body.as_ref().unwrap().len(), 1);
        }
        _ => panic!("expected Stmt::TryCatch, got {:?}", result[0]),
    }
}

#[test]
fn test_convert_stmt_list_try_catch_finally_generates_try_catch_ir() {
    let stmts = parse_fn_body(
        "function f() { try { const x = 1; } catch (e) { const y = 2; } finally { const z = 3; } }",
    );
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::TryCatch {
            try_body,
            catch_param,
            catch_body,
            finally_body,
        } => {
            assert_eq!(try_body.len(), 1);
            assert_eq!(catch_param, &Some("e".to_string()));
            assert!(catch_body.is_some());
            assert!(finally_body.is_some());
        }
        _ => panic!("expected Stmt::TryCatch, got {:?}", result[0]),
    }
}

#[test]
fn test_convert_stmt_nested_try_catch_generates_nested_try_catch_ir() {
    let stmts = parse_fn_body(
        "function f() { try { try { x(); } catch (inner) { y(); } } catch (outer) { z(); } }",
    );
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::TryCatch {
            try_body,
            catch_param,
            ..
        } => {
            assert_eq!(catch_param, &Some("outer".to_string()));
            // The inner try/catch should be inside try_body
            assert_eq!(try_body.len(), 1);
            match &try_body[0] {
                Stmt::TryCatch {
                    catch_param: inner_param,
                    ..
                } => {
                    assert_eq!(inner_param, &Some("inner".to_string()));
                }
                _ => panic!("expected inner Stmt::TryCatch, got {:?}", try_body[0]),
            }
        }
        _ => panic!("expected Stmt::TryCatch, got {:?}", result[0]),
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

// -- Object literal in variable declaration tests --

#[test]
fn test_convert_stmt_var_decl_object_literal_with_type_annotation() {
    // const + Named type → let mut (TS const allows field mutation)
    let stmts = parse_fn_body("function f() { const p: Point = { x: 1, y: 2 }; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
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
    let stmts = parse_fn_body(r#"function f() { const s: string = "hello"; }"#);
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
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
    let stmts = parse_fn_body(r#"function f() { const a: string[] = ["a", "b"]; }"#);
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
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
    let stmts = parse_fn_body(r#"function f(): string { return "ok"; }"#);
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), Some(&RustType::String));
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
            assert_eq!(body[0], Stmt::Break { label: None });
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
                    label: Some("outer".to_string())
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
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = n; i >= 0; i--) { console.log(i); } }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    // Should produce: let mut i = n; loop { if !(i >= 0) { break; } body; i--; }
    assert_eq!(result.len(), 2); // init + loop
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
    assert!(matches!(&result[1], Stmt::Loop { .. }));
}

#[test]
fn test_convert_stmt_list_for_step_by_two_becomes_loop() {
    let stmts = parse_fn_body(
        "function f(n: number) { for (let i = 0; i < n; i += 2) { console.log(i); } }",
    );
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
    assert!(matches!(&result[1], Stmt::Loop { .. }));
}

#[test]
fn test_convert_stmt_for_simple_counter_unchanged() {
    // Existing simple counter pattern should still produce ForIn
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { console.log(i); } }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], Stmt::ForIn { .. }));
}

// -- Object destructuring tests --

#[test]
fn test_convert_stmt_list_object_destructuring_basic() {
    let stmts = parse_fn_body("function f() { const { x, y } = obj; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
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
    let stmts = parse_fn_body("function f() { let { x, y } = obj; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "y"));
}

#[test]
fn test_convert_stmt_list_object_destructuring_rename() {
    let stmts = parse_fn_body("function f() { const { x: newX } = obj; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
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
    let stmts = parse_fn_body("function f(arr: number[]) { const [a, b] = arr; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
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
    let stmts = parse_fn_body("function f(arr: number[]) { let [x, y] = arr; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "y"));
}

#[test]
fn test_convert_stmt_list_array_destructuring_single_element() {
    let stmts = parse_fn_body("function f(arr: number[]) { const [a] = arr; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
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
    let stmts = parse_fn_body("function f(x: number) { do { x = x + 1; } while (x < 10); }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
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
                    assert!(matches!(&then_body[0], Stmt::Break { label: None }));
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
    let stmts = parse_fn_body("function f(arr: number[]) { const [a, b, c] = arr; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 3);
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "b"));
    assert!(matches!(&result[2], Stmt::Let { name, .. } if name == "c"));
}

#[test]
fn test_convert_stmt_list_array_destructuring_skip_element() {
    let stmts = parse_fn_body("function f(arr: number[]) { const [a, , b] = arr; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
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
    let stmts = parse_fn_body("function f(arr: number[]) { const [first, ...rest] = arr; }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "first"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "rest"));
}

#[test]
fn test_convert_stmt_nested_fn_decl_generates_closure_let() {
    let stmts =
        parse_fn_body("function outer() { function inner(x: number): number { return x; } }");
    let result =
        convert_stmt_list(&stmts, &TypeRegistry::new(), None, &mut TypeEnv::new()).unwrap();
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

#[test]
fn test_type_env_stmt_list_registers_let_binding_type() {
    let source = "const x: number = 1;";
    let module = parse_typescript(source).expect("parse failed");
    let stmts: Vec<&ast::Stmt> = module
        .body
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Stmt(s) => Some(s),
            _ => None,
        })
        .collect();
    let stmts_ref: Vec<ast::Stmt> = stmts.into_iter().cloned().collect();

    let mut type_env = TypeEnv::new();
    let _result = convert_stmt_list(&stmts_ref, &TypeRegistry::new(), None, &mut type_env).unwrap();

    assert_eq!(
        type_env.get("x"),
        Some(&RustType::F64),
        "convert_stmt_list should register Let binding types in TypeEnv"
    );
}
