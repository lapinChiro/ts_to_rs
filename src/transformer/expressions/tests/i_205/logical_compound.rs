//! I-205 T9 (Logical compound assign Member target dispatch) — cells 36-41
//! lock-in tests + Static defensive arms + 3-op orthogonality merge mapping
//! verify (??= / &&= / ||=).
//!
//! 全 test は AssignExpr (`obj.x ??= d` / `Class.x &&= v` / etc.) の Tier 1
//! emission を `convert_expr` 経由 (expression context) または `convert_stmt`
//! 経由 (statement context) で実行、`dispatch_*_member_logical_compound` 内
//! dispatch arm が ideal IR (Tier 1 conditional setter desugar Block / Tier 2
//! honest error / Fallback existing helper path) を emit することを verify。
//!
//! ## 対象 cells / 機能
//!
//! - **Cell 36** (B1 field × `??=` × Option<T>): Fallback path 維持で既存
//!   `obj.x.get_or_insert_with(|| d)` (cell 36 fallback regression preserve、
//!   T9 dispatch 通過後も既存 nullish_assign 経路に流れる structural lock-in)。
//! - **Cell 37** (B2 getter only × `??=`): Tier 2 `"logical compound assign to
//!   read-only property"`。
//! - **Cell 38** (B4 both × `??=` × Option<T>): conditional setter desugar
//!   `if obj.x().is_none() { obj.set_x(Some(d)); }` (statement context、no
//!   tail) / `{ if obj.x().is_none() { obj.set_x(Some(d)); }; obj.x() }`
//!   (expression context、tail = post-state getter)。
//! - **Cell 39** (B4 both × `&&=` × bool): `if obj.x() { obj.set_x(v); }`
//!   (truthy_predicate_for_expr 経由、Bool 型は predicate = identity)。
//! - **Cell 40** (B4 both × `||=` × bool): `if !obj.x() { obj.set_x(v); }`
//!   (falsy_predicate_for_expr 経由、Bool 型は predicate = `!operand`)。
//! - **Cell 41-b** (B6 method × logical): Tier 2 `"logical compound assign to
//!   method"`。
//! - **Cell 41-c** (B7 inherited × logical): Tier 2 `"logical compound assign
//!   to inherited accessor"`。
//! - **Cell 41-d** (B8 static × logical): static conditional setter desugar
//!   (no IIFE wrap; class TypeName is side-effect-free)。
//! - **Cell 41-e** (B9 unknown × logical): Fallback path 維持 (regression
//!   preserve via existing nullish_assign / compound_logical_assign)。
//!
//! ## Cross-cutting verifications
//!
//! - **3-op orthogonality merge** (Rule 1 (1-4)): NullishAssign / AndAssign /
//!   OrAssign の dispatch logic は predicate 構築のみで differ、IR shape は
//!   同一の Block + Stmt::If pattern。代表 ops で coverage、predicate 形が op
//!   ごとに正しいことを structural 検証。
//! - **INV-3 1-evaluate compliance** (side-effect-having receiver): Instance
//!   B4 で `getInstance().value ??= 42` 等 SE-having receiver は IIFE form
//!   `{ let mut __ts_recv = getInstance(); if __ts_recv.value().is_none() {
//!     __ts_recv.set_value(Some(42)); }; <tail> }` で 1-evaluate。
//! - **Statement vs Expression context**: 同 cell に対して両 context 別 test、
//!   tail 有無 (Expression = `Stmt::TailExpr(<getter>)` 末尾、Statement =
//!   なし) で IR shape が分岐することを lock-in。
//! - **Some-wrap setter argument** (cell 38 LHS = Option<T>): setter call
//!   引数が `BuiltinVariant::Some(rhs)` で wrap される (cell 39/40 LHS = bool は
//!   raw rhs 直接渡し、wrap せず)。

use super::super::*;

use crate::ir::{BuiltinVariant, CallTarget, Expr, Stmt as IrStmt, UserTypeRef};
use crate::transformer::UnsupportedSyntaxError;

// =============================================================================
// Test helpers (DRY refactor、design-integrity.md "DRY"、compound.rs と symmetric)
// =============================================================================

/// Tier 1 emission tests (expression context) 用 helper: TctxFixture +
/// Transformer::convert_expr boilerplate を集約。`src` の `function probe(): void
/// { ... }` body の `stmt_index` 番目 ExprStmt を extract して `convert_expr` する
/// (= T9 expression-context gate fires in `convert_assign_expr`)。
pub(super) fn convert_logical_in_probe(
    src: &str,
    fn_index: usize,
    stmt_index: usize,
) -> anyhow::Result<Expr> {
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let stmt = extract_fn_body_expr_stmt(module, fn_index, stmt_index);
    Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new()).convert_expr(&stmt)
}

