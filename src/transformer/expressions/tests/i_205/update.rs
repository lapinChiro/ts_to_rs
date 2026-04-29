//! I-205 T7 (UpdateExpr `++`/`--` Member target dispatch) — cells 42-45 lock-in tests.
//!
//! 全 test は UpdateExpr (`obj.x++` / `Class.x--` etc.) を `convert_expr` 経由で実行、
//! `convert_update_expr_member_arm` 内 dispatch arm が ideal IR (Tier 1 setter desugar block /
//! Tier 2 honest error / B1/B9 fallback FieldAccess BinOp block) を emit することを verify。
//!
//! ## 対象 cells / 機能
//!
//! - **Cells 42, 45-a, 45-de** (B1 field / B9 unknown): fallback `Expr::Block` with FieldAccess
//!   `+= 1.0` / `-= 1.0` semantic + postfix old-value preservation (regression Tier 2 → Tier 1
//!   transition、convert_update_expr が pre-T7 で Member target を `unsupported update
//!   expression target` として全面 reject していた状態を Tier 1 化)。
//! - **Cells 43, 45-c** (B4 both、numeric): setter desugar block
//!   (postfix `{ let __ts_old = obj.x(); obj.set_x(__ts_old + 1.0); __ts_old }`、
//!   prefix `{ let __ts_new = obj.x() + 1.0; obj.set_x(__ts_new); __ts_new }`)。
//! - **Cell 44** (B4 both、non-numeric): Tier 2 honest error reclassify
//!   `"increment of non-numeric (String/etc.) — TS NaN coercion semantic"` per Rule 3 (3-3)。
//!   `--` symmetric 用に `"decrement of non-numeric (...)"` も verify。
//! - **Cell 45-b** (B2 getter only): Tier 2 honest `"write to read-only property"`
//!   (++/-- は write 必要、setter 不在)。
//! - **B3 setter only** (matrix cell 化なし、本 PRD scope 内 Update-specific Tier 2): Tier 2
//!   honest `"read of write-only property"` (++/-- は read 先行、getter 不在)。
//! - **Cells 45-db, 45-dc** (B6 method / B7 inherited): Tier 2 honest `"write to method"` /
//!   `"write to inherited accessor"` (`dispatch_instance_member_write` と symmetric wording)。
//! - **Cell 45-dd** (B8 static、numeric): static setter desugar block
//!   `{ let __ts_old = Class::x(); Class::set_x(__ts_old - 1.0); __ts_old }`。
//!
//! ## Cross-cutting verifications
//!
//! - **Postfix vs prefix invariant** (matrix cells 42-45): postfix yields **old** value, prefix
//!   yields **new** value。両 form で同 setter desugar / fallback shape を共有しつつ、
//!   `__ts_old` / `__ts_new` binding name で role を区別。
//! - **`__ts_` namespace hygiene** (I-154 + T7 extension): 全 emission binding が `__ts_` prefix
//!   per [I-154 namespace reservation rule]、user code identifier (`_old`、`x` 等) との collision
//!   防止。
//! - **Computed (`obj[i]++`) reject**: matrix scope 外、existing `unsupported update expression
//!   target` error path を維持 (= 既存 `test_convert_expr_update_non_ident_target_errors` で
//!   verify、本 file では補強のため Member Computed pattern も verify)。

use super::super::*;

use crate::ir::{BinOp, CallTarget, Expr, Stmt as IrStmt, UserTypeRef};
use crate::transformer::UnsupportedSyntaxError;

// =============================================================================
// Test helpers (DRY refactor、design-integrity.md "DRY")
// =============================================================================
//
// Tier 1 emission tests (= Block-form IR verification) use `convert_update_in_probe`
// for fixture + convert call boilerplate。Tier 2 honest error tests (= UnsupportedSyntaxError
// kind verification) use `assert_in_probe_unsupported_syntax_error_kind` for fixture +
// convert + downcast + kind assertion boilerplate。D3 raw expr tests (`(x)++` / `this++`
// 等の non-Ident non-Member arg) use `assert_expr_unsupported_syntax_error_kind` for
// parse_expr + convert + downcast + kind assertion boilerplate。

/// Tier 1 emission tests 用 helper: TctxFixture + Transformer::convert_expr boilerplate を
/// 集約。`src` の `function probe(): void { ... }` body の `stmt_index` 番目 ExprStmt を
/// extract して `convert_expr` する。
///
/// `fn_index` = module body 内 function declaration の index、`stmt_index` = function body
/// 内 ExprStmt の index。helper 内で `fx` (TctxFixture) は drop されるが、`Result<Expr>`
/// は owned Expr / anyhow::Error を返すため borrow 不要 (= helper signature lifetime
/// 制約なし)。
fn convert_update_in_probe(src: &str, fn_index: usize, stmt_index: usize) -> anyhow::Result<Expr> {
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let update_stmt = extract_fn_body_expr_stmt(module, fn_index, stmt_index);
    Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&update_stmt)
}

