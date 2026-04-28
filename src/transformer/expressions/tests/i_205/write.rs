//! I-205 T6 (Write context dispatch via dispatch_member_write helper) — cells 11-19 +
//! Iteration v10 second-review C1 coverage 補完 (Static field lookup miss + Write defensive
//! dispatch arms) + T5 fix lock-in for Write LHS。
//!
//! 全 test は AssignExpr (`obj.x = v` or `Class.x = v`) を `convert_expr` 経由で実行、
//! `dispatch_member_write` 内 dispatch arm が ideal IR (Tier 1 setter dispatch / Tier 2
//! honest error / B1/B9 fallback FieldAccess Assign) を emit することを verify。
//!
//! ## 対象 cells / 機能
//!
//! - **Cells 11, 19** (B1 field / B9 unknown): fallback FieldAccess Assign emit (regression、
//!   既存挙動維持)。
//! - **Cells 13, 14, 18** (B3 setter only / B4 getter+setter / B8 static setter):
//!   Tier 1 setter dispatch (MethodCall set_x / FnCall::UserAssocFn set_x)。
//! - **Cells 12, 16, 17** (B2 getter only / B6 method / B7 inherited):
//!   Tier 2 honest error reclassify (UnsupportedSyntaxError)。
//! - **Cell 61 (E2 internal `this.x = v`)** は subsequent T10 で別途 verify、本 T6 では
//!   external (E1) Write のみ。
//!
//! ## Cross-cutting verifications
//!
//! - **Write context regression** (T5 fix lock-in): `convert_member_expr_for_write` (assignment
//!   LHS conversion path) が Read dispatch logic を leak しない (= `f.x = 5` の LHS で
//!   `f.x()` MethodCall に誤変換しない、Iteration v9 deep deep review で発覚した silent
//!   regression の structural lock-in)。
//! - **INV-2 E1 Read/Write symmetry**: external (E1) `obj.x` Read と `obj.x = v` Write が
//!   両方とも MethodCall に dispatch (= `dispatch_*_member_read` と `dispatch_*_member_write`
//!   の semantic symmetry)。E2 internal `this.x` は T10 で verify。
//! - **T6 Fallback equivalence**: `dispatch_member_write` の Fallback path が T5 で導入された
//!   `convert_member_expr_for_write` の `for_write=true` skip path と token-level identical
//!   な IR を emit (= subsequent T7-T9 compound 拡張時の regression lock)。
//! - **Static field lookup miss**: dispatch_member_write Static gate の lookup miss branch
//!   (= static field) は dispatch_static_member_write を経由せず Fallback path に流れる
//!   (T11 (11-d) で associated const path に修正予定の pre-T6 既存挙動 lock-in)。
//! - **Write defensive dispatch arms** (matrix cell 化なし、Iteration v10 second-review C1
//!   補完): static B3 setter only Write / static B6 method Write / static B7 inherited Write
//!   で `dispatch_static_member_write` の defensive arm error message lock-in。

use super::super::*;

use crate::ir::{CallTarget, Expr, UserTypeRef};
use crate::transformer::UnsupportedSyntaxError;

// =============================================================================
// Write context regression (T5 fix lock-in for Write LHS)
// =============================================================================
//
// Iteration v9 deep deep review で発覚した critical bug の regression lock-in。本 PRD T5 で
// 導入した Read context dispatch logic (`resolve_member_access` の class member dispatch) が
// `convert_member_expr_for_write` (= assignment LHS conversion) にも leak すると `f.x = 5;` の
// LHS が `f.x()` (MethodCall) に変換され `f.x() = 5.0;` (invalid Rust LHS、compile error) を
// emit する silent regression が発生する。本 fix で `convert_member_expr_inner` の Ident path で
// `for_write=true` 時 Read dispatch を skip、既存 FieldAccess fallback を維持。setter dispatch
// (`f.set_x(5.0)`) は本 T6 (Write context dispatch、`dispatch_member_write` helper) で別途実装。

