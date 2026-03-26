use super::*;

#[test]
fn test_convert_expr_string_length_to_len_as_f64() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.length;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Cast {
            expr: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "len".to_string(),
                args: vec![],
            }),
            target: RustType::F64,
        }
    );
}

#[test]
fn test_convert_expr_string_includes_to_contains() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"s.includes("x");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "contains".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::StringLit("x".to_string())))],
        }
    );
}

#[test]
fn test_convert_includes_to_contains_with_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.includes(3) → arr.contains(&3.0)
    let expr = parse_expr("arr.includes(3);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "contains".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::NumberLit(3.0)))],
        }
    );
}

#[test]
fn test_convert_expr_string_starts_with() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"s.startsWith("a");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "starts_with".to_string(),
            args: vec![Expr::StringLit("a".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_string_ends_with() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"s.endsWith("z");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "ends_with".to_string(),
            args: vec![Expr::StringLit("z".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_string_trim_adds_to_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.trim();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "trim".to_string(),
                args: vec![],
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_to_lower_case() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.toLowerCase();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "to_lowercase".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_to_upper_case() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.toUpperCase();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "to_uppercase".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_split_generates_vec_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.split(",") → s.split(",").map(|s| s.to_string()).collect::<Vec<String>>()
    let expr = parse_expr(r#"s.split(",");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("s".to_string())),
                    method: "split".to_string(),
                    args: vec![Expr::StringLit(",".to_string())],
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "s".to_string(),
                        ty: None,
                    }],
                    body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("s".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    })),
                    return_type: None,
                }],
            }),
            method: "collect::<Vec<String>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_substring_two_args_generates_slice() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.substring(1, 3) → s[1..3].to_string()
    let expr = parse_expr("s.substring(1, 3);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::Ident("s".to_string())),
                index: Box::new(Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: Some(Box::new(Expr::NumberLit(3.0))),
                }),
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_substring_one_arg_generates_open_range() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.substring(1) → s[1..].to_string()
    let expr = parse_expr("s.substring(1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::Ident("s".to_string())),
                index: Box::new(Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: None,
                }),
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_slice_one_arg_generates_open_range() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.slice(1) → arr[1..].to_vec()
    let expr = parse_expr("arr.slice(1);");
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
                    end: None,
                }),
            }),
            method: "to_vec".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_replace_generates_replacen() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replace("a", "b") → s.replacen("a", "b", 1) (first occurrence only)
    let expr = parse_expr(r#"s.replace("a", "b");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "replacen".to_string(),
            args: vec![
                Expr::StringLit("a".to_string()),
                Expr::StringLit("b".to_string()),
                Expr::IntLit(1),
            ],
        }
    );
}

#[test]
fn test_convert_expr_string_replace_all_generates_replace() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replaceAll("a", "b") → s.replace("a", "b") (Rust replace replaces all)
    let expr = parse_expr(r#"s.replaceAll("a", "b");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "replace".to_string(),
            args: vec![
                Expr::StringLit("a".to_string()),
                Expr::StringLit("b".to_string()),
            ],
        }
    );
}
