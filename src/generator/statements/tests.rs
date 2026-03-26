use crate::generator::generate;
use crate::ir::{BinOp, Expr, Item, MatchPattern as MP, RustType, Stmt, Visibility};

// Statement tests need to be wrapped in Item::Fn to test generate()

#[test]
fn test_generate_let_simple() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(42.0)),
        }],
    };
    let expected = "\
fn f() {
    let x = 42.0;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_let_mut_with_type() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Let {
            mutable: true,
            name: "count".to_string(),
            ty: Some(RustType::F64),
            init: Some(Expr::NumberLit(0.0)),
        }],
    };
    let expected = "\
fn f() {
    let mut count: f64 = 0.0;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_let_no_init() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(RustType::String),
            init: None,
        }],
    };
    let expected = "\
fn f() {
    let x: String;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_if_no_else() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Return(None)],
            else_body: None,
        }],
    };
    let expected = "\
fn f() {
    if true {
        return;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_if_with_else() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::If {
            condition: Expr::Ident("x".to_string()),
            then_body: vec![Stmt::Expr(Expr::Ident("a".to_string()))],
            else_body: Some(vec![Stmt::Expr(Expr::Ident("b".to_string()))]),
        }],
    };
    let expected = "\
fn f() {
    if x {
        a;
    } else {
        b;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_while() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::While {
            label: None,
            condition: Expr::BoolLit(true),
            body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
        }],
    };
    let expected = "\
fn f() {
    while true {
        x;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_while_let_generates_pattern_match_loop() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::WhileLet {
            label: None,
            pattern: "Some(x)".to_string(),
            expr: Expr::FnCall {
                name: "get_value".to_string(),
                args: vec![],
            },
            body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
        }],
    };
    let expected = "\
fn f() {
    while let Some(x) = get_value() {
        x;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_for_in_range() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Some(Box::new(Expr::NumberLit(0.0))),
                end: Some(Box::new(Expr::Ident("n".to_string()))),
            },
            body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
        }],
    };
    let expected = "\
fn f() {
    for i in 0..n as i64 {
        x;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_for_in_iterable() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::ForIn {
            label: None,
            var: "item".to_string(),
            iterable: Expr::Ident("items".to_string()),
            body: vec![Stmt::Expr(Expr::Ident("item".to_string()))],
        }],
    };
    let expected = "\
fn f() {
    for item in items {
        item;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_loop_basic() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Loop {
            label: None,
            body: vec![Stmt::Break {
                label: None,
                value: None,
            }],
        }],
    };
    let expected = "\
fn f() {
    loop {
        break;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_return_bare() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![
            Stmt::Expr(Expr::Ident("something".to_string())),
            Stmt::Return(None),
        ],
    };
    let expected = "\
fn f() {
    something;
    return;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_stmt_tail_expr_ident_outputs_without_semicolon() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: Some(RustType::F64),
        body: vec![Stmt::TailExpr(Expr::Ident("x".to_string()))],
    };
    let expected = "\
fn f() -> f64 {
    x
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_stmt_tail_expr_complex_expr_outputs_without_semicolon() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: Some(RustType::F64),
        body: vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::Ident("b".to_string())),
        })],
    };
    let expected = "\
fn f() -> f64 {
    a + b
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_labeled_block_simple_body_outputs_labeled_block() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::LabeledBlock {
            label: "try_block".to_string(),
            body: vec![Stmt::Expr(Expr::FnCall {
                name: "do_something".to_string(),
                args: vec![],
            })],
        }],
    };
    let expected = "\
fn f() {
    'try_block: {
        do_something();
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_break_with_label_and_value_outputs_break_label_value() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Break {
            label: Some("try_block".to_string()),
            value: Some(Expr::FnCall {
                name: "Err".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::StringLit("error".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }],
            }),
        }],
    };
    let expected = "\
fn f() {
    break 'try_block Err(\"error\".to_string());
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_break_label_only_no_value_outputs_break_label() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Break {
            label: Some("outer".to_string()),
            value: None,
        }],
    };
    let expected = "\
fn f() {
    break 'outer;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_break_no_label_no_value_outputs_break() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Break {
            label: None,
            value: None,
        }],
    };
    let expected = "\
fn f() {
    break;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_return_not_last_uses_return_keyword() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: Some(RustType::F64),
        body: vec![
            Stmt::Return(Some(Expr::NumberLit(1.0))),
            Stmt::TailExpr(Expr::NumberLit(2.0)),
        ],
    };
    let expected = "\
fn f() -> f64 {
    return 1.0;
    2.0
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_stmt_if_let_without_else_renders_if_let() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::IfLet {
            pattern: "Err(e)".to_string(),
            expr: Expr::Ident("result".to_string()),
            then_body: vec![Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident("e".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            })],
            else_body: None,
        }],
    };
    let expected = "\
fn f() {
    if let Err(e) = result {
        e.to_string();
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_stmt_if_let_with_else_renders_else_branch() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::IfLet {
            pattern: "Some(x)".to_string(),
            expr: Expr::Ident("opt".to_string()),
            then_body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
            else_body: Some(vec![Stmt::Return(None)]),
        }],
    };
    let expected = "\
fn f() {
    if let Some(x) = opt {
        x;
    } else {
        return;
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_match_single_arm_renders_match() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![crate::ir::MatchArm {
                patterns: vec![MP::Literal(Expr::IntLit(1))],
                guard: None,
                body: vec![Stmt::Expr(Expr::FnCall {
                    name: "do_a".to_string(),
                    args: vec![],
                })],
            }],
        }],
    };
    let expected = "\
fn f() {
    match x {
        1 => {
            do_a();
        }
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_match_multiple_patterns_renders_or() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![crate::ir::MatchArm {
                patterns: vec![MP::Literal(Expr::IntLit(1)), MP::Literal(Expr::IntLit(2))],
                guard: None,
                body: vec![Stmt::Expr(Expr::FnCall {
                    name: "do_ab".to_string(),
                    args: vec![],
                })],
            }],
        }],
    };
    let expected = "\
fn f() {
    match x {
        1 | 2 => {
            do_ab();
        }
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_match_wildcard_renders_underscore() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![crate::ir::MatchArm {
                patterns: vec![MP::Wildcard],
                guard: None,
                body: vec![Stmt::Expr(Expr::FnCall {
                    name: "do_default".to_string(),
                    args: vec![],
                })],
            }],
        }],
    };
    let expected = "\
fn f() {
    match x {
        _ => {
            do_default();
        }
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_match_multiple_arms_renders_all() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![
                crate::ir::MatchArm {
                    patterns: vec![MP::Literal(Expr::IntLit(1))],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_a".to_string(),
                        args: vec![],
                    })],
                },
                crate::ir::MatchArm {
                    patterns: vec![MP::Literal(Expr::IntLit(2)), MP::Literal(Expr::IntLit(3))],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_bc".to_string(),
                        args: vec![],
                    })],
                },
                crate::ir::MatchArm {
                    patterns: vec![MP::Wildcard],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_default".to_string(),
                        args: vec![],
                    })],
                },
            ],
        }],
    };
    let expected = "\
fn f() {
    match x {
        1 => {
            do_a();
        }
        2 | 3 => {
            do_bc();
        }
        _ => {
            do_default();
        }
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_match_string_patterns_renders_as_str_from_ir() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Match {
            expr: Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "as_str".to_string(),
                args: vec![],
            },
            arms: vec![
                crate::ir::MatchArm {
                    patterns: vec![MP::Literal(Expr::StringLit("hello".to_string()))],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_hello".to_string(),
                        args: vec![],
                    })],
                },
                crate::ir::MatchArm {
                    patterns: vec![MP::Wildcard],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_default".to_string(),
                        args: vec![],
                    })],
                },
            ],
        }],
    };
    let expected = "\
fn f() {
    match s.as_str() {
        \"hello\" => {
            do_hello();
        }
        _ => {
            do_default();
        }
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_match_enum_variant_with_bindings_renders_field_names() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "f".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![Stmt::Match {
            expr: Expr::Ref(Box::new(Expr::Ident("s".to_string()))),
            arms: vec![
                crate::ir::MatchArm {
                    patterns: vec![MP::EnumVariant {
                        path: "Shape::Circle".to_string(),
                        bindings: vec!["radius".to_string()],
                    }],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::Ident("radius".to_string()))],
                },
                crate::ir::MatchArm {
                    patterns: vec![MP::EnumVariant {
                        path: "Shape::Rect".to_string(),
                        bindings: vec!["width".to_string(), "height".to_string()],
                    }],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("width".to_string())),
                        op: BinOp::Add,
                        right: Box::new(Expr::Ident("height".to_string())),
                    })],
                },
                crate::ir::MatchArm {
                    patterns: vec![MP::Wildcard],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::IntLit(0))],
                },
            ],
        }],
    };
    let expected = "\
fn f() {
    match &s {
        Shape::Circle { radius, .. } => {
            radius;
        }
        Shape::Rect { width, height, .. } => {
            width + height;
        }
        _ => {
            0;
        }
    }
}";
    assert_eq!(generate(&[item]), expected);
}
