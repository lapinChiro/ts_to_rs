use super::*;

#[test]
fn test_typeof_equals_string_known_type_resolves_true() {
    let f = TctxFixture::from_source(r#"function f(x: string) { typeof x === "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_typeof_equals_string_mismatched_type_resolves_false() {
    let f = TctxFixture::from_source(r#"function f(x: number) { typeof x === "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_typeof_equals_number_known_type_resolves_true() {
    let f = TctxFixture::from_source(r#"function f(x: number) { typeof x === "number"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_typeof_not_equals_string_known_type_resolves_false() {
    let f = TctxFixture::from_source(r#"function f(x: string) { typeof x !== "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_typeof_equals_string_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("typeof x === \"string\";");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Unknown type → todo!() (compile error, not silent true)
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_equals_string_any_type_generates_todo() {
    // Any type → todo!() (compile error, not silent true).
    // For function params, any_narrowing generates enum and if-let instead.
    let f = TctxFixture::from_source(r#"function f(x: any) { typeof x === "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_equals_number_any_type_generates_todo() {
    let f = TctxFixture::from_source(r#"function f(x: any) { typeof x === "number"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_not_equals_string_any_type_generates_todo() {
    // !== with Any → todo!() (compile error, not silent true).
    let f = TctxFixture::from_source(r#"function f(x: any) { typeof x !== "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_instanceof_any_type_generates_todo() {
    // Any type → todo!() (compile error, not silent true).
    // For function params, any_narrowing generates enum and if-let instead.
    let f = TctxFixture::from_source("function f(x: any) { x instanceof Foo; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_equals_undefined_option_resolves_is_none() {
    let f =
        TctxFixture::from_source(r#"function f(x: number | null) { typeof x === "undefined"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_none"),
        "expected is_none call, got: {:?}",
        result
    );
}

#[test]
fn test_typeof_standalone_known_type_resolves_string_lit() {
    let f = TctxFixture::from_source("function f(x: string) { typeof x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::StringLit("string".to_string()));
}

#[test]
fn test_instanceof_known_type_match_resolves_true() {
    // x: Foo, x instanceof Foo → true
    let f = TctxFixture::from_source("class Foo {} function f(x: Foo) { x instanceof Foo; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 1, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_instanceof_known_type_mismatch_resolves_false() {
    // x: Bar, x instanceof Foo → false
    let f = TctxFixture::from_source(
        "class Foo {} class Bar {} function f(x: Bar) { x instanceof Foo; }",
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 2, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_instanceof_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Unknown type → todo!() (compile error, not silent true).
    let swc_expr = parse_expr("x instanceof Foo;");

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
    }
    .convert_expr(&swc_expr)
    .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_in_operator_struct_field_exists_generates_true() {
    // "x" in point → true (Point has field x)
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(r#"function f(point: Point) { "x" in point; }"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_in_operator_struct_field_missing_generates_false() {
    // "z" in point → false (Point has no field z)
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(r#"function f(point: Point) { "z" in point; }"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_in_operator_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // "x" in unknown → todo!() (not silent true)
    let expr = parse_expr(r#""x" in unknown"#);

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
    }
    .convert_expr(&expr)
    .unwrap();
    match &result {
        Expr::FnCall { name, .. } => assert_eq!(name, "todo!"),
        other => panic!("expected todo!() for unknown in operator, got: {other:?}"),
    }
}

#[test]
fn test_convert_instanceof_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Unknown type → todo!() (compile error, not silent true).
    let expr = parse_expr("x instanceof Foo");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_convert_instanceof_known_matching_type_returns_true() {
    // x: Foo, x instanceof Foo → true (correct static resolution)
    let f = TctxFixture::from_source("class Foo {} function f(x: Foo) { x instanceof Foo; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 1, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_convert_instanceof_option_type_returns_is_some() {
    // x: Foo | null, x instanceof Foo → x.is_some()
    let f =
        TctxFixture::from_source("class Foo {} function f(x: Foo | null) { x instanceof Foo; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 1, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "is_some");
        }
        other => panic!("expected MethodCall(is_some), got: {other:?}"),
    }
}

#[test]
fn test_convert_typeof_static_number_returns_string_lit() {
    // typeof 42 → "number" (static, no change needed)
    let f = TctxFixture::from_source("function f() { typeof 42; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::StringLit("number".to_string()));
}

#[test]
fn test_convert_typeof_option_type_returns_runtime_if() {
    // typeof x where x: number | null → runtime branch
    let f = TctxFixture::from_source("function f(x: number | null) { typeof x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Should be an If expression, NOT a static StringLit("undefined")
    match &result {
        Expr::If { .. } => {} // runtime branch — correct
        Expr::StringLit(s) if s == "undefined" => {
            panic!("typeof Option should NOT be static 'undefined' — must be runtime branch")
        }
        other => panic!("expected If for typeof Option, got: {other:?}"),
    }
}

#[test]
fn test_convert_typeof_unknown_type_returns_error() {
    // typeof x where type is unresolvable → UnsupportedSyntaxError (not silent "object")
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("typeof x");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&expr);
    assert!(
        result.is_err(),
        "typeof on unresolved type should return error, not silent 'object'"
    );
}

#[test]
fn test_convert_typeof_any_type_standalone_generates_runtime_typeof() {
    // typeof x where x: any → Expr::RuntimeTypeof (runtime helper call)
    let f = TctxFixture::from_source("function f(x: any) { typeof x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::RuntimeTypeof { .. } => {} // correct — runtime helper
        other => panic!("expected RuntimeTypeof for typeof any, got: {other:?}"),
    }
}

#[test]
fn test_resolve_if_let_pattern_option_via_type_resolution() {
    let source = r#"function f(x: string | null): string { return x !== null ? x : ""; }"#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();

    // Create Transformer — FileTypeResolution provides the type info
    let mut synthetic = SyntheticTypeRegistry::new();
    let transformer = Transformer::for_module(&tctx, &mut synthetic);

    // Extract the CondExpr from the AST
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let return_stmt = &fn_decl.function.body.as_ref().unwrap().stmts[0];
    let cond_expr = match return_stmt {
        Stmt::Return(ret) => match ret.arg.as_deref() {
            Some(ast::Expr::Cond(cond)) => cond,
            _ => panic!("expected cond expr in return"),
        },
        _ => panic!("expected return stmt"),
    };

    // Extract narrowing guard from the condition
    let guard = patterns::extract_narrowing_guard(&cond_expr.test)
        .expect("should extract NonNullish guard from x !== null");

    // resolve_if_let_pattern should work via FileTypeResolution
    let result = transformer.resolve_if_let_pattern(&guard);
    // is_swap=false because `!==` means is_neq=true, and !is_neq=false (no swap needed:
    // then-branch is the "matched" branch, else-branch is the fallback)
    assert_eq!(
        result,
        Some(("Some(x)".to_string(), false)),
        "should resolve NonNullish guard on Option<String> to Some(x) pattern"
    );
}
