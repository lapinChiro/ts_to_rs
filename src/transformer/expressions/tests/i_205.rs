//! I-205 Iteration v9 (T5 Read context dispatch + B7 traversal helper) lock-in tests.
//!
//! Spec stage Problem Space matrix の Read context (A1) cells に対し、
//! `resolve_member_access` の dispatch arm が ideal output を emit することを verify する。
//!
//! Cells 1, 10 は B1 field / B9 unknown の fallback regression (FieldAccess preserve)、
//! Cells 2-5/9 は Tier 1 dispatch arm (MethodCall / FnCall::UserAssocFn)、
//! Cells 4/7/8 は Tier 2 honest error reclassify (UnsupportedSyntaxError)。
//!
//! 各 test は cell 単位の matrix mapping を test name に明示。`Transformer::for_module(...)
//! .convert_expr(&swc_expr)` で direct invoke、IR Expr を assert_eq! で token-level verify。
//! Tier 2 path は `convert_expr` の Err を `downcast::<UnsupportedSyntaxError>` で kind verify。

use super::*;

use crate::ir::{CallTarget, Expr, MethodKind, RustType, UserTypeRef};
use crate::registry::{MethodSignature, ParamDef, TypeDef, TypeRegistry};
use crate::transformer::UnsupportedSyntaxError;

// -----------------------------------------------------------------------------
// Cell 1: A1 Read × B1 (regular field) → fallback FieldAccess (regression)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_1_b1_field_read_emits_field_access() {
    // Matrix cell 1: receiver type に method 登録なし、field のみ → FieldAccess
    let src = "class Foo { x: number = 0; }\nconst f = new Foo();\nconst v = f.x;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 2);
    let tctx = fx.tctx();
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect("cell 1 must succeed (B1 fallback)");
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("f".to_string())),
            field: "x".to_string(),
        },
        "cell 1 B1 field: must emit FieldAccess (no MethodCall dispatch)"
    );
}

// -----------------------------------------------------------------------------
// Cell 2: A1 Read × B2 (getter only、Copy T) → MethodCall { method: x, args: [] }
// -----------------------------------------------------------------------------

#[test]
fn test_cell_2_b2_getter_only_copy_read_emits_method_call() {
    // Matrix cell 2: getter only (B2)、return type number (Copy) → `f.x()` MethodCall
    let src = "class Foo { get x(): number { return 42; } }\n\
               const f = new Foo();\n\
               const v = f.x;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 2);
    let tctx = fx.tctx();
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect("cell 2 must succeed (B2 getter dispatch)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("f".to_string())),
            method: "x".to_string(),
            args: vec![],
        },
        "cell 2 B2 getter Copy: must emit MethodCall `f.x()`"
    );
}

// -----------------------------------------------------------------------------
// Cell 3: A1 Read × B2 (getter only、non-Copy T = String) → MethodCall (body clone は T12 で)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_3_b2_getter_only_string_read_emits_method_call() {
    // Matrix cell 3: getter only、return String (non-Copy) → `f.x()` (.clone() insertion は T12)
    let src = "class Foo { get x(): string { return \"abc\"; } }\n\
               const f = new Foo();\n\
               const v = f.x;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 2);
    let tctx = fx.tctx();
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect("cell 3 must succeed (B2 getter String)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("f".to_string())),
            method: "x".to_string(),
            args: vec![],
        },
        "cell 3 B2 getter non-Copy: dispatch arm は同 (clone insertion は body emit 側 = T12)"
    );
}

// -----------------------------------------------------------------------------
// Cell 4: A1 Read × B3 (setter only) → Tier 2 honest error "read of write-only property"
// -----------------------------------------------------------------------------

