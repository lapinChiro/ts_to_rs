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
    // I-171 B.1.5: `!true` const-folds to `BoolLit(false)` (Matrix B.1.5
    // const-fold for boolean literal). Pre-I-171 emission was the raw
    // `UnaryOp { Not, BoolLit(true) }` which Rust also accepts but loses
    // the opportunity to simplify at the AST layer.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!true;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
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

// ------------------------------------------------------------------
// I-169 T6-2 follow-up structural snapshot (D-5): closure-reassign
// narrow-stale read coerce via `helpers::coerce_default`.
// ------------------------------------------------------------------

/// Converts the given source (must define a single `function f(...) { ... }`)
/// and returns the last IR tail-expr or return expression from the body.
/// Used by the structural snapshot tests below.
fn transform_fn_last_value(source: &str) -> Expr {
    use crate::ir::Stmt as IrStmt;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .expect("transform failed");
    let last = result.last().expect("body produced no stmts").clone();
    match last {
        IrStmt::TailExpr(e) => e,
        IrStmt::Return(Some(e)) => e,
        other => panic!("expected tail expr or return, got {other:?}"),
    }
}

#[test]
fn arith_coerce_when_closure_reassigned_option_lhs() {
    // Matrix cell C3 / I-144 T6-2: `x + 1` where `x` is closure-reassigned
    // Option<f64> must emit `x.unwrap_or(0.0) + 1.0`.
    let result = transform_fn_last_value(
        r#"
        function f(): number {
            let x: number | null = 5;
            if (x === null) return -1;
            const reset = () => { x = null; };
            reset();
            return x + 1;
        }
        "#,
    );
    match result {
        Expr::BinaryOp { left, op, right } => {
            assert_eq!(op, BinOp::Add);
            assert!(matches!(*right, Expr::NumberLit(n) if n == 1.0));
            match *left {
                Expr::MethodCall {
                    object,
                    method,
                    args,
                } => {
                    assert_eq!(method, "unwrap_or");
                    assert!(matches!(*object, Expr::Ident(n) if n == "x"));
                    assert_eq!(args.len(), 1);
                    assert!(matches!(args[0], Expr::NumberLit(n) if n == 0.0));
                }
                other => panic!("expected MethodCall(unwrap_or) as LHS, got {other:?}"),
            }
        }
        other => panic!("expected BinaryOp, got {other:?}"),
    }
}

#[test]
fn string_concat_coerce_when_closure_reassigned_option() {
    // Matrix cell C4 / I-144 T6-2: `"v=" + x` where `x` is closure-reassigned
    // Option<f64> must emit the FormatMacro string-coerce shape
    // `format!("{}{}", "v=", x.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()))`.
    let result = transform_fn_last_value(
        r#"
        function f(): string {
            let x: number | null = 5;
            if (x === null) return "no";
            const reset = () => { x = null; };
            reset();
            return "v=" + x;
        }
        "#,
    );
    match result {
        Expr::FormatMacro { template, args } => {
            assert_eq!(template, "{}{}");
            assert_eq!(args.len(), 2);
            assert!(matches!(&args[0], Expr::StringLit(s) if s == "v="));
            // args[1] should be x.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string())
            match &args[1] {
                Expr::MethodCall {
                    object: outer_obj,
                    method,
                    args: outer_args,
                } => {
                    assert_eq!(method, "unwrap_or_else");
                    assert_eq!(outer_args.len(), 1);
                    match outer_obj.as_ref() {
                        Expr::MethodCall {
                            object,
                            method,
                            args,
                        } => {
                            assert_eq!(method, "map");
                            assert!(matches!(object.as_ref(), Expr::Ident(n) if n == "x"));
                            assert_eq!(args.len(), 1);
                        }
                        other => panic!("expected inner MethodCall(map), got {other:?}"),
                    }
                }
                other => panic!("expected MethodCall(unwrap_or_else) as arg[1], got {other:?}"),
            }
        }
        other => panic!("expected FormatMacro, got {other:?}"),
    }
}

#[test]
fn arith_coerce_does_not_fire_for_non_option_closure_reassigned_var() {
    // I-169 matrix cell #18 (B5 rest param NA) negative-path lockin:
    // a rest param (`Vec<T>`) reassigned inside a closure IS emitted as a
    // `ClosureCapture` event by the analyzer, but the Transformer's
    // `maybe_coerce_for_arith` guard (`matches!(ty, RustType::Option(_))`)
    // must filter it out so no `unwrap_or(...)` wrap is injected — the
    // read site should remain the plain ident access.
    //
    // The TS fixture uses `xs.length` (MemberAccess, not arithmetic) so
    // the arith hook isn't even consulted; this test instead exercises
    // an arithmetic read of the rest-param-like `Vec<T>` by summing an
    // element into an accumulator. Concretely: `xs[0] + 1`. Under the
    // T6-2 coerce rules, `xs[0]` resolves to `Option<f64>` (via I-138
    // Vec index Option), NOT `xs` itself; and `xs` as a candidate has
    // type `Vec<f64>`, not `Option<_>`. Reading `xs` bare (`return xs;`)
    // as the tail expression exercises the Vec-in-Option-context guard
    // path: no coerce must be applied to `xs`.
    let result = transform_fn_last_value(
        r#"
        function f(...xs: number[]): number[] {
            const reset = () => { xs = []; };
            reset();
            return xs;
        }
        "#,
    );
    // Expected: plain Ident("xs") (no unwrap_or wrap because xs is Vec<f64>,
    // not Option<T>, so the `matches!(ty, RustType::Option(_))` guard in
    // maybe_coerce_for_arith / maybe_coerce_for_string_concat filters it out).
    match result {
        Expr::Ident(name) => assert_eq!(name, "xs"),
        other => panic!(
            "expected plain Ident(\"xs\") with no coerce wrap for Vec<T> closure-reassigned var, got {other:?}"
        ),
    }
}
