use super::*;

#[test]
fn test_convert_expr_ternary_basic_identifiers() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = flag ? a : b;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::Ident("flag".to_string())),
            then_expr: Box::new(Expr::Ident("a".to_string())),
            else_expr: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_ternary_with_comparison_condition() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a > 0 ? a : b;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            then_expr: Box::new(Expr::Ident("a".to_string())),
            else_expr: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_ternary_with_string_literals() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init(r#"const x = flag ? "yes" : "no";"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::Ident("flag".to_string())),
            then_expr: Box::new(Expr::StringLit("yes".to_string())),
            else_expr: Box::new(Expr::StringLit("no".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_ternary_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x > 0 ? "positive" : x < 0 ? "negative" : "zero"
    let swc_expr = parse_var_init(r#"const s = x > 0 ? "positive" : x < 0 ? "negative" : "zero";"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            then_expr: Box::new(Expr::StringLit("positive".to_string())),
            else_expr: Box::new(Expr::If {
                condition: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Lt,
                    right: Box::new(Expr::NumberLit(0.0)),
                }),
                then_expr: Box::new(Expr::StringLit("negative".to_string())),
                else_expr: Box::new(Expr::StringLit("zero".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_ternary_heterogeneous_branches_produces_if() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // cond ? "a" : 1 → if-else with different types (no type coercion)
    let swc_expr = parse_var_init(r#"const x = flag ? "a" : 1;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::Ident("flag".to_string())),
            then_expr: Box::new(Expr::StringLit("a".to_string())),
            else_expr: Box::new(Expr::NumberLit(1.0)),
        }
    );
}