/// Tier 2 honest error tests 用 helper: fixture + convert + downcast + kind assertion を
/// 集約。`expected_kind` で `UnsupportedSyntaxError.kind` を exact match verify。
fn assert_in_probe_unsupported_syntax_error_kind(
    src: &str,
    fn_index: usize,
    stmt_index: usize,
    expected_kind: &str,
) {
    let err = convert_update_in_probe(src, fn_index, stmt_index)
        .expect_err(&format!("expected Err with kind={expected_kind}"));
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .unwrap_or_else(|e| panic!("error must be UnsupportedSyntaxError, got: {e:?}"));
    assert_eq!(usx.kind, expected_kind, "kind mismatch");
}

/// D3 raw expr tests 用 helper: empty fixture + parse_expr + convert + downcast + kind
/// assertion を集約 (Ident form `_ =>` arm の direct trigger 用、`(x)++` / `this++` 等)。
fn assert_expr_unsupported_syntax_error_kind(expr_src: &str, expected_kind: &str) {
    let expr = parse_expr(expr_src);
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let err = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .expect_err(&format!("expected Err with kind={expected_kind}"));
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .unwrap_or_else(|e| panic!("error must be UnsupportedSyntaxError, got: {e:?}"));
    assert_eq!(usx.kind, expected_kind, "kind mismatch");
}

// =============================================================================
// Cells 42 / 45-a (B1 field, fallback) — regression Tier 2 → Tier 1 transition
// =============================================================================

