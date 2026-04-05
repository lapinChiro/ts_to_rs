use super::*;

#[test]
fn test_convert_expr_arrow_expr_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `(x: number) => x + 1`
    let swc_expr = parse_var_init("const f = (x: number) => x + 1;");
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
            assert!(return_type.is_none());
            assert!(matches!(body, crate::ir::ClosureBody::Expr(_)));
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_block_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `(x: number): number => { return x + 1; }`
    let swc_expr = parse_var_init("const f = (x: number): number => { return x + 1; };");
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
            assert!(return_type.is_some());
            assert_eq!(return_type.unwrap(), crate::ir::RustType::F64);
            assert!(matches!(body, crate::ir::ClosureBody::Block(_)));
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_no_params() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = () => 42;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert!(params.is_empty());
            assert!(matches!(body, crate::ir::ClosureBody::Expr(_)));
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_no_type_annotation_param_ty_is_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = (x) => x + 1;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, None);
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_mixed_type_annotations() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Only first param has type annotation
    let swc_expr = parse_var_init("const f = (x: number, y) => x + y;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].ty, Some(crate::ir::RustType::F64));
            assert_eq!(params[1].ty, None);
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_object_destructuring_generates_expansion() {
    // ({ x, y }: Point) => x + y → closure with synthetic param + expansion stmts
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
    let swc_expr = parse_var_init("const f = ({ x, y }: Point) => x + y;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            // Should have a synthetic parameter named after the type
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "point");
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
            // Body should be a Block with expansion stmts + the expression
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    // At least 2 expansion stmts (let x = point.x; let y = point.y;) + return
                    assert!(
                        stmts.len() >= 3,
                        "expected at least 3 stmts, got {}",
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
fn test_convert_expr_arrow_array_destructuring_param_generates_tuple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ([k, v]: [string, number]) => ... → closure with (k, v) tuple param
    let swc_expr = parse_var_init("const f = ([k, v]: [string, number]) => k;");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&swc_expr);
    assert!(
        result.is_ok(),
        "array destructuring with type should not error: {:?}",
        result.err()
    );
    if let Ok(Expr::Closure { params, .. }) = &result {
        assert_eq!(params.len(), 1, "should have 1 tuple param");
        assert_eq!(params[0].name, "(k, v)");
    } else {
        panic!("expected Closure, got: {:?}", result);
    }
}

#[test]
fn test_convert_expr_arrow_array_destructuring_no_type_generates_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ([a, b]) => ... → should not crash (fallback to untyped)
    let swc_expr = parse_var_init("const f = ([a, b]) => a;");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&swc_expr);
    assert!(
        result.is_ok(),
        "array destructuring without type should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_expr_arrow_object_destructuring_no_type_generates_value_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ({ x, y }) => ... → should not crash (fallback to serde_json::Value)
    let swc_expr = parse_var_init("const f = ({ x, y }) => x;");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&swc_expr);
    assert!(
        result.is_ok(),
        "object destructuring without type should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_expr_arrow_default_param_generates_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (x: number = 0) => x + 1 → closure with Option<f64> param + unwrap_or
    let swc_expr = parse_var_init("const f = (x: number = 0) => x + 1;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            // Should be Option<f64>
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Option(Box::new(
                    crate::ir::RustType::F64
                )))
            );
            // Body should be Block with unwrap_or expansion + expression
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
fn test_convert_expr_arrow_rest_param_generates_vec() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (...args: number[]) => args → rest param becomes Vec<f64>
    let swc_expr = parse_var_init("const f = (...args: number[]) => args;");
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

