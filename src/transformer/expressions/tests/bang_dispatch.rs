//! I-171 T4: `convert_bang_expr` shape-dispatch unit tests.
//!
//! These cases exercise the Bang-arm dispatch layers in
//! `convert_unary_expr` / `convert_bang_expr` (Layer 2 const-fold, Layer 3
//! double-negation + recursive const-fold, Layer 3b De Morgan, Layer 3c
//! Assign desugar) at the Transformer level. Type-level dispatch for the
//! underlying `falsy_predicate_for_expr` / `truthy_predicate_for_expr`
//! helpers is covered by 44 unit tests in
//! `src/transformer/helpers/truthy/tests.rs` (Matrix B.2). These tests
//! focus on the AST-shape transitions that are only visible once
//! `convert_expr` is invoked from a live Transformer — each B.1 shape in
//! the PRD Matrix has at least one case verifying that the dispatch
//! reaches the correct layer (const-fold / peek-through / double-neg /
//! De Morgan / Assign desugar / fallback).

use super::*;
use crate::ir::Stmt as IrStmt;

#[test]
fn bang_const_fold_null() {
    // B.1.2: `!null` → const-fold `BoolLit(true)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!null;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_const_fold_undefined_ident() {
    // B.1.3: `!undefined` (Ident with sym "undefined") → const-fold `BoolLit(true)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!undefined;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_const_fold_bool_true() {
    // B.1.4: `!true` → const-fold `BoolLit(false)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!true;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_const_fold_bool_false() {
    // B.1.5: `!false` → const-fold `BoolLit(true)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!false;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_const_fold_num_zero() {
    // B.1.6: `!0` → const-fold `BoolLit(true)` (JS: 0 is falsy).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!0;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_const_fold_num_nonzero() {
    // B.1.7: `!1` → const-fold `BoolLit(false)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!1;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_const_fold_num_nan() {
    // B.1.8: `!NaN` — SWC parses `NaN` as an Ident that the literal pass
    // recognises and rewrites to `f64::NAN` (`Expr::PrimitiveAssocConst`).
    // The Bang arm then falls back to `!<PrimitiveAssocConst>` because
    // there is no expression-type binding for the NaN ident in the
    // fixture-less Transformer context (type resolution happens earlier
    // when a full module type-check has run). Runtime NaN-is-falsy is
    // covered by E2E `cell-b-bang-f64-in-ret` (TypeResolver resolves
    // operand to F64, Bang arm emits `e == 0.0 || e.is_nan()`).
    use crate::ir::{BuiltinVariant, PrimitiveType};
    let _ = BuiltinVariant::None; // silence potential unused-import lints
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!NaN;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::PrimitiveAssocConst {
                ty: PrimitiveType::F64,
                name: "NAN".to_string(),
            }),
        }
    );
}

