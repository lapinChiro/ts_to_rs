//! Statement generation: converts IR statements into Rust source strings.

use crate::ir::Stmt;

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
        Stmt::Break { label, value } => match (label, value) {
            (Some(l), Some(v)) => format!("{pad}break '{l} {};", generate_expr(v)),
            (Some(l), None) => format!("{pad}break '{l};"),
            (None, None) => format!("{pad}break;"),
            (None, Some(v)) => format!("{pad}break {};", generate_expr(v)),
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
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            let mut out = format!("{pad}if let {pattern} = {} {{\n", generate_expr(expr));
            for s in then_body {
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
        Stmt::Match { expr, arms } => {
            use crate::ir::{Expr as IrExpr, MatchPattern};

            // Detect if any arm has a string literal pattern → need .as_str()
            let has_string_patterns = arms.iter().any(|arm| {
                arm.patterns
                    .iter()
                    .any(|p| matches!(p, MatchPattern::Literal(IrExpr::StringLit(_))))
            });

            let discriminant_str = if has_string_patterns {
                format!("{}.as_str()", generate_expr(expr))
            } else {
                generate_expr(expr)
            };

            let mut out = format!("{pad}match {discriminant_str} {{\n");
            for arm in arms {
                let patterns_str = arm
                    .patterns
                    .iter()
                    .map(|p| match p {
                        MatchPattern::Literal(e) => generate_expr(e),
                        MatchPattern::Wildcard => "_".to_string(),
                        MatchPattern::EnumVariant { path } => {
                            format!("{path} {{ .. }}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                let guard_str = arm
                    .guard
                    .as_ref()
                    .map(|g| format!(" if {}", generate_expr(g)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{}{}{} => {{\n",
                    indent_str(indent + 1),
                    patterns_str,
                    guard_str
                ));
                for s in &arm.body {
                    out.push_str(&generate_stmt(s, indent + 2));
                    out.push('\n');
                }
                out.push_str(&format!("{}}}\n", indent_str(indent + 1)));
            }
            out.push_str(&format!("{pad}}}"));
            out
        }
        Stmt::LabeledBlock { label, body } => {
            let mut out = format!("{pad}'{label}: {{\n");
            for s in body {
                out.push_str(&generate_stmt(s, indent + 1));
                out.push('\n');
            }
            out.push_str(&format!("{pad}}}"));
            out
        }
    }
}

/// Returns the indentation string for the given level (4 spaces per level).
fn indent_str(level: usize) -> String {
    "    ".repeat(level)
}

#[cfg(test)]
mod tests {
    use crate::generator::generate;
    use crate::ir::{Expr, Item, MatchPattern as MP, RustType, Stmt, Visibility};

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
                body: vec![Stmt::Break {
                    label: None,
                    value: None,
                }],
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
    fn test_generate_labeled_block_simple_body_outputs_labeled_block() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::LabeledBlock {
                label: "try_block".to_string(),
                body: vec![Stmt::Expr(Expr::FnCall {
                    name: "do_something".to_string(),
                    args: vec![],
                })],
            }],
        };
        let expected = "\
fn f() {
    'try_block: {
        do_something();
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_break_with_label_and_value_outputs_break_label_value() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Break {
                label: Some("try_block".to_string()),
                value: Some(Expr::FnCall {
                    name: "Err".to_string(),
                    args: vec![Expr::MethodCall {
                        object: Box::new(Expr::StringLit("error".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    }],
                }),
            }],
        };
        let expected = "\
fn f() {
    break 'try_block Err(\"error\".to_string());
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_break_label_only_no_value_outputs_break_label() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Break {
                label: Some("outer".to_string()),
                value: None,
            }],
        };
        let expected = "\
fn f() {
    break 'outer;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_break_no_label_no_value_outputs_break() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Break {
                label: None,
                value: None,
            }],
        };
        let expected = "\
fn f() {
    break;
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

    #[test]
    fn test_generate_stmt_if_let_without_else_renders_if_let() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::IfLet {
                pattern: "Err(e)".to_string(),
                expr: Expr::Ident("result".to_string()),
                then_body: vec![Stmt::Expr(Expr::MethodCall {
                    object: Box::new(Expr::Ident("e".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                })],
                else_body: None,
            }],
        };
        let expected = "\
fn f() {
    if let Err(e) = result {
        e.to_string();
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_stmt_if_let_with_else_renders_else_branch() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::IfLet {
                pattern: "Some(x)".to_string(),
                expr: Expr::Ident("opt".to_string()),
                then_body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
                else_body: Some(vec![Stmt::Return(None)]),
            }],
        };
        let expected = "\
fn f() {
    if let Some(x) = opt {
        x;
    } else {
        return;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_match_single_arm_renders_match() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Match {
                expr: Expr::Ident("x".to_string()),
                arms: vec![crate::ir::MatchArm {
                    patterns: vec![MP::Literal(Expr::IntLit(1))],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_a".to_string(),
                        args: vec![],
                    })],
                }],
            }],
        };
        let expected = "\
fn f() {
    match x {
        1 => {
            do_a();
        }
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_match_multiple_patterns_renders_or() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Match {
                expr: Expr::Ident("x".to_string()),
                arms: vec![crate::ir::MatchArm {
                    patterns: vec![MP::Literal(Expr::IntLit(1)), MP::Literal(Expr::IntLit(2))],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_ab".to_string(),
                        args: vec![],
                    })],
                }],
            }],
        };
        let expected = "\
fn f() {
    match x {
        1 | 2 => {
            do_ab();
        }
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_match_wildcard_renders_underscore() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Match {
                expr: Expr::Ident("x".to_string()),
                arms: vec![crate::ir::MatchArm {
                    patterns: vec![MP::Wildcard],
                    guard: None,
                    body: vec![Stmt::Expr(Expr::FnCall {
                        name: "do_default".to_string(),
                        args: vec![],
                    })],
                }],
            }],
        };
        let expected = "\
fn f() {
    match x {
        _ => {
            do_default();
        }
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_match_multiple_arms_renders_all() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Match {
                expr: Expr::Ident("x".to_string()),
                arms: vec![
                    crate::ir::MatchArm {
                        patterns: vec![MP::Literal(Expr::IntLit(1))],
                        guard: None,
                        body: vec![Stmt::Expr(Expr::FnCall {
                            name: "do_a".to_string(),
                            args: vec![],
                        })],
                    },
                    crate::ir::MatchArm {
                        patterns: vec![MP::Literal(Expr::IntLit(2)), MP::Literal(Expr::IntLit(3))],
                        guard: None,
                        body: vec![Stmt::Expr(Expr::FnCall {
                            name: "do_bc".to_string(),
                            args: vec![],
                        })],
                    },
                    crate::ir::MatchArm {
                        patterns: vec![MP::Wildcard],
                        guard: None,
                        body: vec![Stmt::Expr(Expr::FnCall {
                            name: "do_default".to_string(),
                            args: vec![],
                        })],
                    },
                ],
            }],
        };
        let expected = "\
fn f() {
    match x {
        1 => {
            do_a();
        }
        2 | 3 => {
            do_bc();
        }
        _ => {
            do_default();
        }
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_match_string_patterns_adds_as_str() {
        let item = Item::Fn {
            vis: Visibility::Private,
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Match {
                expr: Expr::Ident("s".to_string()),
                arms: vec![
                    crate::ir::MatchArm {
                        patterns: vec![MP::Literal(Expr::StringLit("hello".to_string()))],
                        guard: None,
                        body: vec![Stmt::Expr(Expr::FnCall {
                            name: "do_hello".to_string(),
                            args: vec![],
                        })],
                    },
                    crate::ir::MatchArm {
                        patterns: vec![MP::Wildcard],
                        guard: None,
                        body: vec![Stmt::Expr(Expr::FnCall {
                            name: "do_default".to_string(),
                            args: vec![],
                        })],
                    },
                ],
            }],
        };
        let expected = "\
fn f() {
    match s.as_str() {
        \"hello\" => {
            do_hello();
        }
        _ => {
            do_default();
        }
    }
}";
        assert_eq!(generate(&[item]), expected);
    }
}
