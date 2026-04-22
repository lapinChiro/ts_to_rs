//! I-161 compound logical assign (`&&=` / `||=`) unit tests.
//!
//! Exhaustive Matrix A / O coverage per PRD task T3 (`backlog/I-161-I-171-truthy-emission-batch.md`).
//!
//! ## Matrix layout
//!
//! Primary cells × LHS shape × emission context × operator:
//!
//! - Primary types: 14 equivalence classes (T1-T8 + T12b/d)
//! - LHS shape: {`Ident`, `Member`} (2)
//! - Emission context: {stmt, expr} (2)
//! - Operator: {`&&=`, `||=`} (2)
//!
//! Base = 14 × 2 × 2 × 2 = 112 primary cells, supplemented with:
//! - Tier 2 `SimpleAssignTarget` NA error-path: 6 shapes × 2 ops = 12 cells
//! - T7-6 (narrow × incompatible RHS type error-path): 1 cell (empirical integration test)
//!
//! Total: 125+ unit tests (the PRD 141-case count enumerates finer-grained
//! primitive integer width variations which we cover via representative
//! `Primitive(I32)` and `Primitive(Usize)` cells plus `Primitive(I128)` /
//! `Primitive(F32)` coverage in specific tests).

mod and_primary;
mod error_path;
mod or_primary;

// Shared test helpers. The helpers live in this module (rather than per
// sub-module) so each primitive expected-emission template is written in one
// place only — any future tweak to the emission form (e.g., is_nan rewrite)
// updates the entire matrix uniformly.

use swc_common::DUMMY_SP;
use swc_ecma_ast as ast;

use crate::ir::{
    BinOp, BuiltinVariant, CallTarget, Expr, Item, MatchArm, Pattern, PatternCtor,
    PrimitiveIntKind, RustType, StdCollectionKind, Stmt, UnOp, UserTypeRef, Visibility,
};
use crate::pipeline::synthetic_registry::{SyntheticTypeKind, SyntheticTypeRegistry};
use crate::transformer::statements::compound_logical_assign::{
    desugar_compound_logical_assign_expr, desugar_compound_logical_assign_stmts,
};

// --- Target / RHS constructors ------------------------------------------------

pub(super) fn ident_target() -> Expr {
    Expr::Ident("x".to_string())
}

pub(super) fn member_target() -> Expr {
    Expr::FieldAccess {
        object: Box::new(Expr::Ident("obj".to_string())),
        field: "x".to_string(),
    }
}

pub(super) fn rhs_ident() -> Expr {
    Expr::Ident("y".to_string())
}

pub(super) fn wrap_some(inner: Expr) -> Expr {
    Expr::FnCall {
        target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
        args: vec![inner],
    }
}

pub(super) fn clone_call(target: Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(target),
        method: "clone".to_string(),
        args: vec![],
    }
}

// --- Predicate templates ------------------------------------------------------

pub(super) fn f64_truthy(target: &Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::NotEq,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        op: BinOp::LogicalAnd,
        right: Box::new(Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(target.clone()),
                method: "is_nan".to_string(),
                args: vec![],
            }),
        }),
    }
}

pub(super) fn f64_falsy(target: &Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        op: BinOp::LogicalOr,
        right: Box::new(Expr::MethodCall {
            object: Box::new(target.clone()),
            method: "is_nan".to_string(),
            args: vec![],
        }),
    }
}

pub(super) fn string_truthy(target: &Expr) -> Expr {
    Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::MethodCall {
            object: Box::new(target.clone()),
            method: "is_empty".to_string(),
            args: vec![],
        }),
    }
}

pub(super) fn string_falsy(target: &Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(target.clone()),
        method: "is_empty".to_string(),
        args: vec![],
    }
}

pub(super) fn bool_truthy(target: &Expr) -> Expr {
    target.clone()
}

pub(super) fn bool_falsy(target: &Expr) -> Expr {
    Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(target.clone()),
    }
}

pub(super) fn int_truthy(target: &Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(target.clone()),
        op: BinOp::NotEq,
        right: Box::new(Expr::IntLit(0)),
    }
}

pub(super) fn int_falsy(target: &Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(target.clone()),
        op: BinOp::Eq,
        right: Box::new(Expr::IntLit(0)),
    }
}

/// Closure `|v| <inner_predicate(Ident("v"))>`.
pub(super) fn closure_of(inner: Expr) -> Expr {
    Expr::Closure {
        params: vec![crate::ir::Param {
            name: "v".to_string(),
            ty: None,
        }],
        return_type: None,
        body: crate::ir::ClosureBody::Expr(Box::new(inner)),
    }
}