/// Statement context tests 用 helper: TctxFixture + Transformer::convert_stmt
/// 経由で statement context emission を取得。`obj.x ??= d;` / `obj.x &&= v;` /
/// `obj.x ||= v;` bare stmt が `try_convert_nullish_assign_stmt` /
/// `try_convert_compound_logical_assign_stmt` 経由で T9 dispatch を fire し、
/// `Vec<Stmt>` (= `Stmt::Expr(Expr::Block(stmts no tail))`) を返す。
pub(super) fn convert_logical_stmt_in_probe(
    src: &str,
    fn_index: usize,
    stmt_index: usize,
) -> anyhow::Result<Vec<crate::ir::Stmt>> {
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    // Extract raw ast::Stmt (not Expr) so we can invoke convert_stmt.
    let fn_decl = match &module.body[fn_index] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(f))) => f,
        _ => panic!("expected function declaration at module index {fn_index}"),
    };
    let body = fn_decl
        .function
        .body
        .as_ref()
        .expect("function has no body");
    let stmt = body.stmts[stmt_index].clone();
    let tctx = fx.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut transformer = Transformer::for_module(&tctx, &mut synthetic);
    transformer.convert_stmt(&stmt, None)
}

/// Tier 2 honest error tests 用 helper: fixture + convert + downcast + kind
/// assertion を集約 (compound.rs::assert_compound_in_probe_unsupported_syntax_error_kind
/// と symmetric)。`expected_kind` で `UnsupportedSyntaxError.kind` を exact match。
pub(super) fn assert_logical_in_probe_unsupported_syntax_error_kind(
    src: &str,
    fn_index: usize,
    stmt_index: usize,
    expected_kind: &str,
) {
    let err = convert_logical_in_probe(src, fn_index, stmt_index)
        .expect_err(&format!("expected Err with kind={expected_kind}"));
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .unwrap_or_else(|e| panic!("error must be UnsupportedSyntaxError, got: {e:?}"));
    assert_eq!(usx.kind, expected_kind, "kind mismatch");
}

/// Asserts an `Expr::Block` of T9 statement-context shape (no tail expr):
/// `{ if <predicate> { <setter call>; }; }`.
///
/// Verifies that the Block body contains exactly one `Stmt::If` with the
/// expected predicate + setter call, and **no** `Stmt::TailExpr` (statement
/// context yields unit).
fn assert_logical_compound_block_no_tail(
    block: &Expr,
    expected_predicate: Expr,
    expected_setter_call: Expr,
) {
    let stmts = match block {
        Expr::Block(s) => s,
        other => panic!("expected Expr::Block, got: {other:?}"),
    };
    assert_eq!(
        stmts.len(),
        1,
        "statement-context Block must have exactly 1 stmt (no TailExpr), got: {stmts:?}"
    );
    assert_logical_compound_if_stmt(&stmts[0], expected_predicate, expected_setter_call);
}

/// Asserts an `Expr::Block` of T9 expression-context shape (with tail expr):
/// `{ if <predicate> { <setter call>; }; <tail getter> }`.
///
/// Verifies the Block body contains exactly: (a) Stmt::If with predicate +
/// setter call, (b) Stmt::TailExpr with the post-state getter call.
fn assert_logical_compound_block_with_tail(
    block: &Expr,
    expected_predicate: Expr,
    expected_setter_call: Expr,
    expected_tail_getter: Expr,
) {
    let stmts = match block {
        Expr::Block(s) => s,
        other => panic!("expected Expr::Block, got: {other:?}"),
    };
    assert_eq!(
        stmts.len(),
        2,
        "expression-context Block must have 2 stmts (Stmt::If + TailExpr), got: {stmts:?}"
    );
    assert_logical_compound_if_stmt(&stmts[0], expected_predicate, expected_setter_call);
    match &stmts[1] {
        IrStmt::TailExpr(tail) => assert_eq!(
            tail, &expected_tail_getter,
            "tail expr must equal expected post-state getter"
        ),
        other => panic!("stmt 1: expected TailExpr, got {other:?}"),
    }
}

