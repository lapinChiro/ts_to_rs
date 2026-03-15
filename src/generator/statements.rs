//! Statement generation: converts IR statements into Rust source strings.

use crate::ir::{Expr, Stmt};

use super::expressions::{escape_ident, generate_expr};
use super::types::generate_type;

/// Generates a statement with the given indentation level.
pub(super) fn generate_stmt(stmt: &Stmt, indent: usize) -> String {
    let pad = indent_str(indent);
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            let mut out = format!("{pad}let ");
            if *mutable {
                out.push_str("mut ");
            }
            out.push_str(&escape_ident(name));
            if let Some(ty) = ty {
                out.push_str(&format!(": {}", generate_type(ty)));
            }
            if let Some(init) = init {
                out.push_str(&format!(" = {}", generate_expr(init)));
            }
            out.push(';');
            out
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            let mut out = format!("{pad}if {} {{\n", generate_expr(condition));
            for s in then_body {
                // Statements inside if are not tail-position of a function
                out.push_str(&generate_stmt(s, indent + 1));
                out.push('\n');
            }
            match else_body {
                Some(stmts) => {
                    out.push_str(&format!("{pad}}} else {{\n"));
                    for s in stmts {
                        out.push_str(&generate_stmt(s, indent + 1));
                        out.push('\n');
                    }
                    out.push_str(&format!("{pad}}}"));
                }
                None => {
                    out.push_str(&format!("{pad}}}"));
                }
            }
            out
        }
        Stmt::While {
            label,
            condition,
            body,
        } => {
            let label_prefix = label
                .as_ref()
                .map(|l| format!("'{l}: "))
                .unwrap_or_default();
            let mut out = format!("{pad}{label_prefix}while {} {{\n", generate_expr(condition));
            for s in body {
                out.push_str(&generate_stmt(s, indent + 1));
                out.push('\n');
            }
            out.push_str(&format!("{pad}}}"));
            out
        }
        Stmt::ForIn {
            label,
            var,
            iterable,
            body,
        } => {
            let label_prefix = label
                .as_ref()
                .map(|l| format!("'{l}: "))
                .unwrap_or_default();
            let mut out = format!(
                "{pad}{label_prefix}for {var} in {} {{\n",
                generate_expr(iterable)
            );
            for s in body {
                out.push_str(&generate_stmt(s, indent + 1));
                out.push('\n');
            }
            out.push_str(&format!("{pad}}}"));
            out
        }
        Stmt::Loop { label, body } => {
            let label_prefix = label
                .as_ref()
                .map(|l| format!("'{l}: "))
                .unwrap_or_default();
            let mut out = format!("{pad}{label_prefix}loop {{\n");
            for s in body {
                out.push_str(&generate_stmt(s, indent + 1));
                out.push('\n');
            }
            out.push_str(&format!("{pad}}}"));
            out
        }
        Stmt::Break { label } => match label {
            Some(l) => format!("{pad}break '{l};"),
            None => format!("{pad}break;"),
        },
        Stmt::Continue { label } => match label {
            Some(l) => format!("{pad}continue '{l};"),
            None => format!("{pad}continue;"),
        },
        Stmt::Return(expr) => match expr {
            Some(e) => format!("{pad}return {};", generate_expr(e)),
            None => format!("{pad}return;"),
        },
        Stmt::Expr(expr) => {
            format!("{pad}{};", generate_expr(expr))
        }
        Stmt::TailExpr(expr) => {
            format!("{pad}{}", generate_expr(expr))
        }
        Stmt::TryCatch {
            try_body,
            catch_param,
            catch_body,
            finally_body,
        } => {
            let mut lines = Vec::new();
            let inner_pad = "    ".repeat(indent + 1);

            // Emit scopeguard for finally block
            if let Some(finally_stmts) = finally_body {
                lines.push(format!(
                    "{pad}let _finally_guard = scopeguard::guard((), |_| {{"
                ));
                for s in finally_stmts {
                    lines.push(generate_stmt(s, indent + 1));
                }
                lines.push(format!("{pad}}});"));
            }

            // Emit try/catch as labeled block + if let Err
            if let Some(catch_stmts) = catch_body {
                let param_name = catch_param.as_deref().unwrap_or("_e");
                lines.push(format!(
                    "{pad}let _try_result: Result<(), String> = 'try_block: {{"
                ));
                for s in try_body {
                    lines.push(generate_try_body_stmt(s, indent + 1));
                }
                lines.push(format!("{inner_pad}Ok(())"));
                lines.push(format!("{pad}}};"));
                lines.push(format!("{pad}if let Err({param_name}) = _try_result {{"));
                for s in catch_stmts {
                    lines.push(generate_stmt(s, indent + 1));
                }
                lines.push(format!("{pad}}}"));
            } else {
                // No catch block — just emit try body inline
                for s in try_body {
                    lines.push(generate_stmt(s, indent));
                }
            }

            lines.join("\n")
        }
    }
}