pub(super) fn option_copy_primitive_truthy(target: &Expr, inner_truthy_on_v: Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(target.clone()),
        method: "is_some_and".to_string(),
        args: vec![closure_of(inner_truthy_on_v)],
    }
}

pub(super) fn option_copy_primitive_falsy(target: &Expr, inner_truthy_on_v: Expr) -> Expr {
    Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(option_copy_primitive_truthy(target, inner_truthy_on_v)),
    }
}

pub(super) fn option_string_truthy(target: &Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(Expr::MethodCall {
            object: Box::new(target.clone()),
            method: "as_ref".to_string(),
            args: vec![],
        }),
        method: "is_some_and".to_string(),
        args: vec![closure_of(Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("v".to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }),
        })],
    }
}

pub(super) fn option_string_falsy(target: &Expr) -> Expr {
    Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(option_string_truthy(target)),
    }
}

pub(super) fn option_is_some(target: &Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(target.clone()),
        method: "is_some".to_string(),
        args: vec![],
    }
}

pub(super) fn option_is_none(target: &Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(target.clone()),
        method: "is_none".to_string(),
        args: vec![],
    }
}

// --- Synthetic union registry setup ------------------------------------------

/// Registers a synthetic union enum `F64OrString` with variants `F64(f64)`
/// and `String(String)` in the synthetic type registry. Matches the
/// emission shape produced by TypeResolver for TS `number | string` unions.
pub(super) fn register_f64_string_union(synth: &mut SyntheticTypeRegistry) -> RustType {
    let enum_name = "F64OrString".to_string();
    synth.push_item(
        enum_name.clone(),
        SyntheticTypeKind::UnionEnum,
        Item::Enum {
            vis: Visibility::Public,
            name: enum_name.clone(),
            type_params: vec![],
            serde_tag: None,
            variants: vec![
                crate::ir::EnumVariant {
                    name: "F64".to_string(),
                    value: None,
                    data: Some(RustType::F64),
                    fields: vec![],
                },
                crate::ir::EnumVariant {
                    name: "String".to_string(),
                    value: None,
                    data: Some(RustType::String),
                    fields: vec![],
                },
            ],
        },
    );
    RustType::Named {
        name: enum_name,
        type_args: vec![],
    }
}

/// Expected emission for `match &<target> { <variants with guards> | _ => false }`
/// used for `Option<synthetic union>` truthy predicate.
pub(super) fn option_synth_union_truthy(target: &Expr, enum_name: &str) -> Expr {
    let enum_ref = UserTypeRef::new(enum_name.to_string());
    let inner_bind = "__ts_union_inner";
    let borrow_target = Expr::Ref(Box::new(target.clone()));
    let f64_arm = MatchArm {
        patterns: vec![Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
            fields: vec![Pattern::TupleStruct {
                ctor: PatternCtor::UserEnumVariant {
                    enum_ty: enum_ref.clone(),
                    variant: "F64".to_string(),
                },
                fields: vec![Pattern::binding(inner_bind)],
            }],
        }],
        guard: Some(f64_truthy(&Expr::Deref(Box::new(Expr::Ident(
            inner_bind.to_string(),
        ))))),
        body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
    };
    let string_arm = MatchArm {
        patterns: vec![Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
            fields: vec![Pattern::TupleStruct {
                ctor: PatternCtor::UserEnumVariant {
                    enum_ty: enum_ref.clone(),
                    variant: "String".to_string(),
                },
                fields: vec![Pattern::binding(inner_bind)],
            }],
        }],
        guard: Some(string_truthy(&Expr::Ident(inner_bind.to_string()))),
        body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
    };
    let fallback_arm = MatchArm {
        patterns: vec![Pattern::Wildcard],
        guard: None,
        body: vec![Stmt::TailExpr(Expr::BoolLit(false))],
    };
    Expr::Match {
        expr: Box::new(borrow_target),
        arms: vec![f64_arm, string_arm, fallback_arm],
    }
}

