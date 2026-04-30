//! I-205 T9 (Logical compound assign Member target dispatch) — Iteration v14
//! deep-deep review structural completeness tests for strategy / orthogonality
//! axes (split from `logical_compound.rs` for the file-line threshold per
//! `check-file-lines.sh`)。
//!
//! ## Test categories
//!
//! - **`??=` strategy dispatch** (`pick_strategy`-based 3-way: ShadowLet /
//!   Identity / BlockedByI050): cells 38 / Identity (non-Option non-Any) /
//!   Any LHS BlockedByI050 wording。Cohesive with existing
//!   `nullish_assign.rs::pick_strategy` Ident-target emission logic。
//! - **`&&=` / `||=` × Any/TypeVar Tier 2 honest error**: pre-check Any/TypeVar
//!   gate for I-050 / generic bounds wording (consistent with existing
//!   `compound_logical_assign.rs::desugar_compound_logical_assign_stmts`
//!   blocked path)。
//! - **`&&=` / `||=` × always-truthy const-fold**: cohesive with
//!   `compound_logical_assign.rs::const_fold_always_truthy_stmts`。`&&=` →
//!   unconditional setter call、`||=` → no-op、INV-3 IIFE for SE-having
//!   receiver。
//! - **Cells 39 / 40 / 41-d Expression context**: coverage gap fix from initial
//!   T9 implementation (Statement context only)。
//! - **SE-having × Expression context tail uses __ts_recv**: INV-3 1-evaluate
//!   compliance for tail expression (otherwise `getInstance()` would be called
//!   twice = INV-3 violation)。
//!
//! ## Sibling reference
//!
//! All shared helpers / fixtures (`convert_logical_in_probe`,
//! `convert_logical_stmt_in_probe`,
//! `assert_logical_in_probe_unsupported_syntax_error_kind`,
//! `B4_CACHE_OPTION_SRC`, `B4_FOO_BOOL_SRC`) imported from sibling
//! [`super::logical_compound`] (`pub(super)` exports for cross-file reuse)。

use super::super::*;

use crate::ir::{BuiltinVariant, CallTarget, Expr, Stmt as IrStmt};

use super::logical_compound::{
    assert_logical_in_probe_unsupported_syntax_error_kind, convert_logical_in_probe,
    convert_logical_stmt_in_probe, B4_CACHE_OPTION_SRC, B4_FOO_BOOL_SRC,
};

// =============================================================================
// `??=` strategy dispatch (Layer 4 deep-deep review、Iteration v14 structural
// completeness): pick_strategy-based 3-way dispatch (ShadowLet / Identity /
// BlockedByI050) with cohesive emission per strategy。
// =============================================================================

#[test]
fn test_nullish_assign_b4_non_option_non_any_lhs_emits_identity_expression_yield() {
    // `??=` × class member with B4 setter pair where getter return = non-Option
    // non-Any T (= `f64` here)。pick_strategy(F64) = Identity → TS `??=` is dead
    // code on non-nullable T、ideal Tier 1 emission yields current getter value
    // (no setter call)。Expression context with SE-free receiver: direct getter
    // call `f.value()`。
    //
    // Pre-Iteration v14 (initial T9): Tier 2 honest error "nullish-assign on
    // non-Option class member" (post-T9 improvement over pre-T9 "unresolved
    // member type" but still Tier 2)。
    // Post-Iteration v14 (deep-deep review structural fix): Tier 1 Identity
    // emission via `pick_strategy` integration、cohesive with existing
    // nullish_assign.rs Ident-target Identity strategy logic (semantic = no
    // setter call、yield current value or no-op)。
    let src = "class Foo { _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } }\n\
               function probe(): void { const f = new Foo(); f.value ??= 42; }";
    let result =
        convert_logical_in_probe(src, 1, 1).expect("Identity emission must succeed (Tier 1 ideal)");
    // Expression context with SE-free receiver: direct getter call
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("f".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        "non-Option non-Any ??= × SE-free instance × Expression context must \
         emit direct getter call (Identity strategy、no setter, yield current)"
    );
}

