use super::*;

#[test]
fn test_convert_expr_regex_no_flags_generates_regex_new() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/ → Expr::Regex { global: false, sticky: false }
    let expr = parse_var_init(r#"const r = /pattern/;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: false,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_global_flag_preserved() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/g → Expr::Regex { global: true }
    let expr = parse_var_init(r#"const r = /pattern/g;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_case_insensitive_flag_inlined() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/i → Expr::Regex with (?i) prefix
    let expr = parse_var_init(r#"const r = /pattern/i;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "(?i)pattern".to_string(),
            global: false,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_multiple_flags_inlined() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/gim → Expr::Regex with (?i)(?m) prefix and global: true
    let expr = parse_var_init(r#"const r = /pattern/gim;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "(?i)(?m)pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_no_flags_generates_regex_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/ → Expr::Regex { global: false, sticky: false }
    let expr = parse_var_init(r#"const r = /pattern/;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: false,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_global_flag_preserved_in_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/g → Expr::Regex { global: true, sticky: false }
    let expr = parse_var_init(r#"const r = /pattern/g;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_sticky_flag_preserved_in_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/y → Expr::Regex { global: false, sticky: true }
    let expr = parse_var_init(r#"const r = /pattern/y;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: false,
            sticky: true,
        }
    );
}

#[test]
fn test_convert_expr_regex_multiple_flags_preserved_in_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/gims → Expr::Regex { global: true, sticky: false } with (?i)(?m)(?s) prefix
    let expr = parse_var_init(r#"const r = /pattern/gims;"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "(?i)(?m)(?s)pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_replace_with_global_regex_generates_replace_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replace(/p/g, "r") → Regex::new(r"p").unwrap().replace_all(&s, "r").to_string()
    let expr = parse_expr(r#"s.replace(/p/g, "r");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Regex {
                    pattern: "p".to_string(),
                    global: false,
                    sticky: false,
                }),
                method: "replace_all".to_string(),
                args: vec![
                    Expr::Ref(Box::new(Expr::Ident("s".to_string()))),
                    Expr::StringLit("r".to_string()),
                ],
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_replace_with_non_global_regex_generates_replace() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replace(/p/, "r") → Regex::new(r"p").unwrap().replace(&s, "r").to_string()
    let expr = parse_expr(r#"s.replace(/p/, "r");"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Regex {
                    pattern: "p".to_string(),
                    global: false,
                    sticky: false,
                }),
                method: "replace".to_string(),
                args: vec![
                    Expr::Ref(Box::new(Expr::Ident("s".to_string()))),
                    Expr::StringLit("r".to_string()),
                ],
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_regex_test_generates_is_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /p/.test(s) → Regex::new(r"p").unwrap().is_match(&s)
    let expr = parse_expr(r#"/p/.test(s);"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "is_match".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}

#[test]
fn test_convert_expr_string_match_regex_generates_find() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.match(/p/) → Regex::new(r"p").unwrap().find(&s)
    let expr = parse_expr(r#"s.match(/p/);"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "find".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}

#[test]
fn test_convert_expr_string_match_global_regex_generates_find_iter() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.match(/p/g) → Regex::new(r"p").unwrap().find_iter(&s)
    let expr = parse_expr(r#"s.match(/p/g);"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "find_iter".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}

#[test]
fn test_convert_expr_regex_exec_generates_captures() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /p/.exec(s) → Regex::new(r"p").unwrap().captures(&s)
    let expr = parse_expr(r#"/p/.exec(s);"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "captures".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}
