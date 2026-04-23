//! I-171 T4 Layer 3c: `convert_bang_assign` desugar unit tests.
//!
//! The Bang arm's Layer 3c converts `!<x = rhs>` into a block that captures
//! the RHS value, performs the side-effecting assign, and then evaluates the
//! JS falsy predicate on the captured value. This test module focuses on
//! that specific desugar path; the surrounding dispatch layers (const-fold,
//! peek-through, double-neg, De Morgan, general fallback) live in
//! `bang_dispatch`.
//!
//! ## Dual fix coverage (post /check_job deep review)
//!
//! - **IG-3 regression**: `let tmp: T = <value>` annotation uses the LHS
//!   storage type (from `assign_target_type`) rather than the RHS span's
//!   inferred type, so `Option<T>`-wrapped RHS (e.g., `x: T | null` with
//!   literal RHS) annotates `tmp: Option<T>` not `tmp: T`.
//! - **IG-4 regression**: For non-Copy LHS storage types the assignment is
//!   emitted as `x = tmp.clone()` so the original `tmp` stays owned for the
//!   subsequent predicate. Copy types emit the bare `x = tmp` assignment.
//! - **IG-5 regression**: arithmetic/bitwise compound (`+=`/`-=`/`*=`/`/=`/
//!   `%=`/bitwise) fires the desugar as long as `convert_assign_expr` yields
//!   `Expr::Assign { target, BinaryOp(target, op, rhs) }`, matching the TS
//!   semantics `!(x += v) = !<new x>`. Logical compound (`&&=`/`||=`/`??=`)
//!   lower to non-Assign IR and therefore fall through to Layer 4 (preserving
//!   their conditional / lazy-RHS semantics).

use super::*;
use crate::ir::Stmt as IrStmt;

#[test]
fn bang_assign_desugar_primitive_f64_rhs() {
    // B.1.33 (Assign desugar): `!(x = 5)` with `x: number` desugars to
    // `{ let __ts_tmp_assign_0: f64 = 5.0; x = __ts_tmp_assign_0;
    //   __ts_tmp_assign_0 == 0.0 || __ts_tmp_assign_0.is_nan() }`.
    //
    // Rust `x = rhs` evaluates to `()` whereas TS evaluates to rhs — the
    // desugar captures rhs into a tmp so the predicate can read the value
    // after the side-effecting assign.
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: number = 0; return !(x = 5); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::Block(stmts) = result else {
        panic!("expected Block, got {result:?}");
    };
    assert_eq!(stmts.len(), 3, "expected 3 stmts, got {stmts:?}");
    let IrStmt::Let {
        mutable,
        name: tmp_name,
        ty: Some(RustType::F64),
        init: Some(Expr::NumberLit(n)),
    } = &stmts[0]
    else {
        panic!("expected `let tmp: f64 = <num>`, got {:?}", stmts[0]);
    };
    assert!(!mutable);
    assert!(tmp_name.starts_with("__ts_tmp_assign_"));
    assert_eq!(*n, 5.0);
    let IrStmt::Expr(Expr::Assign { target, value }) = &stmts[1] else {
        panic!("expected `x = tmp` Assign stmt, got {:?}", stmts[1]);
    };
    assert!(matches!(&**target, Expr::Ident(n) if n == "x"));
    // Copy F64: no .clone() wrap.
    assert!(matches!(&**value, Expr::Ident(n) if n == tmp_name));
    let IrStmt::TailExpr(Expr::BinaryOp { op, .. }) = &stmts[2] else {
        panic!("expected TailExpr(BinaryOp), got {:?}", stmts[2]);
    };
    assert_eq!(*op, BinOp::LogicalOr);
}