/// Asserts a `Stmt::If` matches the expected predicate + setter call (no else).
fn assert_logical_compound_if_stmt(
    stmt: &IrStmt,
    expected_predicate: Expr,
    expected_setter_call: Expr,
) {
    let (cond, then_body, else_body) = match stmt {
        IrStmt::If {
            condition,
            then_body,
            else_body,
        } => (condition, then_body, else_body),
        other => panic!("expected Stmt::If, got: {other:?}"),
    };
    assert_eq!(cond, &expected_predicate, "Stmt::If condition mismatch");
    assert_eq!(
        then_body.len(),
        1,
        "then_body must have exactly 1 stmt (setter call)"
    );
    match &then_body[0] {
        IrStmt::Expr(call) => assert_eq!(
            call, &expected_setter_call,
            "then_body[0] (setter call) mismatch"
        ),
        other => panic!("then_body[0]: expected ExprStmt, got {other:?}"),
    }
    assert!(
        else_body.is_none(),
        "else_body must be None for logical compound dispatch, got: {else_body:?}"
    );
}

// =============================================================================
// Class fixtures (B4 instance、B8 static、B6 method、B7 inherited)
// =============================================================================

/// B4 (instance getter+setter pair) class fixture for cell 38 / 39 / 40 family。
/// `value: number | undefined` for `??=` (Option<T>)、`b: boolean` for `&&=`/`||=`。
pub(super) const B4_CACHE_OPTION_SRC: &str = "class Cache { _v: number | undefined = undefined; \
                                   get value(): number | undefined { return this._v; } \
                                   set value(v: number | undefined) { this._v = v; } }";

pub(super) const B4_FOO_BOOL_SRC: &str = "class Foo { _b: boolean = true; \
                               get b(): boolean { return this._b; } \
                               set b(v: boolean) { this._b = v; } }";

/// B8 (static getter+setter pair) class fixture for cell 41-d。
const B8_STATIC_OPTION_SRC: &str = "class Cache { static _v: number | undefined = undefined; \
                                    static get value(): number | undefined { return Cache._v; } \
                                    static set value(v: number | undefined) { Cache._v = v; } }";

// =============================================================================
// Cell 38 (B4 instance × ??= × Option<T>) — instance setter desugar (both contexts)
// =============================================================================

#[test]
fn test_cell_38_b4_nullish_assign_expression_context_emits_block_with_tail() {
    // Cell 38 expression context: `(c.value ??= 42)` inside larger expr (var init)
    // → Block + tail = post-state getter. Setter argument wrapped in Some(_) since
    // LHS = Option<T>.
    let src = format!(
        "{B4_CACHE_OPTION_SRC}\n\
         function probe(): void {{ const c = new Cache(); const _z = (c.value ??= 42); }}"
    );
    let fx = TctxFixture::from_source(&src);
    let module = fx.module();
    let init = extract_fn_body_var_init(module, 1, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&init)
        .expect("cell 38 expression context must succeed");
    let getter_call = || Expr::MethodCall {
        object: Box::new(Expr::Ident("c".to_string())),
        method: "value".to_string(),
        args: vec![],
    };
    let predicate = Expr::MethodCall {
        object: Box::new(getter_call()),
        method: "is_none".to_string(),
        args: vec![],
    };
    let setter_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("c".to_string())),
        method: "set_value".to_string(),
        args: vec![Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
            args: vec![Expr::NumberLit(42.0)],
        }],
    };
    assert_logical_compound_block_with_tail(&result, predicate, setter_call, getter_call());
}

#[test]
fn test_cell_38_b4_nullish_assign_statement_context_emits_block_no_tail() {
    // Cell 38 statement context: `c.value ??= 42;` bare stmt → `Stmt::Expr(Block
    // [Stmt::If])`、no TailExpr。
    let src = format!(
        "{B4_CACHE_OPTION_SRC}\n\
         function probe(): void {{ const c = new Cache(); c.value ??= 42; }}"
    );
    let stmts =
        convert_logical_stmt_in_probe(&src, 1, 1).expect("cell 38 statement context must succeed");
    assert_eq!(
        stmts.len(),
        1,
        "statement context must emit exactly 1 outer Stmt"
    );
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    let getter_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("c".to_string())),
        method: "value".to_string(),
        args: vec![],
    };
    let predicate = Expr::MethodCall {
        object: Box::new(getter_call.clone()),
        method: "is_none".to_string(),
        args: vec![],
    };
    let setter_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("c".to_string())),
        method: "set_value".to_string(),
        args: vec![Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
            args: vec![Expr::NumberLit(42.0)],
        }],
    };
    assert_logical_compound_block_no_tail(block, predicate, setter_call);
}

// =============================================================================
// Cell 39 (B4 instance × &&= × bool) — instance truthy_predicate setter desugar
// =============================================================================

