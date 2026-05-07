//! `synthesize_fn_main` dispatch tree tests (T3-1: 12 reachable arms + body
//! emission semantics + defensive `unreachable!()` lock-ins) — extracted from
//! `tests/mod.rs` by the `/check_job deep deep` review structural fix
//! (2026-05-08) to keep `mod.rs` under the 1000-line file-line check threshold
//! while preserving all test contracts.

use super::*;

// === synthesize_fn_main (T3-1: 12 reachable dispatch arms + body-emission semantics) ====
//
// Tests cover the [`Transformer::synthesize_fn_main`] dispatch tree per the
// PRD Design section #2 + #5. Each of the 12 reachable arms of [`DispatchArm`]
// gets one representative test that pins down the (`is_async`, `attributes`,
// `name`, `body.len()`) shape of the returned Vec<Item>; body emission semantics
// (= INV-1 source-order preservation, MainStmt → IrStmt mapping totality, await
// wrapping) are tested separately to scope each invariant to one assertion.
//
// **Caller construction**: synthesize_fn_main is a method on
// `Transformer<'a>`; tests instantiate one via [`TctxFixture`] using a minimal
// dummy module source (`""`) since the method body does not depend on
// `Transformer` state — this keeps the fixture lifetime trivially short and
// the assertions focused on the input tuple alone.
//
// **Equivalence partitioning** drives the dispatch-arm test selection:
// - Library arms (cells 1 / 3 / 5 / 7): all 4 return empty Vec — 4 tests.
// - Exec sync arms (cells 11 / 13 / 17): 3 representative tests, sync fn main.
// - Exec async arms (cells 12 / 14 / 15 / 16 / 18): 5 representative tests,
//   `#[tokio::main] async fn main`.
// - Synthesis-suppressed arm: 1 test asserting Collision returns empty `Vec`
//   (T4-1 contract: collecting-mode reachability of Collision means the arm
//   cannot panic; the upstream namespace lint is the single contract surface
//   for surfacing the violation).

/// Builds a minimal Transformer instance for synthesize_fn_main invocation.
/// The fixture source is empty because synthesize_fn_main does not consult
/// any Transformer state — only the input tuple drives the dispatch.
fn synth_fixture() -> TctxFixture {
    TctxFixture::from_source("")
}

/// Convenience: builds an [`MainStmt::Expr`] holding a simple `Ident` IR expr
/// for shape assertions in dispatch-arm tests.
fn dummy_expr_main_stmt() -> MainStmt {
    MainStmt::Expr(IrExpr::Ident("x".to_string()))
}

/// Convenience: builds an [`MainStmt::ExprAwait`] holding a simple `Ident` IR
/// expr (the awaitee). Used to seed `has_top_level_await=true` arms.
fn dummy_expr_await_main_stmt() -> MainStmt {
    MainStmt::ExprAwait(IrExpr::Ident("p".to_string()))
}

/// Asserts that `items` contains exactly one `Item::Fn` with the documented
/// shape of the synthesized `fn main`.
///
/// - `expected_async`: matches `is_async` and the `tokio::main` attribute.
/// - `expected_body_len`: matches the number of statements in the synthesized
///   body (= length of `main_stmts` passed in).
fn assert_single_synthesized_fn_main(
    items: &[Item],
    expected_async: bool,
    expected_body_len: usize,
) {
    assert_eq!(
        items.len(),
        1,
        "synthesize_fn_main must return exactly one Item::Fn for executable arms"
    );
    let Item::Fn {
        vis,
        attributes,
        is_async,
        name,
        type_params,
        params,
        return_type,
        body,
    } = &items[0]
    else {
        panic!("expected Item::Fn, got {:?}", items[0]);
    };
    assert_eq!(*vis, Visibility::Private, "fn main must be private");
    assert_eq!(name, "main", "synthesized fn name must be `main`");
    assert!(type_params.is_empty(), "fn main has no type parameters");
    assert!(params.is_empty(), "fn main has no parameters");
    assert!(return_type.is_none(), "fn main returns ()");
    assert_eq!(*is_async, expected_async, "is_async mismatch");
    if expected_async {
        assert_eq!(
            attributes,
            &vec!["tokio::main".to_string()],
            "async dispatch must apply #[tokio::main]"
        );
    } else {
        assert!(
            attributes.is_empty(),
            "sync dispatch must not apply attributes; got {attributes:?}"
        );
    }
    assert_eq!(body.len(), expected_body_len, "body length mismatch");
}

// --- Library arms: all 4 return empty Vec<Item> --------------------------------------

