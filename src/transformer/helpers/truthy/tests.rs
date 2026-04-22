//! Unit tests for `truthy.rs` helpers. Split to sibling file to keep the
//! production module under the 1000-LOC threshold.

use super::*;
use crate::ir::{CallTarget, Item, Param};
use crate::pipeline::synthetic_registry::SyntheticTypeRegistry;

fn empty_synth() -> SyntheticTypeRegistry {
    SyntheticTypeRegistry::new()
}

fn ident(name: &str) -> Expr {
    Expr::Ident(name.to_string())
}

// --- Existing Ident API regression ------------------------------------

#[test]
fn f64_truthy_emits_ne_zero_and_not_nan() {
    let expr = truthy_predicate("v", &RustType::F64).expect("F64 supported");
    assert_eq!(
        expr,
        Expr::BinaryOp {
            left: Box::new(Expr::BinaryOp {
                left: Box::new(ident("v")),
                op: BinOp::NotEq,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            op: BinOp::LogicalAnd,
            right: Box::new(Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(Expr::MethodCall {
                    object: Box::new(ident("v")),
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
                left: Box::new(ident("v")),
                op: BinOp::Eq,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            op: BinOp::LogicalOr,
            right: Box::new(Expr::MethodCall {
                object: Box::new(ident("v")),
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
                object: Box::new(ident("s")),
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
            object: Box::new(ident("s")),
            method: "is_empty".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn bool_truthy_is_identity() {
    let expr = truthy_predicate("flag", &RustType::Bool).expect("Bool supported");
    assert_eq!(expr, ident("flag"));
}

#[test]
fn bool_falsy_is_negation() {
    let expr = falsy_predicate("flag", &RustType::Bool).expect("Bool supported");
    assert_eq!(
        expr,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(ident("flag")),
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
            left: Box::new(ident("n")),
            op: BinOp::NotEq,
            right: Box::new(Expr::IntLit(0)),
        }
    );
}

#[test]
fn primitive_int_falsy_emits_eq_zero() {
    let expr = falsy_predicate("n", &RustType::Primitive(PrimitiveIntKind::Usize))
        .expect("Primitive int supported");
    assert_eq!(
        expr,
        Expr::BinaryOp {
            left: Box::new(ident("n")),
            op: BinOp::Eq,
            right: Box::new(Expr::IntLit(0)),
        }
    );
}

#[test]
fn ident_api_returns_none_for_option() {
    assert!(truthy_predicate("x", &RustType::Option(Box::new(RustType::F64))).is_none());
}

#[test]
fn ident_api_returns_none_for_named() {
    assert!(truthy_predicate(
        "x",
        &RustType::Named {
            name: "Foo".into(),
            type_args: vec![]
        }
    )
    .is_none());
}

// --- is_always_truthy_type --------------------------------------------

#[test]
fn is_always_truthy_vec_fn_stdcollection_dyntrait_ref_tuple() {
    let synth = empty_synth();
    assert!(is_always_truthy_type(
        &RustType::Vec(Box::new(RustType::F64)),
        &synth
    ));
    assert!(is_always_truthy_type(
        &RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::F64),
        },
        &synth
    ));
    assert!(is_always_truthy_type(
        &RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        },
        &synth
    ));
    assert!(is_always_truthy_type(
        &RustType::DynTrait("MyTrait".to_string()),
        &synth
    ));
    assert!(is_always_truthy_type(
        &RustType::Ref(Box::new(RustType::String)),
        &synth
    ));
    assert!(is_always_truthy_type(
        &RustType::Tuple(vec![RustType::F64, RustType::String]),
        &synth
    ));
}

#[test]
fn is_always_truthy_named_struct_is_true() {
    let synth = empty_synth();
    // Unknown Named (not registered as synthetic union) is treated as struct
    // / non-union enum and instance is always truthy.
    assert!(is_always_truthy_type(
        &RustType::Named {
            name: "MyStruct".into(),
            type_args: vec![]
        },
        &synth
    ));
}

#[test]
fn is_always_truthy_false_for_primitives_and_option() {
    let synth = empty_synth();
    for ty in [
        RustType::Bool,
        RustType::F64,
        RustType::String,
        RustType::Primitive(PrimitiveIntKind::I32),
        RustType::Option(Box::new(RustType::F64)),
        RustType::Any,
        RustType::Never,
        RustType::Unit,
        RustType::TypeVar {
            name: "T".to_string(),
        },
    ] {
        assert!(
            !is_always_truthy_type(&ty, &synth),
            "{ty:?} should not be always truthy"
        );
    }
}

#[test]
fn is_always_truthy_synthetic_union_is_false() {
    let mut synth = empty_synth();
    synth.push_item(
        "UF64OrString".to_string(),
        SyntheticTypeKind::UnionEnum,
        Item::Enum {
            vis: crate::ir::Visibility::Public,
            name: "UF64OrString".to_string(),
            type_params: vec![],
            serde_tag: None,
            variants: vec![],
        },
    );
    assert!(!is_always_truthy_type(
        &RustType::Named {
            name: "UF64OrString".into(),
            type_args: vec![]
        },
        &synth
    ));
}

// --- TempBinder -------------------------------------------------------

#[test]
fn temp_binder_generates_unique_names() {
    let mut binder = TempBinder::new();
    let a = binder.fresh("op");
    let b = binder.fresh("op");
    let c = binder.fresh("eval");
    assert_eq!(a, "__ts_tmp_op_0");
    assert_eq!(b, "__ts_tmp_op_1");
    assert_eq!(c, "__ts_tmp_eval_2");
}

// --- truthy_predicate_for_expr: primitive cases -----------------------

#[test]
fn expr_truthy_bool_passthrough() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(&ident("b"), &RustType::Bool, &synth, &mut binder)
        .expect("Bool supported");
    assert_eq!(pred, ident("b"));
}

#[test]
fn expr_truthy_f64_pure_ident() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(&ident("x"), &RustType::F64, &synth, &mut binder)
        .expect("F64 supported");
    // `x != 0.0 && !x.is_nan()` — no tmp binding for pure Ident.
    assert!(matches!(
        pred,
        Expr::BinaryOp {
            op: BinOp::LogicalAnd,
            ..
        }
    ));
}

#[test]
fn expr_truthy_f64_non_pure_uses_tmp_bind() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let call = Expr::FnCall {
        target: CallTarget::Free("f".to_string()),
        args: vec![],
    };
    let pred = truthy_predicate_for_expr(&call, &RustType::F64, &synth, &mut binder)
        .expect("F64 supported");
    assert!(
        matches!(&pred, Expr::Block(stmts) if matches!(&stmts[0], Stmt::Let { name, .. } if name.starts_with("__ts_tmp_op_")))
    );
}

#[test]
fn expr_falsy_string_emits_is_empty() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = falsy_predicate_for_expr(&ident("s"), &RustType::String, &synth, &mut binder)
        .expect("String supported");
    assert_eq!(
        pred,
        Expr::MethodCall {
            object: Box::new(ident("s")),
            method: "is_empty".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn expr_truthy_int_primitive_uses_int_literal() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(
        &ident("n"),
        &RustType::Primitive(PrimitiveIntKind::I64),
        &synth,
        &mut binder,
    )
    .expect("Primitive int supported");
    assert_eq!(
        pred,
        Expr::BinaryOp {
            left: Box::new(ident("n")),
            op: BinOp::NotEq,
            right: Box::new(Expr::IntLit(0)),
        }
    );
}

// --- truthy_predicate_for_expr: Option<T> cases -----------------------

#[test]
fn expr_truthy_option_f64_uses_is_some_and_deref() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(
        &ident("x"),
        &RustType::Option(Box::new(RustType::F64)),
        &synth,
        &mut binder,
    )
    .expect("Option<F64> supported");
    // Top-level: `x.is_some_and(|v| ...)`
    let Expr::MethodCall { ref method, .. } = pred else {
        panic!("expected MethodCall, got {pred:?}");
    };
    assert_eq!(method, "is_some_and");
}

#[test]
fn expr_truthy_option_string_uses_as_ref_is_some_and() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(
        &ident("s"),
        &RustType::Option(Box::new(RustType::String)),
        &synth,
        &mut binder,
    )
    .expect("Option<String> supported");
    // Top-level: `<receiver>.is_some_and(|v| !v.is_empty())`
    // Receiver should be `s.as_ref()`
    let Expr::MethodCall {
        ref method,
        ref object,
        ..
    } = pred
    else {
        panic!("expected MethodCall, got {pred:?}");
    };
    assert_eq!(method, "is_some_and");
    assert!(matches!(
        object.as_ref(),
        Expr::MethodCall { method: m, .. } if m == "as_ref"
    ));
}

#[test]
fn expr_truthy_option_named_other_uses_is_some() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(
        &ident("opt"),
        &RustType::Option(Box::new(RustType::Named {
            name: "MyStruct".into(),
            type_args: vec![],
        })),
        &synth,
        &mut binder,
    )
    .expect("Option<Named other> supported");
    assert_eq!(
        pred,
        Expr::MethodCall {
            object: Box::new(ident("opt")),
            method: "is_some".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn expr_falsy_option_named_other_uses_is_none() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = falsy_predicate_for_expr(
        &ident("opt"),
        &RustType::Option(Box::new(RustType::Named {
            name: "MyStruct".into(),
            type_args: vec![],
        })),
        &synth,
        &mut binder,
    )
    .expect("Option<Named other> supported");
    assert_eq!(
        pred,
        Expr::MethodCall {
            object: Box::new(ident("opt")),
            method: "is_none".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn expr_truthy_option_vec_uses_is_some() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(
        &ident("opt"),
        &RustType::Option(Box::new(RustType::Vec(Box::new(RustType::F64)))),
        &synth,
        &mut binder,
    )
    .expect("Option<Vec> supported");
    assert_eq!(
        pred,
        Expr::MethodCall {
            object: Box::new(ident("opt")),
            method: "is_some".to_string(),
            args: vec![],
        }
    );
}

// --- truthy_predicate_for_expr: always-truthy cases -------------------

#[test]
fn expr_truthy_vec_const_true_for_pure() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(
        &ident("v"),
        &RustType::Vec(Box::new(RustType::F64)),
        &synth,
        &mut binder,
    )
    .expect("Vec always truthy");
    assert_eq!(pred, Expr::BoolLit(true));
}

#[test]
fn expr_falsy_vec_const_false_for_pure() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = falsy_predicate_for_expr(
        &ident("v"),
        &RustType::Vec(Box::new(RustType::F64)),
        &synth,
        &mut binder,
    )
    .expect("Vec always truthy");
    assert_eq!(pred, Expr::BoolLit(false));
}

#[test]
fn expr_truthy_named_struct_const_true() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let pred = truthy_predicate_for_expr(
        &ident("p"),
        &RustType::Named {
            name: "Point".into(),
            type_args: vec![],
        },
        &synth,
        &mut binder,
    )
    .expect("Named struct always truthy");
    assert_eq!(pred, Expr::BoolLit(true));
}

#[test]
fn expr_truthy_vec_with_side_effect_operand_wraps_block() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    let call = Expr::FnCall {
        target: CallTarget::Free("make_vec".to_string()),
        args: vec![],
    };
    let pred = truthy_predicate_for_expr(
        &call,
        &RustType::Vec(Box::new(RustType::F64)),
        &synth,
        &mut binder,
    )
    .expect("Vec always truthy");
    assert!(matches!(pred, Expr::Block(_)));
}

// --- truthy_predicate_for_expr: unsupported / blocked -----------------

#[test]
fn expr_truthy_any_returns_none() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    assert!(truthy_predicate_for_expr(&ident("x"), &RustType::Any, &synth, &mut binder).is_none());
}