#[test]
fn bang_assign_desugar_handles_arithmetic_compound_assign() {
    // IG-5 regression: arithmetic compound assigns (`+=`/`-=`/`*=`/`/=`/`%=`/
    // bitwise) are normalised by `convert_assign_expr` into
    // `Expr::Assign { target, BinaryOp(target, op, rhs) }`, so the Layer 3c
    // desugar path fires correctly and captures the NEW target value.
    // Matches TS semantics `!(x += v) = !<new x>`.
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: number = 0; return !(x += 5); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::Block(stmts) = result else {
        panic!("expected Block desugar for `+=`, got {result:?}");
    };
    assert_eq!(stmts.len(), 3);
    let IrStmt::Let {
        ty: Some(RustType::F64),
        init: Some(Expr::BinaryOp { op, .. }),
        ..
    } = &stmts[0]
    else {
        panic!("expected Let f64 = x + 5, got {:?}", stmts[0]);
    };
    assert_eq!(*op, BinOp::Add);
    assert!(matches!(&stmts[1], IrStmt::Expr(Expr::Assign { .. })));
    assert!(matches!(
        &stmts[2],
        IrStmt::TailExpr(Expr::BinaryOp {
            op: BinOp::LogicalOr,
            ..
        })
    ));
}

#[test]
fn bang_assign_desugar_skips_logical_compound_assign() {
    // B.1.33 edge: `!(x &&= 5)` / `!(x ||= 5)` — logical compound assigns have
    // conditional semantics. `convert_assign_expr` lowers them to non-Assign
    // IR (If / Block from T3 desugar), so Layer 3c's `Expr::Assign` destructure
    // returns None and the dispatch correctly falls through to Layer 4.
    //
    // Guards against Layer 3c silently handling `&&=`/`||=` (which would
    // bypass the conditional semantics and use only the RHS value).
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: number | null = 5; return !(x ||= 10); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Layer 3c emits a specific 3-stmt Block `[Let, Expr(Assign), TailExpr]`.
    // The T3 `||=` lowering uses a different IR shape (If or consolidated
    // Block), so the outer should NOT match the Layer 3c triple.
    let is_layer_3c_triple = matches!(
        &result,
        Expr::Block(stmts)
            if stmts.len() == 3
                && matches!(&stmts[0], IrStmt::Let { .. })
                && matches!(&stmts[1], IrStmt::Expr(Expr::Assign { .. }))
                && matches!(&stmts[2], IrStmt::TailExpr(_))
    );
    assert!(
        !is_layer_3c_triple,
        "Layer 3c desugar should NOT fire for `||=` (conditional semantics); got: {result:?}"
    );
}

#[test]
fn bang_assign_desugar_option_f64_lhs_uses_lhs_type() {
    // IG-3 regression: `let x: number | null = null; !(x = 5)` — TypeResolver
    // wraps the literal `5` to `Some(5.0)` to match the `Option<f64>` LHS.
    // Pre-fix the desugar used the RHS span's type (F64 from literal `5`),
    // emitting `let tmp: f64 = Some(5.0)` — Rust E0308 mismatched types.
    // Post-fix the tmp annotation is `Option<f64>` (LHS storage type).
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: number | null = null; return !(x = 5); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::Block(stmts) = result else {
        panic!("expected Block desugar, got {result:?}");
    };
    assert_eq!(stmts.len(), 3);
    let IrStmt::Let {
        ty: Some(tmp_ty),
        init: Some(init),
        ..
    } = &stmts[0]
    else {
        panic!("expected Let with Option<f64> type, got {:?}", stmts[0]);
    };
    let RustType::Option(inner) = tmp_ty else {
        panic!("expected tmp_ty = Option<_>, got {tmp_ty:?}");
    };
    assert_eq!(**inner, RustType::F64);
    assert!(
        matches!(init, Expr::FnCall { .. }),
        "expected `Some(5.0)` wrapped init, got {init:?}"
    );
    // Option<f64> is Copy, so no .clone() wrap on the assign.
    let IrStmt::Expr(Expr::Assign { value, .. }) = &stmts[1] else {
        panic!("expected Assign stmt, got {:?}", stmts[1]);
    };
    assert!(
        matches!(value.as_ref(), Expr::Ident(_)),
        "Copy LHS (Option<f64>) should assign tmp directly without clone, got {value:?}"
    );
}

