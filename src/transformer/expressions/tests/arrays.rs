use super::*;
use crate::ir::Stmt as IrStmt;

#[test]
fn test_convert_expr_array_numbers() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [1, 2, 3];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::NumberLit(1.0),
                Expr::NumberLit(2.0),
                Expr::NumberLit(3.0),
            ],
        }
    );
}

#[test]
fn test_convert_expr_array_strings() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init(r#"const a = ["x", "y"];"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::StringLit("x".to_string()),
                Expr::StringLit("y".to_string()),
            ],
        }
    );
}

#[test]
fn test_convert_expr_array_empty() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::Vec { elements: vec![] });
}

#[test]
fn test_convert_expr_array_single_element() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [42];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![Expr::NumberLit(42.0)],
        }
    );
}

#[test]
fn test_convert_expr_array_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [[1, 2], [3]];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::Vec {
                    elements: vec![Expr::NumberLit(1.0), Expr::NumberLit(2.0)],
                },
                Expr::Vec {
                    elements: vec![Expr::NumberLit(3.0)],
                },
            ],
        }
    );
}

#[test]
fn test_convert_expr_array_nested_vec_string_expected() {
    let f = TctxFixture::from_source(r#"const a: string[][] = [["a"]];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![Expr::Vec {
                elements: vec![Expr::MethodCall {
                    object: Box::new(Expr::StringLit("a".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }],
            }],
        }
    );
}

#[test]
fn test_convert_expr_array_map_to_iter_map_collect() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.map((x: number) => x + 1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // arr.map((x: number) => x + 1) → arr.iter().cloned().map(|x| x + 1).collect()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("arr".to_string())),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "x".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("x".to_string())),
                        op: BinOp::Add,
                        right: Box::new(Expr::NumberLit(1.0)),
                    })),
                }],
            }),
            method: "collect::<Vec<_>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_array_filter_to_iter_filter_collect() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.filter((x: number) => x > 0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // arr.filter((x: number) => x > 0) → arr.iter().cloned().filter(|x| x > 0).collect()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("arr".to_string())),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }),
                method: "filter".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "x".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("x".to_string())),
                        op: BinOp::Gt,
                        right: Box::new(Expr::NumberLit(0.0)),
                    })),
                }],
            }),
            method: "collect::<Vec<_>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_array_find_to_iter_find() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.find((x: number) => x > 0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // arr.find((x: number) => x > 0) → arr.iter().cloned().find(|x| x > 0)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "find".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Gt,
                    right: Box::new(Expr::NumberLit(0.0)),
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_array_some_to_iter_any() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.some((x: number) => x > 0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // arr.some((x: number) => x > 0) → arr.iter().cloned().any(|x| x > 0)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "any".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Gt,
                    right: Box::new(Expr::NumberLit(0.0)),
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_array_every_to_iter_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.every((x: number) => x > 0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // arr.every((x: number) => x > 0) → arr.iter().cloned().all(|x| x > 0)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "all".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Gt,
                    right: Box::new(Expr::NumberLit(0.0)),
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_array_foreach_to_for_loop() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // forEach は式→文の変換なので、statement レベルで別途テストする
    // ここではメソッド呼び出しとしての変換を確認
    let expr = parse_expr("arr.forEach((x: number) => console.log(x));");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // forEach は map_method_call で ForEach 用の IR に変換される
    // 初版: arr.iter().cloned().for_each(|x| ...) に変換
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "for_each".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::MacroCall {
                    name: "println".to_string(),
                    args: vec![Expr::Ident("x".to_string())],
                    use_debug: vec![false],
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_slice_to_range_to_vec() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.slice(1, 3) → arr[1..3].to_vec()
    let expr = parse_expr("arr.slice(1, 3);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::Ident("arr".to_string())),
                index: Box::new(Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: Some(Box::new(Expr::NumberLit(3.0))),
                }),
            }),
            method: "to_vec".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_splice_to_drain_collect() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.splice(1, 2) → arr.drain(1..3).collect::<Vec<_>>()
    let expr = parse_expr("arr.splice(1, 2);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("arr".to_string())),
                method: "drain".to_string(),
                args: vec![Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: Some(Box::new(Expr::NumberLit(3.0))),
                }],
            }),
            method: "collect::<Vec<_>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_reverse_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.reverse() → arr.reverse() (same name, in-place)
    let expr = parse_expr("arr.reverse();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "reverse".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_sort_no_args_generates_sort_by_partial_cmp() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort() → arr.sort_by(|a, b| a.partial_cmp(b).unwrap())
    let expr = parse_expr("arr.sort();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "sort_by".to_string(),
            args: vec![Expr::Closure {
                params: vec![
                    Param {
                        name: "a".to_string(),
                        ty: None,
                    },
                    Param {
                        name: "b".to_string(),
                        ty: None,
                    },
                ],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("a".to_string())),
                        method: "partial_cmp".to_string(),
                        args: vec![Expr::Ident("b".to_string())],
                    }),
                    method: "unwrap".to_string(),
                    args: vec![],
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_sort_with_comparator_to_sort_by() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort((a, b) => a - b) → arr.sort_by(|a, b| (a - b).partial_cmp(&0.0).unwrap())
    let expr = parse_expr("arr.sort((a, b) => a - b);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "sort_by");
        if let Some(Expr::Closure { params, body, .. }) = args.first() {
            assert_eq!(params.len(), 2);
            assert!(params[0].ty.is_none());
            // Body should be (a - b).partial_cmp(&0.0).unwrap()
            if let ClosureBody::Expr(body_expr) = body {
                assert!(
                    matches!(body_expr.as_ref(), Expr::MethodCall { method, .. } if method == "unwrap"),
                    "expected .unwrap() at top level, got: {body_expr:?}"
                );
                return;
            }
        }
    }
    panic!("expected sort_by with partial_cmp closure, got: {result:?}");
}

