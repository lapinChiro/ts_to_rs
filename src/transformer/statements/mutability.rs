//! Post-processing pass to infer `let mut` for variables.
//!
//! Scans a statement list for field assignments, mutating method calls,
//! and closure captures to determine which variables need `let mut`.

use crate::ir::{ClosureBody, Expr, Stmt};

/// Mutating methods that require `&mut self` on the receiver.
const MUTATING_METHODS: &[&str] = &[
    "reverse", "sort", "sort_by", "drain", "push", "pop", "remove", "insert", "clear", "truncate",
    "retain",
];

/// Post-processes a statement list to mark immutable variables as `let mut`
/// when subsequent statements mutate them (field assignment or mutating method call).
/// Also marks closure bindings as `let mut` when the closure captures mutably (FnMut).
pub(super) fn mark_mutated_vars(stmts: &mut [Stmt]) {
    let mut needs_mut = std::collections::HashSet::new();
    collect_mutated_vars(stmts, &mut needs_mut);

    // Detect closures that capture outer variables mutably → closure binding needs `let mut`
    let mut closure_needs_mut = std::collections::HashSet::new();
    for stmt in stmts.iter() {
        if let Stmt::Let {
            name,
            init: Some(Expr::Closure { body, .. }),
            ..
        } = stmt
        {
            let mut closure_mutations = std::collections::HashSet::new();
            match body {
                ClosureBody::Block(body_stmts) => {
                    collect_closure_assigns(body_stmts, &mut closure_mutations);
                }
                ClosureBody::Expr(expr) => {
                    collect_assigns_from_expr(expr, &mut closure_mutations);
                }
            }
            if !closure_mutations.is_empty() {
                closure_needs_mut.insert(name.clone());
            }
        }
    }
    needs_mut.extend(closure_needs_mut);

    for stmt in stmts.iter_mut() {
        if let Stmt::Let { mutable, name, .. } = stmt {
            if !*mutable && needs_mut.contains(name.as_str()) {
                *mutable = true;
            }
        }
    }
}

/// Collects variable names that are assigned to inside closure bodies (direct assignment).
fn collect_closure_assigns(stmts: &[Stmt], names: &mut std::collections::HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::TailExpr(expr) => {
                collect_assigns_from_expr(expr, names);
            }
            _ => {}
        }
    }
}

/// Collects variable names from direct assignment expressions (`x = ...`, `x += ...`).
fn collect_assigns_from_expr(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    if let Expr::Assign { target, .. } = expr {
        if let Expr::Ident(name) = target.as_ref() {
            names.insert(name.clone());
        }
    }
}

/// Recursively collects variable names that are targets of field assignments or mutating methods.
fn collect_mutated_vars(stmts: &[Stmt], names: &mut std::collections::HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::TailExpr(expr) => {
                collect_mutated_vars_from_expr(expr, names);
            }
            Stmt::Let {
                init: Some(expr), ..
            } => {
                collect_mutated_vars_from_expr(expr, names);
            }
            Stmt::Return(Some(expr)) => {
                collect_mutated_vars_from_expr(expr, names);
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect_mutated_vars(then_body, names);
                if let Some(els) = else_body {
                    collect_mutated_vars(els, names);
                }
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::Loop { body, .. } => {
                collect_mutated_vars(body, names);
            }
            _ => {}
        }
    }
}

/// Checks if an expression mutates a variable via field assignment or mutating method call.
fn collect_mutated_vars_from_expr(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    match expr {
        // Field assignment: obj.field = value
        Expr::Assign { target, value, .. } => {
            if let Expr::FieldAccess { object, .. } = target.as_ref() {
                if let Expr::Ident(name) = object.as_ref() {
                    names.insert(name.clone());
                }
            }
            collect_mutated_vars_from_expr(value, names);
        }
        // Mutating method call: arr.push(...)
        Expr::MethodCall { object, method, .. } => {
            if MUTATING_METHODS.contains(&method.as_str()) {
                if let Expr::Ident(name) = object.as_ref() {
                    names.insert(name.clone());
                }
            }
            collect_mutated_vars_from_expr(object, names);
        }
        _ => {}
    }
}