#[test]
fn bang_assign_desugar_string_lhs_clones_for_assign() {
    // IG-4 regression: `let x: string = ""; !(x = "hi")` — String is not Copy,
    // so direct `x = tmp` would move tmp, breaking the subsequent predicate
    // `tmp.is_empty()`. Post-fix emits `x = tmp.clone()` so tmp stays owned.
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: string = ""; return !(x = "hi"); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::Block(stmts) = result else {
        panic!("expected Block desugar, got {result:?}");
    };
    let IrStmt::Let {
        ty: Some(RustType::String),
        ..
    } = &stmts[0]
    else {
        panic!("expected `let tmp: String`, got {:?}", stmts[0]);
    };
    let IrStmt::Expr(Expr::Assign { value, .. }) = &stmts[1] else {
        panic!("expected Assign stmt, got {:?}", stmts[1]);
    };
    let Expr::MethodCall { method, .. } = value.as_ref() else {
        panic!("non-Copy LHS (String) should assign via `.clone()`, got {value:?}");
    };
    assert_eq!(method, "clone");
    let IrStmt::TailExpr(Expr::MethodCall {
        method: tail_method,
        ..
    }) = &stmts[2]
    else {
        panic!("expected TailExpr MethodCall, got {:?}", stmts[2]);
    };
    assert_eq!(tail_method, "is_empty");
}

#[test]
fn bang_assign_desugar_option_string_lhs_clones_and_as_ref() {
    // IG-3 + IG-4 combined: `let x: string | null = null; !(x = "hi")` — LHS
    // is `Option<String>` (non-Copy). tmp type = `Option<String>` (IG-3), assign
    // uses `.clone()` (IG-4), predicate uses `.as_ref().is_some_and(...)` so
    // the original tmp stays borrowed.
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: string | null = null; return !(x = "hi"); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::Block(stmts) = result else {
        panic!("expected Block desugar, got {result:?}");
    };
    let IrStmt::Let {
        ty: Some(tmp_ty), ..
    } = &stmts[0]
    else {
        panic!("expected Let with type, got {:?}", stmts[0]);
    };
    let RustType::Option(inner) = tmp_ty else {
        panic!("expected tmp_ty = Option<String>, got {tmp_ty:?}");
    };
    assert_eq!(**inner, RustType::String);
    let IrStmt::Expr(Expr::Assign { value, .. }) = &stmts[1] else {
        panic!("expected Assign stmt");
    };
    assert!(
        matches!(value.as_ref(), Expr::MethodCall { method, .. } if method == "clone"),
        "expected `.clone()` for non-Copy Option<String>, got {value:?}"
    );
}

#[test]
fn bang_assign_desugar_named_struct_lhs_always_truthy_const() {
    // B.1.33 × B-T8 Named non-union: `!(x = { a: 1 })` with x: {a: number}.
    // LHS is a synthetic Named struct (always-truthy) → predicate const-folds
    // to `BoolLit(false)` via `const_truthiness_with_side_effect`. tmp Ident
    // is pure, so the helper returns `BoolLit(false)` directly (no eval wrap).
    // Named struct is non-Copy → `.clone()` for the assign.
    let f = TctxFixture::from_source(
        r#"
        type P = { a: number };
        function f(): boolean {
            let x: P = { a: 0 };
            return !(x = { a: 1 });
        }
        "#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 1, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::Block(stmts) = result else {
        panic!("expected Block desugar, got {result:?}");
    };
    let IrStmt::Expr(Expr::Assign { value, .. }) = &stmts[1] else {
        panic!("expected Assign stmt");
    };
    assert!(
        matches!(value.as_ref(), Expr::MethodCall { method, .. } if method == "clone"),
        "expected `.clone()` on Named struct LHS, got {value:?}"
    );
    let IrStmt::TailExpr(Expr::BoolLit(false)) = &stmts[2] else {
        panic!(
            "expected TailExpr(BoolLit(false)) for always-truthy Named, got {:?}",
            stmts[2]
        );
    };
}

