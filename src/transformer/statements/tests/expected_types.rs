use super::*;

// -- Expected type propagation tests --

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