#[test]
fn test_write_context_lhs_does_not_leak_read_dispatch() {
    // 本 test は B4 (getter+setter pair) class の `f.x = 5;` で **LHS は FieldAccess**
    // を維持することを direct verify (= `convert_member_expr_for_write` 経由 path で Read
    // dispatch leak 排除を structural lock-in、subsequent T7-T9 compound 拡張時に compound
    // path が `convert_member_expr_for_write` を経由する場合に regression detection)。
    let src = "class Foo { _v: number = 0; \
               get x(): number { return this._v; } \
               set x(v: number) { this._v = v; } }\n\
               function probe(): void { const f = new Foo(); f.x = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    // probe fn body の 2 番目 stmt = `f.x = 5;` ExprStmt (AssignExpr)
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 1);
    let assign_expr = match &assign_stmt {
        ast::Expr::Assign(a) => a,
        other => panic!("expected AssignExpr, got: {other:?}"),
    };
    // AssignTarget::Simple(SimpleAssignTarget::Member(...)) の inner MemberExpr を抽出
    let target_member = match &assign_expr.left {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(m)) => m,
        other => panic!("expected SimpleAssignTarget::Member, got: {other:?}"),
    };
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_member_expr_for_write(target_member)
        .expect("Write context conversion must succeed");
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("f".to_string())),
            field: "x".to_string(),
        },
        "Write context LHS must emit FieldAccess (NOT MethodCall — Read dispatch \
         leak regression check)、setter dispatch は本 T6 で実装"
    );
}

// =============================================================================
// T6 Write context cells (11-19)
// =============================================================================

// -----------------------------------------------------------------------------
// Cell 11: A2 Write × B1 (regular field) → fallback FieldAccess Assign (regression)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_11_b1_field_write_emits_field_access_assign() {
    // Matrix cell 11: receiver type = Foo (registered)、x は field のみ (methods 不在)
    // → lookup_method_sigs_in_inheritance_chain returns None → Fallback path で
    // Expr::Assign { FieldAccess, value } emit (regression、既存挙動維持)
    let src = "class Foo { x: number = 0; }\n\
               function probe(): void { const f = new Foo(); f.x = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect("cell 11 must succeed (B1 field fallback)");
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::FieldAccess {
                object: Box::new(Expr::Ident("f".to_string())),
                field: "x".to_string(),
            }),
            value: Box::new(Expr::NumberLit(5.0)),
        },
        "cell 11 B1 field Write: must emit Expr::Assign {{ FieldAccess, NumberLit }} (no setter dispatch)"
    );
}

// -----------------------------------------------------------------------------
// Cell 12: A2 Write × B2 (getter only) → Tier 2 honest error "write to read-only property"
// -----------------------------------------------------------------------------

#[test]
fn test_cell_12_b2_getter_only_write_emits_unsupported_syntax_error() {
    // Matrix cell 12: getter only (B2) class instance に Write は read-only property への
    // 代入 = TS では TypeError (strict mode) / silent fail (sloppy mode)、Rust では setter
    // 不在のため emission 不能 = Tier 2 honest error reclassify が ideal
    let src = "class Foo { _v: number = 0; get x(): number { return this._v; } }\n\
               function probe(): void { const f = new Foo(); f.x = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 1);
    let err = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect_err("cell 12 must Err (B2 write to read-only)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("cell 12: error must be UnsupportedSyntaxError");
    assert_eq!(
        usx.kind, "write to read-only property",
        "cell 12 B2: kind mismatch"
    );
}

// -----------------------------------------------------------------------------
// Cell 13: A2 Write × B3 (setter only) → MethodCall { method: set_x, args: [value] }
// -----------------------------------------------------------------------------

#[test]
fn test_cell_13_b3_setter_only_write_emits_method_call_set_x() {
    // Matrix cell 13: setter only (B3) → `b.set_x(v)` MethodCall dispatch
    let src = "class Box { _v: number = 0; set x(v: number) { this._v = v; } }\n\
               function probe(): void { const b = new Box(); b.x = 7; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect("cell 13 must succeed (B3 setter dispatch)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("b".to_string())),
            method: "set_x".to_string(),
            args: vec![Expr::NumberLit(7.0)],
        },
        "cell 13 B3 setter only Write: must emit MethodCall {{ method: set_x, args: [value] }}"
    );
}

// -----------------------------------------------------------------------------
// Cell 14: A2 Write × B4 (getter+setter pair) → MethodCall { method: set_x, args: [value] }
// -----------------------------------------------------------------------------

#[test]
fn test_cell_14_b4_getter_setter_pair_write_dispatches_to_setter() {
    // Matrix cell 14: getter + setter pair (B4) → setter dispatch (Read = getter dispatch
    // と symmetric、Write context は has_setter check が has_getter check より先に fire)
    let src = "class Foo { _v: number = 0; \
               get x(): number { return this._v; } \
               set x(v: number) { this._v = v; } }\n\
               function probe(): void { const f = new Foo(); f.x = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect("cell 14 must succeed (B4 setter dispatch)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("f".to_string())),
            method: "set_x".to_string(),
            args: vec![Expr::NumberLit(5.0)],
        },
        "cell 14 B4 getter+setter Write: must dispatch to setter (MethodCall set_x)"
    );
}