#[test]
fn test_synthesize_library_none_returns_empty() {
    // cell 1 / 21 (LibraryNone)
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![], UserMainKind::None, false);
    assert!(items.is_empty(), "LibraryNone must emit no fn main");
}

#[test]
fn test_synthesize_library_fn_sync_direct_returns_empty() {
    // cell 3 / 23 (LibraryFnSyncDirect): user `function main()` is the binary
    // entry directly via transform_decl; synthesize_fn_main emits nothing.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![], UserMainKind::FnSync, false);
    assert!(items.is_empty(), "LibraryFnSyncDirect must emit no fn main");
}

#[test]
fn test_synthesize_library_fn_async_direct_returns_empty() {
    // cell 5 / 25 (LibraryFnAsyncDirect): user `async function main()` is the
    // binary entry directly; convert_fn_decl applies #[tokio::main] in its
    // existing path. synthesize_fn_main emits nothing.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![], UserMainKind::FnAsync, false);
    assert!(
        items.is_empty(),
        "LibraryFnAsyncDirect must emit no fn main"
    );
}

#[test]
fn test_synthesize_library_non_fn_returns_empty() {
    // cell 7 / 27 (LibraryNonFn): non-callable `main` (class / interface /
    // type alias / enum / namespace) preserved by transform_decl; no fn main.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![], UserMainKind::NonFn, false);
    assert!(items.is_empty(), "LibraryNonFn must emit no fn main");
}

// --- Executable sync arms (3): sync fn main, no attributes ---------------------------

#[test]
fn test_synthesize_exec_none_sync_emits_sync_fn_main() {
    // cell 11 / 31 / 71 (ExecNoneSync)
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![dummy_expr_main_stmt()], UserMainKind::None, false);
    assert_single_synthesized_fn_main(&items, /* async = */ false, /* body_len = */ 1);
}

#[test]
fn test_synthesize_exec_fn_sync_rename_emits_sync_fn_main() {
    // cell 13 / 33 / 73 (ExecFnSyncRename): user `function main()` (sync) is
    // renamed to __ts_main by the T3-2 emit path; synthesize_fn_main here
    // emits the binary-entry `fn main()` that wraps the captured exec stmts
    // (the rename + main() substitution is observed by the helper test
    // `test_axis_b_b1a_b_c_rename_dispatch_symmetric`).
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![dummy_expr_main_stmt()], UserMainKind::FnSync, false);
    assert_single_synthesized_fn_main(&items, /* async = */ false, /* body_len = */ 1);
}

#[test]
fn test_synthesize_exec_non_fn_sync_emits_sync_fn_main() {
    // cell 17 / 37 / 77 (ExecNonFnSync)
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![dummy_expr_main_stmt()], UserMainKind::NonFn, false);
    assert_single_synthesized_fn_main(&items, /* async = */ false, /* body_len = */ 1);
}

// --- Executable async arms (5): #[tokio::main] async fn main -------------------------

#[test]
fn test_synthesize_exec_none_async_emits_tokio_main_async_fn_main() {
    // cell 12 / 32 / 72 (ExecNoneAsync): top-await + no user main.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![dummy_expr_await_main_stmt()], UserMainKind::None, true);
    assert_single_synthesized_fn_main(&items, /* async = */ true, /* body_len = */ 1);
}

#[test]
fn test_synthesize_exec_fn_sync_rename_async_emits_tokio_main_async_fn_main() {
    // cell 14 / 34 / 74 (ExecFnSyncRenameAsync): sync user main + top-await
    // cohabitation. INV-3 (c) edge case — the synthesized fn main is
    // `#[tokio::main] async fn main()` regardless of user main's sync nature.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![dummy_expr_await_main_stmt()],
        UserMainKind::FnSync,
        true,
    );
    assert_single_synthesized_fn_main(&items, /* async = */ true, /* body_len = */ 1);
}

#[test]
fn test_synthesize_exec_fn_async_rename_emits_tokio_main_async_fn_main() {
    // cell 15 / 35 / 75 (ExecFnAsyncRename): async user main fires Trigger 1
    // (FnAsync) → async dispatch even with no top-await.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![dummy_expr_main_stmt()], UserMainKind::FnAsync, false);
    assert_single_synthesized_fn_main(&items, /* async = */ true, /* body_len = */ 1);
}

#[test]
fn test_synthesize_exec_fn_async_rename_async_emits_tokio_main_async_fn_main() {
    // cell 16 / 36 / 76 (ExecFnAsyncRenameAsync): Trigger 1 + Trigger 2
    // combined.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![dummy_expr_await_main_stmt()],
        UserMainKind::FnAsync,
        true,
    );
    assert_single_synthesized_fn_main(&items, /* async = */ true, /* body_len = */ 1);
}