#[test]
fn test_nullish_assign_b4_non_option_non_any_lhs_statement_context_emits_empty_block() {
    // `??=` × class member non-Option non-Any × Statement context × SE-free:
    // empty Block (TS dead code → no-op emission)。
    let src = "class Foo { _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } }\n\
               function probe(): void { const f = new Foo(); f.value ??= 42; }";
    let stmts =
        convert_logical_stmt_in_probe(src, 1, 1).expect("Identity emission statement must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    match block {
        Expr::Block(inner_stmts) => assert!(
            inner_stmts.is_empty(),
            "non-Option non-Any ??= × Statement × SE-free must emit empty Block (no-op), got: {inner_stmts:?}"
        ),
        other => panic!("expected Expr::Block, got {other:?}"),
    }
}

#[test]
fn test_nullish_assign_b4_non_option_se_having_statement_emits_evaluate_discard() {
    // `??=` × class member non-Option non-Any × Statement context × SE-having
    // receiver (`getInstance().value ??= 42;` where getInstance() returns Foo
    // with non-Option getter)。Identity emission with INV-3: evaluate-discard
    // receiver (= `Stmt::Expr(<obj>)` discards return value but preserves
    // side effect)。
    let src = "class Foo { _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } }\n\
               function getInstance(): Foo { return new Foo(); }\n\
               function probe(): void { getInstance().value ??= 42; }";
    let stmts = convert_logical_stmt_in_probe(src, 2, 0)
        .expect("Identity SE-having statement must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    match block {
        Expr::Block(inner_stmts) => {
            assert_eq!(
                inner_stmts.len(),
                1,
                "SE-having Identity statement must have exactly 1 stmt (evaluate-discard), got: {inner_stmts:?}"
            );
            // Stmt::Expr(<obj FnCall>) — receiver evaluated, value discarded
            assert!(
                matches!(&inner_stmts[0], IrStmt::Expr(Expr::FnCall { .. })),
                "stmt 0 must be Stmt::Expr(FnCall) (evaluate-discard receiver), got: {:?}",
                inner_stmts[0]
            );
        }
        other => panic!("expected Expr::Block, got {other:?}"),
    }
}

#[test]
fn test_nullish_assign_b4_any_lhs_errs_with_i050_umbrella_wording() {
    // `??=` × class member with B4 setter pair where getter return = `any`
    // (TS `any` → Rust `serde_json::Value` = `RustType::Any`)。
    // pick_strategy(Any) = BlockedByI050 → Tier 2 honest error "nullish-assign
    // on Any class member (I-050 Any coercion umbrella)"。
    //
    // Wording consistency with existing nullish_assign.rs::try_convert_nullish_assign_stmt
    // BlockedByI050 strategy wording (Ident-target Any LHS case)。Subsequent I-050
    // umbrella PRD will lift the block via `serde_json::Value`-aware runtime null
    // check + RHS coercion。
    let src = "class Foo { _v: any = undefined; \
               get value(): any { return this._v; } \
               set value(v: any) { this._v = v; } }\n\
               function probe(): void { const f = new Foo(); f.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "nullish-assign on Any class member (I-050 Any coercion umbrella)",
    );
}

// =============================================================================
// `&&=` / `||=` × Any/TypeVar Tier 2 honest error (Iteration v14 deep-deep
// review F-L3-2): wording consistency with existing
// compound_logical_assign.rs::desugar_compound_logical_assign_stmts blocked
// path
// =============================================================================

