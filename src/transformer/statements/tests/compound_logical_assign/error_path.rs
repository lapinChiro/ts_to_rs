//! Tier 2 `SimpleAssignTarget` NA error-path + blocked-type error-path tests.
//!
//! PRD T3 (L740) 「error-path unit test で確認 × 6 Tier 2 variant × 2 op = **12 case**」。
//!
//! SimpleAssignTarget variants `SuperProp` / `Paren` / `OptChain` / `TsAs` /
//! `TsSatisfies` / `Invalid` are Tier 2 unsupported per `ast-variants.md` §9.
//! The existing `convert_assign_expr` path explicitly rejects non-Ident / non-Member
//! SimpleAssignTargets with `UnsupportedSyntaxError` in the compound-assign
//! entry block (see `assignments.rs:239-244`). This module cross-verifies the
//! core desugar refuses corresponding LHS types (Any / TypeVar) directly, and
//! the entry layer rejection is tested via integration in `assignments.rs`.
//!
//! Additionally covers T7-6 (narrow × incompatible RHS type error-path:
//! narrow-scope compound assign with type-incompatible RHS). Because
//! `desugar_compound_logical_assign_stmts` does not type-check RHS vs LHS
//! inner type (it relies on TypeResolver expected propagation via T3-TR),
//! the compile-time error surfaces at rustc. We cover the dispatch path
//! that propagates the `UnsupportedSyntaxError` for Any/TypeVar LHS, which
//! is the only emission-time error in scope.

use super::*;
use swc_common::DUMMY_SP;
use swc_ecma_ast::AssignOp;

use crate::transformer::statements::compound_logical_assign::{
    desugar_compound_logical_assign_expr, desugar_compound_logical_assign_stmts,
};

// --- Blocked LHS types: Any / TypeVar × {&&=, ||=} × {stmts, expr} = 8 cases -

#[test]
fn blocked_any_and_assign_stmts_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Any,
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(
        result.is_err(),
        "Any &&= must surface as UnsupportedSyntaxError"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Any") || err.contains("TypeVar"),
        "error message should mention Any/TypeVar; got: {err}"
    );
}

#[test]
fn blocked_any_or_assign_stmts_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Any,
        AssignOp::OrAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

#[test]
fn blocked_any_and_assign_expr_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Any,
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

#[test]
fn blocked_any_or_assign_expr_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Any,
        AssignOp::OrAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

#[test]
fn blocked_typevar_and_assign_stmts_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::TypeVar {
            name: "T".to_string(),
        },
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

#[test]
fn blocked_typevar_or_assign_stmts_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::TypeVar {
            name: "T".to_string(),
        },
        AssignOp::OrAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

#[test]
fn blocked_typevar_and_assign_expr_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::TypeVar {
            name: "T".to_string(),
        },
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

#[test]
fn blocked_typevar_or_assign_expr_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::TypeVar {
            name: "T".to_string(),
        },
        AssignOp::OrAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

// --- NA cells that return None predicate → UnsupportedSyntaxError ------------

/// T11 Never: unreachable by IR invariant, but if it reaches the dispatch
/// path it must surface as unsupported (no predicate is defined).
#[test]
fn blocked_never_and_assign_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Never,
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(result.is_err(), "Never predicate must be unsupported");
}

/// T12a Unit: ts_to_rs `void` variable. Matrix A-12a PRD notes Unit LHS has
/// no ts_to_rs emission path, but the core dispatch still returns err.
#[test]
fn blocked_unit_and_assign_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Unit,
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

/// T12e Result: user-visible Result local var is not produced by TS
/// conversion (empirically verified 2026-04-22 in PRD NA cell doc). If it
/// reaches the dispatch, we refuse.
#[test]
fn blocked_result_and_assign_returns_err() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Result {
            ok: Box::new(RustType::F64),
            err: Box::new(RustType::String),
        },
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(result.is_err());
}

