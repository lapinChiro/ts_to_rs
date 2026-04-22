//! Matrix A primary cells: `&&=` × {14 types} × {Ident, Member} × {stmt, expr}.
//!
//! Each cell exercises the ideal emission shape from
//! `backlog/I-161-I-171-truthy-emission-batch.md` Matrix A (non-narrow scope).
//!
//! Cells are grouped by type for readability; each type yields 4 tests
//! (Ident/Member × stmt/expr). Always-truthy types (T8 group) additionally
//! assert const-fold behaviour.

use super::*;
use swc_ecma_ast::AssignOp;

// --- A-1 Bool ----------------------------------------------------------------

#[test]
fn a1_bool_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(bool_truthy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn a1_bool_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::AndAssign,
    );
    // Bool is Copy; tail is bare Ident.
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(bool_truthy(&ident_target()), ident_target(), rhs_ident()),
            ident_target(),
        )
    );
}

#[test]
fn a1_bool_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(bool_truthy(&member_target()), member_target(), rhs_ident())
    );
}

#[test]
fn a1_bool_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(bool_truthy(&member_target()), member_target(), rhs_ident()),
            member_target(),
        )
    );
}

// --- A-2 F64 -----------------------------------------------------------------

#[test]
fn a2_f64_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(f64_truthy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn a2_f64_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(f64_truthy(&ident_target()), ident_target(), rhs_ident()),
            ident_target(),
        )
    );
}

#[test]
fn a2_f64_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(f64_truthy(&member_target()), member_target(), rhs_ident())
    );
}

#[test]
fn a2_f64_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(f64_truthy(&member_target()), member_target(), rhs_ident()),
            member_target(),
        )
    );
}

// --- A-3 String (!Copy) ------------------------------------------------------

#[test]
fn a3_string_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(string_truthy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn a3_string_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::AndAssign,
    );
    // String is !Copy; tail is `x.clone()`.
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(string_truthy(&ident_target()), ident_target(), rhs_ident()),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn a3_string_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            string_truthy(&member_target()),
            member_target(),
            rhs_ident()
        )
    );
}

#[test]
fn a3_string_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                string_truthy(&member_target()),
                member_target(),
                rhs_ident()
            ),
            clone_call(member_target()),
        )
    );
}

// --- A-4 Primitive(int) ------------------------------------------------------

#[test]
fn a4_primitive_i32_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(int_truthy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn a4_primitive_i32_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(int_truthy(&ident_target()), ident_target(), rhs_ident()),
            ident_target(),
        )
    );
}

#[test]
fn a4_primitive_usize_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::Usize);
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(int_truthy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn a4_primitive_i128_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I128);
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(int_truthy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn a4_primitive_i32_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(int_truthy(&member_target()), member_target(), rhs_ident())
    );
}

#[test]
fn a4_primitive_i32_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(int_truthy(&member_target()), member_target(), rhs_ident()),
            member_target(),
        )
    );
}

// --- A-5 Option<F64> (Copy inner) --------------------------------------------

fn option_f64_type() -> RustType {
    RustType::Option(Box::new(RustType::F64))
}

#[test]
fn a5_option_f64_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::AndAssign,
    );
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_copy_primitive_truthy(&ident_target(), inner_truthy),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a5_option_f64_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::AndAssign,
    );
    // Option<F64> is Copy (F64 is Copy); tail is bare Ident.
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_copy_primitive_truthy(&ident_target(), inner_truthy),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            ident_target(),
        )
    );
}

#[test]
fn a5_option_f64_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::AndAssign,
    );
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_copy_primitive_truthy(&member_target(), inner_truthy),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a5_option_f64_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::AndAssign,
    );
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_copy_primitive_truthy(&member_target(), inner_truthy),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            member_target(),
        )
    );
}

// --- A-5s Option<String> (!Copy inner) ---------------------------------------

fn option_string_type() -> RustType {
    RustType::Option(Box::new(RustType::String))
}

#[test]
fn a5s_option_string_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_string_truthy(&ident_target()),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a5s_option_string_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::AndAssign,
    );
    // Option<String> is !Copy (String is !Copy); tail is `x.clone()`.
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_string_truthy(&ident_target()),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn a5s_option_string_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_string_truthy(&member_target()),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a5s_option_string_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_string_truthy(&member_target()),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(member_target()),
        )
    );
}