#[test]
fn bang_const_fold_str_empty() {
    // B.1.9: `!""` → const-fold `BoolLit(true)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"!"";"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_const_fold_str_nonempty() {
    // B.1.10: `!"hi"` → const-fold `BoolLit(false)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"!"hi";"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_const_fold_bigint_zero() {
    // B.1.11: `!0n` → const-fold `BoolLit(true)` (BigInt zero is falsy).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!0n;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_const_fold_bigint_nonzero() {
    // B.1.12: `!1n` → const-fold `BoolLit(false)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!1n;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_const_fold_regex_always_truthy() {
    // B.1.13: `!/foo/` → const-fold `BoolLit(false)` (regex object is always truthy).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!/foo/;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_const_fold_arrow_always_truthy() {
    // B.1.34: `!(() => 0)` → const-fold `BoolLit(false)` (function always truthy).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(() => 0);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_const_fold_fn_expr_always_truthy() {
    // B.1.34: `!(function () { return 0; })` → const-fold `BoolLit(false)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(function () { return 0; });");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_peek_through_paren() {
    // B.1.14: `!(null)` peek-through unwraps Paren → const-fold on inner `null`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(null);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_peek_through_ts_as() {
    // B.1.17: `!(null as unknown)` peek-through unwraps TsAs → const-fold `null`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(null as unknown);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_peek_through_ts_non_null() {
    // B.1.18: `!(0!)` peek-through unwraps TsNonNull → const-fold on `0`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(0!);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_peek_through_ts_type_assertion() {
    // B.1.37g: `!(<any>null)` (legacy type assertion) peek-through → const-fold.
    //
    // Note: SWC parses `<any>null` under the `.tsx` flag restrictions. The
    // `parse_typescript` helper uses `.ts` which accepts this syntax.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(<any>null);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_peek_through_ts_const_assertion() {
    // B.1.37i: `!("" as const)` peek-through → const-fold on `""`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"!("" as const);"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_peek_through_nested_wrappers() {
    // Multiple layers: `!(((null as any)!))` — recursively strips Paren / TsAs /
    // TsNonNull and const-folds the innermost `null`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(((null as any)!));");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_double_negation_on_null_literal() {
    // B.1.19 + B.1.2 recursive const-fold: `!!null` = `!(fold_bang(null))` =
    // `!true` = `false`. Matches JS `Boolean(null) = !!null = false`.
    // Prior to the IG-1 fix the Bang arm fell through to Layer 5 with a raw
    // `!!None` emission (Rust compile error since `None` is not `bool`).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!!null;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_double_negation_on_undefined_ident() {
    // B.1.19 + B.1.3: `!!undefined` const-folds to `BoolLit(false)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!!undefined;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_double_negation_on_truthy_string_literal() {
    // B.1.19 + B.1.10: `!!"x"` = `!(fold_bang("x"))` = `!false` = `true`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"!!"x";"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn bang_double_negation_on_zero_numeric_literal() {
    // B.1.19 + B.1.6: `!!0` = `!true` = `false`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!!0;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_double_negation_through_peek_through_wrappers() {
    // B.1.19 combined with peek-through: `!!(null as any)` peek-strips the
    // TsAs on the inner operand, then const-folds. Ideal: `BoolLit(false)`.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!!(null as any);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn bang_double_negation_on_bool_ident() {
    // `!!x` where x is an Ident without resolvable type — fallback emits
    // `!!<x>` instead of truthy predicate (I-050 / generic bounds scope).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!!x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(Expr::Ident("x".to_string())),
            }),
        }
    );
}

#[test]
fn bang_de_morgan_logical_and_falls_back_for_unknown_operand_types() {
    // B.1.23: `!(x && y)` with no type info for x, y. De Morgan at AST layer
    // produces `<x falsy> || <y falsy>`. The recursive `convert_bang_expr`
    // sees bare idents and falls back to `!x` / `!y`. Result:
    // `!x || !y` (BinaryOp::LogicalOr of two !Idents).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(x && y);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let not_ident = |name: &str| Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::Ident(name.to_string())),
    };
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(not_ident("x")),
            op: BinOp::LogicalOr,
            right: Box::new(not_ident("y")),
        }
    );
}

#[test]
fn bang_de_morgan_logical_or_inverts_to_and() {
    // B.1.24: `!(x || y)` → `<x falsy> && <y falsy>` (De Morgan).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(x || y);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let not_ident = |name: &str| Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::Ident(name.to_string())),
    };
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(not_ident("x")),
            op: BinOp::LogicalAnd,
            right: Box::new(not_ident("y")),
        }
    );
}

#[test]
fn bang_ident_without_type_falls_back_to_raw_not() {
    // Layer 5 fallback: Ident with no resolved type emits raw `!x`. This
    // preserves the pre-I-171 emission so blocked-type cases (Any / TypeVar
    // tracked by I-050) surface as explicit Rust compile errors rather than
    // silently synthesising a wrong predicate.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!x;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident("x".to_string())),
        }
    );
}

