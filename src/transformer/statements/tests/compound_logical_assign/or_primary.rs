//! Matrix O primary cells: `||=` × {14 types} × {Ident, Member} × {stmt, expr}.
//!
//! Symmetric to `and_primary`, but with falsy-predicate emission and `||=`
//! const-fold semantic for always-truthy types (the falsy branch never
//! fires, so stmt-context emits empty stmts; expr-context emits just the
//! tail Ident/clone).

use super::*;
use swc_ecma_ast::AssignOp;

// --- O-1 Bool ----------------------------------------------------------------

#[test]
fn o1_bool_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(bool_falsy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn o1_bool_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(bool_falsy(&ident_target()), ident_target(), rhs_ident()),
            ident_target(),
        )
    );
}

#[test]
fn o1_bool_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(bool_falsy(&member_target()), member_target(), rhs_ident())
    );
}

#[test]
fn o1_bool_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::Bool,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(bool_falsy(&member_target()), member_target(), rhs_ident()),
            member_target(),
        )
    );
}

// --- O-2 F64 -----------------------------------------------------------------

#[test]
fn o2_f64_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(f64_falsy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn o2_f64_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(f64_falsy(&ident_target()), ident_target(), rhs_ident()),
            ident_target(),
        )
    );
}

#[test]
fn o2_f64_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(f64_falsy(&member_target()), member_target(), rhs_ident())
    );
}

#[test]
fn o2_f64_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::F64,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(f64_falsy(&member_target()), member_target(), rhs_ident()),
            member_target(),
        )
    );
}

// --- O-3 String --------------------------------------------------------------

#[test]
fn o3_string_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(string_falsy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn o3_string_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(string_falsy(&ident_target()), ident_target(), rhs_ident()),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn o3_string_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(string_falsy(&member_target()), member_target(), rhs_ident())
    );
}

#[test]
fn o3_string_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &RustType::String,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(string_falsy(&member_target()), member_target(), rhs_ident()),
            clone_call(member_target()),
        )
    );
}

// --- O-4 Primitive(int) ------------------------------------------------------

#[test]
fn o4_primitive_i32_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let stmts = run_stmts(&synth, ident_target(), rhs_ident(), &ty, AssignOp::OrAssign);
    assert_eq!(
        stmts,
        expected_stmt_if(int_falsy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn o4_primitive_i32_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let expr = run_expr(&synth, ident_target(), rhs_ident(), &ty, AssignOp::OrAssign);
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(int_falsy(&ident_target()), ident_target(), rhs_ident()),
            ident_target(),
        )
    );
}

#[test]
fn o4_primitive_usize_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::Usize);
    let stmts = run_stmts(&synth, ident_target(), rhs_ident(), &ty, AssignOp::OrAssign);
    assert_eq!(
        stmts,
        expected_stmt_if(int_falsy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn o4_primitive_i128_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I128);
    let stmts = run_stmts(&synth, ident_target(), rhs_ident(), &ty, AssignOp::OrAssign);
    assert_eq!(
        stmts,
        expected_stmt_if(int_falsy(&ident_target()), ident_target(), rhs_ident())
    );
}

#[test]
fn o4_primitive_i32_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(int_falsy(&member_target()), member_target(), rhs_ident())
    );
}

#[test]
fn o4_primitive_i32_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let ty = RustType::Primitive(PrimitiveIntKind::I32);
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(int_falsy(&member_target()), member_target(), rhs_ident()),
            member_target(),
        )
    );
}

// --- O-5 Option<F64> ---------------------------------------------------------

fn option_f64_type() -> RustType {
    RustType::Option(Box::new(RustType::F64))
}

#[test]
fn o5_option_f64_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::OrAssign,
    );
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_copy_primitive_falsy(&ident_target(), inner_truthy),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o5_option_f64_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::OrAssign,
    );
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_copy_primitive_falsy(&ident_target(), inner_truthy),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            ident_target(),
        )
    );
}

#[test]
fn o5_option_f64_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::OrAssign,
    );
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_copy_primitive_falsy(&member_target(), inner_truthy),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o5_option_f64_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &option_f64_type(),
        AssignOp::OrAssign,
    );
    let inner_truthy = f64_truthy(&Expr::Ident("v".to_string()));
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_copy_primitive_falsy(&member_target(), inner_truthy),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            member_target(),
        )
    );
}

// --- O-5s Option<String> -----------------------------------------------------

fn option_string_type() -> RustType {
    RustType::Option(Box::new(RustType::String))
}

#[test]
fn o5s_option_string_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_string_falsy(&ident_target()),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o5s_option_string_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_string_falsy(&ident_target()),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn o5s_option_string_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_string_falsy(&member_target()),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o5s_option_string_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &option_string_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_string_falsy(&member_target()),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(member_target()),
        )
    );
}

// --- O-6 Option<synthetic union> ---------------------------------------------

