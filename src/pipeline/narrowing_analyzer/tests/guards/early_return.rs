//! Early-return complement narrow tests + OptChain compound narrow
//! tests for the narrowing analyzer.
//!
//! Split from the parent `guards` module to keep each file under the
//! per-file line budget. Shares `MockCtx` / `run_guard` / `variant`
//! with the positive-guard suite via [`super`].

use swc_common::Spanned;

use crate::ir::RustType;
use crate::parser::parse_typescript;
use crate::pipeline::narrowing_analyzer::{
    detect_early_return_narrowing, NarrowTrigger, NullCheckKind, PrimaryTrigger,
};

use swc_ecma_ast as ast;

use super::{run_guard, variant, MockCtx};

#[test]
fn early_return_null_check_narrows_fallthrough_scope() {
    // `if (x === null) return;` followed by code — the fall-through
    // should narrow `x` to the Option's inner type.
    let source = r#"
        function foo(x: string | null) {
            if (x === null) { return; }
            console.log(x);
        }
    "#;
    // Find the if-stmt inside the function body.
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(_)
    ));
    assert_eq!(ev.scope_start, if_end);
    assert_eq!(ev.scope_end, block_end);
}

#[test]
fn early_return_bang_truthy_narrows_fallthrough_scope() {
    let source = r#"
        function foo(x: string | null) {
            if (!x) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy)
    ));
}

#[test]
fn early_return_bang_peeks_through_ts_as_assertion() {
    // T6 P3a: `if (!(x as T)) return;` must narrow identically to `if (!x)`
    // because `as` is a runtime no-op. Without peek-through, the Bang arm
    // fell through to the catchall and pushed no event, leaving post-if
    // `x` typed as `Option<T>` even though TS narrows it to `T`.
    let source = r#"
        function foo(x: string | null) {
            if (!(x as string | null)) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy)
    ));
}

#[test]
fn early_return_bang_peeks_through_ts_non_null_assertion() {
    // T6 P3a: `if (!(x!)) return;` — TsNonNull is type-only; the AST has
    // `x!` parsed as TsNonNull(Ident(x)). The peeled operand is `x` so
    // narrowing fires identically to `if (!x)`.
    let source = r#"
        function foo(x: string | null) {
            if (!(x!)) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy)
    ));
}

#[test]
fn early_return_bang_optchain_narrows_base_via_optchain_invariant() {
    // T6 P3b: `if (!x?.v) return;` → on the fall-through `x` is non-null.
    // The OptChain invariant: if `x` were null, `x?.v` short-circuits to
    // `undefined`, `!undefined` is `true`, so the early-exit fires and
    // we never reach the fall-through. Reaching it therefore proves
    // `x` non-null. The narrow targets the BASE `x`, not the field.
    let source = r#"
        function foo(x: { v: number } | null) {
            if (!x?.v) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let inner = RustType::Named {
        name: "Payload".into(),
        type_args: vec![],
    };
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(inner.clone())));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert_eq!(*ev.narrowed_type, inner);
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant)
    ));
    assert_eq!(ev.scope_start, if_end);
    assert_eq!(ev.scope_end, block_end);
}

#[test]
fn early_return_bang_optchain_with_paren_peeks_through_to_optchain_branch() {
    // T6 P3a + P3b cohesion: `if (!(x?.v)) return;` — outer Paren is
    // peeled, inner OptChain still triggers base-narrow.
    let source = r#"
        function foo(x: { v: number } | null) {
            if (!(x?.v)) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let inner = RustType::Named {
        name: "Payload".into(),
        type_args: vec![],
    };
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(inner.clone())));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert_eq!(*ev.narrowed_type, inner);
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant)
    ));
}

#[test]
fn early_return_bang_optchain_on_non_option_base_is_no_op() {
    // T6 P3b: only Option<T> bases narrow. If `x` is already a plain
    // struct (no Option), `if (!x?.v) return;` cannot narrow the base
    // any further — no event should be pushed.
    let source = r#"
        function foo(x: { v: number }) {
            if (!x?.v) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new().with_var(
        "x",
        RustType::Named {
            name: "Payload".into(),
            type_args: vec![],
        },
    );
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert!(
        ctx.events.is_empty(),
        "non-Option base must not push narrow"
    );
}

#[test]
fn early_return_typeof_narrows_fallthrough_to_complement() {
    // `if (typeof x === "string") return;` — fall-through has x as the
    // complement type (F64 here) via `EarlyReturnComplement`.
    let source = r#"
        function foo(x: string | number) {
            if (typeof x === "string") { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "StringOrF64".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "StringOrF64",
            vec![
                variant("String", RustType::String),
                variant("F64", RustType::F64),
            ],
        );
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::F64));
    assert!(matches!(
        &ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::TypeofGuard(s)) if s == "string"
    ));
}

#[test]
fn early_return_instanceof_narrows_fallthrough_to_complement() {
    // `if (x instanceof Error) return;` — fall-through has x as the
    // complement (String here).
    let source = r#"
        function foo(x: Error | string) {
            if (x instanceof Error) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "ErrorOrStr".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "ErrorOrStr",
            vec![
                variant(
                    "Error",
                    RustType::Named {
                        name: "Error".into(),
                        type_args: vec![],
                    },
                ),
                variant("String", RustType::String),
            ],
        );
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        &ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::InstanceofGuard(n)) if n == "Error"
    ));
}

