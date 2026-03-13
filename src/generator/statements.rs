//! Statement generation: converts IR statements into Rust source strings.

use crate::ir::Stmt;

use super::expressions::generate_expr;
use super::types::generate_type;

/// Generates a statement with the given indentation level.
///
/// When `is_last_in_fn` is true and the statement is `Stmt::Return(Some(expr))`,
/// it emits just the expression (idiomatic Rust tail expression).
pub(super) fn generate_stmt(stmt: &Stmt, indent: usize, is_last_in_fn: bool) -> String {
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
            out.push_str(name);
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
                out.push_str(&generate_stmt(s, indent + 1, false));
                out.push('\n');
            }
            match else_body {
                Some(stmts) => {
                    out.push_str(&format!("{pad}}} else {{\n"));
                    for s in stmts {
                        out.push_str(&generate_stmt(s, indent + 1, false));
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
                out.push_str(&generate_stmt(s, indent + 1, false));
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
                out.push_str(&generate_stmt(s, indent + 1, false));
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
                out.push_str(&generate_stmt(s, indent + 1, false));
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
        Stmt::Return(expr) => {
            if is_last_in_fn {
                match expr {
                    Some(e) => format!("{pad}{}", generate_expr(e)),
                    None => format!("{pad}return;"),
                }
            } else {
                match expr {
                    Some(e) => format!("{pad}return {};", generate_expr(e)),
                    None => format!("{pad}return;"),
                }
            }
        }
        Stmt::Expr(expr) => {
            format!("{pad}{};", generate_expr(expr))
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
                    start: Box::new(Expr::NumberLit(0.0)),
                    end: Box::new(Expr::Ident("n".to_string())),
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
                Stmt::Return(Some(Expr::NumberLit(2.0))),
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
