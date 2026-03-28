use super::*;

#[test]
fn test_convert_expr_call_simple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo(x, y);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "foo".to_string(),
            args: vec![Expr::Ident("x".to_string()), Expr::Ident("y".to_string()),],
        }
    );
}

#[test]
fn test_convert_expr_call_no_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "foo".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_call_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo(bar(x));");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "foo".to_string(),
            args: vec![Expr::FnCall {
                name: "bar".to_string(),
                args: vec![Expr::Ident("x".to_string())],
            }],
        }
    );
}

#[test]
fn test_convert_expr_method_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("obj.method(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("obj".to_string())),
            method: "method".to_string(),
            args: vec![Expr::Ident("x".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_method_call_this() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("this.doSomething(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("self".to_string())),
            method: "doSomething".to_string(),
            args: vec![Expr::Ident("x".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_method_chain() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("a.b().c();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("a".to_string())),
                method: "b".to_string(),
                args: vec![],
            }),
            method: "c".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_new() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new Foo(x, y);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Foo::new".to_string(),
            args: vec![Expr::Ident("x".to_string()), Expr::Ident("y".to_string()),],
        }
    );
}

#[test]
fn test_convert_expr_new_no_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new Foo();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Foo::new".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_new_expr_string_arg_gets_to_string() {
    // new Foo("hello") with Foo { name: String } → Foo::new("hello".to_string())
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let source = r#"new Foo("hello");"#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "Foo::new");
            assert!(
                matches!(&args[0], Expr::MethodCall { method, .. } if method == "to_string"),
                "expected .to_string() on string arg, got {:?}",
                args[0]
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

#[test]
fn test_convert_expr_console_log_single_arg() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.log(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Ident("x".to_string())],
            use_debug: vec![false],
        }
    );
}

#[test]
fn test_convert_expr_console_error() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.error(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "eprintln".to_string(),
            args: vec![Expr::Ident("x".to_string())],
            use_debug: vec![false],
        }
    );
}

#[test]
fn test_convert_expr_console_warn() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.warn(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "eprintln".to_string(),
            args: vec![Expr::Ident("x".to_string())],
            use_debug: vec![false],
        }
    );
}

#[test]
fn test_convert_expr_console_log_no_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.log();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![],
            use_debug: vec![],
        }
    );
}

#[test]
fn test_convert_expr_console_log_multiple_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.log(x, y);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Ident("x".to_string()), Expr::Ident("y".to_string()),],
            use_debug: vec![false, false],
        }
    );
}

#[test]
fn test_call_with_missing_default_arg_appends_none() {
    // greet("World") where greet has params: (name: String, greeting: Option<String>)
    // Should produce: greet("World".to_string(), None)
    let reg = greet_registry();
    let f = TctxFixture::from_source_with_reg(r#"greet("World");"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "greet");
            assert_eq!(
                args.len(),
                2,
                "expected 2 args (with None appended), got {args:?}"
            );
            // Second arg should be None (Ident("None"))
            assert_eq!(args[1], Expr::Ident("None".to_string()));
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_call_with_option_arg_wraps_some() {
    // greet("World", "Hi") where greeting is Option<String>
    // Should produce: greet("World".to_string(), Some("Hi".to_string()))
    let reg = greet_registry();
    let f = TctxFixture::from_source_with_reg(r#"greet("World", "Hi");"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "greet");
            assert_eq!(args.len(), 2);
            // Second arg should be Some(...)
            assert!(
                matches!(&args[1], Expr::FnCall { name, args: inner } if name == "Some" && inner.len() == 1),
                "expected Some(...), got: {:?}",
                args[1]
            );
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_typeenv_fn_provides_param_expected() {
    // f("hello") where f: (s: string) => boolean is declared
    // → "hello" should become "hello".to_string() because expected=String
    let source = r#"
        function f(s: string): boolean { return true; }
        f("hello");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();

    let swc_expr = extract_expr_stmt(f.module(), 1);
    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
    }
    .convert_expr(&swc_expr)
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "f");
            assert_eq!(args.len(), 1);
            // The string literal should have .to_string() because param type is String
            assert!(
                matches!(
                    &args[0],
                    Expr::MethodCall { method, .. } if method == "to_string"
                ),
                "arg should be .to_string() call, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {:?}", result),
    }
}

#[test]
fn test_convert_call_expr_no_typeenv_fn_no_expected() {
    // f("hello") where type info is unavailable → "hello" stays as StringLit (no .to_string())
    let swc_expr = parse_expr("f(\"hello\");");
    let f = TctxFixture::new();
    let tctx = f.tctx();

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
    }
    .convert_expr(&swc_expr)
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "f");
            assert_eq!(args.len(), 1);
            assert!(
                matches!(&args[0], Expr::StringLit(s) if s == "hello"),
                "arg should be plain StringLit, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {:?}", result),
    }
}