// -----------------------------------------------------------------------------
// Cell 16: A2 Write × B6 (regular method) → Tier 2 honest error "write to method"
// -----------------------------------------------------------------------------

#[test]
fn test_cell_16_b6_method_write_emits_unsupported_syntax_error() {
    // Matrix cell 16: regular method を assignment target にする = TS では TypeError、Rust
    // では method を field-style assign する semantic 不在 = Tier 2 honest error reclassify
    let src = "class Foo { greet(): number { return 1; } }\n\
               function probe(): void { const f = new Foo(); f.greet = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 1);
    let err = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect_err("cell 16 must Err (B6 write to method)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("cell 16: error must be UnsupportedSyntaxError");
    assert_eq!(usx.kind, "write to method", "cell 16 B6: kind mismatch");
}

// -----------------------------------------------------------------------------
// Cell 17: A2 Write × B7 (inherited setter) → Tier 2 honest error "write to inherited accessor"
// -----------------------------------------------------------------------------

#[test]
fn test_cell_17_b7_inherited_setter_write_emits_unsupported_syntax_error() {
    // Matrix cell 17: parent class の setter を sub class instance で呼ぶ = inherited
    // accessor write。Iteration v9 で extends 登録 (class.rs:195) を fix した state では
    // is_inherited = true で B7 dispatch arm が fire、Tier 2 honest error reclassify
    // (orthogonal architectural concern = "Class inheritance dispatch" 別 PRD I-206)
    let src = "class Base { _v: number = 0; set x(v: number) { this._v = v; } }\n\
               class Sub extends Base {}\n\
               function probe(): void { const s = new Sub(); s.x = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 2, 1);
    let err = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect_err("cell 17 must Err (B7 inherited)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("cell 17: error must be UnsupportedSyntaxError");
    assert_eq!(
        usx.kind, "write to inherited accessor",
        "cell 17 B7: kind mismatch"
    );
}

// -----------------------------------------------------------------------------
// Cell 18: A2 Write × B8 (static setter) → FnCall { UserAssocFn { Class, set_x } }
// -----------------------------------------------------------------------------

#[test]
fn test_cell_18_b8_static_setter_write_emits_associated_fn_call_set_x() {
    // Matrix cell 18: static setter (B8) → `Counter::set_count(v)` associated fn call
    // Static-only class (instance method 不在) のため、receiver = Ident(Counter) で
    // is_interface = false な TypeDef::Struct lookup hit、static dispatch arm 経由
    let src = "class Counter { static _n: number = 0; \
               static set count(v: number) { Counter._n = v; } }\n\
               function probe(): void { Counter.count = 7; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 0);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect("cell 18 must succeed (B8 static setter)");
    assert_eq!(
        result,
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new("Counter"),
                method: "set_count".to_string(),
            },
            args: vec![Expr::NumberLit(7.0)],
        },
        "cell 18 B8 static setter: must emit FnCall::UserAssocFn {{ method: set_count, args: [value] }}"
    );
}

// -----------------------------------------------------------------------------
// Cell 19: A2 Write × B9 (unknown receiver type) → fallback FieldAccess Assign (regression)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_19_b9_unknown_receiver_write_emits_field_access_assign() {
    // Matrix cell 19: receiver type が registry 不在 (= Any / unknown) → fallback
    // FieldAccess Assign (= 既存挙動維持、本 PRD で挙動変更なし regression lock)
    let src = "const obj: any = null;\nobj.x = 5;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_expr_stmt(module, 1);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect("cell 19 must succeed (B9 unknown fallback)");
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::FieldAccess {
                object: Box::new(Expr::Ident("obj".to_string())),
                field: "x".to_string(),
            }),
            value: Box::new(Expr::NumberLit(5.0)),
        },
        "cell 19 B9 unknown: must emit Expr::Assign {{ FieldAccess, value }} (fallback)"
    );
}

// =============================================================================
// T6 Fallback path equivalence: dispatch_member_write の None case (B1/B9) と
// T5 で導入した `convert_member_expr_inner(member, for_write=true)` skip path が
// token-level identical な IR を emit することを structural verify
// =============================================================================