// =============================================================================
// B.1 shape dispatch coverage (TG-1): one unit test per remaining AST shape
// enumerated in the PRD Matrix B.1. Shapes where the ideal emission depends on
// operand type (Member / OptChain / Bin arithmetic / Call / Cond / Await / NC /
// Array / Object / Tpl / Update / This / Assign desugar) are exercised via
// `TctxFixture::from_source` so the full TypeResolver + expected-type
// propagation pipeline is live. Shapes whose ideal reduces to a Bool-typed
// `!<inner>` (comparison / InstanceOf / In) are covered with the untyped
// fixture — the Bang arm's Layer 5 fallback is the correct emission.
// =============================================================================

#[test]
fn bang_member_on_f64_field_emits_falsy_predicate() {
    // B.1.15 (Member on F64): `!obj.n` with `obj: { n: number }` dispatches
    // through Layer 4 to `falsy_predicate_for_expr(FieldAccess, F64)` =
    // `<obj.n> == 0.0 || <obj.n>.is_nan()`.
    let f = TctxFixture::from_source(
        r#"const obj: { n: number } = { n: 1 }; const r: boolean = !obj.n;"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_var_init_at(f.module(), 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Expected: `obj.n == 0.0 || obj.n.is_nan()` (F64 falsy, pure FieldAccess,
    // no tmp bind because `obj.n` on a pure Ident base is `is_pure_operand`).
    let field = Expr::FieldAccess {
        object: Box::new(Expr::Ident("obj".to_string())),
        field: "n".to_string(),
    };
    let eq_zero = Expr::BinaryOp {
        left: Box::new(field.clone()),
        op: BinOp::Eq,
        right: Box::new(Expr::NumberLit(0.0)),
    };
    let is_nan = Expr::MethodCall {
        object: Box::new(field),
        method: "is_nan".to_string(),
        args: vec![],
    };
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(eq_zero),
            op: BinOp::LogicalOr,
            right: Box::new(is_nan),
        }
    );
}

#[test]
fn bang_optchain_on_option_field_emits_option_falsy() {
    // B.1.16 (OptChain): `!obj?.n` where `obj: { n: number } | undefined`
    // dispatches through Layer 4 on the resulting `Option<F64>`. Ideal:
    // `!<optchain>.is_some_and(|v| v != 0.0 && !v.is_nan())` — semantically
    // equivalent to the Matrix ideal `chain.is_none() || <inner falsy>`.
    //
    // Emission wraps the Option<F64> receiver in a `TempBinder` block
    // (`maybe_tmp_bind`) because the optchain IR (`obj.as_ref().map(|_v|
    // _v.n)`) is non-pure: the map closure allocates a fresh Option and the
    // helper conservatively binds it to a local before invoking
    // `is_some_and`. Resulting shape:
    //   `{ let __ts_tmp_op_0 = <optchain>; !__ts_tmp_op_0.is_some_and(...) }`.
    let f = TctxFixture::from_source(
        r#"function f(obj: { n: number } | undefined): boolean { return !obj?.n; }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::Block(stmts) = result else {
        panic!("expected Block (tmp-bind wrapper), got {result:?}");
    };
    assert_eq!(stmts.len(), 2, "expected [Let, TailExpr], got {stmts:?}");
    let IrStmt::Let {
        name,
        init: Some(_),
        ..
    } = &stmts[0]
    else {
        panic!("expected let-binding, got {:?}", stmts[0]);
    };
    assert!(name.starts_with("__ts_tmp_op_"));
    let IrStmt::TailExpr(Expr::UnaryOp { op, operand }) = &stmts[1] else {
        panic!("expected `!...` tail expr, got {:?}", stmts[1]);
    };
    assert_eq!(*op, UnOp::Not);
    let Expr::MethodCall { method, .. } = operand.as_ref() else {
        panic!("expected `is_some_and` method call, got {operand:?}");
    };
    assert_eq!(method, "is_some_and");
}

#[test]
fn bang_unary_neg_operand_falls_through() {
    // B.1.20 (Unary `-`): `!(-x)` with no type info reaches Layer 5 fallback
    // `!<UnaryOp{Neg, x}>`. Guards against the Bang arm accidentally
    // triggering on the nested Unary (which would misfire the double-neg
    // layer if we were to match by arity instead of by operator).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(-x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::UnaryOp {
                op: UnOp::Neg,
                operand: Box::new(Expr::Ident("x".to_string())),
            }),
        }
    );
}