// --- A-6 Option<synthetic union> ---------------------------------------------

#[test]
fn a6_option_synthetic_union_ident_stmt() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_synth_union_truthy(&ident_target(), "F64OrString"),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a6_option_synthetic_union_ident_expr() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    // Option<Named> is !Copy; tail is `x.clone()`.
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_synth_union_truthy(&ident_target(), "F64OrString"),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn a6_option_synthetic_union_member_stmt() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_synth_union_truthy(&member_target(), "F64OrString"),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a6_option_synthetic_union_member_expr() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_synth_union_truthy(&member_target(), "F64OrString"),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(member_target()),
        )
    );
}

// --- A-7 Option<Named other> (non-synthetic-union) ---------------------------

fn option_named_other_type() -> RustType {
    RustType::Option(Box::new(RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    }))
}

#[test]
fn a7_option_named_other_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_is_some(&ident_target()),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a7_option_named_other_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::AndAssign,
    );
    // Option<Named> is !Copy.
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_is_some(&ident_target()),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn a7_option_named_other_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_is_some(&member_target()),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn a7_option_named_other_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::AndAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_is_some(&member_target()),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(member_target()),
        )
    );
}

// --- A-8 Always-truthy types (const-fold: &&= → unconditional assign) --------

/// Runs the 4-combination for an always-truthy LHS type. `&&=` const-folds
/// to unconditional `target = rhs;` regardless of LHS value.
fn run_always_truthy_and_assign(ty: &RustType, is_copy: bool) {
    let synth = SyntheticTypeRegistry::new();

    // Ident stmt
    let stmts = run_stmts(&synth, ident_target(), rhs_ident(), ty, AssignOp::AndAssign);
    assert_eq!(
        stmts,
        expected_stmt_unconditional_assign(ident_target(), rhs_ident()),
        "Ident stmt for {ty:?}",
    );

    // Member stmt
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        ty,
        AssignOp::AndAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_unconditional_assign(member_target(), rhs_ident()),
        "Member stmt for {ty:?}",
    );

    // Ident expr
    let expr = run_expr(&synth, ident_target(), rhs_ident(), ty, AssignOp::AndAssign);
    let expected_tail = if is_copy {
        ident_target()
    } else {
        clone_call(ident_target())
    };
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_unconditional_assign(ident_target(), rhs_ident()),
            expected_tail,
        ),
        "Ident expr for {ty:?}",
    );

    // Member expr
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        ty,
        AssignOp::AndAssign,
    );
    let expected_tail = if is_copy {
        member_target()
    } else {
        clone_call(member_target())
    };
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_unconditional_assign(member_target(), rhs_ident()),
            expected_tail,
        ),
        "Member expr for {ty:?}",
    );
}

#[test]
fn a8_named_struct_always_truthy() {
    run_always_truthy_and_assign(
        &RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        },
        false,
    );
}

#[test]
fn a8_vec_always_truthy() {
    run_always_truthy_and_assign(&RustType::Vec(Box::new(RustType::F64)), false);
}

#[test]
fn a8_hashmap_always_truthy() {
    run_always_truthy_and_assign(
        &RustType::StdCollection {
            kind: StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        },
        false,
    );
}

#[test]
fn a8_fn_always_truthy() {
    run_always_truthy_and_assign(
        &RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::F64),
        },
        false,
    );
}

#[test]
fn a8_dyntrait_always_truthy() {
    run_always_truthy_and_assign(&RustType::DynTrait("MyTrait".to_string()), false);
}

// --- A-12b Ref(T) (Copy) -----------------------------------------------------

#[test]
fn a12b_ref_t_always_truthy_copy() {
    // Ref(T) is always Copy per RustType::is_copy_type.
    run_always_truthy_and_assign(&RustType::Ref(Box::new(RustType::String)), true);
}

// --- A-12d Tuple (Copy varies by elements) -----------------------------------

#[test]
fn a12d_tuple_all_copy_always_truthy() {
    // (F64, F64) — all elements Copy → Tuple Copy.
    run_always_truthy_and_assign(&RustType::Tuple(vec![RustType::F64, RustType::F64]), true);
}

#[test]
fn a12d_tuple_with_string_not_copy_always_truthy() {
    // (F64, String) — String !Copy → Tuple !Copy.
    run_always_truthy_and_assign(
        &RustType::Tuple(vec![RustType::F64, RustType::String]),
        false,
    );
}
