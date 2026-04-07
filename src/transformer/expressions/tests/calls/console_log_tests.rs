use super::super::*;
use crate::ir::{ClosureBody, Param};

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