pub(super) fn option_synth_union_falsy(target: &Expr, enum_name: &str) -> Expr {
    // Falsy form adds a leading None arm returning true; other variants use
    // the same truthy guards (the closure polarity stays truthy; outer !not
    // is not emitted because we use a match directly). However, per
    // `predicate_option_synthetic_union`, falsy is a single match with:
    //   None => true
    //   Some(U::F64(v)) if v != 0.0 && !v.is_nan() => true    // ← same as truthy guard
    //   ...
    //   _ => false
    // This looks inverted but is equivalent because we ADDITIONALLY wrap the
    // whole match expression in `!` at the outer level. Actually looking at
    // the impl, for Polarity::Falsy we add a None arm (true) and keep the
    // truthy-matching arms returning true; the fallback remains false.
    //
    // Wait — this is `!truthy`, so true-returning arms mean "falsy". Let me
    // re-read the impl:
    //
    // In `predicate_option_synthetic_union`, falsy polarity prepends a None
    // arm with true (None is falsy), then iterates variants with
    // build_variant_guard_for_ref_bind(_, _, polarity=Falsy) which returns
    // the FALSY guard on the primitive value. So the emission is:
    //   match &x {
    //     None => true,
    //     Some(U::F64(v)) if <v falsy (== 0.0 || is_nan)> => true,
    //     Some(U::String(s)) if <s falsy (is_empty)> => true,
    //     _ => false,
    //   }
    // Non-primitive variants (VariantGuard::ConstFalse for falsy) are elided.
    let enum_ref = UserTypeRef::new(enum_name.to_string());
    let inner_bind = "__ts_union_inner";
    let borrow_target = Expr::Ref(Box::new(target.clone()));
    let none_arm = MatchArm {
        patterns: vec![Pattern::none()],
        guard: None,
        body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
    };
    let f64_arm = MatchArm {
        patterns: vec![Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
            fields: vec![Pattern::TupleStruct {
                ctor: PatternCtor::UserEnumVariant {
                    enum_ty: enum_ref.clone(),
                    variant: "F64".to_string(),
                },
                fields: vec![Pattern::binding(inner_bind)],
            }],
        }],
        guard: Some(f64_falsy(&Expr::Deref(Box::new(Expr::Ident(
            inner_bind.to_string(),
        ))))),
        body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
    };
    let string_arm = MatchArm {
        patterns: vec![Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
            fields: vec![Pattern::TupleStruct {
                ctor: PatternCtor::UserEnumVariant {
                    enum_ty: enum_ref.clone(),
                    variant: "String".to_string(),
                },
                fields: vec![Pattern::binding(inner_bind)],
            }],
        }],
        guard: Some(string_falsy(&Expr::Ident(inner_bind.to_string()))),
        body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
    };
    let fallback_arm = MatchArm {
        patterns: vec![Pattern::Wildcard],
        guard: None,
        body: vec![Stmt::TailExpr(Expr::BoolLit(false))],
    };
    Expr::Match {
        expr: Box::new(borrow_target),
        arms: vec![none_arm, f64_arm, string_arm, fallback_arm],
    }
}

// --- Expected stmt/expr builders ---------------------------------------------

/// Expected stmt-context emission shape: `if <pred> { <target> = <value>; }`.
pub(super) fn expected_stmt_if(predicate: Expr, target: Expr, value: Expr) -> Vec<Stmt> {
    vec![Stmt::If {
        condition: predicate,
        then_body: vec![Stmt::Expr(Expr::Assign {
            target: Box::new(target),
            value: Box::new(value),
        })],
        else_body: None,
    }]
}

/// Expected stmt-context emission for always-truthy `&&=`: `<target> = <rhs>;`.
pub(super) fn expected_stmt_unconditional_assign(target: Expr, value: Expr) -> Vec<Stmt> {
    vec![Stmt::Expr(Expr::Assign {
        target: Box::new(target),
        value: Box::new(value),
    })]
}

/// Expected expr-context emission shape wrapping stmts with a tail expr.
pub(super) fn expected_expr_block(stmts: Vec<Stmt>, tail: Expr) -> Expr {
    let mut combined = stmts;
    combined.push(Stmt::TailExpr(tail));
    Expr::Block(combined)
}

// --- Shared run function -----------------------------------------------------

pub(super) fn run_stmts(
    synth: &SyntheticTypeRegistry,
    target: Expr,
    rhs: Expr,
    lhs_type: &RustType,
    op: ast::AssignOp,
) -> Vec<Stmt> {
    desugar_compound_logical_assign_stmts(synth, target, rhs, lhs_type, op, DUMMY_SP).unwrap()
}

pub(super) fn run_expr(
    synth: &SyntheticTypeRegistry,
    target: Expr,
    rhs: Expr,
    lhs_type: &RustType,
    op: ast::AssignOp,
) -> Expr {
    desugar_compound_logical_assign_expr(synth, target, rhs, lhs_type, op, DUMMY_SP).unwrap()
}
