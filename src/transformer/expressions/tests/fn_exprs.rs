use super::*;

#[test]
fn test_convert_expr_fn_expr_anonymous_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // function(x: number): number { return x + 1; } → Closure
    let swc_expr = parse_var_init("const f = function(x: number): number { return x + 1; };");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, Some(crate::ir::RustType::F64));
            assert_eq!(return_type, Some(crate::ir::RustType::F64));
            assert!(matches!(body, crate::ir::ClosureBody::Block(_)));
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_named_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // function foo(x: number) { return x; } → Closure (name ignored)
    let swc_expr = parse_var_init("const f = function foo(x: number): number { return x; };");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_no_params_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = function(): void {};");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert!(params.is_empty());
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_object_destructuring_param() {
    // const f = function({ x, y }: Point) { return x; }; → Closure with 1 param of type Point
    let reg = {
        let mut r = TypeRegistry::new();
        r.register(
            "Point".to_string(),
            TypeDef::new_struct(
                vec![
                    ("x".to_string(), crate::ir::RustType::F64).into(),
                    ("y".to_string(), crate::ir::RustType::F64).into(),
                ],
                std::collections::HashMap::new(),
                vec![],
            ),
        );
        r
    };
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = function({ x, y }: Point) { return x; };");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
            // Body should be a Block with expansion stmts
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    assert!(
                        stmts.len() >= 2,
                        "expected at least 2 stmts, got {}",
                        stmts.len()
                    );
                }
                _ => panic!("expected Block body with expansion stmts"),
            }
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_default_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const f = function(x: number = 0) { return x; }; → Closure with Option<f64> param
    let swc_expr = parse_var_init("const f = function(x: number = 0) { return x; };");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Option(Box::new(
                    crate::ir::RustType::F64
                )))
            );
            // Body should be Block with unwrap_or expansion + return
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    assert!(
                        stmts.len() >= 2,
                        "expected at least 2 stmts, got {}",
                        stmts.len()
                    );
                }
                _ => panic!("expected Block body with default expansion"),
            }
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_rest_param_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const f = function(...args: number[]): void {};
    let swc_expr = parse_var_init("const f = function(...args: number[]): void {};");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Vec(Box::new(crate::ir::RustType::F64)))
            );
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}
