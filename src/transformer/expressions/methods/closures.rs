//! Closure-argument rewrites used by iterator method arms in
//! [`super::map_method_call`]:
//!
//! - [`deref_closure_params`] wraps parameter references in `*` so the
//!   closure body operates on dereferenced values (used by
//!   `Iterator::filter` / `Iterator::find` whose predicate receives
//!   `&Self::Item`)
//! - [`strip_closure_type_annotations`] removes param / return-type
//!   annotations when Rust's type inference handles the reference
//!   automatically (used by `fold`, `sort_by`, `for_each`, etc.)
//! - [`wrap_sort_comparator_body`] wraps a TS sort comparator's numeric
//!   result with `.partial_cmp(&0.0).unwrap()` so it returns
//!   `std::cmp::Ordering` as `sort_by` requires

use std::collections::HashSet;

use crate::ir::fold::{walk_expr, IrFolder};
use crate::ir::{ClosureBody, Expr, Param};

/// Wraps closure parameter identifier references in `Expr::Deref`.
///
/// `Iterator::filter` / `Iterator::find` pass `&Self::Item` to their predicate,
/// whereas TypeScript passes the value by value. `deref_closure_params` inserts
/// a `*` so that every reference to the closure's parameters inside the body
/// operates on the dereferenced value, matching TypeScript semantics.
///
/// Scope handling:
/// - Only the outermost closure's parameter names are eligible for rewriting.
/// - Nested closures temporarily shadow matching names while their body is
///   folded, so that `|x| xs.find(|x| *x == 0)` (inner `x` shadows outer) is
///   not doubly dereffed.
/// - `let x = ...` within the body is not treated as shadowing (out of scope
///   for iterator predicates, which rarely declare locals).
///
/// Applied only to `filter`/`find` — other iterator methods (`map`, `any`,
/// `all`, `for_each`, `fold`) pass `Self::Item` by value, so no deref is
/// required. Non-closure arguments pass through unchanged.
pub(super) fn deref_closure_params(expr: Expr) -> Expr {
    let Expr::Closure {
        params,
        return_type,
        body,
    } = expr
    else {
        return expr;
    };

    let param_names: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    let mut folder = DerefParams {
        params: param_names,
    };
    let new_body = match body {
        ClosureBody::Expr(inner) => ClosureBody::Expr(Box::new(folder.fold_expr(*inner))),
        ClosureBody::Block(stmts) => {
            ClosureBody::Block(stmts.into_iter().map(|s| folder.fold_stmt(s)).collect())
        }
    };
    Expr::Closure {
        params,
        return_type,
        body: new_body,
    }
}

struct DerefParams {
    params: HashSet<String>,
}

impl IrFolder for DerefParams {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Ident(name) if self.params.contains(&name) => {
                Expr::Deref(Box::new(Expr::Ident(name)))
            }
            Expr::Closure {
                params,
                return_type,
                body,
            } => {
                // Inner closure: its params shadow outer params of the same name
                // while its body is folded.
                let shadowed: Vec<String> = params
                    .iter()
                    .filter(|p| self.params.contains(&p.name))
                    .map(|p| p.name.clone())
                    .collect();
                for n in &shadowed {
                    self.params.remove(n);
                }
                let new_body = match body {
                    ClosureBody::Expr(inner) => ClosureBody::Expr(Box::new(self.fold_expr(*inner))),
                    ClosureBody::Block(stmts) => {
                        ClosureBody::Block(stmts.into_iter().map(|s| self.fold_stmt(s)).collect())
                    }
                };
                for n in shadowed {
                    self.params.insert(n);
                }
                Expr::Closure {
                    params,
                    return_type,
                    body: new_body,
                }
            }
            other => walk_expr(self, other),
        }
    }
}

/// Strips type annotations from closure parameters and return type.
///
/// Used for iterator method closures (`fold`, `sort_by`, etc.) where Rust's type
/// inference handles `&T` references correctly without explicit annotations.
pub(super) fn strip_closure_type_annotations(expr: Expr) -> Expr {
    match expr {
        Expr::Closure {
            params,
            return_type: _,
            body,
        } => Expr::Closure {
            params: params
                .into_iter()
                .map(|p| Param {
                    name: p.name,
                    ty: None,
                })
                .collect(),
            return_type: None,
            body,
        },
        other => other,
    }
}

/// Wraps a TS sort comparator closure body with `partial_cmp(&0.0).unwrap()`.
///
/// TS comparators return a number (negative/zero/positive), but Rust's `sort_by`
/// expects `Ordering`. This wraps the body expression: `body` → `body.partial_cmp(&0.0).unwrap()`.
pub(super) fn wrap_sort_comparator_body(expr: Expr) -> Expr {
    match expr {
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            let new_body = match body {
                ClosureBody::Expr(inner) => {
                    let wrapped = Expr::MethodCall {
                        object: Box::new(Expr::MethodCall {
                            object: inner,
                            method: "partial_cmp".to_string(),
                            args: vec![Expr::Ref(Box::new(Expr::NumberLit(0.0)))],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    };
                    ClosureBody::Expr(Box::new(wrapped))
                }
                other => other, // Block bodies — don't attempt to wrap
            };
            Expr::Closure {
                params,
                return_type,
                body: new_body,
            }
        }
        other => other,
    }
}