#[test]
fn test_cell_42_b1_field_postfix_increment_emits_fallback_block_with_old_value() {
    // Matrix cell 42: B1 field, D1 numeric, postfix `obj.x++`
    // Pre-T7: convert_update_expr が Member target を全面 reject (= broken Tier 2)
    // Post-T7: Fallback path で `{ let __ts_old = obj.x; obj.x = __ts_old + 1.0; __ts_old }` emit
    let result = convert_update_in_probe(
        "class Foo { x: number = 0; }\n\
         function probe(): void { const f = new Foo(); f.x++; }",
        1,
        1,
    )
    .expect("cell 42 must succeed (B1 field fallback)");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 42: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "cell 42 postfix block must have 3 stmts");
    // Stmt 0: let __ts_old = f.x;
    let init = match &stmts[0] {
        IrStmt::Let {
            name,
            init: Some(init),
            ..
        } if name == "__ts_old" => init,
        other => panic!("cell 42 stmt 0: expected Let __ts_old, got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::FieldAccess {
            object: Box::new(Expr::Ident("f".to_string())),
            field: "x".to_string(),
        },
        "cell 42 stmt 0 init: must be FieldAccess f.x"
    );
    // Stmt 1: f.x = __ts_old + 1.0;
    let assign_value = match &stmts[1] {
        IrStmt::Expr(Expr::Assign { value, .. }) => value,
        other => panic!("cell 42 stmt 1: expected ExprStmt Assign, got {other:?}"),
    };
    assert_eq!(
        assign_value.as_ref(),
        &Expr::BinaryOp {
            left: Box::new(Expr::Ident("__ts_old".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "cell 42 stmt 1 value: must be BinaryOp __ts_old + 1.0"
    );
    // Stmt 2: __ts_old (TailExpr)
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_old"),
        "cell 42 stmt 2: must be TailExpr __ts_old"
    );
}

#[test]
fn test_cell_42_b1_field_prefix_increment_emits_fallback_block_with_new_value() {
    // B1 field, prefix `++obj.x` → `{ obj.x = obj.x + 1.0; obj.x }` (no let binding)
    let result = convert_update_in_probe(
        "class Foo { x: number = 0; }\n\
         function probe(): void { const f = new Foo(); ++f.x; }",
        1,
        1,
    )
    .expect("cell 42 prefix must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 42 prefix: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 2, "cell 42 prefix block must have 2 stmts");
    let assign_value = match &stmts[0] {
        IrStmt::Expr(Expr::Assign { value, .. }) => value,
        other => panic!("cell 42 prefix stmt 0: expected ExprStmt Assign, got {other:?}"),
    };
    let field_access = Expr::FieldAccess {
        object: Box::new(Expr::Ident("f".to_string())),
        field: "x".to_string(),
    };
    assert_eq!(
        assign_value.as_ref(),
        &Expr::BinaryOp {
            left: Box::new(field_access.clone()),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "cell 42 prefix stmt 0 value: must be BinaryOp f.x + 1.0"
    );
    assert!(
        matches!(&stmts[1], IrStmt::TailExpr(fa) if fa == &field_access),
        "cell 42 prefix stmt 1: must be TailExpr f.x"
    );
}

#[test]
fn test_cell_45a_b1_field_postfix_decrement_emits_fallback_block() {
    // Matrix cell 45-a: B1 field, D1 numeric, postfix `obj.x--`
    // Symmetric to cell 42 but with BinOp::Sub
    let result = convert_update_in_probe(
        "class Foo { x: number = 0; }\n\
         function probe(): void { const f = new Foo(); f.x--; }",
        1,
        1,
    )
    .expect("cell 45-a must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 45-a: expected Expr::Block, got {other:?}"),
    };
    let assign_value = match &stmts[1] {
        IrStmt::Expr(Expr::Assign { value, .. }) => value,
        other => panic!("cell 45-a stmt 1: expected ExprStmt Assign, got {other:?}"),
    };
    assert_eq!(
        assign_value.as_ref(),
        &Expr::BinaryOp {
            left: Box::new(Expr::Ident("__ts_old".to_string())),
            op: BinOp::Sub,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "cell 45-a stmt 1 value: must be BinaryOp __ts_old - 1.0"
    );
}

#[test]
fn test_cell_45de_b9_unknown_postfix_decrement_emits_fallback_block_with_old_value() {
    // Matrix cell 45-de: B9 unknown receiver (registered class なし) → Fallback path
    // 同 emit shape (postfix old-value preservation block with FieldAccess `-= 1.0` semantic)
    // Note: 完全 B9 (= 任意 receiver、registry に該当 type なし) は class 不在で test 困難。
    // 代替として external library type 風の receiver = 受信者 type が `Any` 等で
    // class member dispatch に fall through するケースを simulate。
    // ここでは `let f: any = ...` で `f.x--` を simulate する代わりに、
    // B9 と同 dispatch path (Fallback) を踏む `Object literal type` で test。
    let result = convert_update_in_probe(
        "function probe(): void { const f = { x: 5 }; f.x--; }",
        0,
        1,
    )
    .expect("cell 45-de must succeed (B9 unknown fallback)");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 45-de: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "cell 45-de postfix block must have 3 stmts");
    // Stmt 0: let __ts_old = f.x;  (FieldAccess read for old-value preservation)
    let init = match &stmts[0] {
        IrStmt::Let {
            name,
            init: Some(init),
            ..
        } if name == "__ts_old" => init,
        other => panic!("cell 45-de stmt 0: expected Let __ts_old, got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::FieldAccess {
            object: Box::new(Expr::Ident("f".to_string())),
            field: "x".to_string(),
        },
        "cell 45-de stmt 0 init: must be FieldAccess f.x (regression Tier 2 → Tier 1)"
    );
    // Stmt 1: f.x = __ts_old - 1.0;  (BinOp::Sub for `--`)
    let assign_value = match &stmts[1] {
        IrStmt::Expr(Expr::Assign { value, .. }) => value,
        other => panic!("cell 45-de stmt 1: expected ExprStmt Assign, got {other:?}"),
    };
    assert_eq!(
        assign_value.as_ref(),
        &Expr::BinaryOp {
            left: Box::new(Expr::Ident("__ts_old".to_string())),
            op: BinOp::Sub,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "cell 45-de stmt 1 value: must be BinaryOp __ts_old - 1.0"
    );
    // Stmt 2: __ts_old (TailExpr、postfix old-value yield)
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_old"),
        "cell 45-de stmt 2: must be TailExpr __ts_old (postfix old-value preservation)"
    );
}

// =============================================================================
// Cells 43 / 45-c (B4 both、numeric) — setter desugar block
// =============================================================================

/// B4 (getter+setter pair) class fixture for cells 43, 45-c, 45-c-prefix。
const B4_COUNTER_CLASS_SRC: &str = "class Counter { _n: number = 5; \
                                    get value(): number { return this._n; } \
                                    set value(v: number) { this._n = v; } }";

#[test]
fn test_cell_43_b4_postfix_increment_emits_setter_desugar_with_old_value() {
    // Matrix cell 43: B4 (getter+setter pair), D1 numeric, postfix `obj.x++`
    // Ideal: { let __ts_old = obj.x(); obj.set_x(__ts_old + 1.0); __ts_old }
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\nfunction probe(): void {{ const c = new Counter(); c.value++; }}"
    );
    let result =
        convert_update_in_probe(&src, 1, 1).expect("cell 43 must succeed (B4 setter desugar)");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 43: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "cell 43 postfix block must have 3 stmts");
    // Stmt 0: let __ts_old = c.value();
    let init = match &stmts[0] {
        IrStmt::Let {
            name,
            init: Some(init),
            ..
        } if name == "__ts_old" => init,
        other => panic!("cell 43 stmt 0: expected Let __ts_old, got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        "cell 43 stmt 0 init: must be MethodCall c.value()"
    );
    // Stmt 1: c.set_value(__ts_old + 1.0)
    let setter_call = match &stmts[1] {
        IrStmt::Expr(call @ Expr::MethodCall { .. }) => call,
        other => panic!("cell 43 stmt 1: expected ExprStmt MethodCall, got {other:?}"),
    };
    assert_eq!(
        setter_call,
        &Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::BinaryOp {
                left: Box::new(Expr::Ident("__ts_old".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(1.0)),
            }],
        },
        "cell 43 stmt 1: must be MethodCall c.set_value(__ts_old + 1.0)"
    );
    // Stmt 2: __ts_old (TailExpr)
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_old"),
        "cell 43 stmt 2: must be TailExpr __ts_old"
    );
}

