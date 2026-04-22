//! Unit tests for expressions.rs helpers (compound-assign expected-type
//! propagation). Split to sibling file to keep the production module under
//! the 1000-LOC threshold.

use super::rhs_expected_for_compound;
use crate::ir::RustType;
use swc_ecma_ast as ast;

#[test]
fn nullish_assign_on_option_returns_inner() {
    let ty = RustType::Option(Box::new(RustType::F64));
    assert_eq!(
        rhs_expected_for_compound(&ast::AssignOp::NullishAssign, &ty),
        Some(RustType::F64)
    );
}

#[test]
fn nullish_assign_on_non_option_returns_none() {
    // Historical no-op: `??=` on non-Option is dead, skip propagation.
    assert_eq!(
        rhs_expected_for_compound(&ast::AssignOp::NullishAssign, &RustType::F64),
        None
    );
}

#[test]
fn and_assign_on_option_returns_inner() {
    let ty = RustType::Option(Box::new(RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    }));
    assert_eq!(
        rhs_expected_for_compound(&ast::AssignOp::AndAssign, &ty),
        Some(RustType::Named {
            name: "Point".to_string(),
            type_args: vec![]
        })
    );
}

#[test]
fn and_assign_on_non_option_returns_lhs_itself() {
    assert_eq!(
        rhs_expected_for_compound(&ast::AssignOp::AndAssign, &RustType::String),
        Some(RustType::String)
    );
}

#[test]
fn or_assign_on_option_returns_inner() {
    let ty = RustType::Option(Box::new(RustType::String));
    assert_eq!(
        rhs_expected_for_compound(&ast::AssignOp::OrAssign, &ty),
        Some(RustType::String)
    );
}

#[test]
fn or_assign_on_non_option_returns_lhs_itself() {
    assert_eq!(
        rhs_expected_for_compound(&ast::AssignOp::OrAssign, &RustType::Bool),
        Some(RustType::Bool)
    );
}

#[test]
fn other_compound_ops_return_none() {
    for op in [
        ast::AssignOp::AddAssign,
        ast::AssignOp::SubAssign,
        ast::AssignOp::MulAssign,
        ast::AssignOp::DivAssign,
        ast::AssignOp::ModAssign,
        ast::AssignOp::BitAndAssign,
        ast::AssignOp::BitOrAssign,
        ast::AssignOp::BitXorAssign,
        ast::AssignOp::LShiftAssign,
        ast::AssignOp::RShiftAssign,
        ast::AssignOp::ZeroFillRShiftAssign,
    ] {
        assert_eq!(
            rhs_expected_for_compound(&op, &RustType::F64),
            None,
            "{op:?} should not propagate"
        );
    }
}
