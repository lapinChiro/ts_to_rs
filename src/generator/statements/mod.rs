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
            let mut out = format!(
                "{pad}{label_prefix}while let {pattern} = {} {{\n",
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
            use crate::ir::MatchPattern;

            // The discriminant is rendered as-is. If `.as_str()` is needed (e.g., for
            // string pattern matching), the Transformer must have already wrapped the
            // expression in `Expr::MethodCall { method: "as_str", .. }`.
            let discriminant_str = generate_expr(expr);

            let mut out = format!("{pad}match {discriminant_str} {{\n");
            for arm in arms {
                let patterns_str = arm
                    .patterns
                    .iter()
                    .map(|p| match p {
                        MatchPattern::Literal(e) => generate_expr(e),
                        MatchPattern::Wildcard => "_".to_string(),
                        MatchPattern::EnumVariant { path, bindings } => {
                            if bindings.is_empty() {
                                format!("{path} {{ .. }}")
                            } else {
                                let fields = bindings.join(", ");
                                format!("{path} {{ {fields}, .. }}")
                            }
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
mod tests;