#[test]
fn test_convert_expr_arrow_rest_param_no_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const f = (...args) => args; → rest param with no type annotation
    let swc_expr = parse_var_init("const f = (...args) => args;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
            assert_eq!(
                params[0].ty, None,
                "rest param without type annotation should have ty=None"
            );
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_call_expr_arrow_iife_generates_closure_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (() => 42)() — arrow IIFE should produce a closure call
    let expr = parse_expr("(() => 42)();");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&expr);
    assert!(
        result.is_ok(),
        "arrow IIFE should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_call_expr_arrow_iife_with_args_generates_closure_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ((x: number) => x + 1)(5) — arrow IIFE with args
    let expr = parse_expr("((x: number): number => x + 1)(5);");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&expr);
    assert!(
        result.is_ok(),
        "arrow IIFE with args should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_expr_arrow_default_param_no_annotation_infers_f64() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (x = 42) => x — no type annotation, infer f64 from number literal
    let swc_expr = parse_var_init("const f = (x = 42) => x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    assert!(stmts.len() >= 2, "expected unwrap_or expansion + body");
                    // First stmt should be `let x = x.unwrap_or(42.0);`
                    match &stmts[0] {
                        crate::ir::Stmt::Let { name, init, .. } => {
                            assert_eq!(name, "x");
                            match init.as_ref().unwrap() {
                                Expr::MethodCall { method, args, .. } => {
                                    assert_eq!(method, "unwrap_or");
                                    assert!(matches!(args[0], Expr::NumberLit(v) if v == 42.0));
                                }
                                other => panic!("expected unwrap_or call, got {:?}", other),
                            }
                        }
                        other => panic!("expected Let stmt, got {:?}", other),
                    }
                }
                _ => panic!("expected Block body"),
            }
        }
        _ => panic!("expected Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_default_param_no_annotation_infers_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (s = "hi") => s — no type annotation, infer String from string literal
    let swc_expr = parse_var_init(r#"const f = (s = "hi") => s;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params[0].name, "s");
            assert_eq!(
                params[0].ty,
                Some(RustType::Option(Box::new(RustType::String)))
            );
        }
        _ => panic!("expected Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_default_param_object_destructuring_with_empty_default() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ({ x }: { x?: number } = {}) => x — object destructuring + default
    let swc_expr = parse_var_init("const f = ({ x }: { x?: number } = {}) => x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            // Param type should be Option<...> (wrapped)
            match &params[0].ty {
                Some(RustType::Option(_)) => {} // good
                other => panic!("expected Option<...> param type, got {:?}", other),
            }
            // Body should contain unwrap_or_default expansion + field expansion
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    assert!(
                        stmts.len() >= 2,
                        "expected unwrap_or_default + field expansion, got {} stmts",
                        stmts.len()
                    );
                    // First stmt: unwrap_or_default
                    match &stmts[0] {
                        crate::ir::Stmt::Let { init, .. } => match init.as_ref().unwrap() {
                            Expr::MethodCall { method, .. } => {
                                assert_eq!(method, "unwrap_or_default");
                            }
                            other => panic!("expected unwrap_or_default call, got {:?}", other),
                        },
                        other => panic!("expected Let stmt, got {:?}", other),
                    }
                }
                _ => panic!("expected Block body"),
            }
        }
        _ => panic!("expected Closure"),
    }
}

#[test]
fn test_narrowing_guard_typeof_captures_var_span() {
    // typeof y === "string" — y's span should be captured
    let module = parse_typescript(r#"typeof y === "string";"#).unwrap();
    let expr = match &module.body[0] {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let guard = patterns::extract_narrowing_guard(expr).expect("should extract guard");
    match &guard {
        patterns::NarrowingGuard::Typeof {
            var_name, var_span, ..
        } => {
            assert_eq!(var_name, "y");
            // var_span should not be a dummy span (lo > 0 for real AST nodes)
            assert!(var_span.lo.0 > 0, "var_span should be a real span from AST");
        }
        _ => panic!("expected Typeof guard, got: {:?}", guard),
    }
}

#[test]
fn test_narrowing_guard_nonnullish_captures_var_span() {
    let module = parse_typescript("x !== null;").unwrap();
    let expr = match &module.body[0] {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let guard = patterns::extract_narrowing_guard(expr).expect("should extract guard");
    match &guard {
        patterns::NarrowingGuard::NonNullish {
            var_name, var_span, ..
        } => {
            assert_eq!(var_name, "x");
            assert!(var_span.lo.0 > 0, "var_span should be a real span from AST");
        }
        _ => panic!("expected NonNullish guard, got: {:?}", guard),
    }
}

#[test]
fn test_narrowing_guard_instanceof_captures_var_span() {
    let module = parse_typescript("x instanceof Error;").unwrap();
    let expr = match &module.body[0] {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let guard = patterns::extract_narrowing_guard(expr).expect("should extract guard");
    match &guard {
        patterns::NarrowingGuard::InstanceOf {
            var_name, var_span, ..
        } => {
            assert_eq!(var_name, "x");
            assert!(var_span.lo.0 > 0, "var_span should be a real span from AST");
        }
        _ => panic!("expected InstanceOf guard, got: {:?}", guard),
    }
}

#[test]
fn test_convert_arrow_optional_param_wraps_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = (x: number, y?: string) => x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].ty, Some(crate::ir::RustType::F64));
            assert_eq!(
                params[1].ty,
                Some(crate::ir::RustType::Option(Box::new(
                    crate::ir::RustType::String
                )))
            );
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_narrowing_guard_truthy_captures_var_span() {
    let module = parse_typescript("x;").unwrap();
    let expr = match &module.body[0] {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let guard = patterns::extract_narrowing_guard(expr).expect("should extract guard");
    match &guard {
        patterns::NarrowingGuard::Truthy {
            var_name, var_span, ..
        } => {
            assert_eq!(var_name, "x");
            assert!(var_span.lo.0 > 0, "var_span should be a real span from AST");
        }
        _ => panic!("expected Truthy guard, got: {:?}", guard),
    }
}