#[test]
fn early_return_skips_empty_fallthrough_scope() {
    // if_end >= block_end → detector must be a no-op, no events.
    let source = r#"
        function foo(x: string | null) {
            if (x === null) { return; }
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    // Simulate a zero-width fall-through by placing block_end AT if_end.
    let block_end = if_end;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert!(
        ctx.events.is_empty(),
        "empty fall-through range must produce no events"
    );
}

// -----------------------------------------------------------------------------
// OptChain compound narrowing (T6-4)
// -----------------------------------------------------------------------------

#[test]
fn optchain_neq_undefined_narrows_base_to_inner_type() {
    // `x?.v !== undefined` → narrow x from Option<Struct> to Struct in cons.
    let inner = RustType::Named {
        name: "Payload".into(),
        type_args: vec![],
    };
    let ctx = run_guard(
        r#"if (x?.v !== undefined) { a(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(inner.clone()))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().expect("Narrow event");
    assert_eq!(ev.var_name, "x");
    assert_eq!(*ev.narrowed_type, inner);
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::OptChainInvariant)
    ));
}

#[test]
fn optchain_eq_undefined_narrows_base_in_alt_branch() {
    // `x?.v === undefined` → narrow x in ALT (else), not cons.
    let inner = RustType::Named {
        name: "Payload".into(),
        type_args: vec![],
    };
    let ctx = run_guard(
        r#"if (x?.v === undefined) { a(); } else { b(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(inner.clone()))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert_eq!(*ev.narrowed_type, inner);
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::OptChainInvariant)
    ));
}

#[test]
fn optchain_reversed_undefined_neq_chain_narrows_base() {
    // `undefined !== x?.v` (reversed order) → narrow x in cons.
    let ctx = run_guard(
        r#"if (undefined !== x?.v) { a(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::F64))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert!(matches!(ev.narrowed_type, RustType::F64));
}

#[test]
fn optchain_on_non_option_is_no_op() {
    // x is String (not Option) — no narrowing should fire.
    let ctx = run_guard(
        r#"if (x?.v !== undefined) { a(); }"#,
        MockCtx::new().with_var("x", RustType::String),
    );
    assert!(
        ctx.events.is_empty(),
        "non-Option base must not generate an OptChain narrow"
    );
}

#[test]
fn optchain_deep_chain_narrows_outermost_base() {
    // `x?.a?.b !== undefined` → narrow x (outermost base).
    let inner = RustType::Named {
        name: "Outer".into(),
        type_args: vec![],
    };
    let ctx = run_guard(
        r#"if (x?.a?.b !== undefined) { a(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(inner.clone()))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert_eq!(*ev.narrowed_type, inner);
}

#[test]
fn optchain_null_rhs_also_narrows_base() {
    // `x?.v !== null` → narrow x in cons. (null, not undefined)
    let ctx = run_guard(
        r#"if (x?.v !== null) { a(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::F64))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert!(matches!(ev.narrowed_type, RustType::F64));
}

#[test]
fn optchain_loose_neq_null_also_narrows_base() {
    // `x?.v != null` (loose !=) → narrow x in cons.
    // JS loose `!= null` covers both null and undefined.
    let ctx = run_guard(
        r#"if (x?.v != null) { a(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::F64))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert!(matches!(ev.narrowed_type, RustType::F64));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::OptChainInvariant)
    ));
}

#[test]
fn optchain_bare_ident_takes_precedence_over_optchain() {
    // `x !== undefined` (bare ident, not OptChain) → should fire null check, not OptChain.
    let ctx = run_guard(
        r#"if (x !== undefined) { a(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    // Trigger must be NullCheck, not OptChainInvariant
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::NotEqEqUndefined))
    ));
}

#[test]
fn optchain_early_return_eq_undefined_narrows_fallthrough() {
    // `if (x?.v === undefined) { return; }` → x is non-null after
    let source = r#"
        function foo(x: { v: number } | null) {
            if (x?.v === undefined) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let inner = RustType::Named {
        name: "Payload".into(),
        type_args: vec![],
    };
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(inner.clone())));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert_eq!(*ev.narrowed_type, inner);
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::OptChainInvariant)
    ));
    assert_eq!(ev.scope_start, if_end);
    assert_eq!(ev.scope_end, block_end);
}

#[test]
fn optchain_and_compound_narrows_both_vars() {
    // `x?.v !== undefined && y !== null` → both x and y narrowed in cons.
    let inner = RustType::Named {
        name: "Payload".into(),
        type_args: vec![],
    };
    let ctx = run_guard(
        r#"if (x?.v !== undefined && y !== null) { a(); }"#,
        MockCtx::new()
            .with_var("x", RustType::Option(Box::new(inner.clone())))
            .with_var("y", RustType::Option(Box::new(RustType::String))),
    );
    assert_eq!(ctx.events.len(), 2);
    let ev_x = ctx.events[0].as_narrow().unwrap();
    let ev_y = ctx.events[1].as_narrow().unwrap();
    assert_eq!(ev_x.var_name, "x");
    assert_eq!(*ev_x.narrowed_type, inner);
    assert!(matches!(
        ev_x.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::OptChainInvariant)
    ));
    assert_eq!(ev_y.var_name, "y");
    assert!(matches!(ev_y.narrowed_type, RustType::String));
    assert!(matches!(
        ev_y.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::NullCheck(_))
    ));
}
