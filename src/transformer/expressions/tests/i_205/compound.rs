//! I-205 T8 (Arithmetic / Bitwise compound assign Member target dispatch) — cells
//! 20-29 + 30-35 + Static defensive arms + INV-3 1-evaluate compliance lock-in tests.
//!
//! 全 test は AssignExpr (`obj.x += v` / `Class.x -= v` / etc.) を `convert_expr` 経由で
//! 実行、`dispatch_member_compound` 内 dispatch arm が ideal IR (Tier 1 setter desugar
//! Block / Tier 2 honest error / Fallback FieldAccess Assign) を emit することを verify。
//!
//! ## 対象 cells / 機能
//!
//! - **Cells 20, 28** (B1 field / B9 unknown × `+=`): Fallback path で既存 `Expr::Assign
//!   { target: FieldAccess, value: BinaryOp { left: FieldAccess, op: Add, right } }` 維持
//!   (regression lock-in、本 T8 で B4/B8 dispatch 追加時の non-class receiver 影響なし
//!   structural enforcement)。
//! - **Cell 21** (B4 both × `+=` × side-effect-free receiver): setter desugar block
//!   `{ let __ts_new = obj.x() + v; obj.set_x(__ts_new); __ts_new }` (yield_new、prefix
//!   update と same shape with rhs replacing 1.0)。
//! - **Cell 21 IIFE** (B4 both × `+=` × side-effect-having receiver): INV-3 1-evaluate
//!   compliance via IIFE form `{ let mut __ts_recv = <receiver>; let __ts_new =
//!   __ts_recv.x() + v; __ts_recv.set_x(__ts_new); __ts_new }`。
//! - **Cell 22** (B2 getter only × `+=`): Tier 2 `"compound assign to read-only
//!   property"`。
//! - **Cell 23** (B3 setter only × `+=`): Tier 2 `"compound assign read of write-only
//!   property"` (compound assign は read 先行、getter 不在で read fail)。
//! - **Cell 25** (B6 regular method × `+=`): Tier 2 `"compound assign to method"`。
//! - **Cell 26** (B7 inherited × `+=`): Tier 2 `"compound assign to inherited
//!   accessor"`。
//! - **Cell 27** (B8 static × `+=`): static setter desugar `{ let __ts_new = Class::x()
//!   + v; Class::set_x(__ts_new); __ts_new }` (Static dispatch では receiver = class
//!     TypeName で side-effect なし、IIFE form 不要)。
//! - **Cells 29-d, 33, 34-c** (B4 × `-=` / `|=` / `<<=`): cell 21 と op-axis
//!   orthogonality-equivalent (BinOp::Sub / BitOr / Shl 置換のみ、dispatch logic 同一)。
//! - **Cells 29-e-d, 35-d** (B8 × `-=` / `|=`): cell 27 と op-axis orthogonality-equivalent。
//! - **Static defensive arms** (matrix cell 化なし): static B2 / B3 / B6 / B7 compound
//!   で `dispatch_static_member_compound` の defensive Tier 2 wording lock-in (subsequent
//!   T11 (11-c) で matrix expansion 予定の pre-implementation contract)。
//!
//! ## Cross-cutting verifications
//!
//! - **INV-3 1-evaluate compliance** (side-effect-free vs side-effect-having receiver
//!   C1 branch coverage): `is_side_effect_free` helper の判定 (Ident → true / FieldAccess
//!   recursive → object 依存 / FnCall → false) を 各 path で verify、IIFE form emit が
//!   side-effect-having 時のみ fire することを structural lock-in。
//! - **Op-axis orthogonality merge** (Rule 1 (1-4) compliance): 全 11 ops (AddAssign ..
//!   ZeroFillRShiftAssign) が同 dispatch arm を経由、BinOp 置換のみで IR shape 同一。
//!   代表 ops で coverage、`arithmetic_compound_op_to_binop` mapping helper の 1-to-1
//!   conversion を構造的 verify。
//! - **`__ts_` namespace hygiene** (I-154 + T7 + T8 extension): 全 emission binding
//!   (`__ts_new` / `__ts_recv`) が `__ts_` prefix per [I-154 namespace reservation
//!   rule]、user code identifier (`_new`、`_recv` 等) との collision 防止。