#[test]
fn test_cell_43_b4_prefix_increment_emits_setter_desugar_with_new_value() {
    // B4 prefix `++obj.x`:
    // { let __ts_new = obj.x() + 1.0; obj.set_x(__ts_new); __ts_new }
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\nfunction probe(): void {{ const c = new Counter(); ++c.value; }}"
    );
    let result = convert_update_in_probe(&src, 1, 1).expect("cell 43 prefix must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 43 prefix: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "cell 43 prefix block must have 3 stmts");
    // Stmt 0: let __ts_new = c.value() + 1.0;
    let init = match &stmts[0] {
        IrStmt::Let {
            name,
            init: Some(init),
            ..
        } if name == "__ts_new" => init,
        other => panic!("cell 43 prefix stmt 0: expected Let __ts_new, got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::BinaryOp {
            left: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("c".to_string())),
                method: "value".to_string(),
                args: vec![],
            }),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "cell 43 prefix stmt 0 init: must be BinaryOp c.value() + 1.0"
    );
    // Stmt 1: c.set_value(__ts_new)
    let setter_call = match &stmts[1] {
        IrStmt::Expr(call @ Expr::MethodCall { .. }) => call,
        other => panic!("cell 43 prefix stmt 1: expected ExprStmt MethodCall, got {other:?}"),
    };
    assert_eq!(
        setter_call,
        &Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::Ident("__ts_new".to_string())],
        },
        "cell 43 prefix stmt 1: must be MethodCall c.set_value(__ts_new)"
    );
    // Stmt 2: __ts_new (TailExpr)
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_new"),
        "cell 43 prefix stmt 2: must be TailExpr __ts_new"
    );
}

#[test]
fn test_cell_45c_b4_postfix_decrement_emits_setter_desugar_with_sub() {
    // Matrix cell 45-c: B4, D1 numeric, postfix `obj.x--`
    // Symmetric to cell 43 with BinOp::Sub
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\nfunction probe(): void {{ const c = new Counter(); c.value--; }}"
    );
    let result = convert_update_in_probe(&src, 1, 1).expect("cell 45-c must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 45-c: expected Expr::Block, got {other:?}"),
    };
    let setter_args = match &stmts[1] {
        IrStmt::Expr(Expr::MethodCall { args, .. }) => args,
        other => panic!("cell 45-c stmt 1: expected MethodCall, got {other:?}"),
    };
    assert_eq!(
        setter_args,
        &vec![Expr::BinaryOp {
            left: Box::new(Expr::Ident("__ts_old".to_string())),
            op: BinOp::Sub,
            right: Box::new(Expr::NumberLit(1.0)),
        }],
        "cell 45-c: setter arg must be __ts_old - 1.0"
    );
}

// =============================================================================
// Cell 44 + symmetric (B4 both、non-numeric) — Tier 2 honest error per Rule 3 (3-3)
// =============================================================================

/// B4 String holder fixture for cell 44 + 44-symmetric (non-numeric T)。
const B4_STR_HOLDER_CLASS_SRC: &str = "class StrHolder { _s: string = \"abc\"; \
                                        get s(): string { return this._s; } \
                                        set s(v: string) { this._s = v; } }";

#[test]
fn test_cell_44_b4_string_increment_emits_unsupported_syntax_error() {
    // Matrix cell 44: B4 (getter+setter pair) with non-numeric (String) return type, `++`
    // → Tier 2 honest "increment of non-numeric (String/etc.) — TS NaN coercion semantic"
    let src = format!(
        "{B4_STR_HOLDER_CLASS_SRC}\nfunction probe(): void {{ const h = new StrHolder(); h.s++; }}"
    );
    assert_in_probe_unsupported_syntax_error_kind(
        &src,
        1,
        1,
        "increment of non-numeric (String/etc.) — TS NaN coercion semantic",
    );
}

#[test]
fn test_cell_44_symmetric_b4_string_decrement_emits_unsupported_syntax_error() {
    // Symmetric to cell 44 for `--` (B4 String + decrement)
    let src = format!(
        "{B4_STR_HOLDER_CLASS_SRC}\nfunction probe(): void {{ const h = new StrHolder(); h.s--; }}"
    );
    assert_in_probe_unsupported_syntax_error_kind(
        &src,
        1,
        1,
        "decrement of non-numeric (String/etc.) — TS NaN coercion semantic",
    );
}

// =============================================================================
// Cell 45-b (B2 getter only) — Tier 2 honest "write to read-only property"
// =============================================================================

#[test]
fn test_cell_45b_b2_getter_only_decrement_emits_unsupported_syntax_error() {
    // Matrix cell 45-b: B2 (getter only), D1 numeric, `--`
    // → Tier 2 honest "write to read-only property"
    assert_in_probe_unsupported_syntax_error_kind(
        "class Foo { _v: number = 0; get x(): number { return this._v; } }\n\
         function probe(): void { const f = new Foo(); f.x--; }",
        1,
        1,
        "write to read-only property",
    );
}