#[test]
fn expr_truthy_typevar_returns_none() {
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    assert!(truthy_predicate_for_expr(
        &ident("x"),
        &RustType::TypeVar {
            name: "T".to_string()
        },
        &synth,
        &mut binder,
    )
    .is_none());
}

#[test]
fn expr_truthy_option_option_returns_none() {
    // Should be unreachable from valid TS input; lock in None contract.
    let synth = empty_synth();
    let mut binder = TempBinder::new();
    assert!(truthy_predicate_for_expr(
        &ident("x"),
        &RustType::Option(Box::new(RustType::Option(Box::new(RustType::F64)))),
        &synth,
        &mut binder,
    )
    .is_none());
}

// --- try_constant_fold_bang ------------------------------------------

fn dummy_span() -> swc_common::Span {
    swc_common::DUMMY_SP
}

#[test]
fn const_fold_null_to_true() {
    let e = ast::Expr::Lit(ast::Lit::Null(ast::Null { span: dummy_span() }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(true)));
}

#[test]
fn const_fold_undefined_ident_to_true() {
    let e = ast::Expr::Ident(ast::Ident {
        span: dummy_span(),
        sym: "undefined".into(),
        optional: false,
        ctxt: Default::default(),
    });
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(true)));
}

#[test]
fn const_fold_bool_true_to_false() {
    let e = ast::Expr::Lit(ast::Lit::Bool(ast::Bool {
        span: dummy_span(),
        value: true,
    }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(false)));
}

