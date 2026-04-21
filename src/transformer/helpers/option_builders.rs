//! Option-shape IR construction helpers.
//!
//! Three cohesive builders share the "eager vs lazy dispatch on
//! [`Expr::is_copy_literal`]" convention to balance idiomatic Rust
//! emission with JS `??` / `??=` lazy semantics:
//!
//! | Builder | Output shape | Use site |
//! |---------|--------------|----------|
//! | [`build_option_unwrap_with_default`] | `opt.unwrap_or(d)` or `opt.unwrap_or_else(\|\| d)` → `T` | destructuring defaults, function param defaults, `??` operator (I-022) |
//! | [`build_option_get_or_insert_with`] | `x.get_or_insert_with(\|\| d);` → `&mut T` stmt | `??=` emission when the outer `Option<T>` must be preserved across a narrow-invalidating mutation (I-144 T6-1 `EmissionHint::GetOrInsertWith`) |
//! | [`build_option_or_option`] | `a.or(b)` or `a.or_else(\|\| b)` → `Option<T>` | `??` chain with Option LHS + Option RHS (I-022) |
//!
//! The three builders were collocated in `transformer/mod.rs` prior to
//! I-144 T6-6 (pre-existing broken window: mod.rs > 1000 LOC). Moving
//! them here both reduces mod.rs size and brings the cohesive Option-
//! shape builders into the existing `helpers/` module alongside
//! [`coerce_default`](super::coerce_default).

use crate::ir::{ClosureBody, Expr};

/// Builds an `unwrap_or` (eager) or `unwrap_or_else` (lazy) expression for an
/// `Option<T>` field with a default value, producing `T`.
///
/// Uses `unwrap_or` only for cheap Copy literals (numbers, bools, unit —
/// see [`Expr::is_copy_literal`]). Everything else uses `unwrap_or_else`
/// to avoid:
///
/// - Eager evaluation of side-effecting expressions (correctness)
/// - Unnecessary String/struct allocation when Option is Some (performance)
/// - Unconditional move of non-Copy values (ownership safety)
///
/// Single source of truth for Option unwrap-with-default emission, used by
/// destructuring defaults, function parameter defaults, and the `??`
/// operator (via [`super::super::convert_bin_expr`]).
pub(crate) fn build_option_unwrap_with_default(field_access: Expr, default_ir: Expr) -> Expr {
    if default_ir.is_copy_literal() {
        Expr::MethodCall {
            object: Box::new(field_access),
            method: "unwrap_or".to_string(),
            args: vec![default_ir],
        }
    } else {
        Expr::MethodCall {
            object: Box::new(field_access),
            method: "unwrap_or_else".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(default_ir)),
            }],
        }
    }
}

/// Builds `x.get_or_insert_with(|| default)` for `??=` emission when the
/// outer `Option<T>` must be preserved across a subsequent narrow-invalidating
/// mutation (direct reassign, null reassign, loop boundary, or closure
/// reassign — see
/// [`EmissionHint::GetOrInsertWith`](crate::pipeline::narrowing_analyzer::EmissionHint::GetOrInsertWith)).
///
/// Unlike [`build_option_unwrap_with_default`], which produces a bare `T`
/// suitable for shadow-let binding, this preserves the `Option<T>` shape so
/// that follow-up mutations (`x = None`, `for const v of ...` rebinding, or
/// a closure body reassigning the captured ident) can continue to typecheck.
/// The default is wrapped in a zero-arg closure regardless of Copy-ness: the
/// `get_or_insert_with` API is always lazy.
pub(crate) fn build_option_get_or_insert_with(target: Expr, default_ir: Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(target),
        method: "get_or_insert_with".to_string(),
        args: vec![Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(default_ir)),
        }],
    }
}