// =============================================================================
// B3 setter only (matrix cell 化なし、Update-specific Tier 2)
// =============================================================================

#[test]
fn test_b3_setter_only_increment_emits_read_of_write_only_error() {
    // B3 (setter only) で `++` は read-then-write 必要、getter 不在で read fail
    // → Tier 2 honest "read of write-only property"
    assert_in_probe_unsupported_syntax_error_kind(
        "class Foo { _v: number = 0; set x(v: number) { this._v = v; } }\n\
         function probe(): void { const f = new Foo(); f.x++; }",
        1,
        1,
        "read of write-only property",
    );
}

// =============================================================================
// Cells 45-db, 45-dc (B6 method / B7 inherited) — Tier 2 honest error
// =============================================================================

#[test]
fn test_cell_45db_b6_method_decrement_emits_unsupported_syntax_error() {
    // Matrix cell 45-db: B6 (regular method) `--` → Tier 2 honest "write to method"
    assert_in_probe_unsupported_syntax_error_kind(
        "class Foo { x(): number { return 1; } }\n\
         function probe(): void { const f = new Foo(); f.x--; }",
        1,
        1,
        "write to method",
    );
}

#[test]
fn test_cell_45dc_b7_inherited_decrement_emits_unsupported_syntax_error() {
    // Matrix cell 45-dc: B7 (inherited setter via parent) `--`
    // → Tier 2 honest "write to inherited accessor"
    assert_in_probe_unsupported_syntax_error_kind(
        "class Base { _v: number = 0; \
         get x(): number { return this._v; } \
         set x(v: number) { this._v = v; } } \
         class Sub extends Base {}\n\
         function probe(): void { const s = new Sub(); s.x--; }",
        2,
        1,
        "write to inherited accessor",
    );
}

// =============================================================================
// Cell 45-dd (B8 static、numeric) — static setter desugar block
// =============================================================================

/// B8 static (getter+setter) class fixture for cell 45-dd, B8 ++ symmetric, and
/// `static_b2_*` defensive arm tests (subset / subset omitted depending on test)。
const B8_STATIC_COUNTER_CLASS_SRC: &str = "class Counter { static _n: number = 5; \
                                            static get value(): number { return Counter._n; } \
                                            static set value(v: number) { Counter._n = v; } }";

#[test]
fn test_cell_45dd_b8_static_postfix_decrement_emits_static_setter_desugar() {
    // Matrix cell 45-dd: B8 (static getter+setter), D1 numeric, postfix `Class.x--`
    // Ideal: { let __ts_old = Class::x(); Class::set_x(__ts_old - 1.0); __ts_old }
    let src =
        format!("{B8_STATIC_COUNTER_CLASS_SRC}\nfunction probe(): void {{ Counter.value--; }}");
    let result = convert_update_in_probe(&src, 1, 0).expect("cell 45-dd must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 45-dd: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "cell 45-dd block must have 3 stmts");
    // Stmt 0: let __ts_old = Counter::value();
    let init = match &stmts[0] {
        IrStmt::Let {
            name,
            init: Some(init),
            ..
        } if name == "__ts_old" => init,
        other => panic!("cell 45-dd stmt 0: expected Let __ts_old, got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new("Counter"),
                method: "value".to_string(),
            },
            args: vec![],
        },
        "cell 45-dd stmt 0 init: must be FnCall Counter::value()"
    );
    // Stmt 1: Counter::set_value(__ts_old - 1.0)
    let setter_call = match &stmts[1] {
        IrStmt::Expr(call @ Expr::FnCall { .. }) => call,
        other => panic!("cell 45-dd stmt 1: expected ExprStmt FnCall, got {other:?}"),
    };
    assert_eq!(
        setter_call,
        &Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new("Counter"),
                method: "set_value".to_string(),
            },
            args: vec![Expr::BinaryOp {
                left: Box::new(Expr::Ident("__ts_old".to_string())),
                op: BinOp::Sub,
                right: Box::new(Expr::NumberLit(1.0)),
            }],
        },
        "cell 45-dd stmt 1: must be FnCall Counter::set_value(__ts_old - 1.0)"
    );
}

// =============================================================================
// Cross-cutting: Computed (`obj[i]++`) → existing unsupported error path
// =============================================================================

#[test]
fn test_member_computed_update_falls_through_to_unsupported_error() {
    // Computed `obj[i]++` は `extract_non_computed_field_name(MemberProp::Computed) = None`
    // で `convert_update_expr_member_arm` の MemberProp shape gate で early return
    // (UnsupportedSyntaxError per Rule 11 (d-2))、`MemberReceiverClassification::Fallback`
    // path には進まない (= matrix scope 外、I-203 codebase-wide AST exhaustiveness で
    // 別 PRD 取り扱い)。
    assert_in_probe_unsupported_syntax_error_kind(
        "function probe(): void { \
         const arr: number[] = [1, 2, 3]; \
         arr[0]++; }",
        0,
        1,
        "unsupported update expression target",
    );
}

