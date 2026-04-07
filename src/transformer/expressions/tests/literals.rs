use super::*;

#[test]
fn test_convert_expr_identifier() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("foo".to_string()));
}

#[test]
fn test_convert_expr_number_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("42;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::NumberLit(42.0));
}

#[test]
fn test_convert_expr_bigint_literal_generates_int_lit() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("123n;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::IntLit(123));
}

#[test]
fn test_convert_expr_bigint_zero_generates_int_lit() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("0n;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::IntLit(0));
}

#[test]
fn test_convert_expr_bigint_i64_overflow_i128_range_generates_i128_lit() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // 2^63 = 9223372036854775808 — exceeds i64::MAX but fits i128
    let swc_expr = parse_expr("9223372036854775808n;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::IntLit(9_223_372_036_854_775_808));
}

#[test]
fn test_convert_expr_bigint_i128_overflow_returns_error() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // i128::MAX + 1 = 170141183460469231731687303715884105728 — exceeds i128
    let swc_expr = parse_expr("170141183460469231731687303715884105728n;");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&swc_expr);
    assert!(result.is_err());
}

#[test]
fn test_convert_expr_string_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("\"hello\";");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_bool_true() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("true;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_convert_expr_bool_false() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("false;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_convert_expr_template_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = `Hello ${name}`;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FormatMacro {
            template: "Hello {}".to_string(),
            args: vec![Expr::Ident("name".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_template_literal_no_exprs() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = `hello world`;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FormatMacro {
            template: "hello world".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_nan_to_f64_nan() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("NaN;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::PrimitiveAssocConst {
            ty: crate::ir::PrimitiveType::F64,
            name: "NAN".to_string()
        }
    );
}

#[test]
fn test_convert_expr_infinity_to_f64_infinity() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Infinity;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::PrimitiveAssocConst {
            ty: crate::ir::PrimitiveType::F64,
            name: "INFINITY".to_string()
        }
    );
}

#[test]
fn test_convert_expr_null_literal_generates_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("null");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("None".to_string()));
}

#[test]
fn test_convert_expr_null_with_option_expected_returns_none_not_some_none() {
    // null with expected=Option<f64> should be None, NOT Some(None)
    let f = TctxFixture::from_source("const x: number | undefined = null;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Ident("None".to_string()),
        "null with Option expected should be None, got: {:?}",
        result
    );
}
