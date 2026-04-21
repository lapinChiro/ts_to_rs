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
//! (F64, RC6) cells required by Cell C-2b / C-2c. Each read context is
//! exposed as a dedicated builder (`build_option_coerce_to_t` /
//! `build_option_coerce_to_string`) rather than a single dispatch function
//! — the emission shapes are structurally different (scalar default vs
//! `map`/`unwrap_or_else` chain), so a shared dispatch enum would add no
//! cohesion and would violate YAGNI until at least three RC contexts
//! share an emission shape.

use crate::ir::{ClosureBody, Expr, Param, RustType};
use crate::transformer::build_option_unwrap_with_default;

/// Wraps `option_expr: Option<inner_ty>` for an Expect-T (RC1) read site
/// such as an arithmetic operand, emitting
/// `option_expr.unwrap_or(<JS coerce default>)`.
///
/// Returns `None` when `inner_ty` is not in the T6-2 scope (currently `F64`
/// only) so the caller can fall through to the unmodified expression.
pub(crate) fn build_option_coerce_to_t(option_expr: Expr, inner_ty: &RustType) -> Option<Expr> {
    let default = match inner_ty {
        RustType::F64 => Expr::NumberLit(0.0),
        _ => return None,
    };
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

    /// Exhaustively exercises every non-`F64` `RustType` variant to lock in
    /// the T6-2 scope contract: both builders return `None` so callers fall
    /// through to the un-wrapped read. Required by `testing.md`
    /// (type-partition exhaustiveness) and PRD Completion Criterion 5
    /// ("全 RustType variant × RC verify"). When a future phase adds a new
    /// (type, RC) cell to the coerce table, the corresponding variant
    /// leaves this list and gets a positive assertion.
    #[test]
    fn build_option_coerce_exhaustive_unsupported_types_return_none() {
        use crate::ir::PrimitiveIntKind;
        let samples: std::vec::Vec<RustType> = vec![
            RustType::String,
            RustType::Bool,
            RustType::Primitive(PrimitiveIntKind::I32),
            RustType::Primitive(PrimitiveIntKind::Usize),
            RustType::Vec(Box::new(RustType::F64)),
            RustType::Tuple(vec![RustType::F64, RustType::String]),
            RustType::Fn {
                params: vec![],
                return_type: Box::new(RustType::F64),
            },
            RustType::DynTrait("MyTrait".to_string()),
            RustType::Any,
            RustType::Unit,
            RustType::Never,
            RustType::Ref(Box::new(RustType::F64)),
            RustType::Result {
                ok: Box::new(RustType::F64),
                err: Box::new(RustType::String),
            },
            RustType::Option(Box::new(RustType::F64)),
            RustType::Named {
                name: "UserStruct".into(),
                type_args: vec![],
            },
            RustType::Named {
                name: "UserEnum".into(),
                type_args: vec![RustType::F64],
            },
            RustType::TypeVar {
                name: "T".to_string(),
            },
        ];
        for ty in samples {
            let x = Expr::Ident("x".to_string());
            assert!(
                build_option_coerce_to_t(x.clone(), &ty).is_none(),
                "build_option_coerce_to_t({ty:?}) must be None outside T6-2 F64 scope"
            );
            assert!(
                build_option_coerce_to_string(x, &ty).is_none(),
                "build_option_coerce_to_string({ty:?}) must be None outside T6-2 F64 scope"
            );
        }
    }
}