// =============================================================================
// Op-axis × postfix-axis cross-coverage (Iteration v11 review L3-1 fill)
// =============================================================================
//
// Iteration v11 `/check_job` Layer 3 で発見した op-axis (`++` vs `--`) × postfix-axis
// (postfix vs prefix) の test coverage gap を埋める。Rule 9 dispatch-arm sub-case
// alignment 観点で、両 op の dispatch path が symmetric に animate することを independent
// test で lock-in (= matrix の `++` 3 cells / `--` 8 cells asymmetric enumeration を
// implementation 側で ✓ symmetric coverage、Rule 1 (1-4) Orthogonality merge
// legitimacy の test-level verification)。

// -----------------------------------------------------------------------------
// B1 field decrement prefix (cell 45-a prefix form、postfix-axis cross-coverage)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_45a_b1_field_prefix_decrement_emits_fallback_block_with_new_value() {
    // B1 field, prefix `--obj.x` → `{ obj.x = obj.x - 1.0; obj.x }` (no let binding)
    let result = convert_update_in_probe(
        "class Foo { x: number = 0; }\n\
         function probe(): void { const f = new Foo(); --f.x; }",
        1,
        1,
    )
    .expect("cell 45-a prefix must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 45-a prefix: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 2, "cell 45-a prefix block must have 2 stmts");
    let assign_value = match &stmts[0] {
        IrStmt::Expr(Expr::Assign { value, .. }) => value,
        other => panic!("cell 45-a prefix stmt 0: expected ExprStmt Assign, got {other:?}"),
    };
    let field_access = Expr::FieldAccess {
        object: Box::new(Expr::Ident("f".to_string())),
        field: "x".to_string(),
    };
    assert_eq!(
        assign_value.as_ref(),
        &Expr::BinaryOp {
            left: Box::new(field_access.clone()),
            op: BinOp::Sub,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "cell 45-a prefix stmt 0 value: must be BinaryOp f.x - 1.0"
    );
    assert!(
        matches!(&stmts[1], IrStmt::TailExpr(fa) if fa == &field_access),
        "cell 45-a prefix stmt 1: must be TailExpr f.x"
    );
}

// -----------------------------------------------------------------------------
// B4 both decrement prefix (cell 45-c prefix form、postfix-axis cross-coverage)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_45c_b4_prefix_decrement_emits_setter_desugar_with_new_value() {
    // B4 prefix `--obj.x`:
    // { let __ts_new = obj.x() - 1.0; obj.set_x(__ts_new); __ts_new }
    let src = format!(
        "{B4_COUNTER_CLASS_SRC}\nfunction probe(): void {{ const c = new Counter(); --c.value; }}"
    );
    let result = convert_update_in_probe(&src, 1, 1).expect("cell 45-c prefix must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 45-c prefix: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "cell 45-c prefix block must have 3 stmts");
    let init = match &stmts[0] {
        IrStmt::Let {
            name,
            init: Some(init),
            ..
        } if name == "__ts_new" => init,
        other => panic!("cell 45-c prefix stmt 0: expected Let __ts_new, got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::BinaryOp {
            left: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("c".to_string())),
                method: "value".to_string(),
                args: vec![],
            }),
            op: BinOp::Sub,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "cell 45-c prefix stmt 0 init: must be BinaryOp c.value() - 1.0"
    );
    let setter_args = match &stmts[1] {
        IrStmt::Expr(Expr::MethodCall { args, .. }) => args,
        other => panic!("cell 45-c prefix stmt 1: expected ExprStmt MethodCall, got {other:?}"),
    };
    assert_eq!(
        setter_args,
        &vec![Expr::Ident("__ts_new".to_string())],
        "cell 45-c prefix stmt 1: setter arg must be __ts_new"
    );
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_new"),
        "cell 45-c prefix stmt 2: must be TailExpr __ts_new"
    );
}

// -----------------------------------------------------------------------------
// B2 getter only `++` (cell 45-b symmetric for ++、op-axis cross-coverage)
// -----------------------------------------------------------------------------

#[test]
fn test_b2_getter_only_increment_emits_unsupported_syntax_error() {
    // B2 (getter only) で `++` は write 必要、setter 不在 → "write to read-only property"
    // (cell 45-b symmetric for `++`、Rule 9 op-axis × B-axis cross-coverage)
    assert_in_probe_unsupported_syntax_error_kind(
        "class Foo { _v: number = 0; get x(): number { return this._v; } }\n\
         function probe(): void { const f = new Foo(); f.x++; }",
        1,
        1,
        "write to read-only property",
    );
}

// -----------------------------------------------------------------------------
// B6 method `++` (cell 45-db symmetric for ++、op-axis cross-coverage)
// -----------------------------------------------------------------------------