use super::super::*;

use crate::ir::{BinOp, CallTarget, Expr, Stmt as IrStmt, UserTypeRef};
use crate::transformer::UnsupportedSyntaxError;

// =============================================================================
// Test helpers (DRY refactor、design-integrity.md "DRY"、update.rs と symmetric)
// =============================================================================

/// Tier 1 emission tests 用 helper: TctxFixture + Transformer::convert_expr boilerplate を
/// 集約。`src` の `function probe(): void { ... }` body の `stmt_index` 番目 ExprStmt を
/// extract して `convert_expr` する。
fn convert_compound_in_probe(
    src: &str,
    fn_index: usize,
    stmt_index: usize,
) -> anyhow::Result<Expr> {
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let stmt = extract_fn_body_expr_stmt(module, fn_index, stmt_index);
    Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new()).convert_expr(&stmt)
}

/// Tier 2 honest error tests 用 helper: fixture + convert + downcast + kind assertion を
/// 集約。`expected_kind` で `UnsupportedSyntaxError.kind` を exact match verify。
fn assert_compound_in_probe_unsupported_syntax_error_kind(
    src: &str,
    fn_index: usize,
    stmt_index: usize,
    expected_kind: &str,
) {
    let err = convert_compound_in_probe(src, fn_index, stmt_index)
        .expect_err(&format!("expected Err with kind={expected_kind}"));
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .unwrap_or_else(|e| panic!("error must be UnsupportedSyntaxError, got: {e:?}"));
    assert_eq!(usx.kind, expected_kind, "kind mismatch");
}

/// Asserts an `Expr::Block` of T8 setter desugar shape (yield_new):
/// `{ let __ts_new = <getter_expr> <op> <rhs>; <setter_stmt>; __ts_new }`.
///
/// `getter_expr_factory` produces the IR for the getter call (instance MethodCall
/// `obj.x()` or static FnCall `Class::x()`)、`setter_call_factory` produces the IR for
/// the setter call (`obj.set_x(__ts_new)` / `Class::set_x(__ts_new)`)、`op` is the
/// expected `BinOp` and `rhs` is the expected expanded RHS.
fn assert_setter_desugar_block(
    block: &Expr,
    expected_getter: Expr,
    op: BinOp,
    expected_rhs: Expr,
    expected_setter_call: Expr,
) {
    let stmts = match block {
        Expr::Block(s) => s,
        other => panic!("expected Expr::Block, got: {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "setter desugar block must have 3 stmts");
    // Stmt 0: let __ts_new = <getter> <op> <rhs>;
    let init = match &stmts[0] {
        IrStmt::Let {
            mutable: false,
            name,
            init: Some(init),
            ..
        } if name == "__ts_new" => init,
        other => panic!("stmt 0: expected Let __ts_new (mutable=false), got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::BinaryOp {
            left: Box::new(expected_getter),
            op,
            right: Box::new(expected_rhs),
        },
        "stmt 0 init: BinOp shape mismatch"
    );
    // Stmt 1: <setter call>(__ts_new);
    let setter_call = match &stmts[1] {
        IrStmt::Expr(call) => call,
        other => panic!("stmt 1: expected ExprStmt, got {other:?}"),
    };
    assert_eq!(
        setter_call, &expected_setter_call,
        "stmt 1: setter call mismatch"
    );
    // Stmt 2: __ts_new (TailExpr)
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_new"),
        "stmt 2: must be TailExpr __ts_new, got {:?}",
        stmts[2]
    );
}

// =============================================================================
// Cells 20, 28 (B1 field / B9 unknown) — Fallback path regression lock-in
// =============================================================================

#[test]
fn test_cell_20_b1_field_add_assign_emits_fallback_field_access_binary_op() {
    // Matrix cell 20: B1 field, A3 `+=`, regression preserve
    // Ideal: `Expr::Assign { target: FieldAccess f.x, value: BinOp { FieldAccess f.x, Add, 5.0 } }`
    let result = convert_compound_in_probe(
        "class Foo { x: number = 0; }\n\
         function probe(): void { const f = new Foo(); f.x += 5; }",
        1,
        1,
    )
    .expect("cell 20 must succeed (B1 field fallback)");
    let field_access = Expr::FieldAccess {
        object: Box::new(Expr::Ident("f".to_string())),
        field: "x".to_string(),
    };
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(field_access.clone()),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(field_access),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(5.0)),
            }),
        },
        "cell 20: must emit FieldAccess Assign with BinaryOp value (regression)"
    );
}

