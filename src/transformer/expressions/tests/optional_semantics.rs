use super::*;

#[test]
fn test_object_lit_omitted_optional_field_gets_none() {
    // struct Item { name: String, value: Option<f64> }
    // { name: "test" } → Item { name: "test".to_string(), value: None }
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Item".to_string(),
        TypeDef::new_struct(
            vec![
                ("name".to_string(), RustType::String),
                (
                    "value".to_string(),
                    RustType::Option(Box::new(RustType::F64)),
                ),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(r#"const i: Item = { name: "test" };"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::StructInit { fields, .. } => {
            assert_eq!(fields.len(), 2, "expected 2 fields (name + value: None)");
            assert!(
                fields
                    .iter()
                    .any(|(k, v)| k == "value" && matches!(v, Expr::Ident(s) if s == "None")),
                "expected value: None, got {:?}",
                fields
            );
        }
        other => panic!("expected StructInit, got {other:?}"),
    }
}

#[test]
fn test_convert_expr_await_simple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("await fetch();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Await(Box::new(Expr::FnCall {
            name: "fetch".to_string(),
            args: vec![],
        }))
    );
}

#[test]
fn test_convert_expr_await_ident() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("await promise;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Await(Box::new(Expr::Ident("promise".to_string())))
    );
}

#[test]
fn test_undefined_literal_converts_to_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("undefined;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("None".to_string()));
}

#[test]
fn test_equals_undefined_converts_to_is_none() {
    let f = TctxFixture::from_source("function f(x: number | null) { x === undefined; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_none"),
        "expected is_none, got: {:?}",
        result
    );
}

#[test]
fn test_not_equals_undefined_converts_to_is_some() {
    let f = TctxFixture::from_source("function f(x: number | null) { x !== undefined; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_some"),
        "expected is_some, got: {:?}",
        result
    );
}

#[test]
fn test_option_expected_wraps_literal_in_some() {
    // Literals with Option expected are wrapped in Some() (for array elements etc.)
    let f = TctxFixture::from_source("const x: number | undefined = 42;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Some".to_string(),
            args: vec![Expr::NumberLit(42.0)],
        }
    );
}

#[test]
fn test_option_expected_undefined_stays_none() {
    let f = TctxFixture::from_source("const x: number | undefined = undefined;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Should be None, not Some(None)
    assert_eq!(result, Expr::Ident("None".to_string()));
}

#[test]
fn test_convert_expr_non_null_assertion_strips_assertion() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x! → x (non-null assertion is type-level only, stripped)
    let expr = parse_expr("x!;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}

#[test]
fn test_convert_expr_ident_with_option_expected_passes_through() {
    // x with expected=Option<String> and unknown type → Some(x) (centralized wrapping)
    let f = TctxFixture::from_source("const y: string | undefined = x;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Some".to_string(),
            args: vec![Expr::Ident("x".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_undefined_with_option_expected_returns_none() {
    // undefined with expected=Option<T> → None (no wrapping)
    let f = TctxFixture::from_source("const y: string | undefined = undefined;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("None".to_string()));
}