#[test]
fn test_convert_expr_index_of_to_iter_position() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.indexOf(x) → arr.iter().position(|item| *item == x).map(|i| i as f64).unwrap_or(-1.0)
    let expr = parse_expr("arr.indexOf(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("arr".to_string())),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "position".to_string(),
                    args: vec![Expr::Closure {
                        params: vec![Param {
                            name: "item".to_string(),
                            ty: None,
                        }],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                            left: Box::new(Expr::Deref(Box::new(Expr::Ident("item".to_string(),)))),
                            op: BinOp::Eq,
                            right: Box::new(Expr::Ident("x".to_string())),
                        })),
                    }],
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "i".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    })),
                }],
            }),
            method: "unwrap_or".to_string(),
            args: vec![Expr::NumberLit(-1.0)],
        }
    );
}

#[test]
fn test_convert_expr_join_string_literal_passes_through() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.join(",") → arr.join(",") — string literals are already &str in Rust
    let expr = parse_expr("arr.join(\",\");");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "join".to_string(),
            args: vec![Expr::StringLit(",".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_reduce_with_init_to_iter_fold() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.reduce((acc, x) => acc + x, 0) → arr.iter().fold(0, |acc, x| acc + x)
    let expr = parse_expr("arr.reduce((acc, x) => acc + x, 0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("arr".to_string())),
                method: "iter".to_string(),
                args: vec![],
            }),
            method: "fold".to_string(),
            args: vec![
                Expr::NumberLit(0.0),
                Expr::Closure {
                    params: vec![
                        Param {
                            name: "acc".to_string(),
                            ty: None,
                        },
                        Param {
                            name: "x".to_string(),
                            ty: None,
                        },
                    ],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("acc".to_string())),
                        op: BinOp::Add,
                        right: Box::new(Expr::Ident("x".to_string())),
                    })),
                },
            ],
        }
    );
}

#[test]
fn test_map_method_reduce_typed_closure_strips_type_annotations() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.reduce((acc: number, x: number) => acc + x, 0)
    // → fold closure params should have NO type annotation (Rust infers &T from iter())
    let expr = parse_expr("arr.reduce((acc: number, x: number) => acc + x, 0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Extract the closure from fold args
    if let Expr::MethodCall { args, .. } = &result {
        if let Some(Expr::Closure { params, .. }) = args.get(1) {
            assert!(
                params[0].ty.is_none(),
                "fold closure param 'acc' should have no type annotation, got: {:?}",
                params[0].ty
            );
            assert!(
                params[1].ty.is_none(),
                "fold closure param 'x' should have no type annotation, got: {:?}",
                params[1].ty
            );
            return;
        }
    }
    panic!("expected MethodCall with fold closure, got: {result:?}");
}

#[test]
fn test_map_method_indexof_position_returns_f64_with_unwrap() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.indexOf(target) → arr.iter().position(...).map(|i| i as f64).unwrap_or(-1.0)
    let expr = parse_expr("arr.indexOf(target);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Should end with .unwrap_or(-1.0)
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "unwrap_or", "expected unwrap_or, got: {result:?}");
        assert_eq!(
            args,
            &[Expr::NumberLit(-1.0)],
            "expected unwrap_or(-1.0), got: {args:?}"
        );
        return;
    }
    panic!("expected MethodCall with unwrap_or, got: {result:?}");
}

#[test]
fn test_map_method_join_passes_borrowed_arg() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.join(sep) → arr.join(&sep)
    let expr = parse_expr("arr.join(sep);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "join");
        // The argument should be a reference: &sep
        assert_eq!(
            args,
            &[Expr::Ref(Box::new(Expr::Ident("sep".to_string())))],
            "expected &sep, got: {args:?}"
        );
        return;
    }
    panic!("expected MethodCall join, got: {result:?}");
}