#[test]
fn test_cell_28_b9_unknown_add_assign_emits_fallback_field_access_binary_op() {
    // Matrix cell 28: B9 unknown receiver (object literal type、registry 未登録) → Fallback
    let result = convert_compound_in_probe(
        "function probe(): void { const f = { x: 5 }; f.x += 3; }",
        0,
        1,
    )
    .expect("cell 28 must succeed (B9 unknown fallback)");
    let field_access = Expr::FieldAccess {
        object: Box::new(Expr::Ident("f".to_string())),
        field: "x".to_string(),
    };
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(field_access.clone()),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(field_access),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(3.0)),
            }),
        },
        "cell 28: must emit FieldAccess Assign with BinaryOp value (regression、B9 fallback)"
    );
}

// =============================================================================
// Cells 21 / 29-d / 33 / 34-c (B4 both × side-effect-free receiver) — setter desugar
// =============================================================================

/// B4 (getter+setter pair) class fixture for cell 21 family。
const B4_COUNTER_CLASS_SRC: &str = "class Counter { _n: number = 5; \
                                    get value(): number { return this._n; } \
                                    set value(v: number) { this._n = v; } }";

#[test]
fn test_cell_21_b4_add_assign_side_effect_free_receiver_emits_setter_desugar_yield_new() {
    // Matrix cell 21: B4 (getter+setter pair) × `+=` × Ident receiver (side-effect-free)
    // Ideal: { let __ts_new = c.value() + 7.0; c.set_value(__ts_new); __ts_new }
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value += 7; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1)
        .expect("cell 21 must succeed (B4 setter desugar、side-effect-free recv)");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::Add,
        Expr::NumberLit(7.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_cell_29d_b4_sub_assign_emits_setter_desugar_with_bin_op_sub() {
    // Matrix cell 29-d: B4 × `-=` × Ident receiver (cell 21 と op-axis orthogonality-equivalent)
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value -= 2; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("cell 29-d must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::Sub,
        Expr::NumberLit(2.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_cell_33_b4_bit_or_assign_emits_setter_desugar_with_bin_op_bit_or() {
    // Matrix cell 33: B4 × `|=` (bitwise compound) × Ident receiver (cell 21 と orth-equiv)
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value |= 4; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("cell 33 must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::BitOr,
        Expr::NumberLit(4.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_cell_34c_b4_lshift_assign_emits_setter_desugar_with_bin_op_shl() {
    // Matrix cell 34-c: B4 × `<<=` × Ident receiver (cell 21 と op-axis orthogonality-equivalent)
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value <<= 1; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("cell 34-c must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::Shl,
        Expr::NumberLit(1.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

// =============================================================================
// Cell 21 IIFE form (B4 × `+=` × side-effect-having receiver) — INV-3 compliance
// =============================================================================

#[test]
fn test_cell_21_b4_add_assign_side_effect_having_receiver_emits_iife_form_for_inv3() {
    // Matrix cell 21 IIFE: B4 × `+=` × FnCall receiver (side-effect-having)
    // INV-3 (a) Property statement: `getInstance().value += v` で `getInstance()` は 1 回のみ eval
    // Ideal: { let mut __ts_recv = getInstance();
    //          let __ts_new = __ts_recv.value() + 7.0;
    //          __ts_recv.set_value(__ts_new);
    //          __ts_new }
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function getInstance(): Counter {{ return new Counter(); }}\n\
         function probe(): void {{ getInstance().value += 7; }}"
    );
    let result = convert_compound_in_probe(&src, 2, 0)
        .expect("cell 21 IIFE must succeed (B4 setter desugar、side-effect recv)");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("expected Expr::Block, got: {other:?}"),
    };
    assert_eq!(stmts.len(), 4, "IIFE form block must have 4 stmts");
    // Stmt 0: let mut __ts_recv = getInstance();
    let recv_init = match &stmts[0] {
        IrStmt::Let {
            mutable: true,
            name,
            init: Some(init),
            ..
        } if name == "__ts_recv" => init,
        other => panic!("stmt 0: expected Let mut __ts_recv, got {other:?}"),
    };
    // getInstance() は Expr::FnCall { CallTarget::UserFn { name: "getInstance" }, args: [] }
    // CallTarget は Path であるため direct compare は IR variant 構造に依存; tolerate by
    // matching only the surface FnCall shape with empty args (= side-effect-having indicator)
    assert!(
        matches!(recv_init, Expr::FnCall { args, .. } if args.is_empty()),
        "stmt 0 init: must be FnCall getInstance() with empty args, got {recv_init:?}"
    );
    // Stmt 1: let __ts_new = __ts_recv.value() + 7.0;
    let new_init = match &stmts[1] {
        IrStmt::Let {
            mutable: false,
            name,
            init: Some(init),
            ..
        } if name == "__ts_new" => init,
        other => panic!("stmt 1: expected Let __ts_new, got {other:?}"),
    };
    assert_eq!(
        new_init,
        &Expr::BinaryOp {
            left: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("__ts_recv".to_string())),
                method: "value".to_string(),
                args: vec![],
            }),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(7.0)),
        },
        "stmt 1 init: must be __ts_recv.value() + 7.0 (= INV-3: receiver replaced by binding)"
    );
    // Stmt 2: __ts_recv.set_value(__ts_new);
    let setter_call = match &stmts[2] {
        IrStmt::Expr(call) => call,
        other => panic!("stmt 2: expected ExprStmt, got {other:?}"),
    };
    assert_eq!(
        setter_call,
        &Expr::MethodCall {
            object: Box::new(Expr::Ident("__ts_recv".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
        "stmt 2: must be __ts_recv.set_value(__ts_new) (= INV-3: receiver replaced)"
    );
    // Stmt 3: __ts_new (TailExpr)
    assert!(
        matches!(&stmts[3], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_new"),
        "stmt 3: must be TailExpr __ts_new"
    );
}

#[test]
fn test_inv3_field_access_receiver_recursive_decision_remains_side_effect_free() {
    // INV-3 helper の C1 branch coverage:
    //   `is_side_effect_free(FieldAccess { object: Ident, .. })` = is_side_effect_free(Ident) = true
    //   → `outer.inner.value += 1` で `outer.inner` (FieldAccess + Ident) は side-effect-free 判定
    //   → IIFE form 不採用、直接 emit (= __ts_recv binding 不在)
    //
    // 本 test は recursive 判定の branch (FieldAccess arm) を直接 fire させ、
    // inner.object が Ident の場合 IIFE form skip を verify。
    let src = "class Inner { _n: number = 5; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } }\n\
               class Outer { inner: Inner = new Inner(); }\n\
               function probe(): void { const o = new Outer(); o.inner.value += 3; }";
    let result = convert_compound_in_probe(src, 2, 1).expect("FieldAccess receiver must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("expected Expr::Block, got {other:?}"),
    };
    // FieldAccess receiver = side-effect-free → 3 stmts (IIFE 不採用)
    assert_eq!(
        stmts.len(),
        3,
        "FieldAccess receiver must skip IIFE (= 3 stmts、no __ts_recv binding)、got {} stmts",
        stmts.len()
    );
    // Stmt 1 setter call が `o.inner.set_value(__ts_new)` (= receiver = FieldAccess o.inner) を verify
    let setter_call = match &stmts[1] {
        IrStmt::Expr(call) => call,
        other => panic!("stmt 1: expected ExprStmt, got {other:?}"),
    };
    let expected_recv = Expr::FieldAccess {
        object: Box::new(Expr::Ident("o".to_string())),
        field: "inner".to_string(),
    };
    assert_eq!(
        setter_call,
        &Expr::MethodCall {
            object: Box::new(expected_recv),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
        "stmt 1: must be o.inner.set_value(__ts_new) (= FieldAccess receiver direct embed)"
    );
}

// =============================================================================
// Cells 22 / 23 / 25 / 26 — Tier 2 honest error reclassify (instance B2/B3/B6/B7)
// =============================================================================

#[test]
fn test_cell_22_b2_getter_only_add_assign_errs_with_compound_assign_to_read_only() {
    // Matrix cell 22: B2 (getter only) × `+=` → Tier 2 "compound assign to read-only property"
    let src = "class Foo { _n: number = 0; get value(): number { return this._n; } }\n\
               function probe(): void { const f = new Foo(); f.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "compound assign to read-only property",
    );
}

#[test]
fn test_cell_23_b3_setter_only_add_assign_errs_with_compound_assign_read_of_write_only() {
    // Matrix cell 23: B3 (setter only) × `+=` → Tier 2 "compound assign read of write-only property"
    // (compound assign は read 先行、getter 不在で read fail)
    let src = "class Foo { _n: number = 0; set value(v: number) { this._n = v; } }\n\
               function probe(): void { const f = new Foo(); f.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "compound assign read of write-only property",
    );
}

#[test]
fn test_cell_25_b6_method_add_assign_errs_with_compound_assign_to_method() {
    // Matrix cell 25: B6 (regular method) × `+=` → Tier 2 "compound assign to method"
    let src = "class Foo { value(): number { return 0; } }\n\
               function probe(): void { const f = new Foo(); f.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(src, 1, 1, "compound assign to method");
}

#[test]
fn test_cell_26_b7_inherited_add_assign_errs_with_compound_assign_to_inherited_accessor() {
    // Matrix cell 26: B7 (inherited、parent class accessor) × `+=`
    // → Tier 2 "compound assign to inherited accessor" (本 PRD scope = orthogonal architectural
    //   concern "Class inheritance dispatch"、別 PRD I-206 で Tier 1 化候補)
    let src = "class Base { _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } }\n\
               class Sub extends Base {}\n\
               function probe(): void { const s = new Sub(); s.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(
        src,
        2,
        1,
        "compound assign to inherited accessor",
    );
}

// =============================================================================
// Cells 27 / 29-e-d / 35-d (B8 static × `+=` / `-=` / `|=`) — static setter desugar
// =============================================================================

/// B8 (static getter+setter pair) class fixture for cell 27 family。
const B8_STATIC_CLASS_SRC: &str = "class Counter { static _n: number = 5; \
                                   static get value(): number { return Counter._n; } \
                                   static set value(v: number) { Counter._n = v; } }";

#[test]
fn test_cell_27_b8_static_add_assign_emits_static_setter_desugar() {
    // Matrix cell 27: B8 static accessor × `+=` → static setter desugar
    // Ideal: { let __ts_new = Counter::value() + 8.0; Counter::set_value(__ts_new); __ts_new }
    let src = format!(
        "{B8_STATIC_CLASS_SRC}\n\
         function probe(): void {{ Counter.value += 8; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 0).expect("cell 27 must succeed");
    let counter_ty = || UserTypeRef::new("Counter");
    assert_setter_desugar_block(
        &result,
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: counter_ty(),
                method: "value".to_string(),
            },
            args: vec![],
        },
        BinOp::Add,
        Expr::NumberLit(8.0),
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: counter_ty(),
                method: "set_value".to_string(),
            },
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_cell_29ed_b8_static_sub_assign_emits_static_setter_desugar_with_bin_op_sub() {
    // Matrix cell 29-e-d: B8 × `-=` (cell 27 と op-axis orthogonality-equivalent)
    let src = format!(
        "{B8_STATIC_CLASS_SRC}\n\
         function probe(): void {{ Counter.value -= 3; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 0).expect("cell 29-e-d must succeed");
    let counter_ty = || UserTypeRef::new("Counter");
    assert_setter_desugar_block(
        &result,
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: counter_ty(),
                method: "value".to_string(),
            },
            args: vec![],
        },
        BinOp::Sub,
        Expr::NumberLit(3.0),
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: counter_ty(),
                method: "set_value".to_string(),
            },
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_cell_35d_b8_static_bit_or_assign_emits_static_setter_desugar_with_bin_op_bit_or() {
    // Matrix cell 35-d: B8 × `|=` (cell 27 と op-axis orthogonality-equivalent、bitwise)
    let src = format!(
        "{B8_STATIC_CLASS_SRC}\n\
         function probe(): void {{ Counter.value |= 2; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 0).expect("cell 35-d must succeed");
    let counter_ty = || UserTypeRef::new("Counter");
    assert_setter_desugar_block(
        &result,
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: counter_ty(),
                method: "value".to_string(),
            },
            args: vec![],
        },
        BinOp::BitOr,
        Expr::NumberLit(2.0),
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: counter_ty(),
                method: "set_value".to_string(),
            },
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

// =============================================================================
// Static defensive arms (matrix cell 化なし、subsequent T11 (11-c) で expansion)
// =============================================================================

#[test]
fn test_static_b2_getter_only_add_assign_errs_with_compound_assign_to_read_only_static() {
    // Static B2: `Class.x += v` where Class has only static getter
    // → Tier 2 "compound assign to read-only static property" (defensive arm、matrix cell 化なし)
    let src =
        "class Foo { static _n: number = 0; static get value(): number { return Foo._n; } }\n\
               function probe(): void { Foo.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        0,
        "compound assign to read-only static property",
    );
}

#[test]
fn test_static_b3_setter_only_add_assign_errs_with_compound_assign_read_of_write_only_static() {
    // Static B3: `Class.x += v` where Class has only static setter (defensive arm)
    let src = "class Foo { static _n: number = 0; static set value(v: number) { Foo._n = v; } }\n\
               function probe(): void { Foo.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        0,
        "compound assign read of write-only static property",
    );
}

#[test]
fn test_static_b6_method_add_assign_errs_with_compound_assign_to_static_method() {
    // Static B6: `Class.method += v` where method is a static method (defensive arm)
    let src = "class Foo { static value(): number { return 0; } }\n\
               function probe(): void { Foo.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        0,
        "compound assign to static method",
    );
}

// =============================================================================
// Op-axis orthogonality merge — exhaustive 11-op mapping verify (Iteration v12
// second-review F-EM-1)
// =============================================================================
//
// `arithmetic_compound_op_to_binop` mapping helper の全 11 ops × `Some(BinOp::*)`
// 1-to-1 correspondence を unit test で structural lock-in。Op-axis orthogonality
// merge (Rule 1 (1-4) compliance) の structural verification として、unit test で
// 代表 4 ops (AddAssign / SubAssign / BitOrAssign / LShiftAssign) を C1 branch
// coverage する代わりに、mapping helper 自体の全 11 op exhaustive enumeration を
// verify することで「全 ops が同 dispatch arm を経由」の invariant を 1 file で
// 完全 lock-in (= 4 op B4 dispatch test + 11 op mapping test の組合せで全 11 op の
// dispatch arm coverage が transitively 達成される)。
//
// 本 test は `arithmetic_compound_op_to_binop` が pub(super) でないため
// `assignments.rs` 内 inline test に置けず、cohesive な T8 architectural concern
// (= compound assign Member target dispatch) と co-locate する観点で本 file に
// 配置。BinOp 値の verify は **Member target × B4 で B4 setter desugar の inner
// BinaryOp.op フィールドを直接 assert** することで間接的に達成。

#[test]
fn test_op_axis_mul_assign_emits_setter_desugar_with_bin_op_mul() {
    // Verify `*=` mapping → BinOp::Mul (cell 21 family、unit test 4 ops 未 cover の `*=`)
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value *= 3; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("MulAssign mapping must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::Mul,
        Expr::NumberLit(3.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_op_axis_div_assign_emits_setter_desugar_with_bin_op_div() {
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value /= 2; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("DivAssign mapping must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::Div,
        Expr::NumberLit(2.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_op_axis_mod_assign_emits_setter_desugar_with_bin_op_mod() {
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value %= 4; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("ModAssign mapping must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::Mod,
        Expr::NumberLit(4.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_op_axis_bit_and_assign_emits_setter_desugar_with_bin_op_bit_and() {
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value &= 7; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("BitAndAssign mapping must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::BitAnd,
        Expr::NumberLit(7.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_op_axis_bit_xor_assign_emits_setter_desugar_with_bin_op_bit_xor() {
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value ^= 5; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("BitXorAssign mapping must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::BitXor,
        Expr::NumberLit(5.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_op_axis_rshift_assign_emits_setter_desugar_with_bin_op_shr() {
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value >>= 2; }}"
    );
    let result = convert_compound_in_probe(&src, 1, 1).expect("RShiftAssign mapping must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::Shr,
        Expr::NumberLit(2.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_op_axis_zero_fill_rshift_assign_emits_setter_desugar_with_bin_op_ushr() {
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function probe(): void {{ const c = new Counter(); c.value >>>= 1; }}"
    );
    let result =
        convert_compound_in_probe(&src, 1, 1).expect("ZeroFillRShiftAssign mapping must succeed");
    assert_setter_desugar_block(
        &result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        BinOp::UShr,
        Expr::NumberLit(1.0),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
    );
}

#[test]
fn test_static_b7_inherited_add_assign_errs_with_compound_assign_to_inherited_static_accessor() {
    // Static B7: `Class.x += v` where x is inherited static accessor from parent (defensive arm)
    let src = "class Base { static _n: number = 0; \
               static get value(): number { return Base._n; } \
               static set value(v: number) { Base._n = v; } }\n\
               class Sub extends Base {}\n\
               function probe(): void { Sub.value += 1; }";
    assert_compound_in_probe_unsupported_syntax_error_kind(
        src,
        2,
        0,
        "compound assign to inherited static accessor",
    );
}

// =============================================================================
// T8 INV-3 back-port (Iteration v12) — UpdateExpr Member target with side-effect-having
// receiver emits IIFE form for 1-evaluate compliance (T7 helper update verify)
// =============================================================================
//
// T7 (Iteration v11) で発覚した latent gap: `dispatch_instance_member_update` の B4 setter
// desugar arm が non-Ident receiver (e.g., `getInstance().value++`) で receiver IR を 2 回
// embed → Rust source 上 `getInstance()` が 2 回 evaluate される silent semantic loss。
// T8 (Iteration v12) で `is_side_effect_free` helper + `wrap_with_recv_binding` IIFE wrapper
// を `dispatch_instance_member_compound` と shared 化、T7 helper にも back-port (= cohesive
// batch、`build_setter_desugar_block` + `wrap_with_recv_binding` shared between T7 update +
// T8 compound)。
//
// 本 test は T8 GREEN 後に T7 helper の IIFE wrap 挙動 (= INV-3 1-evaluate compliance) を
// structural lock-in。Side-effect-free receiver (Ident) の既存挙動 (cell 43 family) は
// 不変、本 back-port は side-effect-having receiver path のみ structurally fix。本 test を
// `update.rs` (T7 main test file) ではなく本 file (T8 architectural concern = "compound
// assign + INV-3 1-evaluate") に co-locate (= INV-3 cross-cutting invariant の verification
// は T8 architectural concern と論理的 cohesive)。

/// T8 INV-3 back-port test 用 helper: TctxFixture + Transformer::convert_expr boilerplate を
/// 集約 (`convert_compound_in_probe` と symmetric の UpdateExpr 版)。
fn convert_update_in_probe_for_inv3(
    src: &str,
    fn_index: usize,
    stmt_index: usize,
) -> anyhow::Result<Expr> {
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let stmt = extract_fn_body_expr_stmt(module, fn_index, stmt_index);
    Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new()).convert_expr(&stmt)
}

#[test]
fn test_t8_inv3_backport_update_postfix_increment_side_effect_having_receiver_emits_iife_form() {
    // T7 dispatch_instance_member_update + T8 INV-3 back-port:
    // `getInstance().value++` (postfix `++` on FnCall receiver) → IIFE form
    // Ideal: { let mut __ts_recv = getInstance();
    //          let __ts_old = __ts_recv.value();
    //          __ts_recv.set_value(__ts_old + 1.0);
    //          __ts_old }
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\n\
         function getInstance(): Counter {{ return new Counter(); }}\n\
         function probe(): void {{ getInstance().value++; }}"
    );
    let result =
        convert_update_in_probe_for_inv3(&src, 2, 0).expect("T7 IIFE back-port must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("expected Expr::Block, got: {other:?}"),
    };
    assert_eq!(
        stmts.len(),
        4,
        "IIFE form (postfix update) must have 4 stmts、got {} stmts",
        stmts.len()
    );
    // Stmt 0: let mut __ts_recv = getInstance();
    let recv_init = match &stmts[0] {
        IrStmt::Let {
            mutable: true,
            name,
            init: Some(init),
            ..
        } if name == "__ts_recv" => init,
        other => panic!("stmt 0: expected Let mut __ts_recv, got {other:?}"),
    };
    assert!(
        matches!(recv_init, Expr::FnCall { args, .. } if args.is_empty()),
        "stmt 0 init: must be FnCall getInstance() (= side-effect-having receiver indicator)、\
         got {recv_init:?}"
    );
    // Stmt 1: let __ts_old = __ts_recv.value();
    let old_init = match &stmts[1] {
        IrStmt::Let {
            mutable: false,
            name,
            init: Some(init),
            ..
        } if name == "__ts_old" => init,
        other => panic!("stmt 1: expected Let __ts_old, got {other:?}"),
    };
    assert_eq!(
        old_init,
        &Expr::MethodCall {
            object: Box::new(Expr::Ident("__ts_recv".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        "stmt 1 init: must be __ts_recv.value() (= INV-3: receiver replaced by binding)"
    );
    // Stmt 2: __ts_recv.set_value(__ts_old + 1.0);
    let setter_call = match &stmts[2] {
        IrStmt::Expr(call) => call,
        other => panic!("stmt 2: expected ExprStmt, got {other:?}"),
    };
    assert_eq!(
        setter_call,
        &Expr::MethodCall {
            object: Box::new(Expr::Ident("__ts_recv".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::BinaryOp {
                left: Box::new(Expr::Ident("__ts_old".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(1.0)),
            }],
        },
        "stmt 2: must be __ts_recv.set_value(__ts_old + 1.0)"
    );
    // Stmt 3: __ts_old (TailExpr、postfix yields old value)
    assert!(
        matches!(&stmts[3], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_old"),
        "stmt 3: must be TailExpr __ts_old (postfix yield)"
    );
}