#[test]
fn test_synthesize_exec_non_fn_async_emits_tokio_main_async_fn_main() {
    // cell 18 / 38 / 78 (ExecNonFnAsync)
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![dummy_expr_await_main_stmt()],
        UserMainKind::NonFn,
        true,
    );
    assert_single_synthesized_fn_main(&items, /* async = */ true, /* body_len = */ 1);
}

// --- Body emission semantic tests (boundary value + decision table) -----------------

#[test]
fn test_synthesize_main_stmts_to_ir_preserves_source_order() {
    // INV-1 source-order preservation: the order of MainStmts is the order of
    // IrStmts in the synthesized body. Three-element sequence detects any
    // accidental reverse / sort that a single-element body would not.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![
            MainStmt::Expr(IrExpr::Ident("a".to_string())),
            MainStmt::Expr(IrExpr::Ident("b".to_string())),
            MainStmt::Expr(IrExpr::Ident("c".to_string())),
        ],
        UserMainKind::None,
        false,
    );
    assert_eq!(items.len(), 1);
    let Item::Fn { body, .. } = &items[0] else {
        panic!("expected Item::Fn");
    };
    assert_eq!(body.len(), 3, "all 3 stmts must be preserved");
    let names: Vec<&str> = body
        .iter()
        .map(|stmt| match stmt {
            IrStmt::Expr(IrExpr::Ident(name)) => name.as_str(),
            other => panic!("expected IrStmt::Expr(Ident), got {other:?}"),
        })
        .collect();
    assert_eq!(names, ["a", "b", "c"], "source order must be preserved");
}

#[test]
fn test_synthesize_expr_await_main_stmt_wraps_in_ir_await() {
    // ExprAwait(inner) → IrStmt::Expr(IrExpr::Await(Box::new(inner))). The
    // awaitee is the bare operand; the IR Await wrapper is restored by the
    // emission helper (= MainStmt doc's "Await-variant invariant").
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![MainStmt::ExprAwait(IrExpr::Ident("p".to_string()))],
        UserMainKind::None,
        true,
    );
    let Item::Fn { body, .. } = &items[0] else {
        panic!("expected Item::Fn");
    };
    assert_eq!(body.len(), 1);
    match &body[0] {
        IrStmt::Expr(IrExpr::Await(inner)) => match inner.as_ref() {
            IrExpr::Ident(name) => assert_eq!(name, "p"),
            other => panic!("expected awaitee to be Ident, got {other:?}"),
        },
        other => panic!("expected IrStmt::Expr(IrExpr::Await(_)), got {other:?}"),
    }
}

#[test]
fn test_synthesize_let_main_stmt_emits_immutable_let_no_type() {
    // Let { name, init } → IrStmt::Let { mutable: false, ty: None, init: Some(init) }.
    // The captured local binding inside fn main body is immutable + uses Rust
    // type inference (no annotation), matching TS `const c = compute();`.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![MainStmt::Let {
            name: "c".to_string(),
            init: IrExpr::Ident("compute".to_string()),
        }],
        UserMainKind::None,
        false,
    );
    let Item::Fn { body, .. } = &items[0] else {
        panic!("expected Item::Fn");
    };
    assert_eq!(body.len(), 1);
    match &body[0] {
        IrStmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            assert!(!mutable, "captured `let` must be immutable");
            assert_eq!(name, "c");
            assert!(ty.is_none(), "no type annotation (Rust inference)");
            match init {
                Some(IrExpr::Ident(n)) => assert_eq!(n, "compute"),
                other => panic!("expected init = Some(Ident), got {other:?}"),
            }
        }
        other => panic!("expected IrStmt::Let, got {other:?}"),
    }
}

#[test]
fn test_synthesize_let_await_main_stmt_wraps_init_in_ir_await() {
    // LetAwait { name, init } → IrStmt::Let { init: Some(IrExpr::Await(Box::new(init))) }.
    // The bare-await invariant (the awaitee, not Expr::Await wrapper) is
    // preserved on the input side, and the helper restores the Await wrapper
    // around the init expression.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![MainStmt::LetAwait {
            name: "v".to_string(),
            init: IrExpr::Ident("p".to_string()),
        }],
        UserMainKind::None,
        true,
    );
    let Item::Fn { body, .. } = &items[0] else {
        panic!("expected Item::Fn");
    };
    assert_eq!(body.len(), 1);
    match &body[0] {
        IrStmt::Let {
            init: Some(IrExpr::Await(inner)),
            ..
        } => match inner.as_ref() {
            IrExpr::Ident(name) => assert_eq!(name, "p"),
            other => panic!("expected awaitee = Ident, got {other:?}"),
        },
        other => panic!("expected IrStmt::Let with init = Some(IrExpr::Await(_)), got {other:?}"),
    }
}

