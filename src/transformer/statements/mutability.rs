//! Post-processing pass to infer `let mut` for variables.
//!
//! Scans a statement list for field assignments, mutating method calls,
//! and closure captures to determine which variables need `let mut`.

use std::collections::HashSet;

use crate::ir::{ClosureBody, Expr, Pattern, Stmt};

/// Mutating methods that require `&mut self` on the receiver.
///
/// `get_or_insert_with` is `&mut self` on `Option<T>` and is emitted by the
/// `??=` expression-context path (I-142). A `let x: Option<T>` whose only
/// observed mutation is `x.get_or_insert_with(|| d)` must be upgraded to
/// `let mut x` — otherwise Rust rejects the call.
const MUTATING_METHODS: &[&str] = &[
    "reverse",
    "sort",
    "sort_by",
    "drain",
    "push",
    "pop",
    "remove",
    "insert",
    "clear",
    "truncate",
    "retain",
    "get_or_insert_with",
    "or_insert_with",
    "entry",
];

/// Post-processes a statement list to mark immutable variables as `let mut`
/// when subsequent statements mutate them (field assignment or mutating method call).
/// Also marks closure bindings as `let mut` when the closure captures mutably (FnMut).
///
/// `extra_mut_methods` contains additional method names (from user-defined classes with
/// `&mut self`) that should be treated as mutating, beyond the hardcoded `MUTATING_METHODS`.
pub(super) fn mark_mutated_vars(stmts: &mut [Stmt], extra_mut_methods: &HashSet<String>) {
    let mut needs_mut = HashSet::new();
    collect_mutated_vars(stmts, &mut needs_mut, extra_mut_methods);

    // Detect closures that perform any mutation → closure binding needs `let mut` (FnMut)
    for stmt in stmts.iter() {
        if let Stmt::Let {
            name,
            init: Some(Expr::Closure { body, .. }),
            ..
        } = stmt
        {
            let mut closure_mutations = HashSet::new();
            match body {
                ClosureBody::Block(body_stmts) => {
                    collect_mutated_vars(body_stmts, &mut closure_mutations, extra_mut_methods);
                }
                ClosureBody::Expr(expr) => {
                    collect_mutated_vars_from_expr(expr, &mut closure_mutations, extra_mut_methods);
                }
            }
            if !closure_mutations.is_empty() {
                needs_mut.insert(name.clone());
            }
        }
    }

    for stmt in stmts.iter_mut() {
        if let Stmt::Let { mutable, name, .. } = stmt {
            if !*mutable && needs_mut.contains(name.as_str()) {
                *mutable = true;
            }
        }
    }

    // I-161 P5: narrow-binding mutability.
    //
    // `if let Some(x) = x { x = 3.0; }` / `match x { Some(x) => { x = 3.0; } }`
    // introduces a *new* (shadow) binding for `x` inside the pattern. The
    // `Stmt::Let` loop above only flips the outer `let` binding, but the
    // inner Pattern::Binding is a separate Rust-level binding that must also
    // be `mut` when the arm body reassigns it (I-161 compound logical assign
    // inside narrow scope triggers this universally).
    //
    // Walks all nested control-flow (`If` / `IfLet` / `Match` / `While` /
    // `WhileLet` / `Loop` / `LabeledBlock` / `ForIn`) so arbitrarily deep
    // narrow pattern bindings are handled.
    mark_mutated_narrow_bindings(stmts, extra_mut_methods);
}