#[test]
fn test_cell_4_b3_setter_only_read_emits_unsupported_syntax_error() {
    // Matrix cell 4: setter only (B3)、Read 試行は Tier 2 honest error
    let src = "class Box { _v: number = 0; set x(v: number) { this._v = v; } }\n\
               const b = new Box();\n\
               const v = b.x;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 2);
    let tctx = fx.tctx();
    let err = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect_err("cell 4 must Err (B3 read of write-only)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("cell 4: error must be UnsupportedSyntaxError");
    assert_eq!(
        usx.kind, "read of write-only property",
        "cell 4 B3: kind mismatch"
    );
}

// -----------------------------------------------------------------------------
// Cell 5: A1 Read × B4 (getter + setter、non-Copy T) → MethodCall (Read = getter dispatch)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_5_b4_getter_setter_pair_read_dispatches_to_getter() {
    // Matrix cell 5: getter + setter pair (B4)、Read context は getter dispatch
    let src = "class Foo { _v: string = \"init\"; \
               get x(): string { return this._v; } \
               set x(v: string) { this._v = v; } }\n\
               const f = new Foo();\n\
               const v = f.x;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 2);
    let tctx = fx.tctx();
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect("cell 5 must succeed (B4 getter dispatch)");
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("f".to_string())),
            method: "x".to_string(),
            args: vec![],
        },
        "cell 5 B4 getter+setter Read: must dispatch to getter (MethodCall)"
    );
}

// -----------------------------------------------------------------------------
// Cell 7: A1 Read × B6 (method-as-fn-reference no-paren) → Tier 2 honest error
// -----------------------------------------------------------------------------

#[test]
fn test_cell_7_b6_method_as_fn_reference_no_paren_emits_unsupported_syntax_error() {
    // Matrix cell 7: regular method の no-paren reference (`const f = obj.method;`) は
    // Rust 上 closure / fn ptr 表現が必要 = 別 architectural concern (= "Function reference
    // semantic"、別 PRD I-209) で扱う、本 PRD では Tier 2 honest error reclassify
    let src = "class Foo { greet(): number { return 1; } }\n\
               const f = new Foo();\n\
               const fn_ref = f.greet;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 2);
    let tctx = fx.tctx();
    let err = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect_err("cell 7 must Err (B6 method-as-fn-ref)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("cell 7: error must be UnsupportedSyntaxError");
    assert_eq!(
        usx.kind, "method-as-fn-reference (no-paren)",
        "cell 7 B6: kind mismatch"
    );
}

// -----------------------------------------------------------------------------
// Cell 8: A1 Read × B7 (inherited getter) → Tier 2 honest error "inherited accessor access"
// -----------------------------------------------------------------------------

#[test]
fn test_cell_8_b7_inherited_getter_read_emits_unsupported_syntax_error() {
    // Matrix cell 8: parent class の getter を sub class instance で読む = inherited
    // accessor access。Rust struct inheritance 不在のため、別 architectural concern
    // (= "Class inheritance dispatch"、別 PRD I-206) で Tier 1 化、本 PRD では Tier 2
    // honest error reclassify。Iteration v9 で extends 登録 (class.rs:195) を fix した
    // ことで本 dispatch arm が機能する。
    let src = "class Base { _n: number = 42; get x(): number { return this._n; } }\n\
               class Sub extends Base {}\n\
               const s = new Sub();\n\
               const v = s.x;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 3);
    let tctx = fx.tctx();
    let err = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect_err("cell 8 must Err (B7 inherited)");
    let usx = err
        .downcast::<UnsupportedSyntaxError>()
        .expect("cell 8: error must be UnsupportedSyntaxError");
    assert!(
        usx.kind.starts_with("inherited accessor access"),
        "cell 8 B7: kind must start with \"inherited accessor access\", got: {}",
        usx.kind
    );
}

// -----------------------------------------------------------------------------
// Cell 9: A1 Read × B8 (static getter) → FnCall { UserAssocFn { Class, x } }
// -----------------------------------------------------------------------------

#[test]
fn test_cell_9_b8_static_getter_read_emits_associated_fn_call() {
    // Matrix cell 9: static getter (B8) → `Config::version()` associated fn call
    // Static-only class (instance method 不在) のため、receiver = Ident(Config) で
    // is_interface = false な TypeDef::Struct lookup が hit、static dispatch arm 経由
    let src = "class Config { static get version(): string { return \"1.0.0\"; } }\n\
               const v = Config.version;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 1);
    let tctx = fx.tctx();
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect("cell 9 must succeed (B8 static getter)");
    assert_eq!(
        result,
        Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new("Config"),
                method: "version".to_string(),
            },
            args: vec![],
        },
        "cell 9 B8 static getter: must emit FnCall::UserAssocFn"
    );
}