/// Generates a statement inside a try block's labeled block.
///
/// Rewrites `return Err(...)` (from throw conversion) to `break 'try_block Err(...)`,
/// so the error exits the labeled block rather than the enclosing function.
fn generate_try_body_stmt(stmt: &Stmt, indent: usize) -> String {
    let pad = indent_str(indent);
    match stmt {
        Stmt::Return(Some(expr)) if is_err_call(expr) => {
            format!("{pad}break 'try_block {};", generate_expr(expr))
        }
        // Recurse into nested blocks (if/else, loops, etc.)
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            let mut lines = Vec::new();
            lines.push(format!("{pad}if {} {{", generate_expr(condition)));
            for s in then_body {
                lines.push(generate_try_body_stmt(s, indent + 1));
            }
            if let Some(else_stmts) = else_body {
                lines.push(format!("{pad}}} else {{"));
                for s in else_stmts {
                    lines.push(generate_try_body_stmt(s, indent + 1));
                }
            }
            lines.push(format!("{pad}}}"));
            lines.join("\n")
        }
        other => generate_stmt(other, indent),
    }
}

/// Checks if an expression is an `Err(...)` call.
fn is_err_call(expr: &Expr) -> bool {
    matches!(expr, Expr::FnCall { name, .. } if name == "Err")
}

/// Returns the indentation string for the given level (4 spaces per level).
fn indent_str(level: usize) -> String {
    "    ".repeat(level)
}

#[cfg(test)]
mod tests {
    use crate::generator::generate;
    use crate::ir::{Expr, Item, RustType, Stmt, Visibility};

    // Statement tests need to be wrapped in Item::Fn to test generate()

