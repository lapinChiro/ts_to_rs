//! Helper functions for function body transformation.
//!
//! Contains utilities for:
//! - Return statement transformation (tail expressions, Ok-wrapping)
//! - Throw detection and Result type wrapping
//! - Mutating method detection and parameter rebinding
//! - Name case conversion

use super::*;

/// Converts a PascalCase name to snake_case.
///
/// Example: `"HonoOptions"` → `"hono_options"`, `"Point"` → `"point"`
pub(super) fn pascal_to_snake(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap_or(ch));
        } else {
            result.push(ch);
        }
    }
    result
}

/// Unwraps `Promise<T>` to `T` for async function return types.
///
/// If the type is `Named { name: "Promise", type_args: [T] }`, returns `Some(T)`.
/// Otherwise returns the type unchanged.
pub(super) fn unwrap_promise_type(ty: RustType) -> Option<RustType> {
    match ty {
        RustType::Named {
            ref name,
            ref type_args,
        } if name == "Promise" && type_args.len() == 1 => Some(type_args[0].clone()),
        other => Some(other),
    }
}

/// Converts the last `Stmt::Return(Some(expr))` in a function body to `Stmt::TailExpr(expr)`.
///
/// This enables idiomatic Rust tail expressions (implicit return without `return` keyword).
/// `Stmt::Return(None)` is not converted because `return;` cannot be a tail expression.
pub(crate) fn convert_last_return_to_tail(body: &mut Vec<Stmt>) {
    if let Some(Stmt::Return(Some(_))) = body.last() {
        if let Some(Stmt::Return(Some(expr))) = body.pop() {
            body.push(Stmt::TailExpr(expr));
        }
    }
}

/// Methods that require `&mut self` on the receiver.
const MUTATING_METHODS: &[&str] = &[
    "reverse", "sort", "sort_by", "drain", "push", "pop", "remove", "insert", "clear", "truncate",
    "retain",
];

/// Scans function body for method calls that require `&mut self` and inserts
/// `let mut name = name;` rebinding statements at the start of the body.
pub(super) fn mark_mut_params_from_body(body: &[Stmt], params: &[Param]) -> Vec<Stmt> {
    let mut needs_mut = std::collections::HashSet::new();
    collect_mut_receivers(body, &mut needs_mut);

    let mut rebindings = Vec::new();
    for param in params {
        if needs_mut.contains(&param.name) {
            rebindings.push(Stmt::Let {
                mutable: true,
                name: param.name.clone(),
                ty: None,
                init: Some(Expr::Ident(param.name.clone())),
            });
        }
    }
    rebindings
}

/// Recursively collects variable names that are receivers of mutating method calls.
///
/// Uses exhaustive pattern matching to ensure new `Stmt` variants are handled.
fn collect_mut_receivers(stmts: &[Stmt], receivers: &mut std::collections::HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::TailExpr(expr) => {
                collect_mut_receivers_from_expr(expr, receivers);
            }
            Stmt::Let {
                init: Some(expr), ..
            } => {
                collect_mut_receivers_from_expr(expr, receivers);
            }
            Stmt::Let { init: None, .. } => {}
            Stmt::Return(Some(expr)) => {
                collect_mut_receivers_from_expr(expr, receivers);
            }
            Stmt::Return(None) => {}
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                collect_mut_receivers_from_expr(condition, receivers);
                collect_mut_receivers(then_body, receivers);
                if let Some(els) = else_body {
                    collect_mut_receivers(els, receivers);
                }
            }
            Stmt::IfLet {
                expr,
                then_body,
                else_body,
                ..
            } => {
                collect_mut_receivers_from_expr(expr, receivers);
                collect_mut_receivers(then_body, receivers);
                if let Some(els) = else_body {
                    collect_mut_receivers(els, receivers);
                }
            }
            Stmt::Match { expr, arms } => {
                collect_mut_receivers_from_expr(expr, receivers);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        collect_mut_receivers_from_expr(guard, receivers);
                    }
                    collect_mut_receivers(&arm.body, receivers);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_mut_receivers_from_expr(condition, receivers);
                collect_mut_receivers(body, receivers);
            }
            Stmt::WhileLet { expr, body, .. } => {
                collect_mut_receivers_from_expr(expr, receivers);
                collect_mut_receivers(body, receivers);
            }
            Stmt::ForIn { iterable, body, .. } => {
                collect_mut_receivers_from_expr(iterable, receivers);
                collect_mut_receivers(body, receivers);
            }
            Stmt::Loop { body, .. } | Stmt::LabeledBlock { body, .. } => {
                collect_mut_receivers(body, receivers);
            }
            Stmt::Break {
                value: Some(expr), ..
            } => {
                collect_mut_receivers_from_expr(expr, receivers);
            }
            Stmt::Break { value: None, .. } | Stmt::Continue { .. } => {}
        }
    }
}

