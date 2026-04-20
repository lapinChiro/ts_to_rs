//! JS `coerce_default` table emission (I-144 T6-2).
//!
//! When the narrowing analyzer detects that a narrowed variable is
//! reassigned inside a closure body (`NarrowEvent::ClosureCapture`), the
//! Transformer suppresses the narrow shadow-let so the variable stays
//! `Option<T>`. Any subsequent T-expected read (`x + 1`, `"v=" + x`) must
//! then be wrapped to reproduce JS runtime coercion semantics:
//!
//! | TS type    | RC                         | null coerce    | implementation                                  |
//! |------------|----------------------------|----------------|-------------------------------------------------|
//! | `F64`      | RC1 arithmetic (`+`/`-`/...)| `0.0`          | `x.unwrap_or(0.0)`                              |
//! | `F64`      | RC6 string concat / interp | `"null"`       | `x.map(\|v\| v.to_string()).unwrap_or_else(...)` |
//!
//! T6-2 scope intentionally limits implementation to the (F64, RC1) and
//! (F64, RC6) cells required by Cell C-2b / C-2c. Other (type, RC) cells
//! return `None` and will be filled in by later T6 phases (T6-3 truthy
//! predicate / T6-4 OptChain narrow / T6-5 implicit None) or by future
//! umbrella PRDs (I-050 Any coercion).

use crate::ir::{ClosureBody, Expr, Param, RustType};
use crate::pipeline::narrowing_analyzer::RcContext;
use crate::transformer::build_option_unwrap_with_default;

/// Returns the JS `coerce_default(null, ...)` value for `(inner_ty, rc)`
/// as an IR expression.
///
/// `None` for combinations not in the T6-2 implementation scope; callers
/// fall through to the un-wrapped read in that case.
pub(crate) fn coerce_default_value(inner_ty: &RustType, rc: RcContext) -> Option<Expr> {
    match (inner_ty, rc) {
        (RustType::F64, RcContext::ExpectT) => Some(Expr::NumberLit(0.0)),
        _ => None,
    }
}

/// Wraps `option_expr: Option<inner_ty>` for an Expect-T (RC1) read site
/// such as an arithmetic operand, emitting
/// `option_expr.unwrap_or(coerce_default_value(inner_ty, ExpectT))`.
///
/// Returns `None` when the (type, RC) cell has no coerce default in the
/// T6-2 scope so the caller can fall through to the unmodified expression.
pub(crate) fn build_option_coerce_to_t(option_expr: Expr, inner_ty: &RustType) -> Option<Expr> {
    let default = coerce_default_value(inner_ty, RcContext::ExpectT)?;
    Some(build_option_unwrap_with_default(option_expr, default))
}

