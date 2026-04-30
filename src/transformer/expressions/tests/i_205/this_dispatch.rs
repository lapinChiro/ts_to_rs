//! I-205 T10 (Inside-class `this.x` dispatch、E2 internal context) — cells 60-64 +
//! INV-2 internal-external dispatch path symmetry + Tier 2 honest error reclassify
//! for B2 (read-only) / B3 (write-only) / B6 (method) in internal context。
//!
//! ## 対象 cells / 機能
//!
//! - **Cell 60** (E2 internal `this.x` Read × B2 getter only):
//!   `self.x()` MethodCall dispatch (Read context、external `obj.x` と uniform)
//! - **Cell 61** (E2 internal `this.x = v` Write × B4 getter+setter):
//!   `self.set_x(<value>)` MethodCall (Write context、external `obj.x = v` と uniform)
//! - **Cell 63** (E2 internal `this.x += v` Compound × B4):
//!   Block form `[ Let __ts_new = self.x() + v;  Expr self.set_x(__ts_new);  TailExpr __ts_new ]`
//!   (T8 dispatch_instance_member_compound 経由、external compound と uniform)
//! - **Cell 64** (E2 internal `this.x++` Update × B4):
//!   Block form `[ Let __ts_old = self.x();  Expr self.set_x(__ts_old + 1.0);  TailExpr __ts_old ]`
//!   (T7 dispatch_instance_member_update 経由、external update と uniform)
//!
//! ## INV-2 verification (External (E1) と internal (E2 this) dispatch path symmetry)
//!
//! 同じ class B-axis (= B2 getter / B3 setter / B4 both / B6 method) で、external context
//! (`obj.x` access) と internal context (`this.x` access in class method body) は **同じ
//! dispatch arm に到達** し、receiver IR のみが異なる (`Expr::Ident("obj")` vs
//! `Expr::Ident("self")`)。本 file の各 internal cell は対応する read.rs / write.rs /
//! update.rs / compound.rs 内 external cell と pair で verify。
//!
//! ## Tier 2 honest error reclassify in internal context
//!
//! - **B2 internal Write `this.x = v`** (read-only): "write to read-only property" error
//!   (cell 12 と symmetric、external/internal 両方とも Tier 2 honest)
//! - **B3 internal Read `this.x`** (write-only): "read of write-only property" error
//!   (cell 4 と symmetric)
//! - **B6 internal Read `this.x` no-paren** (method-as-fn-reference): "method-as-fn-reference
//!   (no-paren)" error (cell 7 と symmetric、I-209 別 PRD で Tier 1 化候補)
//!
//! 全 test は `Transformer::for_module(...).convert_expr(&this_x_member_expr)` で direct
//! invoke、IR Expr を `assert_eq!` で token-level verify。Tier 2 path は `convert_expr` の
//! Err を `downcast::<UnsupportedSyntaxError>` で kind verify。

use super::super::*;

use crate::ir::{BinOp, Expr, Stmt as IrStmt};
use crate::transformer::UnsupportedSyntaxError;

// =============================================================================
// Helpers (DRY refactor; each test extracts ExprStmt[0] from a class method body
// and converts via Transformer::convert_expr)
// =============================================================================

/// Convert the first ExprStmt of class[`class_index`].method[`member_index`] body via
/// `Transformer::convert_expr`. Returns `anyhow::Result<Expr>` so Tier 2 (Err) and Tier 1
/// (Ok) paths can both be tested.
///
/// `TctxFixture::from_source` runs the full pipeline including TypeResolver, so by the
/// time we extract the SWC `Expr` and pass it to `convert_expr`, all relevant `expr_types`
/// entries are populated (in particular, `Expr::This` spans inside class method bodies are
/// resolved to `RustType::Named { class_name }` per `visit_class_body`).
fn convert_in_class_method(
    src: &str,
    class_index: usize,
    member_index: usize,
) -> anyhow::Result<Expr> {
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let stmt_expr = extract_class_method_body_expr_stmt(module, class_index, member_index, 0);
    Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new()).convert_expr(&stmt_expr)
}

