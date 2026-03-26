use super::*;

#[test]
fn test_convert_fn_decl_add() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function add(a: number, b: number): number { return a + b; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    assert_eq!(
        item,
        Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
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
            body: vec![Stmt::TailExpr(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::Ident("b".to_string())),
            })],
        }
    );
}

#[test]
fn test_convert_fn_decl_no_return_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function greet(name: string) { return name; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
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
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function noop(): boolean { return true; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { params, body, .. } => {
            assert!(params.is_empty());
            assert_eq!(body, vec![Stmt::TailExpr(Expr::BoolLit(true))]);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_with_local_vars() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl =
        parse_fn_decl("function calc(x: number): number { const result = x + 1; return result; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
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
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function identity<T>(x: T): T { return x; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { type_params, .. } => {
            assert_eq!(
                type_params,
                vec![TypeParam {
                    name: "T".to_string(),
                    constraint: None
                }]
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_generic_multiple_params() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function pair<A, B>(a: A, b: B): A { return a; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { type_params, .. } => {
            assert_eq!(
                type_params,
                vec![
                    TypeParam {
                        name: "A".to_string(),
                        constraint: None
                    },
                    TypeParam {
                        name: "B".to_string(),
                        constraint: None
                    },
                ]
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_throw_wraps_return_type_in_result() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl =
            parse_fn_decl("function validate(x: number): string { if (x < 0) { throw new Error(\"negative\"); } return \"ok\"; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
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
    let source = "function validate(x: number): string { if (x < 0) { throw new Error(\"negative\"); } return \"ok\"; }";
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => fn_decl.clone(),
        _ => panic!("expected function declaration"),
    };
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { body, .. } => {
            // The last statement should be return Ok("ok".to_string())
            let last = body.last().unwrap();
            assert_eq!(
                *last,
                Stmt::TailExpr(Expr::FnCall {
                    name: "Ok".to_string(),
                    args: vec![Expr::MethodCall {
                        object: Box::new(Expr::StringLit("ok".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    }],
                })
            );
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_throw_no_return_type_becomes_result_unit() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function fail() { throw new Error(\"boom\"); }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
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
fn test_convert_fn_decl_missing_param_type_annotation_falls_back_to_any() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function bad(x) { return x; }");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_fn_decl(
        &fn_decl,
        Visibility::Public,
        false,
    );
    assert!(result.is_ok(), "should fall back to Any, not error");
    let items = result.unwrap().0;
    match items.last().unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(params[0].ty, Some(RustType::Any));
        }
        _ => panic!("expected Item::Fn"),
    }
}

// -- async function tests --

#[test]
fn test_convert_fn_decl_async_is_async() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("async function fetchData(): Promise<number> { return 42; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
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
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("async function doWork() { return; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
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
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function add(a: number, b: number): number { return a + b; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
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
fn test_convert_fn_decl_async_main_has_tokio_main_attribute() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("async function main(): Promise<void> { }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn {
            attributes,
            is_async,
            name,
            ..
        } => {
            assert!(is_async);
            assert_eq!(name, "main");
            assert_eq!(attributes, vec!["tokio::main".to_string()]);
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_async_non_main_has_no_attributes() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("async function fetchData(): Promise<number> { return 42; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { attributes, .. } => {
            assert!(attributes.is_empty());
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_convert_fn_decl_sync_main_has_no_attributes() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function main(): void { }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap().clone();
    match item {
        Item::Fn { attributes, .. } => {
            assert!(attributes.is_empty());
        }
        _ => panic!("expected Item::Fn"),
    }
}