#[test]
fn test_and_assign_b4_any_lhs_errs_with_i050_umbrella_wording() {
    // `&&=` × class member B4 × Any LHS: pre-Iteration v14 emitted generic
    // "logical compound assign on unsupported lhs type (truthy predicate
    // unavailable)" (= truthy.rs returned None for Any). Post-Iteration v14:
    // pre-check Any/TypeVar gate emits specific I-050 / generic bounds wording
    // (consistent with existing compound_logical_assign.rs Ident-target case)。
    let src = "class Foo { _v: any = true; \
               get value(): any { return this._v; } \
               set value(v: any) { this._v = v; } }\n\
               function probe(): void { const f = new Foo(); f.value &&= 5; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "compound logical assign on Any/TypeVar class member \
         (I-050 umbrella / generic bounds)",
    );
}

#[test]
fn test_or_assign_b4_any_lhs_errs_with_i050_umbrella_wording() {
    // `||=` × class member B4 × Any LHS: same I-050 umbrella wording (matches
    // pre-check Any/TypeVar gate in dispatch_b4_strategy)。Verifies both
    // `&&=` and `||=` go through identical Any/TypeVar gate (op-axis
    // orthogonality for the I-050 umbrella check)。
    //
    // Note: TypeVar (generic param T) class member dispatch is a corner case
    // tracked by I-NNN (generic bounds PRD candidate); current test uses Any
    // LHS as the structurally accessible representative (= classify_member_receiver
    // resolves Any-typed receiver via TypeResolver expr_types reliably for
    // non-generic class definitions, while generic class fixture resolution
    // depends on TypeResolver generic parameter handling which is out of T9
    // scope)。
    let src = "class Foo { _v: any = false; \
               get value(): any { return this._v; } \
               set value(v: any) { this._v = v; } }\n\
               function probe(): void { const f = new Foo(); f.value ||= true; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "compound logical assign on Any/TypeVar class member \
         (I-050 umbrella / generic bounds)",
    );
}

// =============================================================================
// `&&=` / `||=` × always-truthy const-fold (Iteration v14 deep-deep review
// F-L4-2): cohesive with compound_logical_assign.rs::const_fold_always_truthy_stmts
// =============================================================================

#[test]
fn test_and_assign_b4_always_truthy_emits_const_fold_unconditional_setter() {
    // `&&=` × class member B4 × always-truthy LHS (Vec<f64>): const-fold to
    // unconditional setter call (no `if` predicate)。
    // Statement context: `<setter call>;` (single Stmt::Expr in Block)。
    let src = "class Foo { _arr: number[] = []; \
               get arr(): number[] { return this._arr; } \
               set arr(v: number[]) { this._arr = v; } }\n\
               function probe(): void { const f = new Foo(); f.arr &&= [1, 2, 3]; }";
    let stmts = convert_logical_stmt_in_probe(src, 1, 1)
        .expect("&&= always-truthy const-fold must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    match block {
        Expr::Block(inner_stmts) => {
            assert_eq!(
                inner_stmts.len(),
                1,
                "&&= always-truthy const-fold must emit exactly 1 setter Stmt (no `if` predicate), got: {inner_stmts:?}"
            );
            // Direct setter call (no Some-wrap since Vec is not Option)
            match &inner_stmts[0] {
                IrStmt::Expr(Expr::MethodCall { method, args, .. }) => {
                    assert_eq!(method, "set_arr", "must call set_arr");
                    assert_eq!(args.len(), 1, "setter must have 1 arg");
                    // arg is raw Vec literal (no Some-wrap)
                    assert!(
                        !matches!(
                            args.first(),
                            Some(Expr::FnCall {
                                target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
                                ..
                            })
                        ),
                        "Vec<T> LHS must NOT wrap setter arg in Some(_)、got: {args:?}"
                    );
                }
                other => panic!("stmt 0 must be Stmt::Expr(MethodCall set_arr), got {other:?}"),
            }
        }
        other => panic!("expected Expr::Block, got {other:?}"),
    }
}