#[test]
fn o6_option_synthetic_union_ident_stmt() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let stmts = run_stmts(&synth, ident_target(), rhs_ident(), &ty, AssignOp::OrAssign);
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_synth_union_falsy(&ident_target(), "F64OrString"),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o6_option_synthetic_union_ident_expr() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let expr = run_expr(&synth, ident_target(), rhs_ident(), &ty, AssignOp::OrAssign);
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_synth_union_falsy(&ident_target(), "F64OrString"),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn o6_option_synthetic_union_member_stmt() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_synth_union_falsy(&member_target(), "F64OrString"),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o6_option_synthetic_union_member_expr() {
    let mut synth = SyntheticTypeRegistry::new();
    let union_ty = register_f64_string_union(&mut synth);
    let ty = RustType::Option(Box::new(union_ty));
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &ty,
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_synth_union_falsy(&member_target(), "F64OrString"),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(member_target()),
        )
    );
}

// --- O-7 Option<Named other> -------------------------------------------------

fn option_named_other_type() -> RustType {
    RustType::Option(Box::new(RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    }))
}

#[test]
fn o7_option_named_other_ident_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_is_none(&ident_target()),
            ident_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o7_option_named_other_ident_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        ident_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_is_none(&ident_target()),
                ident_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(ident_target()),
        )
    );
}

#[test]
fn o7_option_named_other_member_stmt() {
    let synth = SyntheticTypeRegistry::new();
    let stmts = run_stmts(
        &synth,
        member_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        stmts,
        expected_stmt_if(
            option_is_none(&member_target()),
            member_target(),
            wrap_some(rhs_ident()),
        )
    );
}

#[test]
fn o7_option_named_other_member_expr() {
    let synth = SyntheticTypeRegistry::new();
    let expr = run_expr(
        &synth,
        member_target(),
        rhs_ident(),
        &option_named_other_type(),
        AssignOp::OrAssign,
    );
    assert_eq!(
        expr,
        expected_expr_block(
            expected_stmt_if(
                option_is_none(&member_target()),
                member_target(),
                wrap_some(rhs_ident()),
            ),
            clone_call(member_target()),
        )
    );
}

// --- O-8 Always-truthy (const-fold: ||= → no-op) -----------------------------

/// Runs the 4-combination for always-truthy LHS with `||=`. Stmt-context
/// emits empty stmts (the assign branch never fires); expr-context emits
/// just the tail (original LHS value).
fn run_always_truthy_or_assign(ty: &RustType, is_copy: bool) {
    let synth = SyntheticTypeRegistry::new();

    // Ident stmt: empty
    let stmts = run_stmts(&synth, ident_target(), rhs_ident(), ty, AssignOp::OrAssign);
    assert_eq!(stmts, Vec::<Stmt>::new(), "Ident stmt for {ty:?}");

    // Member stmt: empty
    let stmts = run_stmts(&synth, member_target(), rhs_ident(), ty, AssignOp::OrAssign);
    assert_eq!(stmts, Vec::<Stmt>::new(), "Member stmt for {ty:?}");

    // Ident expr: just the tail
    let expr = run_expr(&synth, ident_target(), rhs_ident(), ty, AssignOp::OrAssign);
    let expected_tail = if is_copy {
        ident_target()
    } else {
        clone_call(ident_target())
    };
    assert_eq!(expr, expected_tail, "Ident expr for {ty:?}");

    // Member expr: just the tail
    let expr = run_expr(&synth, member_target(), rhs_ident(), ty, AssignOp::OrAssign);
    let expected_tail = if is_copy {
        member_target()
    } else {
        clone_call(member_target())
    };
    assert_eq!(expr, expected_tail, "Member expr for {ty:?}");
}

#[test]
fn o8_named_struct_no_op() {
    run_always_truthy_or_assign(
        &RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        },
        false,
    );
}

#[test]
fn o8_vec_no_op() {
    run_always_truthy_or_assign(&RustType::Vec(Box::new(RustType::F64)), false);
}

#[test]
fn o8_hashmap_no_op() {
    run_always_truthy_or_assign(
        &RustType::StdCollection {
            kind: StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        },
        false,
    );
}

#[test]
fn o8_fn_no_op() {
    run_always_truthy_or_assign(
        &RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::F64),
        },
        false,
    );
}

#[test]
fn o8_dyntrait_no_op() {
    run_always_truthy_or_assign(&RustType::DynTrait("MyTrait".to_string()), false);
}

// --- O-12b Ref(T) (Copy always-truthy) ---------------------------------------

#[test]
fn o12b_ref_t_no_op() {
    run_always_truthy_or_assign(&RustType::Ref(Box::new(RustType::String)), true);
}

// --- O-12d Tuple -------------------------------------------------------------

#[test]
fn o12d_tuple_all_copy_no_op() {
    run_always_truthy_or_assign(&RustType::Tuple(vec![RustType::F64, RustType::F64]), true);
}

#[test]
fn o12d_tuple_with_string_no_op() {
    run_always_truthy_or_assign(
        &RustType::Tuple(vec![RustType::F64, RustType::String]),
        false,
    );
}