#[test]
fn test_b6_method_increment_emits_unsupported_syntax_error() {
    // B6 (regular method) `++` → "write to method"
    // (cell 45-db symmetric for `++`、op-axis cross-coverage)
    assert_in_probe_unsupported_syntax_error_kind(
        "class Foo { x(): number { return 1; } }\n\
         function probe(): void { const f = new Foo(); f.x++; }",
        1,
        1,
        "write to method",
    );
}

// -----------------------------------------------------------------------------
// B7 inherited `++` (cell 45-dc symmetric for ++、op-axis cross-coverage)
// -----------------------------------------------------------------------------

#[test]
fn test_b7_inherited_increment_emits_unsupported_syntax_error() {
    // B7 (inherited setter via parent) `++` → "write to inherited accessor"
    // (cell 45-dc symmetric for `++`、op-axis cross-coverage)
    assert_in_probe_unsupported_syntax_error_kind(
        "class Base { _v: number = 0; \
         get x(): number { return this._v; } \
         set x(v: number) { this._v = v; } } \
         class Sub extends Base {}\n\
         function probe(): void { const s = new Sub(); s.x++; }",
        2,
        1,
        "write to inherited accessor",
    );
}

// -----------------------------------------------------------------------------
// B8 static `++` (cell 45-dd symmetric for ++、op-axis cross-coverage)
// -----------------------------------------------------------------------------