/// Asserts that converting the first ExprStmt of class[`class_index`].method[`member_index`]
/// returns `UnsupportedSyntaxError` with kind == `expected_kind` (exact match).
fn assert_in_class_method_unsupported_kind(
    src: &str,
    class_index: usize,
    member_index: usize,
    expected_kind: &str,
) {
    let err = match convert_in_class_method(src, class_index, member_index) {
        Ok(v) => panic!("expected Err with kind={expected_kind}, got Ok({v:?})"),
        Err(e) => e,
    };
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .unwrap_or_else(|e| panic!("error must be UnsupportedSyntaxError, got: {e:?}"));
    assert_eq!(usx.kind, expected_kind, "kind mismatch");
}

// =============================================================================
// Cell 60: A1 Read × E2 internal `this.x` × B2 (getter only)
// =============================================================================

#[test]
fn test_cell_60_internal_this_b2_getter_only_read_emits_self_method_call() {
    // Matrix cell 60 — class Logger に getter `prefix(): string` 定義、`test()` method body
    // 内で `this.prefix;` ExprStmt を抽出。期待 IR = `self.prefix()` MethodCall (Read context、
    // external `l.prefix` cell 3 と symmetric、INV-2 verify)。
    let src = "class Logger { \
               _prefix: string = \"[INFO]\"; \
               get prefix(): string { return this._prefix; } \
               test(): void { this.prefix; } }";
    // Class body member layout: [0] _prefix field, [1] prefix getter, [2] test method
    let result = convert_in_class_method(src, 0, 2)
        .expect("cell 60 must succeed (B2 internal getter dispatch)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("self".to_string())),
            method: "prefix".to_string(),
            args: vec![],
        },
        "cell 60 B2 internal getter Read: must emit `self.prefix()` MethodCall"
    );
}

// =============================================================================
// Cell 61: A2 Write × E2 internal `this.x = v` × B4 (getter+setter)
// =============================================================================

#[test]
fn test_cell_61_internal_this_b4_getter_setter_write_emits_self_set_method_call() {
    // Matrix cell 61 — `this.value = this.value + 1` を `incr_internal()` method body 内で
    // 実行。期待 IR = `self.set_value(self.value() + 1.0)` MethodCall (Write context、external
    // `c.value = expr` cell 14 と symmetric、INV-2 verify)。
    //
    // Borrow checker 観点: `&mut self` 内で `self.value()` (immutable borrow、argument
    // evaluation 段階で完了 = Copy f64 return) と `self.set_value(...)` (mutable borrow、
    // method-call dispatch 段階で取得) は NLL + two-phase borrow により共存可能。temp
    // binding は Rust 上不要。
    let src = "class Counter { \
               _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } \
               incr_internal(): void { this.value = this.value + 1; } }";
    // Class body: [0] _n field, [1] value getter, [2] value setter, [3] incr_internal
    let result = convert_in_class_method(src, 0, 3)
        .expect("cell 61 must succeed (B4 internal setter dispatch)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("self".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::BinaryOp {
                left: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("self".to_string())),
                    method: "value".to_string(),
                    args: vec![],
                }),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(1.0)),
            }],
        },
        "cell 61 B4 internal setter Write: must emit `self.set_value(self.value() + 1.0)` \
         (NLL + two-phase borrow で borrow conflict 不発、temp binding 不要)"
    );
}

// =============================================================================
// INV-2 verification: External (E1) と internal (E2 this) dispatch path symmetry
// =============================================================================

#[test]
fn test_inv_2_e1_external_e2_internal_dispatch_path_symmetry_b4_read() {
    // INV-2: external `c.value` (E1) と internal `this.value` (E2) は両方とも `dispatch_*
    // _member_read` の B4 getter dispatch arm に到達、receiver IR のみが異なる
    // (Ident("c") vs Ident("self"))。
    let src = "class Counter { \
               _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } \
               read_self(): void { this.value; } }\n\
               function probe(): void { const c = new Counter(); c.value; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    // Internal: read_self body stmt 0 = `this.value;`
    let internal_expr = extract_class_method_body_expr_stmt(module, 0, 3, 0);
    // External: probe fn body stmt 1 = `c.value;`
    let external_expr = extract_fn_body_expr_stmt(module, 1, 1);

    let tctx = fx.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut transformer = Transformer::for_module(&tctx, &mut synthetic);
    let internal_ir = transformer
        .convert_expr(&internal_expr)
        .expect("internal this.value Read must succeed");
    let external_ir = transformer
        .convert_expr(&external_expr)
        .expect("external c.value Read must succeed");

    assert_eq!(
        internal_ir,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("self".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        "internal this.value: must dispatch to self.value()"
    );
    assert_eq!(
        external_ir,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "value".to_string(),
            args: vec![],
        },
        "external c.value: must dispatch to c.value()"
    );
}