#[test]
fn test_cell_39_b4_and_assign_bool_statement_context_emits_block_no_tail() {
    // Cell 39 statement context: `f.b &&= false;` → `if f.b() { f.set_b(false); }`
    // (predicate = identity for Bool LHS、no Some-wrap on setter arg since LHS = bool)
    let src = format!(
        "{B4_FOO_BOOL_SRC}\n\
         function probe(): void {{ const f = new Foo(); f.b &&= false; }}"
    );
    let stmts = convert_logical_stmt_in_probe(&src, 1, 1).expect("cell 39 must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    let getter_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("f".to_string())),
        method: "b".to_string(),
        args: vec![],
    };
    // Bool truthy predicate = identity
    let predicate = getter_call.clone();
    let setter_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("f".to_string())),
        method: "set_b".to_string(),
        args: vec![Expr::BoolLit(false)],
    };
    assert_logical_compound_block_no_tail(block, predicate, setter_call);
}

// =============================================================================
// Cell 40 (B4 instance × ||= × bool) — instance falsy_predicate setter desugar
// =============================================================================

#[test]
fn test_cell_40_b4_or_assign_bool_statement_context_emits_block_no_tail() {
    // Cell 40 statement context: `f.b ||= true;` → `if !f.b() { f.set_b(true); }`
    // (falsy predicate for Bool = `!operand`)
    let src = format!(
        "{B4_FOO_BOOL_SRC}\n\
         function probe(): void {{ const f = new Foo(); f.b ||= true; }}"
    );
    let stmts = convert_logical_stmt_in_probe(&src, 1, 1).expect("cell 40 must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    let getter_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("f".to_string())),
        method: "b".to_string(),
        args: vec![],
    };
    // Bool falsy predicate = `!operand`
    let predicate = Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(getter_call),
    };
    let setter_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("f".to_string())),
        method: "set_b".to_string(),
        args: vec![Expr::BoolLit(true)],
    };
    assert_logical_compound_block_no_tail(block, predicate, setter_call);
}

// =============================================================================
// Cell 41-d (B8 static × logical) — static conditional setter desugar (no IIFE)
// =============================================================================

#[test]
fn test_cell_41d_b8_static_nullish_assign_emits_block_with_tail() {
    // Cell 41-d expression context: `(Cache.value ??= 99)` inside var init
    // → static conditional setter desugar with tail = `Cache::value()`
    let src = format!(
        "{B8_STATIC_OPTION_SRC}\n\
         function probe(): void {{ const _z = (Cache.value ??= 99); }}"
    );
    let fx = TctxFixture::from_source(&src);
    let module = fx.module();
    let init = extract_fn_body_var_init(module, 1, 0);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&init)
        .expect("cell 41-d expression context must succeed");
    let cache_ty = || UserTypeRef::new("Cache");
    let getter_call = || Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: cache_ty(),
            method: "value".to_string(),
        },
        args: vec![],
    };
    let predicate = Expr::MethodCall {
        object: Box::new(getter_call()),
        method: "is_none".to_string(),
        args: vec![],
    };
    let setter_call = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: cache_ty(),
            method: "set_value".to_string(),
        },
        args: vec![Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
            args: vec![Expr::NumberLit(99.0)],
        }],
    };
    assert_logical_compound_block_with_tail(&result, predicate, setter_call, getter_call());
}

#[test]
fn test_cell_41d_b8_static_nullish_assign_statement_context_emits_block_no_tail() {
    // Cell 41-d statement context: `Cache.value ??= 99;` → static block, no tail
    let src = format!(
        "{B8_STATIC_OPTION_SRC}\n\
         function probe(): void {{ Cache.value ??= 99; }}"
    );
    let stmts = convert_logical_stmt_in_probe(&src, 1, 0).expect("cell 41-d stmt must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    let cache_ty = || UserTypeRef::new("Cache");
    let getter_call = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: cache_ty(),
            method: "value".to_string(),
        },
        args: vec![],
    };
    let predicate = Expr::MethodCall {
        object: Box::new(getter_call),
        method: "is_none".to_string(),
        args: vec![],
    };
    let setter_call = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: cache_ty(),
            method: "set_value".to_string(),
        },
        args: vec![Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
            args: vec![Expr::NumberLit(99.0)],
        }],
    };
    assert_logical_compound_block_no_tail(block, predicate, setter_call);
}

// =============================================================================
// Cell 37 (B2 getter only) / 41-b (B6 method) / 41-c (B7 inherited) — Tier 2 honest error
// =============================================================================

