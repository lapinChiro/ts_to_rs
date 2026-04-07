use super::*;

#[test]
fn test_convert_expr_math_max_three_args_chains() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.max(a, b, c) → a.max(b).max(c)
    let expr = parse_expr("Math.max(a, b, c);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("a".to_string())),
                method: "max".to_string(),
                args: vec![Expr::Ident("b".to_string())],
            }),
            method: "max".to_string(),
            args: vec![Expr::Ident("c".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_math_floor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.floor(3.7);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::NumberLit(3.7)),
            method: "floor".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_ceil() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.ceil(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "ceil".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_round() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.round(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "round".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_abs() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.abs(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "abs".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_sqrt() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.sqrt(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "sqrt".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_max() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.max(a, b);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("a".to_string())),
            method: "max".to_string(),
            args: vec![Expr::Ident("b".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_math_min() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.min(a, b);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("a".to_string())),
            method: "min".to_string(),
            args: vec![Expr::Ident("b".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_math_pow() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.pow(x, 2);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "powf".to_string(),
            args: vec![Expr::NumberLit(2.0)],
        }
    );
}

#[test]
fn test_convert_expr_math_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.floor(Math.sqrt(x));");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "sqrt".to_string(),
                args: vec![],
            }),
            method: "floor".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_parse_int() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"parseInt("42");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // parseInt("42") → "42".parse::<f64>().unwrap_or(f64::NAN)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::StringLit("42".to_string())),
                method: "parse::<f64>".to_string(),
                args: vec![],
            }),
            method: "unwrap_or".to_string(),
            args: vec![Expr::PrimitiveAssocConst {
                ty: crate::ir::PrimitiveType::F64,
                name: "NAN".to_string()
            }],
        }
    );
}

#[test]
fn test_convert_expr_parse_float() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"parseFloat("3.14");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // parseFloat("3.14") → "3.14".parse::<f64>().unwrap_or(f64::NAN)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::StringLit("3.14".to_string())),
                method: "parse::<f64>".to_string(),
                args: vec![],
            }),
            method: "unwrap_or".to_string(),
            args: vec![Expr::PrimitiveAssocConst {
                ty: crate::ir::PrimitiveType::F64,
                name: "NAN".to_string()
            }],
        }
    );
}

#[test]
fn test_convert_expr_is_nan_global() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("isNaN(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // isNaN(x) → x.is_nan()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "is_nan".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_number_is_nan() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Number.isNaN(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Number.isNaN(x) → x.is_nan()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "is_nan".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_number_is_finite() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Number.isFinite(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Number.isFinite(x) → x.is_finite()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "is_finite".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_number_is_integer_to_fract() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Number.isInteger(x) → x.fract() == 0.0
    let expr = parse_expr("Number.isInteger(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "fract".to_string(),
                args: vec![],
            }),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        }
    );
}

#[test]
fn test_convert_expr_math_sign_to_signum() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.sign(x) → x.signum()
    let expr = parse_expr("Math.sign(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "signum".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_trunc() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.trunc(x) → x.trunc()
    let expr = parse_expr("Math.trunc(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "trunc".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_log_to_ln() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.log(x) → x.ln()
    let expr = parse_expr("Math.log(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "ln".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_pi_to_consts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.PI → std::f64::consts::PI
    let expr = parse_expr("Math.PI;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::StdConst(crate::ir::StdConst::F64Pi));
}

#[test]
fn test_convert_expr_math_e_to_consts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.E → std::f64::consts::E
    let expr = parse_expr("Math.E;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::StdConst(crate::ir::StdConst::F64E));
}
