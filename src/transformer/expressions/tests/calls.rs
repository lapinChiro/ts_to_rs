use super::*;
use crate::ir::CallTarget;
use crate::ir::{ClosureBody, Param};

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
            target: CallTarget::simple("foo"),
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
            target: CallTarget::simple("foo"),
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
            target: CallTarget::simple("foo"),
            args: vec![Expr::FnCall {
                target: CallTarget::simple("bar"),
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
            target: CallTarget::assoc("Foo", "new"),
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
            target: CallTarget::assoc("Foo", "new"),
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
            vec![("name".to_string(), RustType::String).into()],
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
        Expr::FnCall { target, args } => {
            assert!(target.is_path(&["Foo", "new"]));
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

/// Helper: build the expected IR for Option<T> unwrapping in console.log.
/// Generates: `expr.as_ref().map_or("undefined".to_string(), |v| v.to_string())`
fn option_display_unwrap(expr: Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(Expr::MethodCall {
            object: Box::new(expr),
            method: "as_ref".to_string(),
            args: vec![],
        }),
        method: "map_or".to_string(),
        args: vec![
            Expr::MethodCall {
                object: Box::new(Expr::StringLit("undefined".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            },
            Expr::Closure {
                params: vec![Param {
                    name: "v".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("v".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                })),
            },
        ],
    }
}

#[test]
fn test_console_log_option_f64_unwraps_for_display() {
    let source = r#"
        function f() {
            let x: number | undefined = 5;
            console.log(x);
        }
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![option_display_unwrap(Expr::Ident("x".to_string()))],
            use_debug: vec![false],
        }
    );
}

#[test]
fn test_console_log_non_option_f64_unchanged() {
    let source = r#"
        function f() {
            let x: number = 5;
            console.log(x);
        }
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 1);
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
fn test_console_log_option_vec_debug_unwraps_with_format() {
    let source = r#"
        function f() {
            let arr: number[] | undefined = [1, 2, 3];
            console.log(arr);
        }
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Option<Vec<f64>> should use format!("{:?}", v) for the inner value
    match &result {
        Expr::MacroCall {
            name,
            args,
            use_debug,
        } => {
            assert_eq!(name, "println");
            assert_eq!(
                use_debug,
                &vec![false],
                "Option wrapper produces String, not Debug"
            );
            // The arg should be as_ref().map_or(..., |v| format!("{:?}", v))
            match &args[0] {
                Expr::MethodCall {
                    object,
                    method,
                    args: map_args,
                } => {
                    assert_eq!(method, "map_or");
                    // object should be as_ref() call
                    assert!(
                        matches!(object.as_ref(), Expr::MethodCall { method, .. } if method == "as_ref"),
                        "should call as_ref() before map_or"
                    );
                    // Second arg (closure) should use format!("{:?}", v)
                    match &map_args[1] {
                        Expr::Closure {
                            body: ClosureBody::Expr(body),
                            ..
                        } => {
                            assert!(
                                matches!(body.as_ref(), Expr::FormatMacro { template, .. } if template == "{:?}"),
                                "Debug inner type should use format!(\"{{:?}}\", v), got: {body:?}"
                            );
                        }
                        other => panic!("expected closure, got: {other:?}"),
                    }
                }
                other => panic!("expected MethodCall, got: {other:?}"),
            }
        }
        other => panic!("expected MacroCall, got: {other:?}"),
    }
}

#[test]
fn test_console_log_mixed_option_and_non_option_args() {
    let source = r#"
        function f() {
            let x: number | undefined = 5;
            let y: number = 10;
            console.log("x:", x, "y:", y);
        }
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 2);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::MacroCall {
            args, use_debug, ..
        } => {
            assert_eq!(args.len(), 4, "should have 4 args");
            assert_eq!(use_debug, &vec![false, false, false, false]);
            // args[0] = "x:" (string literal, unchanged)
            assert!(matches!(&args[0], Expr::StringLit(s) if s == "x:"));
            // args[1] = option-wrapped x
            assert!(
                matches!(&args[1], Expr::MethodCall { method, .. } if method == "map_or"),
                "Option arg should be wrapped with map_or"
            );
            // args[2] = "y:" (string literal, unchanged)
            assert!(matches!(&args[2], Expr::StringLit(s) if s == "y:"));
            // args[3] = y (plain ident, unchanged)
            assert!(matches!(&args[3], Expr::Ident(s) if s == "y"));
        }
        other => panic!("expected MacroCall, got: {other:?}"),
    }
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("greet"));
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("greet"));
            assert_eq!(args.len(), 2);
            // Second arg should be Some(...)
            assert!(
                matches!(&args[1], Expr::FnCall { target, args: inner } if target.as_simple() == Some("Some") && inner.len() == 1),
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
        mut_method_names: std::collections::HashSet::new(),
    }
    .convert_expr(&swc_expr)
    .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("f"));
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
        mut_method_names: std::collections::HashSet::new(),
    }
    .convert_expr(&swc_expr)
    .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("f"));
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
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("sum"));
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
            type_params: vec![],
            params: vec![
                ("prefix".to_string(), RustType::String).into(),
                ("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into(),
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("log"));
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
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("sum"));
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
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("sum"));
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
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("sum"));
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
            params: vec![("name".to_string(), RustType::String).into()],
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
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("foo"));
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
            params: vec![("name".to_string(), RustType::String).into()],
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