#[test]
fn test_b8_static_postfix_increment_emits_static_setter_desugar_with_add() {
    // B8 (static getter+setter), D1 numeric, postfix `Class.x++`
    // Ideal: { let __ts_old = Class::x(); Class::set_x(__ts_old + 1.0); __ts_old }
    // (cell 45-dd symmetric for `++`、op-axis cross-coverage)
    let src =
        format!("{B8_STATIC_COUNTER_CLASS_SRC}\nfunction probe(): void {{ Counter.value++; }}");
    let result = convert_update_in_probe(&src, 1, 0).expect("B8 static ++ must succeed");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("B8 static ++: expected Expr::Block, got {other:?}"),
    };
    assert_eq!(stmts.len(), 3, "B8 static ++ block must have 3 stmts");
    let init = match &stmts[0] {
        IrStmt::Let {
            name,
            init: Some(init),
            ..
        } if name == "__ts_old" => init,
        other => panic!("B8 static ++ stmt 0: expected Let __ts_old, got {other:?}"),
    };
    assert_eq!(
        init,
        &Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new("Counter"),
                method: "value".to_string(),
            },
            args: vec![],
        },
        "B8 static ++ stmt 0 init: must be FnCall Counter::value()"
    );
    let setter_args = match &stmts[1] {
        IrStmt::Expr(Expr::FnCall { args, .. }) => args,
        other => panic!("B8 static ++ stmt 1: expected ExprStmt FnCall, got {other:?}"),
    };
    assert_eq!(
        setter_args,
        &vec![Expr::BinaryOp {
            left: Box::new(Expr::Ident("__ts_old".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        }],
        "B8 static ++: setter arg must be __ts_old + 1.0"
    );
}

// -----------------------------------------------------------------------------
// B9 unknown `++` (cell 45-de symmetric for ++、op-axis cross-coverage)
// -----------------------------------------------------------------------------

#[test]
fn test_b9_unknown_postfix_increment_emits_fallback_block_with_add() {
    // B9 unknown receiver、postfix `obj.x++` → fallback block (regression Tier 2 → Tier 1)
    // (cell 45-de symmetric for `++`、op-axis cross-coverage)
    let result = convert_update_in_probe(
        "function probe(): void { const f = { x: 5 }; f.x++; }",
        0,
        1,
    )
    .expect("B9 unknown ++ must succeed (fallback)");
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("B9 unknown ++: expected Expr::Block, got {other:?}"),
    };
    let assign_value = match &stmts[1] {
        IrStmt::Expr(Expr::Assign { value, .. }) => value,
        other => panic!("B9 unknown ++ stmt 1: expected ExprStmt Assign, got {other:?}"),
    };
    assert_eq!(
        assign_value.as_ref(),
        &Expr::BinaryOp {
            left: Box::new(Expr::Ident("__ts_old".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        },
        "B9 unknown ++ stmt 1 value: must be BinaryOp __ts_old + 1.0"
    );
}

// =============================================================================
// `_ => ` arm direct C1 branch coverage (Iteration v11 deep review D3 fill)
// =============================================================================
//
// `convert_update_expr` Ident form の `_ =>` arm (= 非-Ident 非-Member arg) の direct
// branch coverage。pre-fix では `test_convert_expr_update_non_ident_target_errors`
// (`arr[0]++`) が `_ =>` arm を踏むと誤認識されていたが、`arr[0]` は実は
// `Member { prop: Computed }` で T7 implementation で Member arm に enter、Computed
// gate (`extract_non_computed_field_name = None`) で early return する path を踏む。
// 真の `_ =>` arm trigger には Paren / This / Call 等の non-Member non-Ident arg が
// 必要 (Rule 11 (d-2) Transformer phase mechanism = `UnsupportedSyntaxError` で reject)。

#[test]
fn test_convert_update_expr_paren_wrapped_arg_emits_unsupported_syntax_error() {
    // `(x)++` arg は `Paren(Ident)` で AST 上 SWC が Paren wrap を保持、
    // `up.arg.as_ref() = Paren` で Member でも Ident でもなく `_ =>` arm に直撃。
    // Rule 11 (d-2) per UnsupportedSyntaxError で span 付き user-facing error report。
    assert_expr_unsupported_syntax_error_kind("(x)++", "unsupported update expression target");
}

#[test]
fn test_convert_update_expr_this_arg_emits_unsupported_syntax_error() {
    // `this++` arg は `This` で `_ =>` arm に直撃 (TS で type error 候補だが SWC parser
    // accept、Transformer phase で UnsupportedSyntaxError reject)。
    assert_expr_unsupported_syntax_error_kind("this++", "unsupported update expression target");
}

// =============================================================================
// Static dispatch defensive arms C1 branch coverage (Iteration v11 deep review D4 fill)
// =============================================================================
//
// `dispatch_static_member_update` の defensive arm (matrix cell 化なしだが code 上存在、
// T6 dispatch_static_member_write と symmetric な structural enforcement pattern) の
// branch coverage。Spec → Impl Mapping の "Static + has_getter only / has_setter only /
// has_method only / is_inherited" arms、subsequent T11 (11-c) で matrix expansion 予定だが
// 本 T7 で defensive Tier 2 honest error reclassify として実装済 → branch test で lock-in。
// (T6 が Iteration v10 second-review で同 pattern arm を 3 件 test 追加した integration と整合)。

#[test]
fn test_static_b2_getter_only_update_emits_read_only_static_error() {
    // Static B2 (= static getter only、static setter 不在) で `++/--` → read-then-write 必要、
    // setter 不在で write fail → "write to read-only static property" Tier 2 honest error。
    // matrix cell 化なし (subsequent T11 (11-c) で expansion 予定)、本 T7 で defensive 実装。
    assert_in_probe_unsupported_syntax_error_kind(
        "class Counter { static _n: number = 5; \
         static get value(): number { return Counter._n; } }\n\
         function probe(): void { Counter.value++; }",
        1,
        0,
        "write to read-only static property",
    );
}

#[test]
fn test_static_b3_setter_only_update_emits_write_only_static_error() {
    // Static B3 (= static setter only、static getter 不在) で `++/--` → read-then-write 必要、
    // getter 不在で read fail → "read of write-only static property" Tier 2 honest error。
    // matrix cell 化なし (subsequent T11 (11-c) で expansion 予定)。
    assert_in_probe_unsupported_syntax_error_kind(
        "class Counter { static _n: number = 5; \
         static set value(v: number) { Counter._n = v; } }\n\
         function probe(): void { Counter.value++; }",
        1,
        0,
        "read of write-only static property",
    );
}

#[test]
fn test_static_b6_method_update_emits_write_to_static_method_error() {
    // Static B6 (= static method、accessor 不在) で `++/--` → "write to static method"
    // Tier 2 honest error。matrix cell 化なし (subsequent T11 (11-c))。
    assert_in_probe_unsupported_syntax_error_kind(
        "class Counter { static value(): number { return 1; } }\n\
         function probe(): void { Counter.value++; }",
        1,
        0,
        "write to static method",
    );
}

// Note: Static B7 (inherited static accessor via parent class) は TS の static member が
// prototype chain inheritance を持つが、Rust associated fn は構造的に inherited dispatch を
// 持たない (= 別 PRD I-206 Class inheritance dispatch scope)。`dispatch_static_member_update`
// の `is_inherited = true` arm (= "write to inherited static accessor") は本 PRD scope で
// defensive 実装済だが、test の registration setup が complex (parent class static accessor +
// child class lookup chain) で T6 でも同 arm の test は追加されていない (Iteration v10 second-
// review C1 補完では Read 3 + Write 3 = 6 件追加で inherited static は除外)。本 T7 でも T6
// pattern と整合させ、static B7 inherited は code-level defensive のみで test は subsequent
// T11 (11-c) matrix expansion で Static × {B7 inherited} cell 明示 enumerate 時に追加。

// Note: T8 INV-3 back-port verification (= UpdateExpr Member target with side-effect-having
// receiver emits IIFE form for `dispatch_instance_member_update` cohesive helper update with
// `dispatch_instance_member_compound`) は本 file 単位の cohesive concern boundary を維持
// するため、本 test の存在を `tests/i_205/compound.rs` の architectural concern (= T8
// arithmetic/bitwise compound assign + INV-3 1-evaluate compliance) に co-locate
// (`test_t8_inv3_backport_update_postfix_increment_side_effect_having_receiver_emits_iife_form`)。
// Update Ident form (cells 42-45 + 45-b3) の test は本 file に集中、INV-3 cross-cutting
// invariant の verification は T8 architectural concern と論理的 cohesive のため移動済。