/// Post-processes `Stmt::IfLet` / `Stmt::Match` / `Expr::IfLet` / `Expr::Match`
/// narrow-pattern bindings: each `Pattern::Binding { name, is_mut: false, .. }`
/// that the arm body reassigns is flipped to `is_mut: true`.
///
/// Without this flip, emissions like
/// `if let Some(x) = x { if <pred> { x = y; } }` (I-161 `&&=` inside narrow
/// scope) fail with `E0384 cannot assign twice to immutable variable`.
fn mark_mutated_narrow_bindings(stmts: &mut [Stmt], extra_mut_methods: &HashSet<String>) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::IfLet {
                pattern,
                expr,
                then_body,
                else_body,
            } => {
                let mut then_mut = HashSet::new();
                collect_mutated_vars(then_body, &mut then_mut, extra_mut_methods);
                mark_pattern_bindings_mut(pattern, &then_mut);
                // Expr may contain nested control flow (e.g., a match inside
                // the test position) — walk it for its own narrow bindings.
                mark_mutated_narrow_bindings_in_expr(expr, extra_mut_methods);
                mark_mutated_narrow_bindings(then_body, extra_mut_methods);
                if let Some(els) = else_body {
                    mark_mutated_narrow_bindings(els, extra_mut_methods);
                }
            }
            Stmt::Match { expr, arms } => {
                mark_mutated_narrow_bindings_in_expr(expr, extra_mut_methods);
                for arm in arms.iter_mut() {
                    let mut arm_mut = HashSet::new();
                    collect_mutated_vars(&arm.body, &mut arm_mut, extra_mut_methods);
                    for pat in arm.patterns.iter_mut() {
                        mark_pattern_bindings_mut(pat, &arm_mut);
                    }
                    if let Some(guard) = &mut arm.guard {
                        mark_mutated_narrow_bindings_in_expr(guard, extra_mut_methods);
                    }
                    mark_mutated_narrow_bindings(&mut arm.body, extra_mut_methods);
                }
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                mark_mutated_narrow_bindings_in_expr(condition, extra_mut_methods);
                mark_mutated_narrow_bindings(then_body, extra_mut_methods);
                if let Some(els) = else_body {
                    mark_mutated_narrow_bindings(els, extra_mut_methods);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                mark_mutated_narrow_bindings_in_expr(condition, extra_mut_methods);
                mark_mutated_narrow_bindings(body, extra_mut_methods);
            }
            Stmt::WhileLet {
                pattern,
                expr,
                body,
                ..
            } => {
                let mut body_mut = HashSet::new();
                collect_mutated_vars(body, &mut body_mut, extra_mut_methods);
                mark_pattern_bindings_mut(pattern, &body_mut);
                mark_mutated_narrow_bindings_in_expr(expr, extra_mut_methods);
                mark_mutated_narrow_bindings(body, extra_mut_methods);
            }
            Stmt::ForIn { iterable, body, .. } => {
                mark_mutated_narrow_bindings_in_expr(iterable, extra_mut_methods);
                mark_mutated_narrow_bindings(body, extra_mut_methods);
            }
            Stmt::Loop { body, .. } | Stmt::LabeledBlock { body, .. } => {
                mark_mutated_narrow_bindings(body, extra_mut_methods);
            }
            Stmt::Let {
                init: Some(expr), ..
            }
            | Stmt::Expr(expr)
            | Stmt::TailExpr(expr)
            | Stmt::Return(Some(expr)) => {
                mark_mutated_narrow_bindings_in_expr(expr, extra_mut_methods);
            }
            Stmt::Let { init: None, .. }
            | Stmt::Return(None)
            | Stmt::Break { value: None, .. }
            | Stmt::Continue { .. } => {}
            Stmt::Break {
                value: Some(expr), ..
            } => {
                mark_mutated_narrow_bindings_in_expr(expr, extra_mut_methods);
            }
        }
    }
}

