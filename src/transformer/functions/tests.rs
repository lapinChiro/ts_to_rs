use super::*;
use crate::ir::{BinOp, Expr, Item, Param, RustType, Stmt, StructField, Visibility};
use crate::parser::parse_typescript;
use crate::registry::TypeRegistry;
use swc_ecma_ast::{Decl, ModuleItem};

/// Helper: parse TS source and extract the first FnDecl.
fn parse_fn_decl(source: &str) -> ast::FnDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => fn_decl.clone(),
        _ => panic!("expected function declaration"),
    }
}

#[test]
fn test_convert_fn_decl_add() {
    let fn_decl = parse_fn_decl("function add(a: number, b: number): number { return a + b; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    assert_eq!(
        item,
        Item::Fn {
            vis: Visibility::Public,
            is_async: false,
            name: "add".to_string(),
            type_params: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: Some(RustType::F64),
                },
                Param {
                    name: "b".to_string(),
                    ty: Some(RustType::F64),
                },
            ],
            return_type: Some(RustType::F64),
            body: vec![Stmt::Return(Some(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::Ident("b".to_string())),
            }))],
        }
    );
}

#[test]
fn test_convert_fn_decl_no_return_type() {
    let fn_decl = parse_fn_decl("function greet(name: string) { return name; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn {
            name, return_type, ..
        } => {
            assert_eq!(name, "greet");
            assert_eq!(return_type, None);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_no_params() {
    let fn_decl = parse_fn_decl("function noop(): boolean { return true; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert!(params.is_empty());
            assert_eq!(body, vec![Stmt::Return(Some(Expr::BoolLit(true)))]);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_with_local_vars() {
    let fn_decl =
        parse_fn_decl("function calc(x: number): number { const result = x + 1; return result; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { body, .. } => {
            assert_eq!(body.len(), 2);
            // first statement is a let binding
            match &body[0] {
                Stmt::Let {
                    mutable,
                    name,
                    init,
                    ..
                } => {
                    assert!(!mutable);
                    assert_eq!(name, "result");
                    assert!(init.is_some());
                }
                _ => panic!("expected Stmt::Let"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_generic_single_param() {
    let fn_decl = parse_fn_decl("function identity<T>(x: T): T { return x; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { type_params, .. } => {
            assert_eq!(type_params, vec!["T".to_string()]);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_generic_multiple_params() {
    let fn_decl = parse_fn_decl("function pair<A, B>(a: A, b: B): A { return a; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { type_params, .. } => {
            assert_eq!(type_params, vec!["A".to_string(), "B".to_string()]);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_throw_wraps_return_type_in_result() {
    let fn_decl =
            parse_fn_decl("function validate(x: number): string { if (x < 0) { throw new Error(\"negative\"); } return \"ok\"; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { return_type, .. } => {
            assert_eq!(
                return_type,
                Some(RustType::Result {
                    ok: Box::new(RustType::String),
                    err: Box::new(RustType::String),
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_throw_wraps_return_in_ok() {
    let fn_decl =
            parse_fn_decl("function validate(x: number): string { if (x < 0) { throw new Error(\"negative\"); } return \"ok\"; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { body, .. } => {
            // The last statement should be return Ok("ok".to_string())
            let last = body.last().unwrap();
            assert_eq!(
                *last,
                Stmt::Return(Some(Expr::FnCall {
                    name: "Ok".to_string(),
                    args: vec![Expr::MethodCall {
                        object: Box::new(Expr::StringLit("ok".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    }],
                }))
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_throw_no_return_type_becomes_result_unit() {
    let fn_decl = parse_fn_decl("function fail() { throw new Error(\"boom\"); }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { return_type, .. } => {
            assert_eq!(
                return_type,
                Some(RustType::Result {
                    ok: Box::new(RustType::Named {
                        name: "()".to_string(),
                        type_args: vec![],
                    }),
                    err: Box::new(RustType::String),
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_missing_param_type_annotation() {
    let fn_decl = parse_fn_decl("function bad(x) { return x; }");
    let result = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false);
    assert!(result.is_err());
}

// -- async function tests --

#[test]
fn test_convert_fn_decl_async_is_async() {
    let fn_decl = parse_fn_decl("async function fetchData(): Promise<number> { return 42; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn {
            is_async,
            return_type,
            ..
        } => {
            assert!(is_async);
            // Promise<number> should unwrap to f64
            assert_eq!(return_type, Some(RustType::F64));
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_async_no_return_type() {
    let fn_decl = parse_fn_decl("async function doWork() { return; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn {
            is_async,
            return_type,
            ..
        } => {
            assert!(is_async);
            assert_eq!(return_type, None);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_sync_is_not_async() {
    let fn_decl = parse_fn_decl("function add(a: number, b: number): number { return a + b; }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { is_async, .. } => {
            assert!(!is_async);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_object_destructuring_param_generates_expansion() {
    let fn_decl = parse_fn_decl("function foo({ x, y }: Point): void { console.log(x); }");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();

    match item {
        Item::Fn { params, body, .. } => {
            // Parameter should be renamed to snake_case of the type
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "point");
            assert_eq!(
                params[0].ty,
                Some(RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
            // Body should start with expansion statements
            assert!(body.len() >= 2);
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "x".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident("point".to_string())),
                        field: "x".to_string(),
                    }),
                }
            );
            assert_eq!(
                body[1],
                Stmt::Let {
                    mutable: false,
                    name: "y".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident("point".to_string())),
                        field: "y".to_string(),
                    }),
                }
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_object_destructuring_rename() {
    let fn_decl = parse_fn_decl("function foo({ x: newX, y: newY }: Point): void {}");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();

    match item {
        Item::Fn { body, .. } => {
            assert_eq!(
                body[0],
                Stmt::Let {
                    mutable: false,
                    name: "new_x".to_string(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident("point".to_string())),
                        field: "x".to_string(),
                    }),
                }
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_destructuring_with_normal_params() {
    let fn_decl = parse_fn_decl("function foo(name: string, { x, y }: Point): void {}");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();

    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "name");
            assert_eq!(params[1].name, "point");
            // Expansion statements in body
            assert_eq!(body.len(), 2);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_destructuring_no_type_annotation_fails() {
    let fn_decl = parse_fn_decl("function foo({ x, y }): void {}");
    let result = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false);
    assert!(result.is_err());
}

#[test]
fn test_convert_fn_decl_default_number_param_wraps_in_option() {
    let fn_decl = parse_fn_decl("function foo(x: number = 0): void {}");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            // Parameter type should be Option<f64>
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
            // Body should start with `let x = x.unwrap_or(0.0);`
            assert!(
                !body.is_empty(),
                "body should contain unwrap_or expansion statement"
            );
            match &body[0] {
                Stmt::Let {
                    name,
                    init,
                    mutable,
                    ..
                } => {
                    assert_eq!(name, "x");
                    assert!(!mutable);
                    // init should be a method call: x.unwrap_or(0.0)
                    match init.as_ref().unwrap() {
                        Expr::MethodCall {
                            object,
                            method,
                            args,
                        } => {
                            assert_eq!(method, "unwrap_or");
                            assert!(matches!(object.as_ref(), Expr::Ident(n) if n == "x"));
                            assert_eq!(args.len(), 1);
                            assert!(
                                matches!(&args[0], Expr::NumberLit(n) if *n == 0.0),
                                "expected NumberLit(0.0), got {:?}",
                                &args[0]
                            );
                        }
                        other => panic!("expected MethodCall, got {other:?}"),
                    }
                }
                other => panic!("expected Let statement, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_string_param_wraps_in_option() {
    let fn_decl = parse_fn_decl("function foo(name: string = \"hello\"): void {}");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::String)))
            );
            match &body[0] {
                Stmt::Let { name, init, .. } => {
                    assert_eq!(name, "name");
                    match init.as_ref().unwrap() {
                        Expr::MethodCall { method, args, .. } => {
                            assert_eq!(method, "unwrap_or");
                            // arg should be "hello".to_string()
                            assert_eq!(args.len(), 1);
                            match &args[0] {
                                Expr::MethodCall { object, method, .. } => {
                                    assert_eq!(method, "to_string");
                                    assert!(
                                        matches!(object.as_ref(), Expr::StringLit(s) if s == "hello")
                                    );
                                }
                                other => panic!("expected MethodCall, got {other:?}"),
                            }
                        }
                        other => panic!("expected MethodCall, got {other:?}"),
                    }
                }
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_bool_param_wraps_in_option() {
    let fn_decl = parse_fn_decl("function foo(flag: boolean = true): void {}");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::Bool)))
            );
            match &body[0] {
                Stmt::Let { init, .. } => match init.as_ref().unwrap() {
                    Expr::MethodCall { method, args, .. } => {
                        assert_eq!(method, "unwrap_or");
                        assert!(matches!(&args[0], Expr::BoolLit(true)));
                    }
                    other => panic!("expected MethodCall, got {other:?}"),
                },
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_empty_object_uses_unwrap_or_default() {
    let fn_decl = parse_fn_decl("function foo(options: Config = {}): void {}");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::Named {
                    name: "Config".to_string(),
                    type_args: vec![],
                })))
            );
            match &body[0] {
                Stmt::Let { init, .. } => match init.as_ref().unwrap() {
                    Expr::MethodCall { method, args, .. } => {
                        assert_eq!(method, "unwrap_or_default");
                        assert!(args.is_empty());
                    }
                    other => panic!("expected MethodCall, got {other:?}"),
                },
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_param_mixed_with_normal() {
    let fn_decl = parse_fn_decl("function foo(a: number, b: number = 10): void {}");
    let items = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            // First param: normal
            assert_eq!(params[0].name, "a");
            assert_eq!(params[0].ty, Some(RustType::F64));
            // Second param: Option<f64>
            assert_eq!(params[1].name, "b");
            assert_eq!(
                params[1].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
            // Body should have unwrap_or expansion for b
            match &body[0] {
                Stmt::Let { name, .. } => assert_eq!(name, "b"),
                other => panic!("expected Let, got {other:?}"),
            }
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_default_unsupported_value_errors() {
    // new Map() is an unsupported default value
    let fn_decl = parse_fn_decl("function foo(m: Map = new Map()): void {}");
    let result = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false);
    assert!(result.is_err());
}

#[test]
fn test_convert_fn_decl_inline_type_literal_single_field_generates_struct() {
    let fn_decl = parse_fn_decl("function foo(opts: { x: number }): void {}");
    let (items, _) =
        convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Public,
            name: "FooOpts".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: None,
                name: "x".to_string(),
                ty: RustType::F64,
            }],
        }
    );
    match &items[1] {
        Item::Fn { params, .. } => {
            assert_eq!(
                params[0].ty,
                Some(RustType::Named {
                    name: "FooOpts".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_inline_type_literal_multiple_fields_generates_struct() {
    let fn_decl = parse_fn_decl("function bar(config: { x: number, y: string }): void {}");
    let (items, _) =
        convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Public,
            name: "BarConfig".to_string(),
            type_params: vec![],
            fields: vec![
                StructField {
                    vis: None,
                    name: "x".to_string(),
                    ty: RustType::F64,
                },
                StructField {
                    vis: None,
                    name: "y".to_string(),
                    ty: RustType::String,
                },
            ],
        }
    );
}

#[test]
fn test_convert_fn_decl_inline_type_literal_mixed_with_normal_param_generates_struct() {
    let fn_decl = parse_fn_decl("function baz(name: string, opts: { x: number }): void {}");
    let (items, _) =
        convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false).unwrap();
    assert_eq!(items.len(), 2);
    match &items[0] {
        Item::Struct { name, .. } => assert_eq!(name, "BazOpts"),
        _ => panic!("expected Item::Struct"),
    }
    match &items[1] {
        Item::Fn { params, .. } => {
            assert_eq!(params[0].ty, Some(RustType::String));
            assert_eq!(
                params[1].ty,
                Some(RustType::Named {
                    name: "BazOpts".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_inline_type_literal_empty_generates_empty_struct() {
    let fn_decl = parse_fn_decl("function qux(opts: {}): void {}");
    let (items, _) =
        convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Public,
            name: "QuxOpts".to_string(),
            type_params: vec![],
            fields: vec![],
        }
    );
}

#[test]
fn test_convert_fn_decl_default_param_inline_type_generates_struct() {
    let decl = parse_fn_decl("function f(x: { a: string } = {}): void { }");
    let (items, _warnings) =
        convert_fn_decl(&decl, Visibility::Public, &TypeRegistry::new(), false).unwrap();
    // Should produce: struct + function
    let has_struct = items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "FX"));
    assert!(
        has_struct,
        "inline type literal in default param should generate struct, got: {items:?}"
    );
}