// -----------------------------------------------------------------------------
// Cell 10: A1 Read × B9 (unknown receiver type) → fallback FieldAccess (regression)
// -----------------------------------------------------------------------------

#[test]
fn test_cell_10_b9_unknown_receiver_field_read_emits_field_access() {
    // Matrix cell 10: receiver type が registry に登録されていない → fallback FieldAccess
    // 既存 dispatch path (B1 FieldAccess) を維持、本 PRD で挙動変更なし regression lock
    // `obj` Ident は registry 不在 (= type unknown)、`get_expr_type` も None or Any 系
    let src = "const obj: any = null;\nconst v = obj.x;";
    let fx = TctxFixture::from_source(src);
    let module = fx.module();
    let v_init = extract_var_init_at(module, 1);
    let tctx = fx.tctx();
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&v_init)
        .expect("cell 10 must succeed (B9 unknown fallback)");
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "x".to_string(),
        },
        "cell 10 B9 unknown: must emit FieldAccess fallback"
    );
}

// -----------------------------------------------------------------------------
// Write context regression: Read dispatch logic must NOT leak into LHS conversion
// (Iteration v9 deep deep review で発覚した silent regression の structural lock-in)
// -----------------------------------------------------------------------------

#[test]
fn test_write_context_lhs_does_not_leak_read_dispatch() {
    // Iteration v9 deep deep review で発覚した critical bug の regression lock-in:
    // 本 T5 で導入した Read context dispatch logic (`resolve_member_access` の
    // class member dispatch) が `convert_member_expr_for_write` (= assignment LHS
    // conversion) にも leak すると、`f.x = 5;` の LHS が `f.x()` (MethodCall) に
    // 変換され `f.x() = 5.0;` (invalid Rust LHS、compile error) を emit する silent
    // regression が発生する。本 fix で `convert_member_expr_inner` の Ident path で
    // `for_write=true` 時 Read dispatch を skip、既存 FieldAccess fallback を維持。
    // setter dispatch (`f.set_x(5.0)`) は subsequent T6 で別途実装。
    //
    // 本 test は B4 (getter+setter pair) class の `f.x = 5;` で **LHS は FieldAccess**
    // を維持することを direct verify。
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
         leak regression check)、setter dispatch は subsequent T6 で実装"
    );
}

// -----------------------------------------------------------------------------
// B7 traversal helper cycle-safety regression (degenerate case)
// -----------------------------------------------------------------------------

#[test]
fn test_b7_traversal_cycle_does_not_infinite_loop() {
    // Direct registry-level test: cycle (A extends B / B extends A) を構築した state で
    // lookup_method_sigs_in_inheritance_chain が None を return し infinite loop しない
    // ことを verify (visited HashSet による cycle prevention)。
    let mut reg = TypeRegistry::new();
    reg.register(
        "A".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec!["B".to_string()],
            is_interface: false,
        },
    );
    reg.register(
        "B".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec!["A".to_string()],
            is_interface: false,
        },
    );
    let result = reg.lookup_method_sigs_in_inheritance_chain("A", "missing_field");
    assert!(
        result.is_none(),
        "cycle A↔B with no method must return None (no infinite loop), got: {result:?}"
    );
}

// -----------------------------------------------------------------------------
// Single-step inheritance positive case: parent has method, child does not
// -----------------------------------------------------------------------------