#[test]
fn bang_assign_desugar_unresolved_target_type_falls_back() {
    // B.1.33 edge: `!(x = y)` without any declared variable (untyped context)
    // — `assign_target_type` returns None because `x` is not in any scope.
    // Layer 3c returns None → Layer 4 also returns None → Layer 5 fallback
    // emits `!<Assign ir>`. Confirms the desugar gate is closed when the LHS
    // storage type is unknowable (avoids emitting an incorrectly-typed tmp).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(x = y);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp fallback, got {result:?}");
    };
    assert_eq!(op, UnOp::Not);
    assert!(
        matches!(*operand, Expr::Assign { .. }),
        "expected inner Assign ir, got {operand:?}"
    );
}

// ---------------------------------------------------------------------------
// TG-4: typed double-negation (Matrix B.1.19 main feature). The literal
// const-fold cases are covered in `bang_dispatch`; these cases verify the
// TypeResolver-grounded `truthy_predicate_for_expr` dispatch path.
// ---------------------------------------------------------------------------

#[test]
fn bang_double_negation_typed_option_f64() {
    // B.1.19 + B-T5 (Option<primitive> Copy): `!!x` with `x: number | null` →
    // `x.is_some_and(|v| v != 0.0 && !v.is_nan())`. Confirms double-neg routes
    // to `truthy_predicate_for_expr` with the resolved `Option<f64>` type
    // (rather than falling to Layer 5 raw `!!x` fallback).
    let f = TctxFixture::from_source(r#"function f(x: number | null): boolean { return !!x; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Option<F64 Copy> truthy routes through `maybe_tmp_bind`. For a pure
    // Ident operand the bind is skipped → direct `x.is_some_and(|v| <truthy>)`.
    // Accept either the bare method-call form or the tmp-bind block form
    // (I-182 relaxation may trim the latter).
    let outer = match &result {
        Expr::Block(stmts) => match stmts.last() {
            Some(IrStmt::TailExpr(e)) => e.clone(),
            _ => panic!("expected Block TailExpr, got {result:?}"),
        },
        other => other.clone(),
    };
    let Expr::MethodCall { method, .. } = &outer else {
        panic!("expected MethodCall on is_some_and, got {outer:?}");
    };
    assert_eq!(method, "is_some_and");
}

#[test]
fn bang_double_negation_typed_option_string() {
    // B.1.19 + B-T5s (Option<String> !Copy): `!!x` with `x: string | null` →
    // `x.as_ref().is_some_and(|v| !v.is_empty())`. Verifies the !Copy path
    // uses `.as_ref()` to borrow the Option before `is_some_and` consumes it.
    let f = TctxFixture::from_source(r#"function f(x: string | null): boolean { return !!x; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let final_expr = match &result {
        Expr::Block(stmts) => match stmts.last() {
            Some(IrStmt::TailExpr(e)) => e.clone(),
            _ => panic!("expected Block TailExpr, got {result:?}"),
        },
        other => other.clone(),
    };
    let Expr::MethodCall { method, object, .. } = &final_expr else {
        panic!("expected outer MethodCall, got {final_expr:?}");
    };
    assert_eq!(method, "is_some_and");
    let Expr::MethodCall {
        method: inner_method,
        ..
    } = object.as_ref()
    else {
        panic!("expected inner `.as_ref()` for Option<String !Copy>, got {object:?}");
    };
    assert_eq!(inner_method, "as_ref");
}

// ---------------------------------------------------------------------------
// IG-6: double-negation on Assign / LogicalAnd / LogicalOr operands. Direct
// truthy_predicate_for_expr dispatch fails for these shapes because the IR
// type diverges from the TS expression type (Rust `x = v` evaluates to `()`,
// Rust `a && b` requires bool operands). Layer 3 routes these specifically
// through recursive `convert_bang_expr` + outer `Not` so the dedicated Layer
// 3b / 3c rewrites fire and produce valid Rust.
// ---------------------------------------------------------------------------

#[test]
fn bang_double_negation_on_assign_inverts_layer_3c_block() {
    // IG-6 regression: `!!(x = 5)` with `x: number`. Pre-fix Layer 3 direct
    // path emitted `let tmp: f64 = (x = 5)` — Rust E0308 (assign evaluates
    // to `()`, not `f64`). Post-fix routes Assign through Layer 3c and wraps
    // the resulting falsy Block with outer `Not` → `!<Block{...}>`.
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: number = 0; return !!(x = 5); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp(Not), got {result:?}");
    };
    assert_eq!(op, UnOp::Not);
    let Expr::Block(stmts) = operand.as_ref() else {
        panic!("expected inner Layer 3c Block, got {operand:?}");
    };
    // Layer 3c canonical triple: [Let tmp, Expr(Assign), TailExpr(falsy)].
    assert_eq!(stmts.len(), 3);
    assert!(matches!(
        &stmts[0],
        IrStmt::Let {
            ty: Some(RustType::F64),
            ..
        }
    ));
    assert!(matches!(&stmts[1], IrStmt::Expr(Expr::Assign { .. })));
    assert!(matches!(
        &stmts[2],
        IrStmt::TailExpr(Expr::BinaryOp {
            op: BinOp::LogicalOr,
            ..
        })
    ));
}