/// Builds an `Option::or` (eager) or `Option::or_else` (lazy) expression for
/// an Option LHS + Option RHS (nullish coalescing chain case), producing
/// `Option<T>`.
///
/// Uses `or` only for cheap Copy literals (see [`Expr::is_copy_literal`]).
/// Everything else uses `or_else` to avoid:
///
/// - Eager evaluation of side-effecting RHS expressions (correctness — TS
///   `??` is lazy, not evaluating RHS when LHS is non-nullish)
/// - Unnecessary allocation when LHS is Some
/// - Unconditional move of non-Copy values (ownership safety)
///
/// Unlike [`build_option_unwrap_with_default`] which produces `T` (unwrapped),
/// this returns `Option<T>` — the LHS's Option layer is preserved so an outer
/// `??` in a chain (`a ?? b ?? c`) can terminate with `.unwrap_or[_else]()`.
///
/// Used exclusively by [`super::super::convert_bin_expr`]'s
/// NullishCoalescing arm when both LHS and RHS produce `Option<T>` (I-022).
pub(crate) fn build_option_or_option(lhs: Expr, rhs: Expr) -> Expr {
    if rhs.is_copy_literal() {
        Expr::MethodCall {
            object: Box::new(lhs),
            method: "or".to_string(),
            args: vec![rhs],
        }
    } else {
        Expr::MethodCall {
            object: Box::new(lhs),
            method: "or_else".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(rhs)),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{CallTarget, Param, RustType};

    // --- build_option_unwrap_with_default ---

    /// Asserts the result is `unwrap_or` with the default as a direct
    /// argument (no closure).
    fn assert_unwrap_or(result: &Expr) {
        match result {
            Expr::MethodCall { method, args, .. } => {
                assert_eq!(method, "unwrap_or");
                assert_eq!(args.len(), 1);
                assert!(
                    !matches!(&args[0], Expr::Closure { .. }),
                    "unwrap_or should receive the value directly, not a closure"
                );
            }
            other => panic!("expected MethodCall, got {other:?}"),
        }
    }

    /// Asserts the result is `unwrap_or_else` with a zero-arg closure
    /// wrapping the default.
    fn assert_unwrap_or_else(result: &Expr) {
        match result {
            Expr::MethodCall { method, args, .. } => {
                assert_eq!(method, "unwrap_or_else");
                assert_eq!(args.len(), 1);
                match &args[0] {
                    Expr::Closure { params, .. } => {
                        assert!(params.is_empty(), "closure should have no parameters");
                    }
                    other => panic!("expected Closure argument, got {other:?}"),
                }
            }
            other => panic!("expected MethodCall, got {other:?}"),
        }
    }

    #[test]
    fn test_build_option_unwrap_number_lit_uses_unwrap_or() {
        let result =
            build_option_unwrap_with_default(Expr::Ident("x".to_string()), Expr::NumberLit(0.0));
        assert_unwrap_or(&result);
    }

    #[test]
    fn test_build_option_unwrap_int_lit_uses_unwrap_or() {
        let result =
            build_option_unwrap_with_default(Expr::Ident("x".to_string()), Expr::IntLit(42));
        assert_unwrap_or(&result);
    }

    #[test]
    fn test_build_option_unwrap_bool_lit_uses_unwrap_or() {
        let result =
            build_option_unwrap_with_default(Expr::Ident("x".to_string()), Expr::BoolLit(false));
        assert_unwrap_or(&result);
    }

    #[test]
    fn test_build_option_unwrap_string_lit_uses_unwrap_or_else() {
        let result = build_option_unwrap_with_default(
            Expr::Ident("x".to_string()),
            Expr::StringLit("hello".to_string()),
        );
        assert_unwrap_or_else(&result);
    }

    #[test]
    fn test_build_option_unwrap_fn_call_uses_unwrap_or_else() {
        let result = build_option_unwrap_with_default(
            Expr::Ident("x".to_string()),
            Expr::FnCall {
                target: CallTarget::Free("compute_default".to_string()),
                args: vec![],
            },
        );
        assert_unwrap_or_else(&result);
    }

    #[test]
    fn test_build_option_unwrap_struct_init_uses_unwrap_or_else() {
        let result = build_option_unwrap_with_default(
            Expr::Ident("x".to_string()),
            Expr::StructInit {
                name: "Foo".to_string(),
                fields: vec![("a".to_string(), Expr::IntLit(0))],
                base: None,
            },
        );
        assert_unwrap_or_else(&result);
    }

    #[test]
    fn test_build_option_unwrap_ident_uses_unwrap_or_else() {
        // Non-literal identifier — behavior preservation requires lazy
        // evaluation (ident might be a mutable variable or costly to move).
        let result = build_option_unwrap_with_default(
            Expr::Ident("x".to_string()),
            Expr::Ident("default_var".to_string()),
        );
        assert_unwrap_or_else(&result);
    }

    #[test]
    fn test_build_option_unwrap_closure_uses_unwrap_or_else() {
        let result = build_option_unwrap_with_default(
            Expr::Ident("x".to_string()),
            Expr::Closure {
                params: vec![Param {
                    name: "y".to_string(),
                    ty: Some(RustType::F64),
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::IntLit(0))),
            },
        );
        assert_unwrap_or_else(&result);
    }

    // --- build_option_get_or_insert_with ---

    /// Asserts `target.get_or_insert_with(|| <body>)` shape. Unlike
    /// `unwrap_or` / `unwrap_or_else`, `get_or_insert_with` is ALWAYS lazy
    /// (closure-wrapped default) regardless of default_ir is_copy_literal.
    /// The API requires `FnOnce() -> T`, so a bare value would not typecheck.
    fn assert_get_or_insert_with_closure(result: &Expr, expected_target_ident: &str) {
        let Expr::MethodCall {
            object,
            method,
            args,
        } = result
        else {
            panic!("expected MethodCall, got {result:?}");
        };
        assert_eq!(method, "get_or_insert_with");
        match object.as_ref() {
            Expr::Ident(n) => assert_eq!(n, expected_target_ident),
            other => panic!("expected target Ident({expected_target_ident}), got {other:?}"),
        }
        assert_eq!(args.len(), 1);
        let Expr::Closure { params, .. } = &args[0] else {
            panic!("expected Closure argument, got {:?}", args[0]);
        };
        assert!(
            params.is_empty(),
            "get_or_insert_with closure must take zero params"
        );
    }

    #[test]
    fn test_build_option_get_or_insert_with_number_lit_wraps_in_closure() {
        let result =
            build_option_get_or_insert_with(Expr::Ident("x".to_string()), Expr::NumberLit(0.0));
        assert_get_or_insert_with_closure(&result, "x");
    }

    #[test]
    fn test_build_option_get_or_insert_with_string_lit_wraps_in_closure() {
        let result = build_option_get_or_insert_with(
            Expr::Ident("x".to_string()),
            Expr::StringLit("default".to_string()),
        );
        assert_get_or_insert_with_closure(&result, "x");
    }

    #[test]
    fn test_build_option_get_or_insert_with_fn_call_wraps_in_closure() {
        let result = build_option_get_or_insert_with(
            Expr::Ident("x".to_string()),
            Expr::FnCall {
                target: CallTarget::Free("make_default".to_string()),
                args: vec![],
            },
        );
        assert_get_or_insert_with_closure(&result, "x");
    }

    #[test]
    fn test_build_option_get_or_insert_with_preserves_target_as_object() {
        // The target expression appears as the method object, not moved into
        // the closure body. Ensures x stays `Option<T>` after the call.
        let result =
            build_option_get_or_insert_with(Expr::Ident("x".to_string()), Expr::NumberLit(0.0));
        let Expr::MethodCall { object, args, .. } = result else {
            panic!("expected MethodCall");
        };
        assert!(matches!(*object, Expr::Ident(ref n) if n == "x"));
        // Closure body carries the default value.
        let Expr::Closure { body, .. } = &args[0] else {
            panic!("expected Closure arg");
        };
        let ClosureBody::Expr(body_expr) = body else {
            panic!("expected Expr closure body");
        };
        assert!(matches!(body_expr.as_ref(), Expr::NumberLit(n) if *n == 0.0));
    }

    // --- build_option_or_option ---

    /// Asserts `lhs.or(rhs)` shape (eager, RHS directly as argument).
    fn assert_or_eager(result: &Expr) {
        match result {
            Expr::MethodCall { method, args, .. } => {
                assert_eq!(method, "or");
                assert_eq!(args.len(), 1);
                assert!(
                    !matches!(&args[0], Expr::Closure { .. }),
                    "or should receive the value directly, not a closure"
                );
            }
            other => panic!("expected MethodCall, got {other:?}"),
        }
    }

    /// Asserts `lhs.or_else(|| rhs)` shape (lazy closure-wrapped).
    fn assert_or_else_lazy(result: &Expr) {
        match result {
            Expr::MethodCall { method, args, .. } => {
                assert_eq!(method, "or_else");
                assert_eq!(args.len(), 1);
                let Expr::Closure { params, .. } = &args[0] else {
                    panic!("expected Closure arg, got {:?}", args[0]);
                };
                assert!(params.is_empty(), "or_else closure must take zero params");
            }
            other => panic!("expected MethodCall, got {other:?}"),
        }
    }

    #[test]
    fn test_build_option_or_number_lit_uses_or() {
        let result = build_option_or_option(Expr::Ident("a".to_string()), Expr::NumberLit(0.0));
        assert_or_eager(&result);
    }

    #[test]
    fn test_build_option_or_bool_lit_uses_or() {
        let result = build_option_or_option(Expr::Ident("a".to_string()), Expr::BoolLit(true));
        assert_or_eager(&result);
    }

    #[test]
    fn test_build_option_or_string_lit_uses_or_else() {
        let result = build_option_or_option(
            Expr::Ident("a".to_string()),
            Expr::StringLit("fallback".to_string()),
        );
        assert_or_else_lazy(&result);
    }

    #[test]
    fn test_build_option_or_fn_call_uses_or_else() {
        let result = build_option_or_option(
            Expr::Ident("a".to_string()),
            Expr::FnCall {
                target: CallTarget::Free("b".to_string()),
                args: vec![],
            },
        );
        assert_or_else_lazy(&result);
    }
}