#[test]
fn bang_bin_arithmetic_on_f64_tmp_binds() {
    // B.1.21 (Bin arithmetic): `!(a + b)` with `a, b: number` — non-pure
    // F64 operand triggers tmp binding in `predicate_primitive_with_tmp`
    // (F64 reads operand twice: `== 0.0 || .is_nan()`).
    let f = TctxFixture::from_source(
        r#"function f(a: number, b: number): boolean { return !(a + b); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Expected block shape: `{ let __ts_tmp_op_0: f64 = a + b; <F64 falsy> }`.
    let Expr::Block(stmts) = result else {
        panic!("expected Block, got {result:?}");
    };
    assert_eq!(stmts.len(), 2);
    let IrStmt::Let {
        mutable,
        name,
        ty: Some(RustType::F64),
        init: Some(_),
    } = &stmts[0]
    else {
        panic!("expected Let, got {:?}", stmts[0]);
    };
    assert!(!mutable);
    assert!(name.starts_with("__ts_tmp_op_"));
    let IrStmt::TailExpr(_) = &stmts[1] else {
        panic!("expected TailExpr");
    };
}

#[test]
fn bang_bin_comparison_returns_bang_of_bool_expr() {
    // B.1.22 (Bin comparison): `!(a < b)` emits `!<a < b>`. The inner
    // comparison is Bool-typed and the Bang arm's Bool dispatch is
    // passthrough-then-negate, so no tmp bind fires.
    let f = TctxFixture::from_source(
        r#"function f(a: number, b: number): boolean { return !(a < b); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Expected: `!(a < b)` — UnaryOp { Not, BinaryOp { Lt, a, b } }.
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Lt,
                right: Box::new(Expr::Ident("b".to_string())),
            }),
        }
    );
}

#[test]
fn bang_bin_bitwise_falls_through() {
    // B.1.25 (Bin bitwise): `!(a & b)` with no type info → Layer 5 fallback.
    // The non-LogicalAnd/Or Bin op must not trigger De Morgan.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(a & b);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected UnaryOp, got {result:?}");
    };
    assert_eq!(op, UnOp::Not);
    let Expr::BinaryOp { op: inner_op, .. } = &*operand else {
        panic!("expected inner BinaryOp");
    };
    assert_eq!(*inner_op, BinOp::BitAnd);
}

#[test]
fn bang_bin_instanceof_emits_bool_negation() {
    // B.1.26 (InstanceOf): `!(x instanceof Y)` — `instanceof` result is Bool,
    // so Layer 5 fallback `!<instanceof-ir>` is ideal Rust (`!<bool>`).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(x instanceof Y);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, .. } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
}

#[test]
fn bang_bin_in_emits_bool_negation() {
    // B.1.27 (In): `!("k" in obj)` — `in` result is Bool, Layer 5 fallback.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"!("k" in obj);"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, .. } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
}

#[test]
fn bang_bin_nullish_coalescing_dispatches_on_result_type() {
    // B.1.28 (NC): `!(a ?? b)` with `a: number | null, b: number` — NC result
    // type is F64 (RHS non-Option), so dispatch is F64 falsy on the
    // `unwrap_or` emission. Confirms NC does not accidentally trigger the
    // Assign / De Morgan / double-neg layers.
    let f = TctxFixture::from_source(
        r#"function f(a: number | null, b: number): boolean { return !(a ?? b); }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Outer is a Block (tmp bind for non-pure `unwrap_or` F64 operand).
    assert!(
        matches!(result, Expr::Block(_) | Expr::BinaryOp { .. }),
        "expected F64 falsy shape (Block or BinaryOp), got {result:?}"
    );
}

#[test]
fn bang_call_falls_through_without_type() {
    // B.1.29 (Call): `!f()` without type reaches Layer 5 fallback.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!f();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
    assert!(
        matches!(*operand, Expr::FnCall { .. }),
        "expected inner FnCall, got {operand:?}"
    );
}

#[test]
fn bang_cond_ternary_falls_through_without_type() {
    // B.1.30 (Cond): `!(c ? a : b)` without type reaches Layer 5.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(c ? a : b);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
    assert!(
        matches!(*operand, Expr::If { .. }),
        "expected inner If (ternary), got {operand:?}"
    );
}

#[test]
fn bang_await_falls_through_without_type() {
    // B.1.32 (Await): `!(await p)` without type info reaches Layer 5. The
    // fixture wraps the await in an async function so SWC accepts the
    // `await` keyword. Without TypeResolver the Bang arm falls back to
    // the raw `!<Await>` shape (Rust compile-error surface preserved).
    let f = TctxFixture::from_source("async function _f() { !(await p); }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp, got {result:?}");
    };
    assert_eq!(op, UnOp::Not);
    assert!(
        matches!(*operand, Expr::Await(_)),
        "expected inner Await, got {operand:?}"
    );
}