/// Checks if an expression contains a mutating method call and collects the receiver name.
fn collect_mut_receivers_from_expr(expr: &Expr, receivers: &mut std::collections::HashSet<String>) {
    if let Expr::MethodCall { object, method, .. } = expr {
        if MUTATING_METHODS.contains(&method.as_str()) {
            // Extract root variable from chains like obj.items.push(...)
            let mut current = object.as_ref();
            loop {
                match current {
                    Expr::Ident(name) => {
                        receivers.insert(name.clone());
                        break;
                    }
                    Expr::FieldAccess { object: inner, .. } | Expr::Index { object: inner, .. } => {
                        current = inner;
                    }
                    _ => break,
                }
            }
        }
        // Also recurse into chained calls (e.g., arr.drain(...).collect())
        collect_mut_receivers_from_expr(object, receivers);
    }
}

/// Checks whether a list of SWC statements contains a `throw` statement.
///
/// Recursively scans all control flow structures. `try` block throw is excluded
/// (caught by `catch`), but `catch` block throw is included (re-throw).
pub(super) fn contains_throw(stmts: &[ast::Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        ast::Stmt::Throw(_) => true,
        ast::Stmt::If(if_stmt) => {
            stmt_contains_throw(&if_stmt.cons)
                || if_stmt
                    .alt
                    .as_ref()
                    .is_some_and(|alt| stmt_contains_throw(alt))
        }
        ast::Stmt::Block(block) => contains_throw(&block.stmts),
        ast::Stmt::While(w) => stmt_contains_throw(&w.body),
        ast::Stmt::DoWhile(dw) => stmt_contains_throw(&dw.body),
        ast::Stmt::For(f) => stmt_contains_throw(&f.body),
        ast::Stmt::ForOf(fo) => stmt_contains_throw(&fo.body),
        ast::Stmt::ForIn(fi) => stmt_contains_throw(&fi.body),
        ast::Stmt::Labeled(l) => stmt_contains_throw(&l.body),
        ast::Stmt::Switch(s) => s.cases.iter().any(|c| contains_throw(&c.cons)),
        ast::Stmt::Try(t) => {
            // try block throw is excluded (caught by catch)
            // catch block throw is included (re-throw escapes the function)
            let catch_has = t
                .handler
                .as_ref()
                .is_some_and(|h| contains_throw(&h.body.stmts));
            let finally_has = t
                .finalizer
                .as_ref()
                .is_some_and(|f| contains_throw(&f.stmts));
            catch_has || finally_has
        }
        _ => false,
    })
}

/// Checks whether a single statement contains a `throw`.
fn stmt_contains_throw(stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Block(block) => contains_throw(&block.stmts),
        ast::Stmt::Throw(_) => true,
        other => contains_throw(std::slice::from_ref(other)),
    }
}

/// Wraps `return expr` statements in `Ok(expr)` for functions that use `Result`.
///
/// `throw` statements are already converted to `return Err(...)` by `convert_stmt`,
/// so only non-Err returns need wrapping.
pub(super) fn wrap_returns_in_ok(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts.into_iter().map(wrap_stmt_return).collect()
}

/// Recursively wraps return expressions in `Ok(...)`.
fn wrap_stmt_return(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Return(Some(expr)) => {
            // Don't wrap if already an Err(...) call
            if matches!(&expr, Expr::FnCall { name, .. } if name == "Err") {
                Stmt::Return(Some(expr))
            } else {
                Stmt::Return(Some(Expr::FnCall {
                    name: "Ok".to_string(),
                    args: vec![expr],
                }))
            }
        }
        Stmt::Return(None) => Stmt::Return(Some(Expr::FnCall {
            name: "Ok".to_string(),
            args: vec![Expr::Unit],
        })),
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => Stmt::If {
            condition,
            then_body: wrap_returns_in_ok(then_body),
            else_body: else_body.map(wrap_returns_in_ok),
        },
        Stmt::While {
            label,
            condition,
            body,
        } => Stmt::While {
            label,
            condition,
            body: wrap_returns_in_ok(body),
        },
        Stmt::WhileLet {
            label,
            pattern,
            expr,
            body,
        } => Stmt::WhileLet {
            label,
            pattern,
            expr,
            body: wrap_returns_in_ok(body),
        },
        Stmt::ForIn {
            label,
            var,
            iterable,
            body,
        } => Stmt::ForIn {
            label,
            var,
            iterable,
            body: wrap_returns_in_ok(body),
        },
        Stmt::Loop { label, body } => Stmt::Loop {
            label,
            body: wrap_returns_in_ok(body),
        },
        Stmt::Match { expr, arms } => Stmt::Match {
            expr,
            arms: arms
                .into_iter()
                .map(|arm| MatchArm {
                    body: wrap_returns_in_ok(arm.body),
                    ..arm
                })
                .collect(),
        },
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => Stmt::IfLet {
            pattern,
            expr,
            then_body: wrap_returns_in_ok(then_body),
            else_body: else_body.map(wrap_returns_in_ok),
        },
        Stmt::LabeledBlock { label, body } => Stmt::LabeledBlock {
            label,
            body: wrap_returns_in_ok(body),
        },
        other => other,
    }
}