    #[test]
    fn test_generate_let_simple() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(42.0)),
            }],
        };
        let expected = "\
fn f() {
    let x = 42.0;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_let_mut_with_type() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Let {
                mutable: true,
                name: "count".to_string(),
                ty: Some(RustType::F64),
                init: Some(Expr::NumberLit(0.0)),
            }],
        };
        let expected = "\
fn f() {
    let mut count: f64 = 0.0;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_let_no_init() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: Some(RustType::String),
                init: None,
            }],
        };
        let expected = "\
fn f() {
    let x: String;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_if_no_else() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::If {
                condition: Expr::BoolLit(true),
                then_body: vec![Stmt::Return(None)],
                else_body: None,
            }],
        };
        let expected = "\
fn f() {
    if true {
        return;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_if_with_else() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::If {
                condition: Expr::Ident("x".to_string()),
                then_body: vec![Stmt::Expr(Expr::Ident("a".to_string()))],
                else_body: Some(vec![Stmt::Expr(Expr::Ident("b".to_string()))]),
            }],
        };
        let expected = "\
fn f() {
    if x {
        a;
    } else {
        b;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_while() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::While {
                label: None,
                condition: Expr::BoolLit(true),
                body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
            }],
        };
        let expected = "\
fn f() {
    while true {
        x;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_for_in_range() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::ForIn {
                label: None,
                var: "i".to_string(),
                iterable: Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(0.0))),
                    end: Some(Box::new(Expr::Ident("n".to_string()))),
                },
                body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
            }],
        };
        let expected = "\
fn f() {
    for i in 0..n as i64 {
        x;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_for_in_iterable() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::ForIn {
                label: None,
                var: "item".to_string(),
                iterable: Expr::Ident("items".to_string()),
                body: vec![Stmt::Expr(Expr::Ident("item".to_string()))],
            }],
        };
        let expected = "\
fn f() {
    for item in items {
        item;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_loop_basic() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Loop {
                label: None,
                body: vec![Stmt::Break { label: None }],
            }],
        };
        let expected = "\
fn f() {
    loop {
        break;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_return_bare() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![
                Stmt::Expr(Expr::Ident("something".to_string())),
                Stmt::Return(None),
            ],
        };
        let expected = "\
fn f() {
    something;
    return;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_stmt_try_catch_generates_labeled_block() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::TryCatch {
                try_body: vec![Stmt::Expr(Expr::FnCall {
                    name: "do_something".to_string(),
                    args: vec![],
                })],
                catch_param: Some("e".to_string()),
                catch_body: Some(vec![Stmt::Expr(Expr::MacroCall {
                    name: "eprintln".to_string(),
                    args: vec![
                        Expr::StringLit("{}".to_string()),
                        Expr::Ident("e".to_string()),
                    ],
                })]),
                finally_body: None,
            }],
        };
        let output = generate(&[item]);
        assert!(
            output.contains("let _try_result: Result<(), String> = 'try_block: {"),
            "expected labeled block, got:\n{output}"
        );
        assert!(
            output.contains("do_something()"),
            "expected try body, got:\n{output}"
        );
        assert!(
            output.contains("if let Err(e) = _try_result {"),
            "expected if let Err, got:\n{output}"
        );
        assert!(
            output.contains("eprintln!"),
            "expected catch body, got:\n{output}"
        );
        assert!(
            !output.contains("scopeguard"),
            "should not contain scopeguard without finally, got:\n{output}"
        );
    }

    #[test]
    fn test_generate_stmt_try_finally_generates_scopeguard() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::TryCatch {
                try_body: vec![Stmt::Expr(Expr::FnCall {
                    name: "do_something".to_string(),
                    args: vec![],
                })],
                catch_param: None,
                catch_body: None,
                finally_body: Some(vec![Stmt::Expr(Expr::FnCall {
                    name: "cleanup".to_string(),
                    args: vec![],
                })]),
            }],
        };
        let output = generate(&[item]);
        assert!(
            output.contains("scopeguard::guard((), |_|"),
            "expected scopeguard, got:\n{output}"
        );
        assert!(
            output.contains("cleanup()"),
            "expected finally body, got:\n{output}"
        );
        assert!(
            output.contains("do_something()"),
            "expected try body, got:\n{output}"
        );
        assert!(
            !output.contains("match (||"),
            "should not contain match without catch, got:\n{output}"
        );
    }

    #[test]
    fn test_generate_stmt_try_catch_finally_generates_both() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::TryCatch {
                try_body: vec![Stmt::Expr(Expr::FnCall {
                    name: "do_something".to_string(),
                    args: vec![],
                })],
                catch_param: Some("e".to_string()),
                catch_body: Some(vec![Stmt::Expr(Expr::FnCall {
                    name: "handle_error".to_string(),
                    args: vec![Expr::Ident("e".to_string())],
                })]),
                finally_body: Some(vec![Stmt::Expr(Expr::FnCall {
                    name: "cleanup".to_string(),
                    args: vec![],
                })]),
            }],
        };
        let output = generate(&[item]);
        assert!(
            output.contains("scopeguard::guard((), |_|"),
            "expected scopeguard, got:\n{output}"
        );
        assert!(
            output.contains("let _try_result: Result<(), String> = 'try_block: {"),
            "expected labeled block, got:\n{output}"
        );
        assert!(
            output.contains("cleanup()"),
            "expected finally body in scopeguard, got:\n{output}"
        );
        assert!(
            output.contains("if let Err(e) = _try_result {"),
            "expected if let Err, got:\n{output}"
        );
    }

    #[test]
    fn test_generate_stmt_tail_expr_ident_outputs_without_semicolon() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            body: vec![Stmt::TailExpr(Expr::Ident("x".to_string()))],
        };
        let expected = "\
fn f() -> f64 {
    x
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_stmt_tail_expr_complex_expr_outputs_without_semicolon() {
        use crate::ir::BinOp;
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            body: vec![Stmt::TailExpr(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::Ident("b".to_string())),
            })],
        };
        let expected = "\
fn f() -> f64 {
    a + b
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_return_not_last_uses_return_keyword() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            body: vec![
                Stmt::Return(Some(Expr::NumberLit(1.0))),
                Stmt::TailExpr(Expr::NumberLit(2.0)),
            ],
        };
        let expected = "\
fn f() -> f64 {
    return 1.0;
    2.0
}";
        assert_eq!(generate(&[item]), expected);
    }
}