#[test]
fn test_or_assign_b4_always_truthy_emits_const_fold_no_op() {
    // `||=` × class member B4 × always-truthy LHS (Vec<f64>): const-fold to
    // no-op (predicate always-false → setter never called)。
    // Statement context: empty Block (SE-free receiver)。
    let src = "class Foo { _arr: number[] = []; \
               get arr(): number[] { return this._arr; } \
               set arr(v: number[]) { this._arr = v; } }\n\
               function probe(): void { const f = new Foo(); f.arr ||= [1, 2, 3]; }";
    let stmts = convert_logical_stmt_in_probe(src, 1, 1)
        .expect("||= always-truthy const-fold must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    match block {
        Expr::Block(inner_stmts) => assert!(
            inner_stmts.is_empty(),
            "||= always-truthy const-fold must emit empty Block (no-op), got: {inner_stmts:?}"
        ),
        other => panic!("expected Expr::Block, got {other:?}"),
    }
}

#[test]
fn test_and_assign_b4_always_truthy_se_having_emits_iife_with_const_fold() {
    // `&&=` always-truthy × SE-having receiver: const-fold + INV-3 IIFE wrap
    // for receiver 1-evaluate compliance。
    // Shape: `{ let mut __ts_recv = <obj>; __ts_recv.set_arr(...); }`
    let src = "class Foo { _arr: number[] = []; \
               get arr(): number[] { return this._arr; } \
               set arr(v: number[]) { this._arr = v; } }\n\
               function getInstance(): Foo { return new Foo(); }\n\
               function probe(): void { getInstance().arr &&= [1]; }";
    let stmts = convert_logical_stmt_in_probe(src, 2, 0)
        .expect("&&= always-truthy SE-having const-fold must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    match block {
        Expr::Block(inner_stmts) => {
            assert_eq!(
                inner_stmts.len(),
                2,
                "SE-having &&= const-fold must emit 2 stmts (Let __ts_recv + setter), got: {inner_stmts:?}"
            );
            // Stmt 0: let mut __ts_recv = getInstance()
            assert!(
                matches!(
                    &inner_stmts[0],
                    IrStmt::Let { mutable: true, name, .. } if name == "__ts_recv"
                ),
                "stmt 0 must be Let mut __ts_recv (INV-3 binding), got: {:?}",
                inner_stmts[0]
            );
            // Stmt 1: __ts_recv.set_arr(...)
            assert!(
                matches!(
                    &inner_stmts[1],
                    IrStmt::Expr(Expr::MethodCall { object, method, .. })
                        if matches!(object.as_ref(), Expr::Ident(n) if n == "__ts_recv")
                            && method == "set_arr"
                ),
                "stmt 1 must be __ts_recv.set_arr(...), got: {:?}",
                inner_stmts[1]
            );
        }
        other => panic!("expected Expr::Block, got {other:?}"),
    }
}

// =============================================================================
// Cell 39 / 40 / 41-d Expression context (Iteration v14 deep-deep review
// F-T-1/2/3): coverage gap fix
// =============================================================================

#[test]
fn test_cell_39_b4_and_assign_bool_expression_context_emits_block_with_tail() {
    // Cell 39 expression context: `(f.b &&= false)` inside var init → Block +
    // tail = post-state getter call。
    let src = format!(
        "{B4_FOO_BOOL_SRC}\n\
         function probe(): void {{ const f = new Foo(); const _z = (f.b &&= false); }}"
    );
    let fx = TctxFixture::from_source(&src);
    let module = fx.module();
    let init = extract_fn_body_var_init(module, 1, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&init)
        .expect("cell 39 expression context must succeed");
    // Tail must be the post-state getter call `f.b()`
    if let Expr::Block(stmts) = &result {
        match stmts.last() {
            Some(IrStmt::TailExpr(Expr::MethodCall { method, .. })) if method == "b" => {}
            other => {
                panic!("cell 39 expression must have TailExpr = f.b() getter call, got: {other:?}")
            }
        }
    } else {
        panic!("expected Expr::Block, got: {result:?}");
    }
}

