use super::*;
use crate::ir::{Expr, RustType, Stmt};
use crate::parser::parse_typescript;
use crate::registry::TypeRegistry;
use swc_ecma_ast::{Decl, ModuleItem};

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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
}

#[test]
fn test_convert_stmt_return_no_value() {
    let stmts = parse_fn_body("function f() { return; }");
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(result, Stmt::Return(None));
}

#[test]
fn test_convert_stmt_const_decl() {
    let stmts = parse_fn_body("function f() { const x = 1; }");
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Box::new(Expr::NumberLit(0.0)),
                end: Box::new(Expr::Ident("n".to_string())),
            },
            body: vec![Stmt::Expr(Expr::Ident("i".to_string()))],
        }
    );
}

#[test]
fn test_convert_stmt_for_counter_start_to_literal() {
    let stmts = parse_fn_body("function f() { for (let i = 1; i < 10; i++) { i; } }");
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Box::new(Expr::NumberLit(1.0)),
                end: Box::new(Expr::NumberLit(10.0)),
            },
            body: vec![Stmt::Expr(Expr::Ident("i".to_string()))],
        }
    );
}

#[test]
fn test_convert_stmt_for_of() {
    let stmts = parse_fn_body("function f() { for (const item of items) { item; } }");
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(
        result,
        Stmt::While {
            label: None,
            condition: Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: ">".to_string(),
                right: Box::new(Expr::NumberLit(0.0)),
            },
            body: vec![Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident("x".to_string())),
                value: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: "-".to_string(),
                    right: Box::new(Expr::NumberLit(1.0)),
                }),
            })],
        }
    );
}

#[test]
fn test_convert_stmt_list_try_catch_expands_try_body() {
    let stmts =
        parse_fn_body("function f() { try { const x = 1; return x; } catch (e) { return 0; } }");
    // try/catch is expanded: try body is inlined, catch is dropped
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(1.0)),
        }
    );
    assert_eq!(result[1], Stmt::Return(Some(Expr::Ident("x".to_string()))));
}

#[test]
fn test_convert_stmt_list_try_catch_empty_catch() {
    let stmts = parse_fn_body("function f() { try { const x = 1; } catch (e) { } }");
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(1.0)),
        }
    );
}

#[test]
fn test_convert_stmt_throw_new_error_string() {
    let stmts = parse_fn_body("function f() { throw new Error(\"something went wrong\"); }");
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let stmts = parse_fn_body("function f() { const p: Point = { x: 1, y: 2 }; }");
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(result, Stmt::Expr(Expr::Ident("foo".to_string())));
}

// -- Expected type propagation tests --

#[test]
fn test_convert_stmt_var_decl_string_type_annotation_adds_to_string() {
    let stmts = parse_fn_body(r#"function f() { const s: string = "hello"; }"#);
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let stmts = parse_fn_body(r#"function f() { const a: string[] = ["a", "b"]; }"#);
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), Some(&RustType::String)).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), Some(&RustType::F64)).unwrap();
    assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
}

// -- break / continue tests --

#[test]
fn test_convert_stmt_break_no_label() {
    let stmts = parse_fn_body("function f() { while (true) { break; } }");
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
    assert!(matches!(&result[1], Stmt::Loop { .. }));
}

#[test]
fn test_convert_stmt_for_simple_counter_unchanged() {
    // Existing simple counter pattern should still produce ForIn
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { console.log(i); } }");
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], Stmt::ForIn { .. }));
}

// -- Object destructuring tests --

#[test]
fn test_convert_stmt_list_object_destructuring_basic() {
    let stmts = parse_fn_body("function f() { const { x, y } = obj; }");
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "y"));
}

#[test]
fn test_convert_stmt_list_object_destructuring_rename() {
    let stmts = parse_fn_body("function f() { const { x: newX } = obj; }");
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "y"));
}

#[test]
fn test_convert_stmt_list_array_destructuring_single_element() {
    let stmts = parse_fn_body("function f(arr: number[]) { const [a] = arr; }");
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
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
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
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
                    assert!(matches!(condition, Expr::UnaryOp { op, .. } if op == "!"));
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
    let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
    assert_eq!(result.len(), 3);
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "b"));
    assert!(matches!(&result[2], Stmt::Let { name, .. } if name == "c"));
}