#[test]
fn test_t6_fallback_emits_same_ir_as_t5_skip_path() {
    // T6 Fallback (B1 field) は dispatch_member_write 内の最終 fallback で
    // `convert_member_expr_for_write` 経由 FieldAccess を emit、`Expr::Assign { FieldAccess,
    // value }` で wrap する。T5 で導入した `for_write=true` skip path と LHS が token-level
    // identical (= Read dispatch leak 排除、subsequent T7-T9 compound 拡張時の regression lock)
    let src = "class Foo { x: number = 0; }\n\
               function probe(): void { const f = new Foo(); f.x = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 1);

    // T6 path: full convert_assign_expr → Expr::Assign { FieldAccess, NumberLit }
    let t6_result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect("T6 path must succeed (B1 fallback)");

    // T5 path: convert_member_expr_for_write の LHS のみ → FieldAccess (skip path 経由)
    let assign_expr = match &assign_stmt {
        ast::Expr::Assign(a) => a,
        other => panic!("expected AssignExpr, got: {other:?}"),
    };
    let target_member = match &assign_expr.left {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(m)) => m,
        other => panic!("expected SimpleAssignTarget::Member, got: {other:?}"),
    };
    let t5_lhs = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_member_expr_for_write(target_member)
        .expect("T5 LHS path must succeed");

    // Verify T5 LHS is FieldAccess (not MethodCall = Read dispatch leak 排除 lock-in)
    assert_eq!(
        t5_lhs,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("f".to_string())),
            field: "x".to_string(),
        },
        "T5 skip-path LHS must be FieldAccess (regression lock for Read dispatch leak)"
    );

    // Verify T6 path wraps T5 LHS into Expr::Assign with NumberLit value (= token-level
    // identical fallback emit)
    assert_eq!(
        t6_result,
        Expr::Assign {
            target: Box::new(t5_lhs),
            value: Box::new(Expr::NumberLit(5.0)),
        },
        "T6 Fallback emit must wrap T5 skip-path LHS in Expr::Assign with converted value"
    );
}

// =============================================================================
// INV-2 verification (E1 external write only — E2 internal `this.x = v` は T10 で verify)
// =============================================================================
//
// External (E1) `obj.x = v` の B4 dispatch が `obj.set_x(v)` MethodCall を emit、
// Read context cell 5 (`obj.x` getter dispatch via `obj.x()`) と symmetric
// (= Read MethodCall name=field、Write MethodCall name=set_field、両方とも Method dispatch)。

#[test]
fn test_inv_2_e1_read_write_dispatch_symmetry_b4() {
    // INV-2 (External E1 と internal E2 dispatch path symmetry) の Read/Write 両方向 cohesion:
    // 外部 access の Read (cell 5) と Write (cell 14) が両方とも MethodCall に dispatch
    // することを 1 test で structural verify (= dispatch_*_member_read と dispatch_*_member_write の
    // semantic symmetry lock-in)
    let src = "class Foo { _v: number = 0; \
               get x(): number { return this._v; } \
               set x(v: number) { this._v = v; } }\n\
               function probe(): void { const f = new Foo(); const r = f.x; f.x = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    // Read at probe fn body stmt 1 (`const r = f.x;`) — extract var init
    let read_init = extract_fn_body_var_init(module, 1, 1);
    let read_result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&read_init)
        .expect("Read context (cell 5) must succeed");
    assert_eq!(
        read_result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("f".to_string())),
            method: "x".to_string(),
            args: vec![],
        },
        "INV-2 E1 Read: cell 5 must emit MethodCall (getter dispatch)"
    );

    // Write at probe fn body stmt 2 (`f.x = 5;`)
    let write_stmt = extract_fn_body_expr_stmt(module, 1, 2);
    let write_result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&write_stmt)
        .expect("Write context (cell 14) must succeed");
    assert_eq!(
        write_result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("f".to_string())),
            method: "set_x".to_string(),
            args: vec![Expr::NumberLit(5.0)],
        },
        "INV-2 E1 Write: cell 14 must emit MethodCall set_x (setter dispatch、Read symmetric)"
    );
}

// =============================================================================
// I-205 T6 Iteration v10 second-review C1 branch coverage additions (Write side)
// =============================================================================
//
// `/check_job` 4-layer review (deep deep) で発覚した C1 branch coverage gaps:
// - Defect 3: dispatch_member_write Static gate の lookup miss branch (= static field)
// - Defect 4 (Write side): dispatch_static_member_write の defensive 3 dispatch arm
//   (Getter only Write / Method Write / inherited static Write)
//
// matrix cell 化されていない arm でも、testing.md "Branch Coverage (C1) — Every `if`,
// `match` arm, `if let Some/None`, and early `return` must have at least one test
// exercising each branch direction" 要件を満たすため、本 section に追加。
// 各 test は error message string を lock-in し、subsequent change で error wording が
// 変化した場合の structural defense-in-depth として機能。

