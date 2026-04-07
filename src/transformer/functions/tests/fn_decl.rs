use super::*;
use crate::ir::CallTarget;

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

// I-383 T8: クラスメソッドの class generic + method generic が scope に append-merge され、
// メソッド本体で両方の型パラメータを参照する anonymous union が generic 化されることを検証。
#[test]
fn test_convert_class_method_with_generic_propagates_class_and_method_scope() {
    use crate::transformer::Transformer;
    let f = TctxFixture::from_source("class C<S> { foo<T>(x: S | T): void { return; } }");
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut transformer = Transformer::for_module(&tctx, &mut synthetic);
    // Find class decl in the module and call extract_class_info
    let module = f.module();
    let class_decl = module
        .body
        .iter()
        .find_map(|item| match item {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
                swc_ecma_ast::Decl::Class(cd),
            )) => Some(cd),
            _ => None,
        })
        .expect("expected class decl");
    transformer
        .extract_class_info(class_decl, Visibility::Public)
        .unwrap();
    // anonymous union for `S | T` should be generic over both S and T (alphabetical order: S, T).
    let union_enum = synthetic
        .all_items()
        .into_iter()
        .find(|item| matches!(item, Item::Enum { name, .. } if name == "SOrT" || name == "TOrS"))
        .expect("expected synthetic union enum for `S | T`");
    match union_enum {
        Item::Enum {
            type_params, name, ..
        } => {
            let names: Vec<&str> = type_params.iter().map(|tp| tp.name.as_str()).collect();
            assert!(
                names.contains(&"S") && names.contains(&"T"),
                "anonymous union {name} should be generic over both S and T, got {names:?}"
            );
        }
        _ => panic!("unreachable"),
    }
}

// I-383 T7: generic 関数の type_params が SyntheticTypeRegistry の scope に push され、
// 関数本体内の anonymous union が generic 化されることを検証する。
//
// 期待: `function f<M>(x: M | M[]): M[]` を変換すると、`M | M[]` の anonymous union
// が `Item::Enum { name: "MOrVecM", type_params: [M], .. }` として synthetic に登録される。
// PRD-A T7 適用前は scope が push されないため type_params が空の anonymous enum が
// 生成され、後続の Step 3 (PRD-A-2) で `unknown type ref: M` を引き起こす。
#[test]
fn test_convert_fn_decl_generic_propagates_scope_to_anonymous_union() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let fn_decl = parse_fn_decl("function f<M>(x: M | M[]): void { return; }");
    let mut synthetic = SyntheticTypeRegistry::new();
    Transformer::for_module(&tctx, &mut synthetic)
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap();
    // 生成された synthetic enum を探す。命名規則: "MOrVecM" (M < Vec<M> アルファベット順)。
    // 以前は type_params が空 vec で生成されていた (silent fallback で動いていた)。
    let union_enum = synthetic
        .all_items()
        .into_iter()
        .find(|item| {
            matches!(item, Item::Enum { name, .. } if name.contains("M") && name.contains("Or"))
        })
        .expect("expected synthetic union enum for `M | M[]`");
    match union_enum {
        Item::Enum {
            type_params, name, ..
        } => {
            assert!(
                type_params.iter().any(|tp| tp.name == "M"),
                "anonymous union {name} should be generic over M, got type_params={type_params:?}"
            );
        }
        _ => panic!("unreachable"),
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
                    target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
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
