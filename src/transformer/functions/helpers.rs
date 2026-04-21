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

/// Wraps closure expressions in `Box::new(...)` throughout a function body when
/// the return type is `Box<dyn Fn(...)>`.
///
/// Recursively walks the body to handle closures in all positions:
/// - `return || { ... }` → `return Box::new(|| { ... })`
/// - `|| { ... }` (tail) → `Box::new(|| { ... })`
/// - `if cond { return || { ... } } else { || { ... } }` → both wrapped
///
/// Only wraps `Expr::Closure` — other expressions (Ident, FnCall) are assumed
/// to already have the correct Box type. (I-020)
///
/// Mirrors the recursive structure of [`wrap_returns_in_ok`].
pub(crate) fn wrap_closures_in_box(body: Vec<Stmt>, return_type: Option<&RustType>) -> Vec<Stmt> {
    let Some(return_type) = return_type else {
        return body;
    };
    // Generator renders RustType::Fn as `Box<dyn Fn(...) -> R>` in return position,
    // so the IR return_type is Fn { params, return_type }, not StdCollection { Box, [DynTrait] }.
    let needs_box = matches!(
        return_type,
        RustType::Fn { .. }
            | RustType::StdCollection {
                kind: crate::ir::StdCollectionKind::Box,
                ..
            }
    );
    if !needs_box {
        return body;
    }
    body.into_iter().map(wrap_stmt_closure_in_box).collect()
}

/// Wraps a closure expression in `Box::new(...)`.
fn box_wrap_closure(expr: Expr) -> Expr {
    if !matches!(expr, Expr::Closure { .. }) {
        return expr;
    }
    Expr::FnCall {
        target: crate::ir::CallTarget::ExternalPath(vec!["Box".to_string(), "new".to_string()]),
        args: vec![expr],
    }
}

/// Recursively wraps closure expressions in return/tail positions with `Box::new(...)`.
fn wrap_stmt_closure_in_box(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Return(Some(expr)) => Stmt::Return(Some(box_wrap_closure(expr))),
        Stmt::TailExpr(expr) => Stmt::TailExpr(box_wrap_closure(expr)),
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => Stmt::If {
            condition,
            then_body: then_body
                .into_iter()
                .map(wrap_stmt_closure_in_box)
                .collect(),
            else_body: else_body.map(|b| b.into_iter().map(wrap_stmt_closure_in_box).collect()),
        },
        Stmt::While {
            label,
            condition,
            body,
        } => Stmt::While {
            label,
            condition,
            body: body.into_iter().map(wrap_stmt_closure_in_box).collect(),
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
            body: body.into_iter().map(wrap_stmt_closure_in_box).collect(),
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
            body: body.into_iter().map(wrap_stmt_closure_in_box).collect(),
        },
        Stmt::Loop { label, body } => Stmt::Loop {
            label,
            body: body.into_iter().map(wrap_stmt_closure_in_box).collect(),
        },
        Stmt::Match { expr, arms } => Stmt::Match {
            expr,
            arms: arms
                .into_iter()
                .map(|arm| MatchArm {
                    body: arm.body.into_iter().map(wrap_stmt_closure_in_box).collect(),
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
            then_body: then_body
                .into_iter()
                .map(wrap_stmt_closure_in_box)
                .collect(),
            else_body: else_body.map(|b| b.into_iter().map(wrap_stmt_closure_in_box).collect()),
        },
        Stmt::LabeledBlock { label, body } => Stmt::LabeledBlock {
            label,
            body: body.into_iter().map(wrap_stmt_closure_in_box).collect(),
        },
        other => other,
    }
}

/// Appends `None` as a tail expression when the return type is `Option<T>` and
/// the body does not already produce a definite return value on every path.
///
/// A body is considered "definite" when it ends with a `TailExpr` (the value
/// IS the return) or when [`ir_body_always_exits`] reports that every path
/// exits via `return` / `break` / `continue`.  All other endings (if-without-
/// else, if-with-else where some branches fall through, while, for, plain
/// expression statements, let bindings, empty body) need an explicit `None`
/// appended so Rust's type checker sees a value of type `Option<T>`.
///
/// This supersedes the previous pattern-matching heuristic (if-no-else /
/// while / for) which missed cases like if-with-else where inner branches
/// can fall through (I-025 cell-i025).
pub(crate) fn append_implicit_none_if_needed(body: &mut Vec<Stmt>, return_type: Option<&RustType>) {
    use crate::transformer::statements::control_flow::ir_body_always_exits;

    let Some(return_type) = return_type else {
        return;
    };
    if !matches!(return_type, RustType::Option(_)) {
        return;
    }

    // Body already produces a definite value — no implicit None needed.
    let has_definite_value = body
        .last()
        .is_some_and(|last| matches!(last, Stmt::TailExpr(_)) || ir_body_always_exits(body));

    if !has_definite_value {
        body.push(Stmt::TailExpr(Expr::BuiltinVariantValue(
            crate::ir::BuiltinVariant::None,
        )));
    }
}

/// Scans function body for mutations (assignments, mutating method calls, closure captures)
/// and inserts `let mut name = name;` rebinding statements for affected parameters.
///
/// Delegates mutation detection to [`crate::transformer::statements::mutability::collect_mutated_vars`]
/// to avoid duplicating the traversal logic (DRY).
pub(super) fn mark_mut_params_from_body(
    body: &[Stmt],
    params: &[Param],
    extra_mut_methods: &std::collections::HashSet<String>,
) -> Vec<Stmt> {
    let mut mutated = std::collections::HashSet::new();
    crate::transformer::statements::mutability::collect_mutated_vars(
        body,
        &mut mutated,
        extra_mut_methods,
    );

    params
        .iter()
        .filter(|p| mutated.contains(&p.name))
        .map(|p| Stmt::Let {
            mutable: true,
            name: p.name.clone(),
            ty: None,
            init: Some(Expr::Ident(p.name.clone())),
        })
        .collect()
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
            if matches!(
                &expr,
                Expr::FnCall {
                    target: crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
                    ..
                }
            ) {
                Stmt::Return(Some(expr))
            } else {
                Stmt::Return(Some(Expr::FnCall {
                    target: crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
                    args: vec![expr],
                }))
            }
        }
        Stmt::Return(None) => Stmt::Return(Some(Expr::FnCall {
            target: crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
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
