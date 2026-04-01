use super::*;

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
fn test_convert_stmt_list_object_destructuring_let_initially_immutable() {
    // `let` destructuring starts immutable; `mark_mutated_vars` adds mut when needed.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { let { x, y } = obj; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: false, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: false, name, .. } if name == "y"));
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
            init: Some(build_safe_index_expr(
                Expr::Ident("arr".to_string()),
                convert_index_to_usize(Expr::NumberLit(0.0)),
            )),
        }
    );
    assert_eq!(
        result[1],
        Stmt::Let {
            mutable: false,
            name: "b".to_string(),
            ty: None,
            init: Some(build_safe_index_expr(
                Expr::Ident("arr".to_string()),
                convert_index_to_usize(Expr::NumberLit(1.0)),
            )),
        }
    );
}

#[test]
fn test_convert_stmt_list_array_destructuring_let_initially_immutable() {
    // `let` destructuring starts immutable; `mark_mutated_vars` adds mut when needed.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(arr: number[]) { let [x, y] = arr; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: false, name, .. } if name == "x"));
    assert!(matches!(&result[1], Stmt::Let { mutable: false, name, .. } if name == "y"));
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
            init: Some(build_safe_index_expr(
                Expr::Ident("arr".to_string()),
                convert_index_to_usize(Expr::NumberLit(0.0)),
            )),
        }
    );
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
    // Verify correct indices: a = arr.get(0).cloned(), b = arr.get(2).cloned()
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "a".to_string(),
            ty: None,
            init: Some(build_safe_index_expr(
                Expr::Ident("arr".to_string()),
                convert_index_to_usize(Expr::NumberLit(0.0)),
            )),
        }
    );
    assert_eq!(
        result[1],
        Stmt::Let {
            mutable: false,
            name: "b".to_string(),
            ty: None,
            init: Some(build_safe_index_expr(
                Expr::Ident("arr".to_string()),
                convert_index_to_usize(Expr::NumberLit(2.0)),
            )),
        }
    );
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
    // first = arr.get(0).cloned() (safe indexing)
    assert_eq!(
        result[0],
        Stmt::Let {
            mutable: false,
            name: "first".to_string(),
            ty: None,
            init: Some(build_safe_index_expr(
                Expr::Ident("arr".to_string()),
                convert_index_to_usize(Expr::NumberLit(0.0)),
            )),
        }
    );
    // rest = arr[1..].to_vec() (Range index — unchanged)
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "rest"));
    if let Stmt::Let {
        init: Some(Expr::MethodCall { object, method, .. }),
        ..
    } = &result[1]
    {
        assert_eq!(method, "to_vec");
        assert!(
            matches!(object.as_ref(), Expr::Index { index, .. }
                if matches!(index.as_ref(), Expr::Range { .. })),
            "rest element should use Range index, got: {:?}",
            object
        );
    } else {
        panic!("expected MethodCall with to_vec for rest element");
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
