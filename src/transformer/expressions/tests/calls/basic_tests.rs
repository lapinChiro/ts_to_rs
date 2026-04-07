use super::super::*;
use crate::ir::CallTarget;

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
            target: CallTarget::Free("foo".to_string()),
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
            target: CallTarget::Free("foo".to_string()),
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
            target: CallTarget::Free("foo".to_string()),
            args: vec![Expr::FnCall {
                target: CallTarget::Free("bar".to_string()),
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
            target: CallTarget::UserAssocFn {
                ty: crate::ir::UserTypeRef::new("Foo"),
                method: "new".to_string()
            },
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
            target: CallTarget::UserAssocFn {
                ty: crate::ir::UserTypeRef::new("Foo"),
                method: "new".to_string()
            },
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
            assert!(matches!(
                target,
                CallTarget::UserAssocFn { ty, method }
                    if ty.as_str() == "Foo" && method == "new"
            ));
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

// I-378 T9 回帰テスト群: `Type.method()` static call の構造化分類は、
// console / Math / Number / fs の特殊ハンドラの**後**に実行されなければ
// ならない。それらビルトインは synthetic registry 内で TypeDef::Struct
// として登録されているため、順序を間違えると `Math.sign(x)` が
// `Math::sign(x)` に誤分類され、`x.signum()` への変換が失われる。
// E2E テストでも検出されるが、回帰の根本原因を局所化するため
// 単体テストレベルでもガードする。

#[test]
fn t9_regression_math_call_must_lower_to_method_call_not_user_assoc_fn() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("Math.sign(2);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Math.sign(x) → x.signum() (MethodCall on receiver)
    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "signum");
        }
        Expr::FnCall {
            target: CallTarget::UserAssocFn { ty, method },
            ..
        } => panic!(
            "REGRESSION: T9 ordering broken. Math.sign was misclassified as \
             UserAssocFn {{ ty: {:?}, method: {} }}. The static-method-call \
             classification must run AFTER the Math/Number/fs/console handlers.",
            ty.as_str(),
            method
        ),
        other => panic!("expected x.signum() MethodCall, got {other:?}"),
    }
}

#[test]
fn t9_regression_number_isnan_must_lower_to_method_call_not_user_assoc_fn() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("Number.isNaN(0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::MethodCall { method, .. } => assert_eq!(method, "is_nan"),
        Expr::FnCall {
            target: CallTarget::UserAssocFn { .. },
            ..
        } => panic!(
            "REGRESSION: T9 ordering broken. Number.isNaN was misclassified \
             as UserAssocFn instead of producing x.is_nan()."
        ),
        other => panic!("expected x.is_nan() MethodCall, got {other:?}"),
    }
}
