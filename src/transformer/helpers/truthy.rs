//! JS truthy/falsy predicate emission (I-144 T6-3 E10).
//!
//! JavaScript の `if (x)` truthy semantics を Rust に 1:1 で再現するために、
//! 値の RustType に応じた predicate expression を組み立てる。
//!
//! | TS LHS type           | Truthy predicate                        | Source cell |
//! |-----------------------|-----------------------------------------|-------------|
//! | `F64`                 | `x != 0.0 && !x.is_nan()`               | cell-t4d    |
//! | `String`              | `!x.is_empty()`                          | cell-t4c    |
//! | `Bool`                | `x`                                     | —           |
//! | `Primitive(int)`      | `x != 0`                                | —           |
//!
//! Falsy predicate は truthy の De Morgan 反転。`if (!x)` の primitive emission
//! で使用する (`if (!x)` 早期 return + Option<Union> は per-variant match
//! consolidation — `try_generate_narrowing_match` 側で別ルート)。
//!
//! Option/Union 複合 (`Option<Union<T,U>>`) の composite truthy は
//! `try_generate_narrowing_match` 内 early-return branch で直接 match arm
//! guard として合成するため、本モジュールでは primitive 単体に限定する。

use crate::ir::{BinOp, Expr, PrimitiveIntKind, RustType, UnOp};

/// JS truthy predicate expression for a named variable of the given type.
///
/// Returns `None` for types not supported in the current T6-3 scope
/// (`Option`, `Named`, `Vec`, `Tuple`, `Fn`, `Any`, `Unknown`, ...).
/// Caller is expected to fall back to the existing Option narrow path or
/// emit the value directly when the predicate is not applicable.
pub(crate) fn truthy_predicate(name: &str, ty: &RustType) -> Option<Expr> {
    match ty {
        RustType::F64 => Some(f64_truthy(name)),
        RustType::String => Some(string_truthy(name)),
        RustType::Bool => Some(Expr::Ident(name.to_string())),
        RustType::Primitive(kind) => Some(int_truthy(name, *kind)),
        _ => None,
    }
}

/// JS falsy predicate (De Morgan inverse of [`truthy_predicate`]).
///
/// For `RustType::F64` this yields `x == 0.0 || x.is_nan()` (NaN falsy is
/// required for JS parity — naive `x == 0.0` misclassifies `NaN` as truthy).
pub(crate) fn falsy_predicate(name: &str, ty: &RustType) -> Option<Expr> {
    match ty {
        RustType::F64 => Some(f64_falsy(name)),
        RustType::String => Some(Expr::MethodCall {
            object: Box::new(Expr::Ident(name.to_string())),
            method: "is_empty".to_string(),
            args: vec![],
        }),
        RustType::Bool => Some(Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident(name.to_string())),
        }),
        RustType::Primitive(kind) => Some(int_falsy(name, *kind)),
        _ => None,
    }
}

fn f64_truthy(name: &str) -> Expr {
    let ne_zero = Expr::BinaryOp {
        left: Box::new(Expr::Ident(name.to_string())),
        op: BinOp::NotEq,
        right: Box::new(Expr::NumberLit(0.0)),
    };
    let not_nan = Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::MethodCall {
            object: Box::new(Expr::Ident(name.to_string())),
            method: "is_nan".to_string(),
            args: vec![],
        }),
    };
    Expr::BinaryOp {
        left: Box::new(ne_zero),
        op: BinOp::LogicalAnd,
        right: Box::new(not_nan),
    }
}

fn f64_falsy(name: &str) -> Expr {
    let eq_zero = Expr::BinaryOp {
        left: Box::new(Expr::Ident(name.to_string())),
        op: BinOp::Eq,
        right: Box::new(Expr::NumberLit(0.0)),
    };
    let is_nan = Expr::MethodCall {
        object: Box::new(Expr::Ident(name.to_string())),
        method: "is_nan".to_string(),
        args: vec![],
    };
    Expr::BinaryOp {
        left: Box::new(eq_zero),
        op: BinOp::LogicalOr,
        right: Box::new(is_nan),
    }
}

fn string_truthy(name: &str) -> Expr {
    Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::MethodCall {
            object: Box::new(Expr::Ident(name.to_string())),
            method: "is_empty".to_string(),
            args: vec![],
        }),
    }
}

fn int_truthy(name: &str, _kind: PrimitiveIntKind) -> Expr {
    Expr::BinaryOp {
        left: Box::new(Expr::Ident(name.to_string())),
        op: BinOp::NotEq,
        right: Box::new(Expr::IntLit(0)),
    }
}