#[test]
fn test_inv_2_e1_external_e2_internal_dispatch_path_symmetry_b4_write() {
    // INV-2 Write 側: external `c.value = 7` (E1) と internal `this.value = 7` (E2) が両方
    // とも setter dispatch arm に到達、receiver IR のみ異なる。
    let src = "class Counter { \
               _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } \
               set_self(): void { this.value = 7; } }\n\
               function probe(): void { const c = new Counter(); c.value = 7; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let internal_expr = extract_class_method_body_expr_stmt(module, 0, 3, 0);
    let external_expr = extract_fn_body_expr_stmt(module, 1, 1);

    let tctx = fx.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut transformer = Transformer::for_module(&tctx, &mut synthetic);
    let internal_ir = transformer
        .convert_expr(&internal_expr)
        .expect("internal this.value = v Write must succeed");
    let external_ir = transformer
        .convert_expr(&external_expr)
        .expect("external c.value = v Write must succeed");

    assert_eq!(
        internal_ir,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("self".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::NumberLit(7.0)],
        },
        "internal this.value = 7: must dispatch to self.set_value(7.0)"
    );
    assert_eq!(
        external_ir,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("c".to_string())),
            method: "set_value".to_string(),
            args: vec![Expr::NumberLit(7.0)],
        },
        "external c.value = 7: must dispatch to c.set_value(7.0)"
    );
}

// =============================================================================
// Cell 63: A3 += × E2 internal `this.x += v` × B4 (getter+setter) — compound
// =============================================================================

#[test]
fn test_cell_63_internal_this_b4_compound_add_assign_emits_block_form() {
    // Matrix cell 63 — `this.value += 1` を `incr()` method body 内で実行。期待 IR =
    // T8 dispatch_instance_member_compound 経由 Block form (yield_new shape):
    //   `{ let __ts_new = self.value() + 1.0; self.set_value(__ts_new); __ts_new }`
    //
    // SE-free receiver (`Expr::Ident("self")` は is_side_effect_free=true) のため IIFE wrap
    // 不要、bare Block form。Borrow checker は Block 内で sequential 評価 (immutable borrow
    // → mutable borrow) なので NLL で問題なし。
    let src = "class Counter { \
               _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } \
               incr(): void { this.value += 1; } }";
    // Class body: [0] _n, [1] value getter, [2] value setter, [3] incr method
    let result = convert_in_class_method(src, 0, 3)
        .expect("cell 63 must succeed (B4 internal compound dispatch)");

    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 63: expected Expr::Block, got: {other:?}"),
    };
    assert_eq!(
        stmts.len(),
        3,
        "cell 63: setter desugar block must have 3 stmts"
    );

    // Stmt 0: let __ts_new = self.value() + 1.0;
    match &stmts[0] {
        IrStmt::Let {
            mutable: false,
            name,
            init: Some(init),
            ..
        } if name == "__ts_new" => {
            assert_eq!(
                init,
                &Expr::BinaryOp {
                    left: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("self".to_string())),
                        method: "value".to_string(),
                        args: vec![],
                    }),
                    op: BinOp::Add,
                    right: Box::new(Expr::NumberLit(1.0)),
                },
                "cell 63 stmt 0 init: BinOp shape mismatch"
            );
        }
        other => panic!("cell 63 stmt 0: expected Let __ts_new, got {other:?}"),
    }
    // Stmt 1: self.set_value(__ts_new);
    match &stmts[1] {
        IrStmt::Expr(call) => assert_eq!(
            call,
            &Expr::MethodCall {
                object: Box::new(Expr::Ident("self".to_string())),
                method: "set_value".to_string(),
                args: vec![Expr::Ident("__ts_new".to_string())],
            },
            "cell 63 stmt 1: setter call mismatch"
        ),
        other => panic!("cell 63 stmt 1: expected ExprStmt, got {other:?}"),
    }
    // Stmt 2: __ts_new (TailExpr)
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_new"),
        "cell 63 stmt 2: must be TailExpr __ts_new, got {:?}",
        stmts[2]
    );
}