#[test]
fn bang_double_negation_on_logical_and_inverts_de_morgan() {
    // IG-6 regression: `!!(a && b)` with Option-typed a/b. Pre-fix direct
    // truthy emission would be `<a && b>.is_some_and(...)` — invalid Rust
    // because `Option<T> && Option<U>` is a type error. Post-fix routes
    // through Layer 3b De Morgan: inner `!(a && b)` = `<a falsy> || <b falsy>`,
    // then outer `!` inverts → `!(<a falsy> || <b falsy>)` which by De Morgan
    // computes `<a truthy> && <b truthy>` (the TS semantic of `!!(a && b)`).
    let f = TctxFixture::from_source(
        r#"function f(a: number | null, b: string | null): boolean { return !!(a && b); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Outer shape: UnaryOp(Not, BinaryOp(LogicalOr, <a falsy>, <b falsy>)).
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp(Not), got {result:?}");
    };
    assert_eq!(op, UnOp::Not);
    let Expr::BinaryOp { op: inner_op, .. } = operand.as_ref() else {
        panic!("expected inner De Morgan BinaryOp, got {operand:?}");
    };
    assert_eq!(*inner_op, BinOp::LogicalOr);
}

#[test]
fn bang_double_negation_on_logical_or_inverts_de_morgan() {
    // IG-6 companion: `!!(a || b)`. Inner `!(a || b)` = `<a falsy> && <b falsy>`,
    // outer `!` wraps → `!(<a falsy> && <b falsy>)` = `<a truthy> || <b truthy>`.
    let f = TctxFixture::from_source(
        r#"function f(a: number | null, b: string | null): boolean { return !!(a || b); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp(Not), got {result:?}");
    };
    assert_eq!(op, UnOp::Not);
    let Expr::BinaryOp { op: inner_op, .. } = operand.as_ref() else {
        panic!("expected inner De Morgan BinaryOp, got {operand:?}");
    };
    assert_eq!(*inner_op, BinOp::LogicalAnd);
}

#[test]
fn bang_double_negation_on_arithmetic_compound_assign() {
    // IG-6 × IG-5 composition: `!!(x += 5)` routes through Layer 3 recursion
    // to Layer 3c (IG-5 plain-assign-normalised desugar for arithmetic
    // compound). Final shape: `!<Block{Let tmp = x + 5; x = tmp; falsy(tmp)}>`.
    let f = TctxFixture::from_source(
        r#"function f(): boolean { let x: number = 0; return !!(x += 5); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp(Not), got {result:?}");
    };
    assert_eq!(op, UnOp::Not);
    let Expr::Block(stmts) = operand.as_ref() else {
        panic!("expected inner Layer 3c Block, got {operand:?}");
    };
    // Layer 3c for `+=`: Let init is `x + 5`, Assign stmt, TailExpr predicate.
    let IrStmt::Let {
        ty: Some(RustType::F64),
        init: Some(Expr::BinaryOp { op: add_op, .. }),
        ..
    } = &stmts[0]
    else {
        panic!("expected Let f64 = x + 5, got {:?}", stmts[0]);
    };
    assert_eq!(*add_op, BinOp::Add);
}