fn int_falsy(name: &str, _kind: PrimitiveIntKind) -> Expr {
    Expr::BinaryOp {
        left: Box::new(Expr::Ident(name.to_string())),
        op: BinOp::Eq,
        right: Box::new(Expr::IntLit(0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_truthy_emits_ne_zero_and_not_nan() {
        let expr = truthy_predicate("v", &RustType::F64).expect("F64 supported");
        assert_eq!(
            expr,
            Expr::BinaryOp {
                left: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("v".to_string())),
                    op: BinOp::NotEq,
                    right: Box::new(Expr::NumberLit(0.0)),
                }),
                op: BinOp::LogicalAnd,
                right: Box::new(Expr::UnaryOp {
                    op: UnOp::Not,
                    operand: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("v".to_string())),
                        method: "is_nan".to_string(),
                        args: vec![],
                    }),
                }),
            }
        );
    }

    #[test]
    fn f64_falsy_is_de_morgan_inverse() {
        let expr = falsy_predicate("v", &RustType::F64).expect("F64 supported");
        assert_eq!(
            expr,
            Expr::BinaryOp {
                left: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("v".to_string())),
                    op: BinOp::Eq,
                    right: Box::new(Expr::NumberLit(0.0)),
                }),
                op: BinOp::LogicalOr,
                right: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("v".to_string())),
                    method: "is_nan".to_string(),
                    args: vec![],
                }),
            }
        );
    }

    #[test]
    fn string_truthy_emits_not_is_empty() {
        let expr = truthy_predicate("s", &RustType::String).expect("String supported");
        assert_eq!(
            expr,
            Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("s".to_string())),
                    method: "is_empty".to_string(),
                    args: vec![],
                }),
            }
        );
    }

    #[test]
    fn string_falsy_emits_is_empty() {
        let expr = falsy_predicate("s", &RustType::String).expect("String supported");
        assert_eq!(
            expr,
            Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }
        );
    }

    #[test]
    fn bool_truthy_is_identity() {
        let expr = truthy_predicate("flag", &RustType::Bool).expect("Bool supported");
        assert_eq!(expr, Expr::Ident("flag".to_string()));
    }

    #[test]
    fn bool_falsy_is_negation() {
        let expr = falsy_predicate("flag", &RustType::Bool).expect("Bool supported");
        assert_eq!(
            expr,
            Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(Expr::Ident("flag".to_string())),
            }
        );
    }

    #[test]
    fn int_truthy_emits_ne_zero() {
        let expr = truthy_predicate("n", &RustType::Primitive(PrimitiveIntKind::I32))
            .expect("Primitive int supported");
        assert_eq!(
            expr,
            Expr::BinaryOp {
                left: Box::new(Expr::Ident("n".to_string())),
                op: BinOp::NotEq,
                right: Box::new(Expr::IntLit(0)),
            }
        );
    }

    #[test]
    fn option_returns_none_out_of_scope() {
        // Option<T> の truthy は match 経路 (try_generate_narrowing_match) で処理するため、
        // 本モジュールでは明示的に None を返し、caller が分岐する契約。
        assert!(truthy_predicate("x", &RustType::Option(Box::new(RustType::F64))).is_none());
    }

    #[test]
    fn named_returns_none_out_of_scope() {
        assert!(truthy_predicate(
            "x",
            &RustType::Named {
                name: "Foo".into(),
                type_args: vec![]
            }
        )
        .is_none());
    }

    /// Exhaustively exercises every `RustType` variant that is NOT a supported
    /// primitive, locking in the contract that `truthy_predicate` /
    /// `falsy_predicate` return `None` so callers can dispatch to richer
    /// match-based narrow paths. Required by `testing.md` (type-partition
    /// exhaustiveness) after T6-3 introduced the new predicate entry point.
    #[test]
    fn non_primitive_rust_types_return_none() {
        let samples: std::vec::Vec<RustType> = vec![
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
            RustType::Option(Box::new(RustType::String)),
            RustType::Option(Box::new(RustType::Bool)),
            RustType::Option(Box::new(RustType::Option(Box::new(RustType::F64)))),
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
            assert!(
                truthy_predicate("x", &ty).is_none(),
                "truthy_predicate({ty:?}) must be None"
            );
            assert!(
                falsy_predicate("x", &ty).is_none(),
                "falsy_predicate({ty:?}) must be None"
            );
        }
    }

    #[test]
    fn primitive_int_falsy_emits_eq_zero() {
        let expr = falsy_predicate("n", &RustType::Primitive(PrimitiveIntKind::Usize))
            .expect("Primitive int supported");
        assert_eq!(
            expr,
            Expr::BinaryOp {
                left: Box::new(Expr::Ident("n".to_string())),
                op: BinOp::Eq,
                right: Box::new(Expr::IntLit(0)),
            }
        );
    }
}