// =============================================================================
// Cell 38 internal counterpart: A5 ??= × E2 internal `this.x ??= v` × B4 (logical compound)
// =============================================================================

#[test]
fn test_internal_this_b4_nullish_assign_emits_block_form_with_predicate() {
    // Cell 38 internal counterpart — `this.value ??= 42` を method body 内で実行。
    // 期待 IR = T9 try_dispatch_member_logical_compound 経由 Block form (statement context):
    //   `if self.value().is_none() { self.set_value(Some(42.0)); }` 等の predicate-conditional
    //   setter dispatch。
    //
    // **Structural test rationale (Layer 3 Spec gap fix)**: T6/T7/T8 は internal context
    // unit test を T10 で個別 cover (cell 61/63/64) するが、T9 (logical compound) は matrix
    // で `this.x ??= v` cell が無く、orthogonality merge inheritance のみに依存。本 test を
    // 追加することで T7/T8/T9 全 dispatch helper が `Expr::This` receiver で uniformly fire
    // する事を直接 lock-in、subsequent 変更で T9 internal regression を防ぐ。
    let src = "class Counter { \
               _v: number | undefined = undefined; \
               get value(): number | undefined { return this._v; } \
               set value(v: number | undefined) { this._v = v; } \
               init(): void { this.value ??= 42; } }";
    // Class body: [0] _v field, [1] value getter, [2] value setter, [3] init method
    let result = convert_in_class_method(src, 0, 3)
        .expect("internal this.x ??= v must succeed (B4 internal logical compound dispatch)");

    // Verify result is Expr::Block — T9 emits Block form for logical compound dispatch
    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!(
            "internal ??=: expected Expr::Block (T9 logical compound dispatch), got: {other:?}"
        ),
    };
    assert!(
        !stmts.is_empty(),
        "internal ??= block must contain at least one statement"
    );

    // Verify the block contains setter dispatch on `self`:
    // `self.set_value(...)` MethodCall must be present somewhere in the block (within
    // the `if predicate { ... }` setter dispatch shape per T9 emit pattern). Scan
    // recursively rather than asserting exact structure, since T9 details (predicate
    // shape, Some-wrap behavior, etc.) are covered by T9-specific tests in
    // logical_compound.rs — here we verify only that Internal context goes through T9.
    fn block_contains_self_setter(stmts: &[IrStmt]) -> bool {
        stmts.iter().any(stmt_contains_self_setter)
    }
    fn stmt_contains_self_setter(stmt: &IrStmt) -> bool {
        match stmt {
            IrStmt::Expr(e) | IrStmt::TailExpr(e) => expr_contains_self_setter(e),
            IrStmt::Let { init, .. } => init.as_ref().is_some_and(expr_contains_self_setter),
            IrStmt::If {
                condition,
                then_body,
                else_body,
            } => {
                expr_contains_self_setter(condition)
                    || block_contains_self_setter(then_body)
                    || else_body
                        .as_ref()
                        .is_some_and(|b| block_contains_self_setter(b))
            }
            _ => false,
        }
    }
    fn expr_contains_self_setter(expr: &Expr) -> bool {
        match expr {
            Expr::MethodCall { object, method, .. }
                if matches!(object.as_ref(), Expr::Ident(n) if n == "self")
                    && method == "set_value" =>
            {
                true
            }
            Expr::Block(stmts) => block_contains_self_setter(stmts),
            Expr::BinaryOp { left, right, .. } => {
                expr_contains_self_setter(left) || expr_contains_self_setter(right)
            }
            Expr::MethodCall { object, args, .. } => {
                expr_contains_self_setter(object) || args.iter().any(expr_contains_self_setter)
            }
            _ => false,
        }
    }

    assert!(
        block_contains_self_setter(stmts),
        "internal ??= block must contain `self.set_value(...)` MethodCall \
         (T9 logical compound dispatch through self receiver). Got block stmts: {stmts:#?}"
    );
}

// =============================================================================
// Cell 64: A6 ++ × E2 internal `this.x++` × B4 — update postfix
// =============================================================================