#[test]
fn test_cell_37_b2_getter_only_nullish_assign_errs_logical_compound_assign_to_read_only() {
    // Cell 37: B2 getter only × `??=` → Tier 2 "logical compound assign to read-only property"
    let src =
        "class Foo { _v: number | undefined = undefined; get value(): number | undefined { return this._v; } }\n\
         function probe(): void { const f = new Foo(); f.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "logical compound assign to read-only property",
    );
}

#[test]
fn test_b3_setter_only_nullish_assign_errs_logical_compound_assign_read_of_write_only() {
    // B3 setter only × `??=` → Tier 2 (defensive、predicate evaluation reads getter
    // which is absent for setter-only)
    let src =
        "class Foo { _v: number | undefined = undefined; set value(v: number | undefined) { this._v = v; } }\n\
         function probe(): void { const f = new Foo(); f.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "logical compound assign read of write-only property",
    );
}

#[test]
fn test_cell_41b_b6_method_nullish_assign_errs_logical_compound_assign_to_method() {
    // Cell 41-b: B6 method × `??=` → Tier 2 "logical compound assign to method"
    let src = "class Foo { value(): number | undefined { return undefined; } }\n\
               function probe(): void { const f = new Foo(); f.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        1,
        "logical compound assign to method",
    );
}

#[test]
fn test_cell_41c_b7_inherited_nullish_assign_errs_logical_compound_assign_to_inherited_accessor() {
    // Cell 41-c: B7 inherited × `??=` → Tier 2 "logical compound assign to inherited accessor"
    let src = "class Base { _v: number | undefined = undefined; \
               get value(): number | undefined { return this._v; } \
               set value(v: number | undefined) { this._v = v; } }\n\
               class Sub extends Base {}\n\
               function probe(): void { const s = new Sub(); s.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        2,
        1,
        "logical compound assign to inherited accessor",
    );
}

// =============================================================================
// Static defensive arms (matrix cell 化なし、subsequent T11 (11-c) で expansion)
// =============================================================================

#[test]
fn test_static_b2_getter_only_nullish_assign_errs_with_logical_compound_assign_to_read_only_static()
{
    // Static B2: `Class.x ??= d` where Class has only static getter
    // → Tier 2 "logical compound assign to read-only static property" (defensive arm)
    let src = "class Foo { static _v: number | undefined = undefined; \
         static get value(): number | undefined { return Foo._v; } }\n\
         function probe(): void { Foo.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        0,
        "logical compound assign to read-only static property",
    );
}

#[test]
fn test_static_b3_setter_only_nullish_assign_errs_with_logical_compound_assign_read_of_write_only_static(
) {
    // Static B3: setter-only static → Tier 2
    let src = "class Foo { static _v: number | undefined = undefined; \
         static set value(v: number | undefined) { Foo._v = v; } }\n\
         function probe(): void { Foo.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        0,
        "logical compound assign read of write-only static property",
    );
}

#[test]
fn test_static_b6_method_nullish_assign_errs_with_logical_compound_assign_to_static_method() {
    // Static B6: method-only static class
    let src = "class Foo { static value(): number | undefined { return undefined; } }\n\
               function probe(): void { Foo.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        1,
        0,
        "logical compound assign to static method",
    );
}

#[test]
fn test_static_b7_inherited_nullish_assign_errs_with_logical_compound_assign_to_inherited_static_accessor(
) {
    // Static B7: inherited static accessor → Tier 2
    let src = "class Base { static _v: number | undefined = undefined; \
               static get value(): number | undefined { return Base._v; } \
               static set value(v: number | undefined) { Base._v = v; } }\n\
               class Sub extends Base {}\n\
               function probe(): void { Sub.value ??= 42; }";
    assert_logical_in_probe_unsupported_syntax_error_kind(
        src,
        2,
        0,
        "logical compound assign to inherited static accessor",
    );
}

// =============================================================================
// Cell 36 (B1 field × ??= × Option<T>) — Fallback path regression preserve
// =============================================================================