/// Walks an `Expr` to flip narrow bindings inside `Expr::IfLet` / `Expr::Match`
/// / closure bodies / blocks.
fn mark_mutated_narrow_bindings_in_expr(expr: &mut Expr, extra_mut_methods: &HashSet<String>) {
    match expr {
        Expr::IfLet {
            pattern,
            expr: scrutinee,
            then_expr,
            else_expr,
        } => {
            let tmp = vec![Stmt::Expr((**then_expr).clone())];
            let mut then_mut = HashSet::new();
            collect_mutated_vars(&tmp, &mut then_mut, extra_mut_methods);
            mark_pattern_bindings_mut(pattern, &then_mut);
            mark_mutated_narrow_bindings_in_expr(scrutinee, extra_mut_methods);
            mark_mutated_narrow_bindings_in_expr(then_expr, extra_mut_methods);
            mark_mutated_narrow_bindings_in_expr(else_expr, extra_mut_methods);
        }
        Expr::Match {
            expr: scrutinee,
            arms,
        } => {
            mark_mutated_narrow_bindings_in_expr(scrutinee, extra_mut_methods);
            for arm in arms.iter_mut() {
                let mut arm_mut = HashSet::new();
                collect_mutated_vars(&arm.body, &mut arm_mut, extra_mut_methods);
                for pat in arm.patterns.iter_mut() {
                    mark_pattern_bindings_mut(pat, &arm_mut);
                }
                if let Some(g) = &mut arm.guard {
                    mark_mutated_narrow_bindings_in_expr(g, extra_mut_methods);
                }
                mark_mutated_narrow_bindings(&mut arm.body, extra_mut_methods);
            }
        }
        Expr::Block(stmts) => {
            mark_mutated_narrow_bindings(stmts, extra_mut_methods);
        }
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            mark_mutated_narrow_bindings_in_expr(condition, extra_mut_methods);
            mark_mutated_narrow_bindings_in_expr(then_expr, extra_mut_methods);
            mark_mutated_narrow_bindings_in_expr(else_expr, extra_mut_methods);
        }
        Expr::Closure { body, .. } => match body {
            crate::ir::ClosureBody::Block(body_stmts) => {
                mark_mutated_narrow_bindings(body_stmts, extra_mut_methods);
            }
            crate::ir::ClosureBody::Expr(e) => {
                mark_mutated_narrow_bindings_in_expr(e, extra_mut_methods);
            }
        },
        // Recurse into sub-expressions.
        Expr::BinaryOp { left, right, .. } => {
            mark_mutated_narrow_bindings_in_expr(left, extra_mut_methods);
            mark_mutated_narrow_bindings_in_expr(right, extra_mut_methods);
        }
        Expr::UnaryOp { operand, .. } => {
            mark_mutated_narrow_bindings_in_expr(operand, extra_mut_methods);
        }
        Expr::FnCall { args, .. } => {
            for a in args.iter_mut() {
                mark_mutated_narrow_bindings_in_expr(a, extra_mut_methods);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            mark_mutated_narrow_bindings_in_expr(object, extra_mut_methods);
            for a in args.iter_mut() {
                mark_mutated_narrow_bindings_in_expr(a, extra_mut_methods);
            }
        }
        Expr::Assign { target, value } => {
            mark_mutated_narrow_bindings_in_expr(target, extra_mut_methods);
            mark_mutated_narrow_bindings_in_expr(value, extra_mut_methods);
        }
        Expr::FieldAccess { object, .. } => {
            mark_mutated_narrow_bindings_in_expr(object, extra_mut_methods);
        }
        Expr::Index { object, index } => {
            mark_mutated_narrow_bindings_in_expr(object, extra_mut_methods);
            mark_mutated_narrow_bindings_in_expr(index, extra_mut_methods);
        }
        Expr::Range { start, end } => {
            if let Some(s) = start {
                mark_mutated_narrow_bindings_in_expr(s, extra_mut_methods);
            }
            if let Some(e) = end {
                mark_mutated_narrow_bindings_in_expr(e, extra_mut_methods);
            }
        }
        Expr::Await(inner) | Expr::Deref(inner) | Expr::Ref(inner) => {
            mark_mutated_narrow_bindings_in_expr(inner, extra_mut_methods);
        }
        Expr::Vec { elements } | Expr::Tuple { elements } => {
            for e in elements.iter_mut() {
                mark_mutated_narrow_bindings_in_expr(e, extra_mut_methods);
            }
        }
        Expr::MacroCall { args, .. } | Expr::FormatMacro { args, .. } => {
            for a in args.iter_mut() {
                mark_mutated_narrow_bindings_in_expr(a, extra_mut_methods);
            }
        }
        Expr::Cast { expr: inner, .. } => {
            mark_mutated_narrow_bindings_in_expr(inner, extra_mut_methods);
        }
        Expr::Matches {
            expr: scrutinee, ..
        } => {
            mark_mutated_narrow_bindings_in_expr(scrutinee, extra_mut_methods);
        }
        Expr::StructInit { fields, base, .. } => {
            for (_, f) in fields.iter_mut() {
                mark_mutated_narrow_bindings_in_expr(f, extra_mut_methods);
            }
            if let Some(b) = base {
                mark_mutated_narrow_bindings_in_expr(b, extra_mut_methods);
            }
        }
        Expr::RuntimeTypeof { operand } => {
            mark_mutated_narrow_bindings_in_expr(operand, extra_mut_methods);
        }
        // Leaf / literal-like.
        Expr::NumberLit(_)
        | Expr::IntLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::Unit
        | Expr::EnumVariant { .. }
        | Expr::PrimitiveAssocConst { .. }
        | Expr::StdConst(_)
        | Expr::BuiltinVariantValue(_)
        | Expr::RawCode(_)
        | Expr::Regex { .. } => {}
    }
}