#[test]
fn test_cell_64_internal_this_b4_postfix_increment_emits_block_form() {
    // Matrix cell 64 — `this.value++` postfix form。期待 IR = T7 dispatch_instance_member_update
    // 経由 Block form (postfix old-value preservation):
    //   `{ let __ts_old = self.value(); self.set_value(__ts_old + 1.0); __ts_old }`
    let src = "class Counter { \
               _n: number = 0; \
               get value(): number { return this._n; } \
               set value(v: number) { this._n = v; } \
               incr(): void { this.value++; } }";
    let result = convert_in_class_method(src, 0, 3)
        .expect("cell 64 must succeed (B4 internal update dispatch)");

    let stmts = match &result {
        Expr::Block(s) => s,
        other => panic!("cell 64: expected Expr::Block, got: {other:?}"),
    };
    assert_eq!(
        stmts.len(),
        3,
        "cell 64: postfix update block must have 3 stmts"
    );

    // Stmt 0: let __ts_old = self.value();
    match &stmts[0] {
        IrStmt::Let {
            mutable: false,
            name,
            init: Some(init),
            ..
        } if name == "__ts_old" => {
            assert_eq!(
                init,
                &Expr::MethodCall {
                    object: Box::new(Expr::Ident("self".to_string())),
                    method: "value".to_string(),
                    args: vec![],
                },
                "cell 64 stmt 0 init: getter call mismatch"
            );
        }
        other => panic!("cell 64 stmt 0: expected Let __ts_old, got {other:?}"),
    }
    // Stmt 1: self.set_value(__ts_old + 1.0);
    match &stmts[1] {
        IrStmt::Expr(call) => assert_eq!(
            call,
            &Expr::MethodCall {
                object: Box::new(Expr::Ident("self".to_string())),
                method: "set_value".to_string(),
                args: vec![Expr::BinaryOp {
                    left: Box::new(Expr::Ident("__ts_old".to_string())),
                    op: BinOp::Add,
                    right: Box::new(Expr::NumberLit(1.0)),
                }],
            },
            "cell 64 stmt 1: setter call with __ts_old + 1.0 arg mismatch"
        ),
        other => panic!("cell 64 stmt 1: expected ExprStmt, got {other:?}"),
    }
    // Stmt 2: __ts_old (TailExpr)
    assert!(
        matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "__ts_old"),
        "cell 64 stmt 2: must be TailExpr __ts_old, got {:?}",
        stmts[2]
    );
}

// =============================================================================
// Tier 2 honest error reclassify (internal context、external と symmetric)
// =============================================================================

#[test]
fn test_internal_this_b2_getter_only_write_emits_unsupported_syntax_error() {
    // Cell 12 internal counterpart: `this.x = v` for B2 (getter only) → Tier 2 honest error
    // "write to read-only property" (external cell 12 と symmetric)
    let src = "class Foo { \
               _v: number = 0; \
               get x(): number { return this._v; } \
               attempt_write(): void { this.x = 5; } }";
    // Class body: [0] _v field, [1] x getter, [2] attempt_write method
    assert_in_class_method_unsupported_kind(src, 0, 2, "write to read-only property");
}

#[test]
fn test_internal_this_b3_setter_only_read_emits_unsupported_syntax_error() {
    // Cell 4 internal counterpart: `this.x` Read for B3 (setter only) → Tier 2 honest
    // error "read of write-only property" (external cell 4 と symmetric)
    let src = "class Box { \
               _v: number = 0; \
               set x(v: number) { this._v = v; } \
               attempt_read(): void { this.x; } }";
    // Class body: [0] _v field, [1] x setter, [2] attempt_read method
    assert_in_class_method_unsupported_kind(src, 0, 2, "read of write-only property");
}

#[test]
fn test_internal_this_b6_method_no_paren_read_emits_unsupported_syntax_error() {
    // Cell 7 internal counterpart: `this.greet` (no-paren) for B6 (method) → Tier 2 honest
    // error "method-as-fn-reference (no-paren)" (external cell 7 と symmetric、I-209 別 PRD)
    let src = "class Foo { \
               greet(): number { return 1; } \
               attempt_ref(): void { this.greet; } }";
    // Class body: [0] greet method, [1] attempt_ref method
    assert_in_class_method_unsupported_kind(src, 0, 1, "method-as-fn-reference (no-paren)");
}