#[test]
fn const_fold_bool_false_to_true() {
    let e = ast::Expr::Lit(ast::Lit::Bool(ast::Bool {
        span: dummy_span(),
        value: false,
    }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(true)));
}

#[test]
fn const_fold_num_zero_to_true() {
    let e = ast::Expr::Lit(ast::Lit::Num(ast::Number {
        span: dummy_span(),
        value: 0.0,
        raw: None,
    }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(true)));
}

#[test]
fn const_fold_num_nan_to_true() {
    let e = ast::Expr::Lit(ast::Lit::Num(ast::Number {
        span: dummy_span(),
        value: f64::NAN,
        raw: None,
    }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(true)));
}

#[test]
fn const_fold_num_non_zero_to_false() {
    let e = ast::Expr::Lit(ast::Lit::Num(ast::Number {
        span: dummy_span(),
        value: 42.0,
        raw: None,
    }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(false)));
}

#[test]
fn const_fold_empty_string_to_true() {
    let e = ast::Expr::Lit(ast::Lit::Str(ast::Str {
        span: dummy_span(),
        value: "".into(),
        raw: None,
    }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(true)));
}

#[test]
fn const_fold_non_empty_string_to_false() {
    let e = ast::Expr::Lit(ast::Lit::Str(ast::Str {
        span: dummy_span(),
        value: "hi".into(),
        raw: None,
    }));
    assert_eq!(try_constant_fold_bang(&e), Some(Expr::BoolLit(false)));
}