// --- T7-6: narrow × incompatible RHS type error-path ------------------------
//
// PRD T7-6: `let x: number | null = 5; if (x !== null) { x &&= "text"; }`.
//
// The TS source is a TypeScript type error (cannot assign `string` to `number`).
// SWC parses it, so the IR pipeline still runs. The desugar layer does NOT
// type-check RHS vs LHS-inner type — we rely on TypeResolver expected-type
// propagation (T3-TR) to flag the mismatch in upstream contexts, and on
// rustc to surface the residual mismatch in the emitted Rust.
//
// The empirical contract is therefore: **the desugar produces well-formed IR
// without aborting**, and the type error surfaces at compile time, not as a
// silent miscompile (Tier 1) and not as an `UnsupportedSyntaxError`. This is
// the cohesion contract for narrow × logical-assign × type-incompatible RHS:
// the dispatch is structural-only and stays out of TypeResolver's lane.

/// `f64 LHS &&= string RHS` desugar succeeds at the IR layer; the
/// type mismatch is intentionally deferred to rustc.
#[test]
fn narrow_incompatible_rhs_f64_and_string_does_not_intercept() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        Expr::StringLit("text".to_string()),
        &RustType::F64,
        AssignOp::AndAssign,
        DUMMY_SP,
    );
    assert!(
        result.is_ok(),
        "narrow × incompatible RHS must NOT be intercepted at the desugar; \
         the type mismatch surfaces at rustc (Tier 2 compile error, not Tier 1 \
         silent miscompile, and not Tier 3 UnsupportedSyntaxError). got: {result:?}"
    );
    // Sanity: the emitted IR is the standard `if <truthy(x)> { x = <rhs>; }`
    // shape — no special handling, no type coercion, no None pattern.
    let stmts = result.unwrap();
    assert_eq!(stmts.len(), 1);
    let Stmt::If {
        condition,
        then_body,
        else_body,
    } = &stmts[0]
    else {
        panic!("expected Stmt::If, got {:?}", stmts[0]);
    };
    assert!(
        else_body.is_none(),
        "narrow × incompatible RHS &&= must emit a single-branch If, not else"
    );
    assert!(
        matches!(
            condition,
            Expr::BinaryOp {
                op: BinOp::LogicalAnd,
                ..
            }
        ),
        "F64 truthy predicate must be `<x> != 0.0 && !<x>.is_nan()` LogicalAnd, \
         got {condition:?}"
    );
    // `then_body` should contain the bare assignment with the (incompatible) RHS.
    // The string RHS travels through unmodified — rustc rejects the assign.
    assert_eq!(then_body.len(), 1);
    assert!(
        matches!(
            &then_body[0],
            Stmt::Expr(Expr::Assign { value, .. })
                if matches!(value.as_ref(), Expr::StringLit(s) if s == "text")
        ),
        "RHS string literal must travel unmodified to the assign; got {:?}",
        then_body[0]
    );
}

/// Symmetric `f64 LHS ||= string RHS`: same contract — defer to rustc.
#[test]
fn narrow_incompatible_rhs_f64_or_string_does_not_intercept() {
    let synth = SyntheticTypeRegistry::new();
    let result = desugar_compound_logical_assign_stmts(
        &synth,
        ident_target(),
        Expr::StringLit("text".to_string()),
        &RustType::F64,
        AssignOp::OrAssign,
        DUMMY_SP,
    );
    assert!(
        result.is_ok(),
        "||= cohesion contract: incompatible RHS surfaces at rustc, not at desugar"
    );
    let stmts = result.unwrap();
    assert_eq!(stmts.len(), 1);
    let Stmt::If {
        condition,
        then_body,
        ..
    } = &stmts[0]
    else {
        panic!("expected Stmt::If, got {:?}", stmts[0]);
    };
    // `||=` desugars to `if <falsy(x)> { x = <rhs>; }` — the F64 falsy
    // predicate is `<x> == 0.0 || <x>.is_nan()` (LogicalOr).
    assert!(
        matches!(
            condition,
            Expr::BinaryOp {
                op: BinOp::LogicalOr,
                ..
            }
        ),
        "F64 falsy predicate must be `<x> == 0.0 || <x>.is_nan()` LogicalOr, \
         got {condition:?}"
    );
    assert_eq!(then_body.len(), 1);
    assert!(matches!(
        &then_body[0],
        Stmt::Expr(Expr::Assign { value, .. })
            if matches!(value.as_ref(), Expr::StringLit(s) if s == "text")
    ));
}