// -----------------------------------------------------------------------------
// Static field write fallback (Static gate lookup miss branch)
// -----------------------------------------------------------------------------

#[test]
fn test_t6_static_field_lookup_miss_falls_through_to_field_access_assign() {
    // dispatch_member_write の Static gate で `lookup_method_sigs_in_inheritance_chain`
    // が None を return する case (= class registered だが methods に該当 entry 不在 =
    // static field) は dispatch_static_member_write を経由せず、Fallback path で
    // FieldAccess Assign emit。pre-T6 既存挙動維持 = Tier 2 等価 compile error
    // (`Class.field = v;` Rust 上 invalid `.` syntax)、subsequent T11 (11-d) で
    // `Class::set_field(v)` associated fn / OnceLock 等の emission strategy 確定予定。
    let src = "class Counter { static _n: number = 0; }\n\
               function probe(): void { Counter._n = 7; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 0);
    let result = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect("static field write must succeed (Fallback fires after lookup miss)");
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::FieldAccess {
                object: Box::new(Expr::Ident("Counter".to_string())),
                field: "_n".to_string(),
            }),
            value: Box::new(Expr::NumberLit(7.0)),
        },
        "static field (lookup miss in Static gate) must fall through to FieldAccess Assign \
         (= pre-T6 既存挙動維持、T11 (11-d) で associated const path 化予定)"
    );
}

// -----------------------------------------------------------------------------
// Static defensive dispatch arms (matrix cell 化なし、C1 coverage 補完)
// -----------------------------------------------------------------------------

#[test]
fn test_t6_static_getter_only_write_emits_unsupported_syntax_error() {
    // dispatch_static_member_write の "Getter present、Setter absent" defensive arm:
    // `Counter.count = v;` for getter only static accessor → Tier 2 honest error
    // "write to read-only static property"。matrix cell 化なし (T11 (11-c) で expansion 予定)、
    // 本 test は C1 coverage + error message lock-in。
    let src = "class Counter { static get count(): number { return 1; } }\n\
               function probe(): void { Counter.count = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 0);
    let err = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect_err("static getter only Write must Err (write to read-only static property)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("error must be UnsupportedSyntaxError");
    assert_eq!(
        usx.kind, "write to read-only static property",
        "static B3 Write: kind mismatch"
    );
}

#[test]
fn test_t6_static_method_write_emits_unsupported_syntax_error() {
    // dispatch_static_member_write の "Method only" defensive arm:
    // `Counter.greet = v;` for static method → Tier 2 honest error "write to static method"。
    // matrix cell 化なし (T11 (11-c) で expansion 予定)、本 test は C1 coverage + error
    // message lock-in。
    let src = "class Counter { static greet(): number { return 1; } }\n\
               function probe(): void { Counter.greet = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 1, 0);
    let err = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect_err("static method Write must Err (write to static method)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("error must be UnsupportedSyntaxError");
    assert_eq!(
        usx.kind, "write to static method",
        "static B6 Write: kind mismatch"
    );
}

#[test]
fn test_t6_static_inherited_setter_write_emits_unsupported_syntax_error() {
    // dispatch_static_member_write の "is_inherited=true" defensive arm:
    // class Sub extends Base { static set count(v) {} } の `Sub.count = v;` →
    // Tier 2 honest error "write to inherited static accessor"。matrix cell 化なし
    // (T11 (11-c) で expansion 予定)、本 test は C1 coverage + error message lock-in。
    let src =
        "class Base { static _n: number = 0; static set count(v: number) { Base._n = v; } }\n\
               class Sub extends Base {}\n\
               function probe(): void { Sub.count = 5; }";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let assign_stmt = extract_fn_body_expr_stmt(module, 2, 0);
    let err = Transformer::for_module(&fx.tctx(), &mut SyntheticTypeRegistry::new())
        .convert_expr(&assign_stmt)
        .expect_err("static inherited setter Write must Err (write to inherited static accessor)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("error must be UnsupportedSyntaxError");
    assert_eq!(
        usx.kind, "write to inherited static accessor",
        "static B7 Write: kind mismatch"
    );
}