#[test]
fn test_b7_traversal_single_step_returns_inherited_flag() {
    // Direct registry-level test: A extends B、B has getter `y`、A から lookup すると
    // is_inherited = true を返すことを verify (B7 detection の core mechanism)。
    let mut reg = TypeRegistry::new();
    let mut b_methods = std::collections::HashMap::new();
    b_methods.insert(
        "y".to_string(),
        vec![MethodSignature {
            params: vec![],
            return_type: Some(RustType::F64),
            has_rest: false,
            type_params: vec![],
            kind: MethodKind::Getter,
        }],
    );
    reg.register(
        "B".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: b_methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );
    reg.register(
        "A".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec!["B".to_string()],
            is_interface: false,
        },
    );
    let (sigs, is_inherited) = reg
        .lookup_method_sigs_in_inheritance_chain("A", "y")
        .expect("expected hit on inherited getter");
    assert!(is_inherited, "must mark as inherited (parent class)");
    assert_eq!(sigs.len(), 1, "must return single signature");
    assert_eq!(
        sigs[0].kind,
        MethodKind::Getter,
        "must preserve MethodKind::Getter from parent"
    );
}

// -----------------------------------------------------------------------------
// Multi-step inheritance: A extends B extends C, C has method (boundary value: N>=2 step)
// -----------------------------------------------------------------------------

#[test]
fn test_b7_traversal_multi_step_inheritance_returns_inherited_flag() {
    // Direct registry-level test: 3-class chain A → B → C で C has setter `w`、A から
    // lookup すると **N=2 step propagation** を経由して is_inherited = true を返す。
    // Boundary value analysis (testing.md "Recursive Function Termination" + "Boundary
    // Value Analysis"): single-step (N=1) と multi-step (N>=2) は recursive traversal の
    // boundary が異なるため独立 test 必須。
    // Iteration v9 second-review で boundary value analysis 観点で追加 (Review insight #2 fix)。
    let mut reg = TypeRegistry::new();
    let mut c_methods = std::collections::HashMap::new();
    c_methods.insert(
        "w".to_string(),
        vec![MethodSignature {
            params: vec![ParamDef {
                name: "v".to_string(),
                ty: RustType::F64,
                optional: false,
                has_default: false,
            }],
            return_type: None,
            has_rest: false,
            type_params: vec![],
            kind: MethodKind::Setter,
        }],
    );
    reg.register(
        "C".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: c_methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );
    reg.register(
        "B".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec!["C".to_string()],
            is_interface: false,
        },
    );
    reg.register(
        "A".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec!["B".to_string()],
            is_interface: false,
        },
    );
    let (sigs, is_inherited) = reg
        .lookup_method_sigs_in_inheritance_chain("A", "w")
        .expect("expected hit on grand-parent (N=2 step) setter");
    assert!(
        is_inherited,
        "N=2 step inheritance must mark as inherited (= grand-parent class)"
    );
    assert_eq!(sigs.len(), 1, "must return single signature");
    assert_eq!(
        sigs[0].kind,
        MethodKind::Setter,
        "must preserve MethodKind::Setter from grand-parent C through B → A"
    );
}

// -----------------------------------------------------------------------------
// Direct hit (not inherited): same class has method
// -----------------------------------------------------------------------------

#[test]
fn test_b7_traversal_direct_hit_returns_not_inherited() {
    // Direct registry-level test: class A has getter `z`、A から lookup すると
    // is_inherited = false を返す (direct hit、B1-B6/B8/B9 dispatch path の前提)。
    let mut reg = TypeRegistry::new();
    let mut a_methods = std::collections::HashMap::new();
    a_methods.insert(
        "z".to_string(),
        vec![MethodSignature {
            params: vec![],
            return_type: Some(RustType::String),
            has_rest: false,
            type_params: vec![],
            kind: MethodKind::Getter,
        }],
    );
    reg.register(
        "A".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: a_methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );
    let (_sigs, is_inherited) = reg
        .lookup_method_sigs_in_inheritance_chain("A", "z")
        .expect("expected direct hit");
    assert!(!is_inherited, "direct hit must mark as not inherited");
}