// =============================================================================
// Body-type coverage (PRD T10 completion criteria: "method body / getter body /
// setter body / constructor body 全 dispatch")
//
// 上記 cells 60/61/63/64 は method body context (`incr_internal()` 等の regular method)
// で verify。本 section では getter body (Read context、boundary fallback) / setter body
// (Write context、内部 `this.<other>` access) / constructor body (Write context、`this.x = v`)
// を追加 verify。dispatch logic は body type に依存しない (= TypeResolver は class body 全体で
// `this` を register、`classify_member_receiver` は body type を見ない) ため、各 body type
// で同 dispatch arm 経由を structural lock-in。
// =============================================================================

#[test]
fn test_setter_body_internal_this_b4_dispatches_to_other_setter() {
    // Setter body 内 `this.<other>` access が Read 側 dispatch arm に到達することを verify。
    // `class Foo { _v = 0; get x(): number { return this._v; } set x(v: number) { this._v = v; }
    //   set proxy(v: number) { this.x = v; } }` で `proxy` setter body の `this.x = v` は
    // 別 accessor (B4 x) への Write dispatch (= cell 14 internal counterpart in setter body)。
    let src = "class Foo { \
               _v: number = 0; \
               get x(): number { return this._v; } \
               set x(v: number) { this._v = v; } \
               set proxy(v: number) { this.x = v; } }";
    // Class body: [0] _v field, [1] x getter, [2] x setter, [3] proxy setter
    let result = convert_in_class_method(src, 0, 3)
        .expect("setter body dispatch must succeed (B4 internal Write inside setter body)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("self".to_string())),
            method: "set_x".to_string(),
            args: vec![Expr::Ident("v".to_string())],
        },
        "setter body `this.x = v`: must dispatch to `self.set_x(v)` (uniform with method body)"
    );
}

// **Constructor body T10 dispatch is intentionally NOT tested here**:
//
// `convert_constructor_body` (`src/transformer/classes/members.rs:190`) bypasses the
// dispatch path entirely — it pattern-matches `this.<name> = <value>` in constructor body
// statements via `try_extract_this_assignment` and folds them into a `Self { name: value,
// ... }` struct-init tail expression, **regardless of whether `<name>` is a struct field
// or a setter**. This is a pre-existing constructor body bug (separately tracked as
// I-222 — "Constructor body `this.<name> = v` does not distinguish struct field from
// accessor、generates invalid `Self { <accessor_name>: value }` for B3/B4 cells")、
// orthogonal to T10 architectural concern (= "Inside-class `this.x` dispatch via
// convert_expr / dispatch_member_*")。
//
// Method body (regular method、getter body、setter body) uses `convert_method_inner` →
// `build_method` → `convert_stmt` → `convert_expr` → T6/T7/T8/T10 dispatch path. Thus
// cells 60/61/63/64 + setter body test cover the dispatch architectural concern. The
// constructor body case is orthogonal and tracked separately in TODO.

// =============================================================================
// Boundary: getter body internal `this._field` access (B1 fallback、no method registry hit)
// =============================================================================

#[test]
fn test_internal_this_field_access_inside_getter_body_emits_field_access() {
    // Getter body 内 `return this._n;` の `this._n` は B1 fallback (= regular field、_n 自体に
    // method 登録なし) → FieldAccess emit (= regression、本 PRD で挙動変更なし)。
    // **Important**: `_n` field は class methods registry に登録されないため、
    // classify_member_receiver の Instance gate で `lookup_method_sigs_in_inheritance_chain`
    // が None を返し、Fallback dispatch で FieldAccess emit。internal context でも同 path。
    let src = "class Counter { \
               _n: number = 0; \
               get value(): number { return this._n; } }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    // value getter body stmt 0 = `return this._n;` — extract Return arg directly
    let class_decl = match &module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Class(c))) => c,
        _ => panic!("expected class declaration"),
    };
    let getter = match &class_decl.class.body[1] {
        ast::ClassMember::Method(m) => m,
        _ => panic!("expected getter method at member index 1"),
    };
    let body = getter.function.body.as_ref().expect("getter has no body");
    let return_arg = match &body.stmts[0] {
        ast::Stmt::Return(ret) => ret
            .arg
            .as_ref()
            .expect("return has no arg")
            .as_ref()
            .clone(),
        _ => panic!("expected return statement"),
    };
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&return_arg)
        .expect("getter body this._n must succeed (B1 field fallback)");
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("self".to_string())),
            field: "_n".to_string(),
        },
        "getter body `return this._n;`: must emit FieldAccess `self._n` (B1 field fallback、\
         method registry に該当 entry 不在で Fallback path)"
    );
}
