use super::*;

#[test]
fn test_convert_expr_binary_add() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a + b;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_binary_greater_than() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a > b;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Gt,
            right: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_binary_strict_equals() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a === b;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Eq,
            right: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_unary_not_bool_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!true;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::BoolLit(true)),
        }
    );
}

#[test]
fn test_convert_expr_unary_not_ident() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident("x".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_unary_minus_ident() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("-x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::Ident("x".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_unary_minus_number_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("-42;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::NumberLit(42.0)),
        }
    );
}

#[test]
fn test_convert_expr_unary_not_complex_expr() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(a > b);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::Ident("b".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_bitwise_xor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a ^ b");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::BitXor,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_bitwise_and() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a & b");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::BitAnd,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_bitwise_or() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a | b");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::BitOr,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_shift_left() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a << b");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert!(matches!(result, Expr::BinaryOp { op: BinOp::Shl, .. }));
}

#[test]
fn test_convert_expr_shift_right() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a >> b");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert!(matches!(result, Expr::BinaryOp { op: BinOp::Shr, .. }));
}

#[test]
fn test_convert_expr_unsigned_right_shift_generates_ushr() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a >>> b");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::UShr,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_compound_assign_ushr_generates_desugar() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x >>>= 2");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Should be Assign { target: x, value: BinaryOp { op: UShr, ... } }
    if let Expr::Assign { value, .. } = &result {
        assert!(
            matches!(
                value.as_ref(),
                Expr::BinaryOp {
                    op: BinOp::UShr,
                    ..
                }
            ),
            "expected UShr binary op in assignment, got: {value:?}"
        );
    } else {
        panic!("expected Assign, got: {result:?}");
    }
}

#[test]
fn test_convert_expr_compound_assign_mod() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x %= 3 → x = x % 3
    let expr = parse_expr("x %= 3");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Mod,
                right: Box::new(Expr::NumberLit(3.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_bitand() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x &= mask → x = x & mask
    let expr = parse_expr("x &= mask");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::BitAnd,
                right: Box::new(Expr::Ident("mask".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_add() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x += 1 → x = x + 1
    let expr = parse_expr("x += 1");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(1.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_sub() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x -= 1 → x = x - 1
    let expr = parse_expr("x -= 1");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Sub,
                right: Box::new(Expr::NumberLit(1.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_mul() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x *= 2 → x = x * 2
    let expr = parse_expr("x *= 2");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Mul,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_div() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x /= 2 → x = x / 2
    let expr = parse_expr("x /= 2");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Div,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_bitor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x |= mask → x = x | mask
    let expr = parse_expr("x |= mask");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::BitOr,
                right: Box::new(Expr::Ident("mask".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_bitxor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x ^= mask → x = x ^ mask
    let expr = parse_expr("x ^= mask");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::BitXor,
                right: Box::new(Expr::Ident("mask".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_shl() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x <<= 2 → x = x << 2
    let expr = parse_expr("x <<= 2");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Shl,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_shr() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x >>= 2 → x = x >> 2
    let expr = parse_expr("x >>= 2");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Shr,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_unary_plus_number_returns_identity() {
    let f = TctxFixture::from_source("function f(x: number) { +x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}

#[test]
fn test_convert_expr_unary_plus_string_returns_parse() {
    let f = TctxFixture::from_source("function f(x: string) { +x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // x.parse::<f64>().unwrap()
    match &result {
        Expr::MethodCall { method, object, .. } if method == "unwrap" => match object.as_ref() {
            Expr::MethodCall { method, .. } if method == "parse::<f64>" => {}
            other => panic!("expected parse::<f64>(), got {other:?}"),
        },
        other => panic!("expected .unwrap(), got {other:?}"),
    }
}

#[test]
fn test_convert_expr_unary_plus_unknown_returns_identity() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("+x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}