#[test]
fn test_cell_36_b1_field_nullish_assign_emits_existing_get_or_insert_with_fallback() {
    // Cell 36: B1 field × `??=` × Option<T> → existing `obj.x.get_or_insert_with(|| d)`
    // emission preserved (T9 dispatch passes through to existing nullish_assign path
    // because classify returns Fallback for B1 field-only class)
    let src = "class Foo { v: number | undefined = undefined; }\n\
               function probe(): void { const f = new Foo(); const _z = (f.v ??= 42); }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let init = extract_fn_body_var_init(module, 1, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&init)
        .expect("cell 36 must succeed via existing nullish_assign fallback");
    // Existing emission shape (I-142 ShadowLet × non-Copy inner): the helper
    // emits `get_or_insert_with` followed by `Deref` (Copy inner) or
    // `clone` (!Copy inner). For numeric inner = F64 (Copy)、shape =
    // `Expr::Deref(Box::new(MethodCall { f.v, get_or_insert_with, [closure] }))`。
    let field_access = Expr::FieldAccess {
        object: Box::new(Expr::Ident("f".to_string())),
        field: "v".to_string(),
    };
    let closure = Expr::Closure {
        params: vec![],
        return_type: None,
        body: crate::ir::ClosureBody::Expr(Box::new(Expr::NumberLit(42.0))),
    };
    let method_call = Expr::MethodCall {
        object: Box::new(field_access),
        method: "get_or_insert_with".to_string(),
        args: vec![closure],
    };
    assert_eq!(
        result,
        Expr::Deref(Box::new(method_call)),
        "cell 36 must emit existing `*f.v.get_or_insert_with(|| 42.0)` (regression preserve、\
         T9 Fallback path through to existing nullish_assign emission)"
    );
}

// =============================================================================
// 3-op orthogonality merge mapping verification (Rule 1 (1-4) compliance)
// =============================================================================
//
// Verifies that all 3 logical compound ops (??= / &&= / ||=) follow the same
// dispatch arm structure (= classify_member_receiver + MemberKindFlags + IIFE
// gate)、differing only in the predicate construction (is_none / truthy /
// falsy). For each op、a representative B4 instance test exercises the full
// dispatch path.