#[test]
fn test_cell_40_b4_or_assign_bool_expression_context_emits_block_with_tail() {
    // Cell 40 expression context: same as cell 39 but with `||=`。
    let src = format!(
        "{B4_FOO_BOOL_SRC}\n\
         function probe(): void {{ const f = new Foo(); const _z = (f.b ||= true); }}"
    );
    let fx = TctxFixture::from_source(&src);
    let module = fx.module();
    let init = extract_fn_body_var_init(module, 1, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&init)
        .expect("cell 40 expression context must succeed");
    if let Expr::Block(stmts) = &result {
        match stmts.last() {
            Some(IrStmt::TailExpr(Expr::MethodCall { method, .. })) if method == "b" => {}
            other => {
                panic!("cell 40 expression must have TailExpr = f.b() getter call, got: {other:?}")
            }
        }
    } else {
        panic!("expected Expr::Block, got: {result:?}");
    }
}

#[test]
fn test_cell_41d_b8_static_and_assign_bool_expression_context_emits_block_with_tail() {
    // Cell 41-d expression context for `&&=` × B8 static × bool: static
    // conditional setter desugar with tail = `Class::b()`。
    let src = "class Foo { static _b: boolean = true; \
               static get b(): boolean { return Foo._b; } \
               static set b(v: boolean) { Foo._b = v; } }\n\
               function probe(): void { const _z = (Foo.b &&= false); }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let init = extract_fn_body_var_init(module, 1, 0);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&init)
        .expect("cell 41-d &&= expression context must succeed");
    if let Expr::Block(stmts) = &result {
        match stmts.last() {
            Some(IrStmt::TailExpr(Expr::FnCall {
                target: CallTarget::UserAssocFn { method, .. },
                ..
            })) if method == "b" => {}
            other => panic!(
                "cell 41-d &&= expression must have TailExpr = Foo::b() FnCall, got: {other:?}"
            ),
        }
    } else {
        panic!("expected Expr::Block, got: {result:?}");
    }
}

// =============================================================================
// SE-having × Expression context tail uses __ts_recv (Iteration v14 deep-deep
// review F-T-4): verifies tail expr also goes through IIFE binding for INV-3
// 1-evaluate compliance
// =============================================================================

#[test]
fn test_se_having_receiver_expression_context_tail_uses_ts_recv_binding() {
    // SE-having receiver `getInstance().value ??= 42` in expression context:
    // tail = post-state getter call must use __ts_recv (not original
    // getInstance()) — otherwise getInstance() would be called twice (Let
    // init + tail) violating INV-3 1-evaluate。
    let src = format!(
        "{B4_CACHE_OPTION_SRC}\n\
         function getInstance(): Cache {{ return new Cache(); }}\n\
         function probe(): void {{ const _z = (getInstance().value ??= 42); }}"
    );
    let fx = TctxFixture::from_source(&src);
    let module = fx.module();
    let init = extract_fn_body_var_init(module, 2, 0);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&init)
        .expect("SE-having Expression context must succeed");
    if let Expr::Block(stmts) = &result {
        // 3 stmts: Let __ts_recv, Stmt::If, Stmt::TailExpr
        assert_eq!(
            stmts.len(),
            3,
            "SE-having Expression context must have 3 stmts (Let + If + TailExpr), got: {stmts:?}"
        );
        // Tail: __ts_recv.value()
        match &stmts[2] {
            IrStmt::TailExpr(Expr::MethodCall { object, method, .. }) => {
                assert!(
                    matches!(object.as_ref(), Expr::Ident(n) if n == "__ts_recv"),
                    "Tail getter must use __ts_recv binding (INV-3), got: {object:?}"
                );
                assert_eq!(method, "value", "Tail must call .value() getter");
            }
            other => panic!("stmt 2 must be TailExpr __ts_recv.value(), got: {other:?}"),
        }
    } else {
        panic!("expected Expr::Block, got: {result:?}");
    }
}