/// Wraps `option_expr: Option<inner_ty>` for an RC6 string-concat / template
/// interpolation read site, emitting
/// `option_expr.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string())`.
///
/// Returns `None` when `inner_ty` is not yet supported (T6-2 covers `F64`
/// only; later phases extend to additional Display-impl types).
pub(crate) fn build_option_coerce_to_string(
    option_expr: Expr,
    inner_ty: &RustType,
) -> Option<Expr> {
    if !matches!(inner_ty, RustType::F64) {
        return None;
    }
    let map_call = Expr::MethodCall {
        object: Box::new(option_expr),
        method: "map".to_string(),
        args: vec![Expr::Closure {
            params: vec![Param {
                name: "v".to_string(),
                ty: None,
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("v".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            })),
        }],
    };
    let null_default = Expr::MethodCall {
        object: Box::new(Expr::StringLit("null".to_string())),
        method: "to_string".to_string(),
        args: vec![],
    };
    Some(Expr::MethodCall {
        object: Box::new(map_call),
        method: "unwrap_or_else".to_string(),
        args: vec![Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(null_default)),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerce_default_value_f64_expect_t_is_zero() {
        let v = coerce_default_value(&RustType::F64, RcContext::ExpectT);
        assert!(matches!(v, Some(Expr::NumberLit(n)) if n == 0.0));
    }

    #[test]
    fn coerce_default_value_unsupported_combinations_return_none() {
        // T6-2 scope: only (F64, ExpectT). Others are not yet populated.
        assert!(coerce_default_value(&RustType::F64, RcContext::StringInterp).is_none());
        assert!(coerce_default_value(&RustType::F64, RcContext::Boolean).is_none());
        assert!(coerce_default_value(&RustType::String, RcContext::ExpectT).is_none());
        assert!(coerce_default_value(&RustType::Bool, RcContext::ExpectT).is_none());
    }

    #[test]
    fn build_option_coerce_to_t_emits_unwrap_or_zero_for_f64() {
        let x = Expr::Ident("x".to_string());
        let wrapped = build_option_coerce_to_t(x, &RustType::F64).expect("F64 supported");
        match wrapped {
            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                assert_eq!(method, "unwrap_or");
                assert!(matches!(*object, Expr::Ident(ref n) if n == "x"));
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0], Expr::NumberLit(n) if n == 0.0));
            }
            other => panic!("expected MethodCall(unwrap_or), got {other:?}"),
        }
    }

    #[test]
    fn build_option_coerce_to_t_unsupported_returns_none() {
        let x = Expr::Ident("x".to_string());
        assert!(build_option_coerce_to_t(x.clone(), &RustType::String).is_none());
        assert!(build_option_coerce_to_t(x, &RustType::Bool).is_none());
    }

    #[test]
    fn build_option_coerce_to_string_emits_map_to_string_unwrap_or_else_null() {
        let x = Expr::Ident("x".to_string());
        let wrapped = build_option_coerce_to_string(x, &RustType::F64).expect("F64 supported");
        let Expr::MethodCall {
            object,
            method,
            args,
        } = wrapped
        else {
            panic!("expected outer MethodCall");
        };
        assert_eq!(method, "unwrap_or_else");
        // outer object: x.map(|v| v.to_string())
        let Expr::MethodCall {
            object: map_obj,
            method: map_method,
            args: map_args,
        } = *object
        else {
            panic!("expected inner MethodCall(map)");
        };
        assert_eq!(map_method, "map");
        assert!(matches!(*map_obj, Expr::Ident(ref n) if n == "x"));
        assert_eq!(map_args.len(), 1);
        // map closure body should be `v.to_string()`
        let Expr::Closure { body, .. } = &map_args[0] else {
            panic!("expected Closure as map arg");
        };
        let ClosureBody::Expr(map_body) = body else {
            panic!("expected Expr ClosureBody");
        };
        let Expr::MethodCall {
            method: ts_method, ..
        } = map_body.as_ref()
        else {
            panic!("expected to_string MethodCall");
        };
        assert_eq!(ts_method, "to_string");
        // unwrap_or_else closure body: `"null".to_string()`
        assert_eq!(args.len(), 1);
        let Expr::Closure {
            body: fallback_body,
            ..
        } = &args[0]
        else {
            panic!("expected Closure as unwrap_or_else arg");
        };
        let ClosureBody::Expr(fallback_expr) = fallback_body else {
            panic!("expected Expr ClosureBody");
        };
        let Expr::MethodCall {
            object: lit_obj,
            method: ts_method2,
            ..
        } = fallback_expr.as_ref()
        else {
            panic!("expected to_string MethodCall on null lit");
        };
        assert_eq!(ts_method2, "to_string");
        assert!(matches!(lit_obj.as_ref(), Expr::StringLit(s) if s == "null"));
    }

    #[test]
    fn build_option_coerce_to_string_unsupported_returns_none() {
        let x = Expr::Ident("x".to_string());
        assert!(build_option_coerce_to_string(x.clone(), &RustType::String).is_none());
        assert!(build_option_coerce_to_string(x, &RustType::Bool).is_none());
    }
}