// ---------------------------------------------------------------------------
// I-375: `convert_call_expr` / `convert_new_expr` `type_ref` classification
// ---------------------------------------------------------------------------

/// `foo(x)` where `foo` is a plain function registered in `TypeRegistry`
/// must classify as `CallTarget::Path { type_ref: None }`. The walker will
/// then skip it — free functions are not type references.
#[test]
fn test_convert_expr_call_ident_function_in_registry_gets_type_ref_none() {
    use crate::registry::{ParamDef, TypeDef};
    let mut reg = TypeRegistry::new();
    reg.register(
        "foo".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![ParamDef {
                name: "x".to_string(),
                ty: RustType::F64,
                optional: false,
                has_default: false,
            }],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg("foo(1);", reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert_eq!(target.as_simple(), Some("foo"));
            assert!(
                matches!(target, CallTarget::Path { type_ref: None, .. }),
                "function call target must have type_ref: None, got {target:?}"
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// When an Ident callee name happens to be registered as a struct/class (a
/// degenerate TS situation — e.g. a callable interface), the Transformer must
/// still classify it as a type reference so the walker can wire the graph.
#[test]
fn test_convert_expr_call_ident_struct_in_registry_gets_type_ref_some() {
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Callable".to_string(),
        TypeDef::new_struct(vec![], std::collections::HashMap::new(), vec![]),
    );
    let f = TctxFixture::from_source_with_reg("Callable(1);", reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert!(
                matches!(
                    target,
                    CallTarget::Path { type_ref: Some(t), .. } if t == "Callable"
                ),
                "struct-typed Ident callee must record type_ref: Some(name), got {target:?}"
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// An unknown identifier (not registered anywhere) must default to
/// `type_ref: None`. This is the common case for imported functions or
/// identifiers whose types the resolver couldn't determine.
#[test]
fn test_convert_expr_call_ident_unknown_name_gets_type_ref_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("unknownFn(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert!(matches!(target, CallTarget::Path { type_ref: None, .. }));
            assert_eq!(target.as_simple(), Some("unknownFn"));
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// `new Foo(x)` must become `CallTarget::Path { segments: ["Foo", "new"],
/// type_ref: Some("Foo") }`. The `type_ref` is critical for the walker to
/// register `Foo` as a referenced type, and is what enables lowercase class
/// names to survive (I-375 correctness fix).
#[test]
fn test_convert_new_expr_sets_type_ref_to_class_name() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new Foo(1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert_eq!(
                target,
                &CallTarget::assoc("Foo", "new"),
                "new Foo(...) must produce assoc(\"Foo\", \"new\") with type_ref Some(\"Foo\")"
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// The lowercase-class correctness scenario, verified directly at the
/// Transformer layer: `new myClass(...)` must still record
/// `type_ref: Some("myClass")` even though `myClass` does not follow Rust
/// PascalCase convention. This proves the structural fix is independent of
/// naming conventions.
#[test]
fn test_convert_new_expr_lowercase_class_records_type_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new myClass(1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => match target {
            CallTarget::Path {
                segments,
                type_ref: Some(t),
            } => {
                assert_eq!(segments, &vec!["myClass".to_string(), "new".to_string()]);
                assert_eq!(t, "myClass");
            }
            _ => panic!("expected Path with type_ref Some, got {target:?}"),
        },
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// `super(args)` in a class constructor context must produce
/// `CallTarget::Super`, never a `Path { segments: ["super"] }`.
#[test]
fn test_convert_callee_super_produces_super_variant() {
    // We parse a class constructor body so that the swc parser accepts super().
    let source = r#"class Child extends Parent { constructor(x: number) { super(x); } }"#;
    let fx = TctxFixture::from_source_with_reg(source, TypeRegistry::new());
    let tctx = fx.tctx();
    // Walk the module to find the super call inside the constructor body.
    let module = fx.module();
    let mut found = false;
    for item in &module.body {
        if let swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
            swc_ecma_ast::Decl::Class(class_decl),
        )) = item
        {
            for member in &class_decl.class.body {
                if let swc_ecma_ast::ClassMember::Constructor(ctor) = member {
                    if let Some(body) = &ctor.body {
                        for stmt in &body.stmts {
                            if let swc_ecma_ast::Stmt::Expr(expr_stmt) = stmt {
                                if let swc_ecma_ast::Expr::Call(call) = expr_stmt.expr.as_ref() {
                                    let result = Transformer::for_module(
                                        &tctx,
                                        &mut SyntheticTypeRegistry::new(),
                                    )
                                    .convert_call_expr(call)
                                    .unwrap();
                                    assert!(
                                        matches!(
                                            result,
                                            Expr::FnCall {
                                                target: CallTarget::Super,
                                                ..
                                            }
                                        ),
                                        "expected CallTarget::Super, got {result:?}"
                                    );
                                    found = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(found, "super() call not found in the parsed module");
}