#[test]
fn test_convert_call_expr_rest_param_packs_args_into_vec() {
    // sum(1, 2, 3) where sum(...nums: number[]) → sum(vec![1.0, 2.0, 3.0])
    let swc_expr = parse_expr("sum(1, 2, 3);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1, "all args should be packed into one vec");
            match &args[0] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 3);
                }
                other => panic!("expected Vec, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_mixed_regular_and_rest() {
    // log("hello", 1, 2) where log(prefix: string, ...nums: number[])
    // → log("hello".to_string(), vec![1.0, 2.0])
    let swc_expr = parse_expr(r#"log("hello", 1, 2);"#);
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "log".to_string(),
        TypeDef::Function {
            params: vec![
                ("prefix".to_string(), RustType::String),
                ("nums".to_string(), RustType::Vec(Box::new(RustType::F64))),
            ],
            return_type: None,
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "log");
            assert_eq!(args.len(), 2, "prefix + packed rest");
            match &args[1] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 2);
                }
                other => panic!("expected Vec for rest args, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_no_rest_args() {
    // sum() where sum(...nums: number[]) → sum(vec![])
    let swc_expr = parse_expr("sum();");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 0, "no rest args → empty vec");
                }
                other => panic!("expected empty Vec, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_spread_single_array() {
    // sum(...arr) where sum(...nums: number[]) → sum(arr)
    let swc_expr = parse_expr("sum(...arr);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1);
            // Should pass arr directly, not wrap in vec!
            assert!(
                matches!(&args[0], Expr::Ident(name) if name == "arr"),
                "spread arg should be passed directly, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_mixed_literal_and_spread() {
    // sum(1, ...arr) where sum(...nums: number[]) → sum([vec![1.0], arr].concat())
    let swc_expr = parse_expr("sum(1, ...arr);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1);
            // Should be [vec![1.0], arr].concat()
            match &args[0] {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method, "concat");
                }
                other => panic!("expected MethodCall(.concat()), got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_method_call_string_arg_gets_to_string_with_registry() {
    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("name".to_string(), RustType::String)],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_struct(vec![], methods, vec![]),
    );

    let source = r#"
        const g: Greeter = new Greeter();
        g.greet("world");
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Should have .to_string() on the string arg
    if let Expr::MethodCall { args, .. } = &result {
        assert!(
            matches!(
                &args[0],
                Expr::MethodCall { method, .. } if method == "to_string"
            ),
            "expected .to_string() on method arg, got: {:?}",
            args[0]
        );
    } else {
        panic!("expected MethodCall, got: {result:?}");
    }
}

#[test]
fn test_convert_call_expr_paren_ident_unwraps_to_fn_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (foo)(1) → foo(1.0)
    let expr = parse_expr("(foo)(1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_paren_member_unwraps_to_method_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (obj.method)() → obj.method()
    let expr = parse_expr("(obj.method)();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "method");
        }
        other => panic!("expected MethodCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_chained_call_does_not_error() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // f(x)(y) — chained call should not error
    let expr = parse_expr("f(1)(2);");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&expr);
    assert!(
        result.is_ok(),
        "chained call should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_opt_chain_method_call_propagates_param_types() {
    use crate::registry::TypeDef;

    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("name".to_string(), RustType::String)],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_struct(vec![], methods, vec![]),
    );

    let source = r#"
        const obj: Greeter | undefined = undefined;
        obj?.greet("hello");
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Expected: obj.as_ref().map(|_v| _v.greet("hello".to_string()))
    // The "hello" arg should have .to_string() because greet's param type is String
    match &result {
        Expr::MethodCall { method, args, .. } if method == "map" => {
            // The closure body should contain a method call with to_string on the arg
            match &args[0] {
                Expr::Closure { body, .. } => match body {
                    ClosureBody::Expr(expr) => match expr.as_ref() {
                        Expr::MethodCall {
                            args: inner_args, ..
                        } => {
                            assert!(
                                matches!(&inner_args[0], Expr::MethodCall { method, .. } if method == "to_string"),
                                "expected .to_string() on string arg, got {:?}",
                                inner_args[0]
                            );
                        }
                        other => panic!("expected MethodCall inside closure, got {other:?}"),
                    },
                    _ => panic!("expected ClosureBody::Expr"),
                },
                other => panic!("expected Closure, got {other:?}"),
            }
        }
        other => panic!("expected MethodCall(map), got {other:?}"),
    }
}