#[test]
fn const_fold_ident_non_undefined_none() {
    let e = ast::Expr::Ident(ast::Ident {
        span: dummy_span(),
        sym: "x".into(),
        optional: false,
        ctxt: Default::default(),
    });
    assert_eq!(try_constant_fold_bang(&e), None);
}

#[test]
fn const_fold_call_not_folded() {
    let e = ast::Expr::Call(ast::CallExpr {
        span: dummy_span(),
        ctxt: Default::default(),
        callee: ast::Callee::Expr(Box::new(ast::Expr::Ident(ast::Ident {
            span: dummy_span(),
            sym: "f".into(),
            optional: false,
            ctxt: Default::default(),
        }))),
        args: vec![],
        type_args: None,
    });
    assert_eq!(try_constant_fold_bang(&e), None);
}

// --- Non-primitive RustType enumeration (backwards-compat lock-in) ---

/// Exhaustively exercises every `RustType` variant that is NOT a supported
/// primitive via the *existing* Ident API (`truthy_predicate`/`falsy_predicate`),
/// locking in the backwards-compat contract that it returns `None` for
/// those types so existing call sites continue to dispatch to match-based
/// emission paths. The new expr-level API handles these cases internally.
#[test]
fn ident_api_non_primitive_rust_types_return_none() {
    let samples: Vec<RustType> = vec![
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

// Suppress `Param` unused-import warning when `closure_param` changes signature.
fn _keep_param_alive(_: Param) {}