#[test]
fn test_map_method_sort_no_args_uses_partial_cmp() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort() → arr.sort_by(|a, b| a.partial_cmp(b).unwrap())
    let expr = parse_expr("arr.sort();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    if let Expr::MethodCall { method, .. } = &result {
        assert_eq!(
            method, "sort_by",
            "expected sort_by for no-arg sort, got: {result:?}"
        );
        return;
    }
    panic!("expected sort_by, got: {result:?}");
}

#[test]
fn test_map_method_sort_with_comparator_strips_type_annotations() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort((a: number, b: number) => b - a) → sort_by closure params have no type annotation
    let expr = parse_expr("arr.sort((a: number, b: number) => b - a);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "sort_by");
        if let Some(Expr::Closure { params, .. }) = args.first() {
            assert!(
                params[0].ty.is_none(),
                "sort_by closure param should have no type, got: {:?}",
                params[0].ty
            );
            return;
        }
    }
    panic!("expected sort_by with untyped closure, got: {result:?}");
}

#[test]
fn test_map_method_splice_generates_integer_range() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.splice(1, 2) → arr.drain(1..3).collect::<Vec<_>>()
    // The range should use integer literals, not float
    let expr = parse_expr("arr.splice(1, 2);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Drill into: MethodCall { object: MethodCall { method: "drain", args: [Range { start, end }] }, method: "collect..." }
    if let Expr::MethodCall {
        object,
        method: collect_method,
        ..
    } = &result
    {
        assert!(
            collect_method.starts_with("collect"),
            "expected collect, got: {result:?}"
        );
        if let Expr::MethodCall { method, args, .. } = object.as_ref() {
            assert_eq!(method, "drain");
            if let Some(Expr::Range {
                start: Some(s),
                end: Some(e),
            }) = args.first()
            {
                // Start should be integer-like (NumberLit 1.0 is ok, generator handles it)
                // End should be 3 (1+2), not a BinaryOp
                assert!(
                    matches!(e.as_ref(), Expr::NumberLit(n) if *n == 3.0),
                    "expected end=3.0 (pre-computed), got: {e:?}"
                );
                assert!(
                    matches!(s.as_ref(), Expr::NumberLit(n) if *n == 1.0),
                    "expected start=1.0, got: {s:?}"
                );
                return;
            }
        }
    }
    panic!("expected drain(1..3).collect(), got: {result:?}");
}

#[test]
fn test_convert_expr_array_spread_in_expression_generates_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // foo([...arr, 1]) — spread in function arg position
    let expr = parse_expr("foo([...arr, 1]);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // The argument should be an Expr::Block
    match &result {
        Expr::FnCall { args, .. } => {
            assert_eq!(args.len(), 1);
            assert!(
                matches!(&args[0], Expr::Block(_)),
                "expected Block for spread array arg, got: {:?}",
                args[0]
            );
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_array_spread_prefix_and_suffix_generates_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // [1, ...arr, 2] in expression position (as function arg)
    let expr = parse_expr("foo([1, ...arr, 2]);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    match &result {
        Expr::FnCall { args, .. } => {
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Block(stmts) => {
                    // Should contain: let mut _v = vec![1.0]; + extend + push + tail
                    assert!(
                        stmts.len() >= 3,
                        "expected at least 3 stmts in block, got {stmts:?}"
                    );
                    // First: let mut _v = vec![1.0];
                    assert!(
                        matches!(&stmts[0], IrStmt::Let { mutable: true, name, .. } if name == "_v"),
                        "expected let mut _v, got: {:?}",
                        stmts[0]
                    );
                    // Last: tail expr _v
                    assert!(
                        matches!(stmts.last(), Some(IrStmt::TailExpr(Expr::Ident(n))) if n == "_v"),
                        "expected tail _v, got: {:?}",
                        stmts.last()
                    );
                }
                other => panic!("expected Block, got: {other:?}"),
            }
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_array_lit_empty_with_expected_vec_string() {
    // [] with expected=Vec<String> → Expr::Vec with no elements (type comes from context)
    let f = TctxFixture::from_source("const x: string[] = [];");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
        used_marker_names: std::collections::HashSet::new(),
        return_wrap_ctx: None,
    }
    .convert_expr(&swc_expr)
    .unwrap();

    assert_eq!(result, Expr::Vec { elements: vec![] });
}

#[test]
fn test_convert_array_lit_elements_get_expected_element_type() {
    // ["a", "b"] with expected=Vec<String> → elements get .to_string()
    let f = TctxFixture::from_source(r#"const x: string[] = ["a", "b"];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
        used_marker_names: std::collections::HashSet::new(),
        return_wrap_ctx: None,
    }
    .convert_expr(&swc_expr)
    .unwrap();

    match &result {
        Expr::Vec { elements } => {
            assert_eq!(elements.len(), 2);
            // Each element should have .to_string() because element expected type is String
            for elem in elements {
                assert!(
                    matches!(elem, Expr::MethodCall { method, .. } if method == "to_string"),
                    "element should be .to_string() call, got: {:?}",
                    elem
                );
            }
        }
        _ => panic!("expected Vec, got: {:?}", result),
    }
}
