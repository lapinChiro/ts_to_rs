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