/// Flips `Pattern::Binding { is_mut: false, .. }` to `is_mut: true` when the
/// binding name is in the mutation set. Recurses into nested patterns.
fn mark_pattern_bindings_mut(pattern: &mut Pattern, mutated: &HashSet<String>) {
    match pattern {
        Pattern::Binding {
            name,
            is_mut,
            subpat,
        } => {
            if mutated.contains(name.as_str()) {
                *is_mut = true;
            }
            if let Some(inner) = subpat {
                mark_pattern_bindings_mut(inner, mutated);
            }
        }
        Pattern::TupleStruct { fields, .. } => {
            for f in fields.iter_mut() {
                mark_pattern_bindings_mut(f, mutated);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields.iter_mut() {
                mark_pattern_bindings_mut(p, mutated);
            }
        }
        Pattern::Or(pats) | Pattern::Tuple(pats) => {
            for p in pats.iter_mut() {
                mark_pattern_bindings_mut(p, mutated);
            }
        }
        Pattern::Ref { inner, .. } => {
            mark_pattern_bindings_mut(inner, mutated);
        }
        Pattern::Wildcard
        | Pattern::Literal(_)
        | Pattern::UnitStruct { .. }
        | Pattern::Range { .. } => {}
    }
}

/// Recursively collects variable names that are targets of mutations.
///
/// Uses exhaustive pattern matching to ensure new `Stmt` variants are handled.
/// Used by both `mark_mutated_vars` (local variables) and `mark_mut_params_from_body`
/// (parameter rebinding) to avoid duplicating the traversal logic.
///
/// `extra_mut_methods` contains additional method names (from user-defined classes with
/// `&mut self`) that should be treated as mutating, beyond the hardcoded `MUTATING_METHODS`.
pub(crate) fn collect_mutated_vars(
    stmts: &[Stmt],
    names: &mut HashSet<String>,
    extra_mut_methods: &HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::TailExpr(expr) => {
                collect_mutated_vars_from_expr(expr, names, extra_mut_methods);
            }
            Stmt::Let {
                init: Some(expr), ..
            } => {
                collect_mutated_vars_from_expr(expr, names, extra_mut_methods);
            }
            Stmt::Let { init: None, .. } => {}
            Stmt::Return(Some(expr)) => {
                collect_mutated_vars_from_expr(expr, names, extra_mut_methods);
            }
            Stmt::Return(None) => {}
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                collect_mutated_vars_from_expr(condition, names, extra_mut_methods);
                collect_mutated_vars(then_body, names, extra_mut_methods);
                if let Some(els) = else_body {
                    collect_mutated_vars(els, names, extra_mut_methods);
                }
            }
            Stmt::IfLet {
                expr,
                then_body,
                else_body,
                ..
            } => {
                collect_mutated_vars_from_expr(expr, names, extra_mut_methods);
                collect_mutated_vars(then_body, names, extra_mut_methods);
                if let Some(els) = else_body {
                    collect_mutated_vars(els, names, extra_mut_methods);
                }
            }
            Stmt::Match { expr, arms } => {
                collect_mutated_vars_from_expr(expr, names, extra_mut_methods);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        collect_mutated_vars_from_expr(guard, names, extra_mut_methods);
                    }
                    collect_mutated_vars(&arm.body, names, extra_mut_methods);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_mutated_vars_from_expr(condition, names, extra_mut_methods);
                collect_mutated_vars(body, names, extra_mut_methods);
            }
            Stmt::WhileLet { expr, body, .. } => {
                collect_mutated_vars_from_expr(expr, names, extra_mut_methods);
                collect_mutated_vars(body, names, extra_mut_methods);
            }
            Stmt::ForIn { iterable, body, .. } => {
                collect_mutated_vars_from_expr(iterable, names, extra_mut_methods);
                collect_mutated_vars(body, names, extra_mut_methods);
            }
            Stmt::Loop { body, .. } | Stmt::LabeledBlock { body, .. } => {
                collect_mutated_vars(body, names, extra_mut_methods);
            }
            Stmt::Break {
                value: Some(expr), ..
            } => {
                collect_mutated_vars_from_expr(expr, names, extra_mut_methods);
            }
            Stmt::Break { value: None, .. } | Stmt::Continue { .. } => {}
        }
    }
}