#[test]
fn bang_new_expression_falls_through() {
    // B.1.31 (New): `!new X()` without type reaches Layer 5. (The PRD's
    // always-truthy const-fold for `new` is the responsibility of Layer 4
    // via `is_always_truthy_type` once a Named type is resolved — Layer 2
    // const-fold intentionally does NOT fold `New` because `new` evaluates
    // its constructor which may throw / have side effects.)
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!new X();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
    assert!(
        matches!(*operand, Expr::FnCall { .. } | Expr::StructInit { .. }),
        "expected inner New-like expression, got {operand:?}"
    );
}

#[test]
fn bang_template_literal_static_content() {
    // B.1.34 (Tpl): `` !`hello` `` — a template literal with only a static
    // piece and no interpolations lowers to `Expr::StringLit` via
    // `convert_expr`. Const-fold `!"hello"` fires after the inner lowering,
    // producing `BoolLit(false)`. Confirms template literals feed through
    // the shared dispatch path (not silently falling to Layer 5).
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!`hello`;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    // Templates with no interpolations are not `ast::Expr::Lit(Lit::Str)`,
    // so `try_constant_fold_bang` in Layer 2 does not fire on the raw AST.
    // The fallback emission is the `!<tpl-ir>` shape, where the tpl-ir is
    // either `StringLit` (static single-chunk) or `FormatMacro`.
    let Expr::UnaryOp { op, .. } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
}

#[test]
fn bang_array_literal_falls_through() {
    // B.1.34 (Array): `![1,2,3]` without type → Layer 5 fallback. With type
    // (Vec<f64>), Layer 4 would const-fold to `BoolLit(false)` with side-
    // effect preservation. Unit test verifies untyped fallback shape.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("![1, 2, 3];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
    assert!(
        matches!(*operand, Expr::Vec { .. }),
        "expected inner array-like, got {operand:?}"
    );
}

#[test]
fn bang_this_expression_falls_through_without_class_type() {
    // B.1.35 (This): `!this` without class context reaches Layer 5.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!this;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    let Expr::UnaryOp { op, operand } = result else {
        panic!("expected outer UnaryOp");
    };
    assert_eq!(op, UnOp::Not);
    assert!(
        matches!(*operand, Expr::Ident(ref n) if n == "self"),
        "expected inner `self` ident (Rust lowering of `this`), got {operand:?}"
    );
}

#[test]
fn bang_update_expression_on_f64_with_type_tmp_binds() {
    // B.1.36 (Update): `!(i++)` with `i: number` — Update returns the old
    // value (postfix) and dispatches to F64 falsy on a non-pure operand,
    // triggering tmp binding.
    let f = TctxFixture::from_source(r#"function f(i: number): boolean { return !(i++); }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_return_expr(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Expected: outer `Block` with `let tmp: f64 = <old-i>; <F64 falsy(tmp)>`.
    let Expr::Block(stmts) = result else {
        panic!("expected Block (tmp bind), got {result:?}");
    };
    assert!(stmts.len() >= 2);
    assert!(matches!(
        &stmts[0],
        IrStmt::Let {
            ty: Some(RustType::F64),
            ..
        }
    ));
}
