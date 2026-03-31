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

    // Detect closures that perform any mutation → closure binding needs `let mut` (FnMut)
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
                    collect_mutated_vars(body_stmts, &mut closure_mutations);
                }
                ClosureBody::Expr(expr) => {
                    collect_mutated_vars_from_expr(expr, &mut closure_mutations);
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
}

/// Recursively collects variable names that are targets of mutations.
///
/// Uses exhaustive pattern matching to ensure new `Stmt` variants are handled.
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
            Stmt::Let { init: None, .. } => {}
            Stmt::Return(Some(expr)) => {
                collect_mutated_vars_from_expr(expr, names);
            }
            Stmt::Return(None) => {}
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                collect_mutated_vars_from_expr(condition, names);
                collect_mutated_vars(then_body, names);
                if let Some(els) = else_body {
                    collect_mutated_vars(els, names);
                }
            }
            Stmt::IfLet {
                expr,
                then_body,
                else_body,
                ..
            } => {
                collect_mutated_vars_from_expr(expr, names);
                collect_mutated_vars(then_body, names);
                if let Some(els) = else_body {
                    collect_mutated_vars(els, names);
                }
            }
            Stmt::Match { expr, arms } => {
                collect_mutated_vars_from_expr(expr, names);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        collect_mutated_vars_from_expr(guard, names);
                    }
                    collect_mutated_vars(&arm.body, names);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_mutated_vars_from_expr(condition, names);
                collect_mutated_vars(body, names);
            }
            Stmt::WhileLet { expr, body, .. } => {
                collect_mutated_vars_from_expr(expr, names);
                collect_mutated_vars(body, names);
            }
            Stmt::ForIn { iterable, body, .. } => {
                collect_mutated_vars_from_expr(iterable, names);
                collect_mutated_vars(body, names);
            }
            Stmt::Loop { body, .. } | Stmt::LabeledBlock { body, .. } => {
                collect_mutated_vars(body, names);
            }
            Stmt::Break {
                value: Some(expr), ..
            } => {
                collect_mutated_vars_from_expr(expr, names);
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
            Expr::FieldAccess { object, .. } | Expr::Index { object, .. } => {
                current = object;
            }
            _ => return None,
        }
    }
}

/// Recursively walks an expression tree to find mutations (assignment, field assignment,
/// index assignment, mutating method calls). Recurses into all sub-expressions including
/// closure bodies and block expressions.
fn collect_mutated_vars_from_expr(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    match expr {
        // Assignment: x = value, obj.field = value, obj.a.b = value, arr[i] = value
        Expr::Assign { target, value, .. } => {
            if let Some(root) = extract_root_ident(target) {
                names.insert(root.to_string());
            }
            collect_mutated_vars_from_expr(value, names);
        }
        // Mutating method call: arr.push(...), obj.items.push(...)
        Expr::MethodCall {
            object,
            method,
            args,
            ..
        } => {
            if MUTATING_METHODS.contains(&method.as_str()) {
                if let Some(root) = extract_root_ident(object) {
                    names.insert(root.to_string());
                }
            }
            collect_mutated_vars_from_expr(object, names);
            for arg in args {
                collect_mutated_vars_from_expr(arg, names);
            }
        }
        // Block expression → recurse into inner statements
        Expr::Block(block_stmts) => {
            collect_mutated_vars(block_stmts, names);
        }
        // Closure → recurse into body to detect mutations of captured variables
        Expr::Closure { body, .. } => match body {
            ClosureBody::Block(stmts) => collect_mutated_vars(stmts, names),
            ClosureBody::Expr(e) => collect_mutated_vars_from_expr(e, names),
        },
        // Recurse into sub-expressions
        Expr::FnCall { args, .. }
        | Expr::FormatMacro { args, .. }
        | Expr::MacroCall { args, .. } => {
            for arg in args {
                collect_mutated_vars_from_expr(arg, names);
            }
        }
        Expr::FieldAccess { object, .. } => collect_mutated_vars_from_expr(object, names),
        Expr::Index { object, index } => {
            collect_mutated_vars_from_expr(object, names);
            collect_mutated_vars_from_expr(index, names);
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_mutated_vars_from_expr(left, names);
            collect_mutated_vars_from_expr(right, names);
        }
        Expr::UnaryOp { operand, .. } => collect_mutated_vars_from_expr(operand, names),
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_mutated_vars_from_expr(condition, names);
            collect_mutated_vars_from_expr(then_expr, names);
            collect_mutated_vars_from_expr(else_expr, names);
        }
        Expr::IfLet {
            expr,
            then_expr,
            else_expr,
            ..
        } => {
            collect_mutated_vars_from_expr(expr, names);
            collect_mutated_vars_from_expr(then_expr, names);
            collect_mutated_vars_from_expr(else_expr, names);
        }
        Expr::Match { expr, arms } => {
            collect_mutated_vars_from_expr(expr, names);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_mutated_vars_from_expr(guard, names);
                }
                collect_mutated_vars(&arm.body, names);
            }
        }
        Expr::StructInit { fields, base, .. } => {
            for (_, val) in fields {
                collect_mutated_vars_from_expr(val, names);
            }
            if let Some(b) = base {
                collect_mutated_vars_from_expr(b, names);
            }
        }
        Expr::Vec { elements } | Expr::Tuple { elements } => {
            for e in elements {
                collect_mutated_vars_from_expr(e, names);
            }
        }
        Expr::Cast { expr, .. }
        | Expr::Await(expr)
        | Expr::Deref(expr)
        | Expr::Ref(expr)
        | Expr::Matches { expr, .. }
        | Expr::RuntimeTypeof { operand: expr } => {
            collect_mutated_vars_from_expr(expr, names);
        }
        Expr::Range { start, end } => {
            if let Some(s) = start {
                collect_mutated_vars_from_expr(s, names);
            }
            if let Some(e) = end {
                collect_mutated_vars_from_expr(e, names);
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
        | Expr::Regex { .. } => {}
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
                name: "get_obj".to_string(),
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
        mark_mutated_vars(&mut stmts);
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
        mark_mutated_vars(&mut stmts);
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
        mark_mutated_vars(&mut stmts);
        assert!(
            matches!(&stmts[0], Stmt::Let { mutable: true, name, .. } if name == "obj"),
            "nested field assignment should mark root variable as mutable"
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
        mark_mutated_vars(&mut stmts);
        assert!(matches!(&stmts[0], Stmt::Let { mutable: true, .. }));
    }
}