/// Extracts the root variable name from a chain of field accesses and index accesses.
///
/// `obj` → `Some("obj")`
/// `obj.field` → `Some("obj")`
/// `obj.a.b.c` → `Some("obj")`
/// `arr[0].field` → `Some("arr")`
fn extract_root_ident(expr: &Expr) -> Option<&str> {
    let mut current = expr;
    loop {
        match current {
            Expr::Ident(name) => return Some(name),
            Expr::FieldAccess { object, .. }
            | Expr::Index { object, .. }
            | Expr::MethodCall { object, .. } => {
                current = object;
            }
            _ => return None,
        }
    }
}

/// Recursively walks an expression tree to find mutations (assignment, field assignment,
/// index assignment, mutating method calls). Recurses into all sub-expressions including
/// closure bodies and block expressions.
fn collect_mutated_vars_from_expr(
    expr: &Expr,
    names: &mut HashSet<String>,
    extra_mut_methods: &HashSet<String>,
) {
    // Shorthand for recursive calls
    let recurse = |e: &Expr, n: &mut HashSet<String>| {
        collect_mutated_vars_from_expr(e, n, extra_mut_methods);
    };

    match expr {
        // Assignment: x = value, obj.field = value, obj.a.b = value, arr[i] = value
        Expr::Assign { target, value, .. } => {
            if let Some(root) = extract_root_ident(target) {
                names.insert(root.to_string());
            }
            recurse(value, names);
        }
        // Mutating method call: arr.push(...), obj.items.push(...)
        Expr::MethodCall {
            object,
            method,
            args,
            ..
        } => {
            if MUTATING_METHODS.contains(&method.as_str())
                || extra_mut_methods.contains(method.as_str())
            {
                if let Some(root) = extract_root_ident(object) {
                    names.insert(root.to_string());
                }
            }
            recurse(object, names);
            for arg in args {
                recurse(arg, names);
            }
        }
        // Block expression → recurse into inner statements
        Expr::Block(block_stmts) => {
            collect_mutated_vars(block_stmts, names, extra_mut_methods);
        }
        // Closure → recurse into body to detect mutations of captured variables
        Expr::Closure { body, .. } => match body {
            ClosureBody::Block(stmts) => {
                collect_mutated_vars(stmts, names, extra_mut_methods);
            }
            ClosureBody::Expr(e) => recurse(e, names),
        },
        // Recurse into sub-expressions
        Expr::FnCall { args, .. }
        | Expr::FormatMacro { args, .. }
        | Expr::MacroCall { args, .. } => {
            for arg in args {
                recurse(arg, names);
            }
        }
        Expr::FieldAccess { object, .. } => recurse(object, names),
        Expr::Index { object, index } => {
            recurse(object, names);
            recurse(index, names);
        }
        Expr::BinaryOp { left, right, .. } => {
            recurse(left, names);
            recurse(right, names);
        }
        Expr::UnaryOp { operand, .. } => recurse(operand, names),
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            recurse(condition, names);
            recurse(then_expr, names);
            recurse(else_expr, names);
        }
        Expr::IfLet {
            expr,
            then_expr,
            else_expr,
            ..
        } => {
            recurse(expr, names);
            recurse(then_expr, names);
            recurse(else_expr, names);
        }
        Expr::Match { expr, arms } => {
            recurse(expr, names);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    recurse(guard, names);
                }
                collect_mutated_vars(&arm.body, names, extra_mut_methods);
            }
        }
        Expr::StructInit { fields, base, .. } => {
            for (_, val) in fields {
                recurse(val, names);
            }
            if let Some(b) = base {
                recurse(b, names);
            }
        }
        Expr::Vec { elements } | Expr::Tuple { elements } => {
            for e in elements {
                recurse(e, names);
            }
        }
        Expr::Cast { expr, .. }
        | Expr::Await(expr)
        | Expr::Deref(expr)
        | Expr::Ref(expr)
        | Expr::Matches { expr, .. }
        | Expr::RuntimeTypeof { operand: expr } => {
            recurse(expr, names);
        }
        Expr::Range { start, end } => {
            if let Some(s) = start {
                recurse(s, names);
            }
            if let Some(e) = end {
                recurse(e, names);
            }
        }
        // Leaf nodes: no sub-expressions
        Expr::NumberLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::Unit
        | Expr::IntLit(_)
        | Expr::RawCode(_)
        | Expr::Regex { .. }
        | Expr::EnumVariant { .. }
        | Expr::PrimitiveAssocConst { .. }
        | Expr::StdConst(_)
        | Expr::BuiltinVariantValue(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_root_ident_simple() {
        let expr = Expr::Ident("x".to_string());
        assert_eq!(extract_root_ident(&expr), Some("x"));
    }

    #[test]
    fn test_extract_root_ident_field_access() {
        // obj.field
        let expr = Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "field".to_string(),
        };
        assert_eq!(extract_root_ident(&expr), Some("obj"));
    }

    #[test]
    fn test_extract_root_ident_nested_field_access() {
        // obj.a.b.c
        let expr = Expr::FieldAccess {
            object: Box::new(Expr::FieldAccess {
                object: Box::new(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("obj".to_string())),
                    field: "a".to_string(),
                }),
                field: "b".to_string(),
            }),
            field: "c".to_string(),
        };
        assert_eq!(extract_root_ident(&expr), Some("obj"));
    }

    #[test]
    fn test_extract_root_ident_index_access() {
        // arr[0]
        let expr = Expr::Index {
            object: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::NumberLit(0.0)),
        };
        assert_eq!(extract_root_ident(&expr), Some("arr"));
    }

    #[test]
    fn test_extract_root_ident_mixed_chain() {
        // obj.items[0].name
        let expr = Expr::FieldAccess {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("obj".to_string())),
                    field: "items".to_string(),
                }),
                index: Box::new(Expr::NumberLit(0.0)),
            }),
            field: "name".to_string(),
        };
        assert_eq!(extract_root_ident(&expr), Some("obj"));
    }

    #[test]
    fn test_extract_root_ident_non_ident_root() {
        // (fn_call()).field — no root ident
        let expr = Expr::FieldAccess {
            object: Box::new(Expr::FnCall {
                target: crate::ir::CallTarget::Free("get_obj".to_string()),
                args: vec![],
            }),
            field: "field".to_string(),
        };
        assert_eq!(extract_root_ident(&expr), None);
    }

    #[test]
    fn test_mark_mutated_vars_direct_reassignment() {
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            },
            Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident("x".to_string())),
                value: Box::new(Expr::NumberLit(2.0)),
            }),
        ];
        mark_mutated_vars(&mut stmts, &HashSet::new());
        assert!(matches!(&stmts[0], Stmt::Let { mutable: true, .. }));
    }

    #[test]
    fn test_mark_mutated_vars_no_mutation_stays_immutable() {
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            },
            Stmt::Return(Some(Expr::Ident("x".to_string()))),
        ];
        mark_mutated_vars(&mut stmts, &HashSet::new());
        assert!(matches!(&stmts[0], Stmt::Let { mutable: false, .. }));
    }

    #[test]
    fn test_mark_mutated_vars_nested_field_assignment() {
        // obj.a.b = 1 → obj needs mut
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "obj".to_string(),
                ty: None,
                init: Some(Expr::Ident("something".to_string())),
            },
            Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::FieldAccess {
                    object: Box::new(Expr::FieldAccess {
                        object: Box::new(Expr::Ident("obj".to_string())),
                        field: "a".to_string(),
                    }),
                    field: "b".to_string(),
                }),
                value: Box::new(Expr::NumberLit(1.0)),
            }),
        ];
        mark_mutated_vars(&mut stmts, &HashSet::new());
        assert!(
            matches!(&stmts[0], Stmt::Let { mutable: true, name, .. } if name == "obj"),
            "nested field assignment should mark root variable as mutable"
        );
    }

    #[test]
    fn test_mark_mutated_vars_user_defined_mut_method() {
        // counter.increment() where increment is in extra_mut_methods
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "counter".to_string(),
                ty: None,
                init: Some(Expr::Ident("c".to_string())),
            },
            Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident("counter".to_string())),
                method: "increment".to_string(),
                args: vec![],
            }),
        ];
        let mut extra = HashSet::new();
        extra.insert("increment".to_string());
        mark_mutated_vars(&mut stmts, &extra);
        assert!(
            matches!(&stmts[0], Stmt::Let { mutable: true, name, .. } if name == "counter"),
            "user-defined &mut self method should mark receiver as mutable"
        );
    }

    #[test]
    fn test_mark_mutated_vars_unknown_method_stays_immutable() {
        // obj.read_only() where read_only is NOT in extra_mut_methods
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "obj".to_string(),
                ty: None,
                init: Some(Expr::Ident("o".to_string())),
            },
            Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident("obj".to_string())),
                method: "read_only".to_string(),
                args: vec![],
            }),
        ];
        mark_mutated_vars(&mut stmts, &HashSet::new());
        assert!(
            matches!(&stmts[0], Stmt::Let { mutable: false, .. }),
            "unknown method should not mark receiver as mutable"
        );
    }

    #[test]
    fn test_mark_mutated_vars_get_or_insert_with_marks_let_mut() {
        // `Option::get_or_insert_with` is `&mut self` — the owning `let x`
        // must be upgraded to `let mut x`. Covers I-142 expression-context
        // emission (`*x.get_or_insert_with(|| d)` and
        // `x.get_or_insert_with(|| d).clone()`).
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::Ident("param".to_string())),
            },
            Stmt::Expr(Expr::Deref(Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "get_or_insert_with".to_string(),
                args: vec![Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::NumberLit(0.0))),
                }],
            }))),
        ];
        mark_mutated_vars(&mut stmts, &HashSet::new());
        assert!(
            matches!(&stmts[0], Stmt::Let { mutable: true, name, .. } if name == "x"),
            "get_or_insert_with must be recognised as mutating"
        );
    }

    #[test]
    fn test_mark_mutated_vars_block_expression() {
        // x++ pattern: { let _old = x; x = x + 1; _old }
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(0.0)),
            },
            Stmt::Expr(Expr::Block(vec![Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident("x".to_string())),
                value: Box::new(Expr::NumberLit(1.0)),
            })])),
        ];
        mark_mutated_vars(&mut stmts, &HashSet::new());
        assert!(matches!(&stmts[0], Stmt::Let { mutable: true, .. }));
    }
}