#[test]
fn test_synthesize_class_await_only_emits_async_fn_main_with_empty_body() {
    // I-228 main scope extension edge case: `class C extends f(await x) {}`
    // makes is_executable_mode=true via class_contains_await_recursive,
    // has_top_level_await=true via the same walker, but
    // collect_top_level_executions does NOT push to main_stmts (Class Decl
    // is the "declarations partition", emitted separately by transform_decl).
    // synthesize_fn_main must therefore correctly handle the
    // (main_stmts.is_empty(), has_top_level_await=true) input by deriving
    // is_executable_mode=true and routing to ExecNoneAsync — emitting an
    // empty-body `#[tokio::main] async fn main()`.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(
        vec![],
        UserMainKind::None,
        /* has_top_level_await */ true,
    );
    assert_single_synthesized_fn_main(&items, /* async = */ true, /* body_len = */ 0);
}

// --- Defensive contract tests (synthesis-suppressed + structurally-impossible) -------
//
// T4-1 contract revision: the Collision arm of `synthesize_fn_main` no longer
// panics — it suppresses synthesis (returns an empty `Vec`) so the function is
// safe to call from collecting mode where the upstream namespace lint
// accumulates the collision rather than aborting. The `(false, _, true)`
// dispatch-arm panic remains (= AST-level mutual-exclusion locked in by the
// SWC parser test suite).

#[test]
fn test_synthesize_emits_no_items_on_collision_arm() {
    // INV-5 + collecting-mode contract: `__ts_main` user identifier collision
    // is reported upstream by `Transformer::transform_module(_collecting)`'s
    // call to `scan_for_ts_namespace_collisions`. In collecting mode the lint
    // accumulates the error and the transform continues, so synthesize_fn_main
    // is reachable with UserMainKind::Collision. The Collision arm returns
    // `Vec::new()` (= synthesis suppressed; any captured `main_stmts` are
    // silently dropped) so the binary doesn't emit a `fn main` that conflicts
    // with the user's `__ts_*` identifier. The upstream lint remains the
    // contract surface for surfacing the violation to the user.
    let f = synth_fixture();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let items = t.synthesize_fn_main(vec![], UserMainKind::Collision, false);
    assert!(
        items.is_empty(),
        "Collision arm must suppress synthesis, got items: {items:?}"
    );

    // Also lock in: even with non-empty captured main_stmts and top-await flag,
    // the Collision arm still suppresses synthesis (= the captured payload is
    // silently dropped, regardless of axis A / axis C state).
    let items_payload = t.synthesize_fn_main(
        vec![MainStmt::Expr(IrExpr::Ident("x".to_string()))],
        UserMainKind::Collision,
        true,
    );
    assert!(
        items_payload.is_empty(),
        "Collision arm with non-empty payload must still suppress synthesis, got items: {items_payload:?}"
    );
}

#[test]
#[should_panic(expected = "Library mode + has_top_level_await=true is structurally impossible")]
fn classify_dispatch_arm_panics_on_library_mode_top_await_direct_call() {
    // Defensive structural lock-in for the `(false, FnSync|FnAsync|None|NonFn,
    // true)` arm of `classify_dispatch_arm`. AST mutual exclusion (locked in
    // by `tests/swc_parser_top_level_await_test.rs`) guarantees that no real
    // module body can produce `(is_executable_mode=false, has_top_level_await=true)`,
    // and `synthesize_fn_main` derives `is_executable_mode = !main_stmts.is_empty()
    // || has_top_level_await` so it can never propagate this combination
    // either. The `unreachable!()` macro is a defensive lock-in for any
    // future direct caller of `classify_dispatch_arm` that bypasses both
    // invariants.
    //
    // **Test rationale**: per Rule 11 (d-1) self-applied compliance, every
    // defensive `unreachable!()` arm requires a `#[should_panic]` test
    // proving the panic message is preserved across refactoring (e.g., a
    // future change that splits the combined `None | FnSync | FnAsync |
    // NonFn` pattern into separate arms must keep the same message).
    classify_dispatch_arm(false, UserMainKind::FnSync, true);
}
