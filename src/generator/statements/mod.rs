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
                match init {
                    Expr::Match { expr, arms } => {
                        out.push_str(&format!(
                            " = {}",
                            super::expressions::generate_match_expr(expr, arms, indent)
                        ));
                    }
                    _ => out.push_str(&format!(" = {}", generate_expr(init))),
                }
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
        Stmt::WhileLet {
            label,
            pattern,
            expr,
            body,
        } => {
            let label_prefix = label
                .as_ref()
                .map(|l| format!("'{l}: "))
                .unwrap_or_default();
            let pat_str = crate::generator::patterns::render_pattern(pattern);
            let mut out = format!(
                "{pad}{label_prefix}while let {pat_str} = {} {{\n",
                generate_expr(expr)
            );
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
            Some(Expr::Match { expr: target, arms }) => format!(
                "{pad}return {};",
                super::expressions::generate_match_expr(target, arms, indent)
            ),
            Some(e) => format!("{pad}return {};", generate_expr(e)),
            None => format!("{pad}return;"),
        },
        Stmt::Expr(expr) => {
            format!("{pad}{};", generate_expr(expr))
        }
        Stmt::TailExpr(expr) => match expr {
            Expr::Match { expr: target, arms } => {
                format!(
                    "{pad}{}",
                    super::expressions::generate_match_expr(target, arms, indent)
                )
            }
            _ => format!("{pad}{}", generate_expr(expr)),
        },
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            let pat_str = crate::generator::patterns::render_pattern(pattern);
            let mut out = format!("{pad}if let {pat_str} = {} {{\n", generate_expr(expr));
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
            // Delegates to the shared match expression generator with correct indent.
            // The discriminant is rendered as-is. If `.as_str()` is needed (e.g., for
            // string pattern matching), the Transformer must have already wrapped the
            // expression in `Expr::MethodCall { method: "as_str", .. }`.
            format!(
                "{pad}{}",
                super::expressions::generate_match_expr(expr, arms, indent)
            )
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
mod tests;