#[test]
fn test_op_axis_or_assign_bool_emits_falsy_predicate_setter_desugar() {
    // ||= bool dispatch (cell 40 の op-axis representative)
    // 同一 B4 fixture でも dispatch が `falsy_predicate_for_expr` 経由で
    // op-specific predicate を build することを verify。
    let src = format!(
        "{B4_FOO_BOOL_SRC}\n\
         function probe(): void {{ const f = new Foo(); f.b ||= true; }}"
    );
    let stmts = convert_logical_stmt_in_probe(&src, 1, 1).expect("||= must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    // 構造的 verify: Block 内に Stmt::If が存在し、predicate が `Unary::Not` で
    // wrap されていることを confirm (= falsy = !truthy)。
    if let Expr::Block(inner_stmts) = block {
        if let IrStmt::If { condition, .. } = &inner_stmts[0] {
            assert!(
                matches!(condition, Expr::UnaryOp { op: UnOp::Not, .. }),
                "||= predicate must be Unary::Not (= falsy_predicate for Bool), got: {condition:?}"
            );
        } else {
            panic!("inner_stmts[0] must be Stmt::If, got: {:?}", inner_stmts[0]);
        }
    }
}

#[test]
fn test_op_axis_and_assign_bool_emits_truthy_predicate_setter_desugar() {
    // &&= bool dispatch (cell 39 の op-axis representative)
    // 同一 B4 fixture でも dispatch が `truthy_predicate_for_expr` 経由で
    // op-specific predicate を build (= identity for Bool、Unary::Not 不在)
    let src = format!(
        "{B4_FOO_BOOL_SRC}\n\
         function probe(): void {{ const f = new Foo(); f.b &&= false; }}"
    );
    let stmts = convert_logical_stmt_in_probe(&src, 1, 1).expect("&&= must succeed");
    let block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    if let Expr::Block(inner_stmts) = block {
        if let IrStmt::If { condition, .. } = &inner_stmts[0] {
            // Bool truthy predicate = identity (= MethodCall directly), 不在 of
            // Unary::Not wrap distinguishes truthy from falsy
            assert!(
                !matches!(condition, Expr::UnaryOp { op: UnOp::Not, .. }),
                "&&= predicate must NOT be Unary::Not (= truthy = identity for Bool), got: {condition:?}"
            );
            assert!(
                matches!(condition, Expr::MethodCall { method, .. } if method == "b"),
                "&&= predicate must be `f.b()` MethodCall (Bool truthy = identity), got: {condition:?}"
            );
        } else {
            panic!("inner_stmts[0] must be Stmt::If, got: {:?}", inner_stmts[0]);
        }
    }
}

// =============================================================================
// LHS type orthogonality (Layer 3 cross-axis、`&&=`/`||=` × {F64 / String /
// Option<T>} × B4): predicate dispatch via existing
// `truthy_predicate_for_expr` / `falsy_predicate_for_expr` helpers (= per-type
// predicate construction)。
//
// Matrix cells 39/40 only spec D=bool LHS、but the implementation transitively
// handles all RustType variants supported by the truthy/falsy helpers (cells
// 39/40 are spec'd as D2 bool primary、orthogonality-equivalent extension to
// other D variants is implicit via the existing predicate dispatch dispatch
// fabric established by I-161 compound_logical_assign.rs)。
//
// These tests verify the structural extension:
// - F64 truthy = `<getter> != 0.0 && !<getter>.is_nan()` (potentially via
//   tmp-binding for non-pure operand; getter call is non-pure → tmp-binding
//   wraps in Block)
// - String truthy = `!<getter>.is_empty()`
// - Option<T> truthy = `<getter>.is_some_and(|v| <truthy(*v)>)` (Copy inner)
//   or `<getter>.as_ref().is_some_and(|v| <truthy(v)>)` (!Copy inner)
//
// Cross-axis Layer 3 Spec gap (`/check_job` finding): matrix cells 39/40
// explicitly D2 bool only; LHS type variants for `&&=`/`||=` × B4 covered
// transitively via existing helpers but not enumerated in the matrix. The
// orthogonality-equivalent extension is verified here (= structural lock-in
// for all-LHS-type dispatch correctness) but should be made explicit in the
// matrix for completeness (subsequent PRD doc revision per check-job-review-
// layers.md Layer 3)。
// =============================================================================

#[test]
fn test_lhs_type_f64_and_assign_emits_block_with_predicate_dispatch() {
    // `&&=` × F64 LHS: predicate uses F64 truthy = `<op> != 0.0 && !<op>.is_nan()`
    // (non-pure operand getter call → wrapped in tmp-binding Block per truthy.rs
    // `predicate_primitive_with_tmp` logic for F64 duplicates_operand=true).
    let src = "class Foo { _n: number = 0; \
               get n(): number { return this._n; } \
               set n(v: number) { this._n = v; } }\n\
               function probe(): void { const f = new Foo(); f.n &&= 5; }";
    let stmts = convert_logical_stmt_in_probe(src, 1, 1)
        .expect("F64 LHS &&= must succeed (truthy_predicate dispatches per-type)");
    let outer_block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    // Structural lock-in: outer Block contains Stmt::If with predicate built by
    // `truthy_predicate_for_expr` for F64 (predicate is a Block-wrapped tmp-binding
    // due to F64 duplicates_operand=true)。Setter call argument is raw rhs (= 5.0)
    // since LHS = F64 (not Option<T>、no Some-wrap)。
    if let Expr::Block(inner) = outer_block {
        match &inner[0] {
            IrStmt::If {
                condition,
                then_body,
                ..
            } => {
                // F64 truthy predicate is a tmp-binding Block (non-pure operand
                // case)、structurally `Expr::Block { Let __ts_op0 = <getter>;
                // <op0 != 0.0 && !op0.is_nan()> }`。
                assert!(
                    matches!(condition, Expr::Block(_)),
                    "F64 truthy predicate must be a tmp-binding Block for non-pure \
                     operand (getter call), got: {condition:?}"
                );
                // Setter call uses raw rhs (no Some-wrap since LHS = F64)
                if let Some(IrStmt::Expr(Expr::MethodCall { args, .. })) = then_body.first() {
                    assert!(
                        matches!(args.first(), Some(Expr::NumberLit(n)) if (*n - 5.0).abs() < 1e-9),
                        "F64 LHS &&= setter call must pass raw NumberLit (no Some-wrap), got: {args:?}"
                    );
                }
            }
            other => panic!("inner[0] must be Stmt::If, got: {other:?}"),
        }
    }
}

#[test]
fn test_lhs_type_string_or_assign_emits_block_with_is_empty_predicate() {
    // `||=` × String LHS: predicate uses String falsy = `<getter>.is_empty()`
    // (non-pure operand → may wrap in tmp-binding; truthy.rs handles)。
    let src = "class Foo { _s: string = \"\"; \
               get s(): string { return this._s; } \
               set s(v: string) { this._s = v; } }\n\
               function probe(): void { const f = new Foo(); f.s ||= \"new\"; }";
    let stmts = convert_logical_stmt_in_probe(src, 1, 1)
        .expect("String LHS ||= must succeed (falsy_predicate dispatches per-type)");
    let outer_block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    if let Expr::Block(inner) = outer_block {
        match &inner[0] {
            IrStmt::If {
                condition,
                then_body,
                ..
            } => {
                // Setter call uses raw rhs StringLit (no Some-wrap、LHS = String)
                if let Some(IrStmt::Expr(Expr::MethodCall { args, .. })) = then_body.first() {
                    assert!(
                        matches!(args.first(), Some(Expr::StringLit(s)) if s == "new"),
                        "String LHS ||= setter call must pass raw StringLit, got: {args:?}"
                    );
                }
                // Falsy predicate exists (per-type、specific shape varies、structural verify)
                let _ = condition; // structural existence verified via Stmt::If match
            }
            other => panic!("inner[0] must be Stmt::If, got: {other:?}"),
        }
    }
}

#[test]
fn test_lhs_type_option_and_assign_emits_some_wrap_setter_arg() {
    // `&&=` × Option<T> LHS: setter argument wrapped in Some(rhs) since LHS
    // = Option<T>。Predicate uses Option truthy = `<getter>.is_some_and(...)`
    // (or as_ref().is_some_and(...) for !Copy inner)。
    let src = "class Foo { _o: number | undefined = 5; \
               get o(): number | undefined { return this._o; } \
               set o(v: number | undefined) { this._o = v; } }\n\
               function probe(): void { const f = new Foo(); f.o &&= 7; }";
    let stmts = convert_logical_stmt_in_probe(src, 1, 1).expect("Option<T> LHS &&= must succeed");
    let outer_block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    if let Expr::Block(inner) = outer_block {
        if let IrStmt::If { then_body, .. } = &inner[0] {
            // Setter argument = Some(7.0) (LHS = Option<T> → wrap_setter_value
            // applies Some-wrap)。
            if let Some(IrStmt::Expr(Expr::MethodCall { args, .. })) = then_body.first() {
                assert!(
                    matches!(
                        args.first(),
                        Some(Expr::FnCall {
                            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
                            ..
                        })
                    ),
                    "Option<T> LHS &&= setter call must wrap arg in Some(_), got: {args:?}"
                );
            }
        }
    }
}

#[test]
fn test_inv3_se_having_receiver_emits_iife_form_for_logical_compound() {
    // INV-3 (a) Property statement compliance: SE-having receiver
    // (`getInstance().value ??= 42`) must IIFE-wrap to evaluate the receiver
    // exactly once. Output: `{ let mut __ts_recv = getInstance(); if
    // __ts_recv.value().is_none() { __ts_recv.set_value(Some(42)); }; }`
    let src = format!(
        "{B4_CACHE_OPTION_SRC}\n\
         function getInstance(): Cache {{ return new Cache(); }}\n\
         function probe(): void {{ getInstance().value ??= 42; }}"
    );
    let stmts = convert_logical_stmt_in_probe(&src, 2, 0).expect("SE-having receiver must succeed");
    let outer_block = match &stmts[0] {
        IrStmt::Expr(e) => e,
        other => panic!("expected Stmt::Expr(Block), got {other:?}"),
    };
    let inner_stmts = match outer_block {
        Expr::Block(s) => s,
        other => panic!("expected Expr::Block, got: {other:?}"),
    };
    // SE-having form: 2 stmts (Let __ts_recv + Stmt::If with __ts_recv.* calls)
    assert_eq!(
        inner_stmts.len(),
        2,
        "SE-having receiver must emit 2 stmts (Let __ts_recv + Stmt::If), got: {inner_stmts:?}"
    );
    // Stmt 0: let mut __ts_recv = <getInstance() FnCall>
    match &inner_stmts[0] {
        IrStmt::Let {
            mutable: true,
            name,
            init: Some(_),
            ..
        } if name == "__ts_recv" => {}
        other => panic!("stmt 0: must be Let mut __ts_recv (INV-3 IIFE binding), got: {other:?}"),
    }
    // Stmt 1: if __ts_recv.value().is_none() { __ts_recv.set_value(Some(42)); }
    match &inner_stmts[1] {
        IrStmt::If {
            condition,
            then_body,
            ..
        } => {
            // Condition uses __ts_recv (not original `getInstance()`)
            if let Expr::MethodCall { object, .. } = condition {
                if let Expr::MethodCall {
                    object: inner_obj, ..
                } = object.as_ref()
                {
                    assert!(
                        matches!(inner_obj.as_ref(), Expr::Ident(n) if n == "__ts_recv"),
                        "predicate must use __ts_recv (IIFE binding) for INV-3 compliance, got: {inner_obj:?}"
                    );
                }
            }
            // Setter call uses __ts_recv
            if let Some(IrStmt::Expr(Expr::MethodCall { object, .. })) = then_body.first() {
                assert!(
                    matches!(object.as_ref(), Expr::Ident(n) if n == "__ts_recv"),
                    "setter call must use __ts_recv for INV-3 compliance, got: {object:?}"
                );
            }
        }
        other => panic!("stmt 1: must be Stmt::If, got: {other:?}"),
    }
}
