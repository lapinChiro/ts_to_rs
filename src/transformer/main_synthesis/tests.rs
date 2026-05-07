use super::*;
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::transformer::test_fixtures::TctxFixture;

// --- Helpers for predicate-only tests (no Transformer instantiation) ---

fn parse(src: &str) -> Module {
    parse_typescript(src).expect("test source should parse")
}

/// Constructs a Transformer instance from `src` and runs `collect_top_level_executions`.
/// Returns `(main_stmts, user_main_kind, has_top_level_await)`.
fn collect(src: &str) -> (Vec<MainStmt>, UserMainKind, bool) {
    let f = TctxFixture::from_source(src);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    t.collect_top_level_executions(f.module())
        .expect("collect_top_level_executions should succeed for test fixtures")
}

// === classify_init_kind (Equivalence partitioning + Boundary) ============================

/// Returns the first concrete (= non-ambient, non-empty-init) `VarDecl` in the
/// module body. Skips `declare const x: T;` (ambient) and `let x;` (no-init)
/// so test sources can prefix declarations with `declare const ...` for type
/// context without polluting the predicate target.
fn first_var_decl(module: &Module) -> &VarDecl {
    for item in &module.body {
        if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) = item {
            if var.declare {
                continue;
            }
            if var.decls.first().is_none_or(|d| d.init.is_none()) {
                continue;
            }
            return var;
        }
    }
    panic!("expected at least one concrete (non-ambient, non-empty-init) Decl::Var");
}

#[test]
fn classify_init_kind_lit_number() {
    let m = parse("const x = 42;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::Lit);
}

#[test]
fn classify_init_kind_lit_string() {
    let m = parse("const x = 'hi';");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::Lit);
}

#[test]
fn classify_init_kind_lit_bool() {
    let m = parse("const x = true;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::Lit);
}

#[test]
fn classify_init_kind_lit_null() {
    let m = parse("const x = null;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::Lit);
}

#[test]
fn classify_init_kind_lit_negative_number() {
    let m = parse("const x = -42;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::Lit);
}

#[test]
fn classify_init_kind_side_effect_call() {
    let m = parse("declare function f(): number;\nconst x = f();");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::SideEffect);
}

#[test]
fn classify_init_kind_side_effect_ident() {
    let m = parse("declare const y: number;\nconst x = y;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::SideEffect);
}

#[test]
fn classify_init_kind_await_init() {
    let m = parse("declare const p: Promise<number>;\nconst x = await p;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::AwaitInit);
}

#[test]
fn classify_init_kind_unary_minus_non_lit_is_side_effect() {
    // Boundary: -<non-literal> is side-effect, not Lit (classify_init_kind only
    // collapses -Lit::Num/-Lit::BigInt to Lit; -<call> stays SideEffect).
    let m = parse("declare function f(): number;\nconst x = -f();");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::SideEffect);
}

#[test]
fn classify_init_kind_lit_regex_is_side_effect() {
    // T2 deep-review fix (PRD design-intent reconciliation): regex literal is NOT
    // Rust-const-compatible (`Regex::new(...)` is a runtime call), so it falls
    // through to InitKind::SideEffect → FnMainBodyCapture path → Rust-compilable
    // `let r = Regex::new("ab").unwrap();` emission. PRD line 970 bullet
    // enumeration mistakenly included Lit::Regex; the prefix's "Rust const 適合"
    // design intent wins, and Spec-stage cleanup is tracked alongside [I-228].
    let m = parse("const r = /ab/g;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::SideEffect);
}

#[test]
#[should_panic(expected = "TS Decl::Var requires init")]
fn classify_init_kind_panics_on_no_init_var() {
    // Defensive precondition test: classify_init_kind documents that callers must
    // filter no-init Var (`declare const x: T;`) via has_side_effect_init /
    // classify_decl_var_path defensive guards before calling. Bypassing those
    // guards (e.g., a future caller that constructs the VarDecl directly) must
    // trigger a loud `unreachable!` panic rather than a silent misclassification.
    // This test locks the panic message in.
    let m = parse("declare const x: number;");
    let var = match m.body.first() {
        Some(ModuleItem::Stmt(Stmt::Decl(Decl::Var(v)))) => v.as_ref(),
        other => panic!("test fixture mismatch — expected ambient Decl::Var, got {other:?}"),
    };
    // Sanity: confirm the precondition-violating shape.
    assert!(var.declare, "test fixture must be ambient (declare-marked)");
    assert!(
        var.decls.first().is_some_and(|d| d.init.is_none()),
        "test fixture must have no init",
    );
    // Bypass upstream defensive guards and call classify_init_kind directly —
    // expected to panic per docstring's "Panics" section.
    let _ = classify_init_kind(var);
}

// === has_side_effect_init ================================================================

#[test]
fn has_side_effect_init_lit_is_false() {
    let m = parse("const x = 42;");
    assert!(!has_side_effect_init(first_var_decl(&m)));
}

#[test]
fn has_side_effect_init_side_effect_is_true() {
    let m = parse("declare function f(): number;\nconst x = f();");
    assert!(has_side_effect_init(first_var_decl(&m)));
}

#[test]
fn has_side_effect_init_await_is_true() {
    // AwaitInit is treated as side-effect for executable-mode trigger purposes
    // (Axis C1 cells must reach is_executable_mode=true).
    let m = parse("declare const p: Promise<number>;\nconst x = await p;");
    assert!(has_side_effect_init(first_var_decl(&m)));
}

// === classify_decl_var_path (Decision Table) ==============================================

#[test]
fn classify_decl_var_path_library_lit() {
    let m = parse("const x = 42;");
    assert_eq!(
        classify_decl_var_path(first_var_decl(&m), false),
        DeclVarPath::LibraryMode
    );
}

#[test]
fn classify_decl_var_path_library_side_effect_unreachable_via_predicate() {
    // The (false, SideEffect) cell of the table maps to LibraryMode in code
    // even though `is_executable_mode` would return true for such a module —
    // reaching this combination requires bypassing the predicate (e.g., direct
    // unit test). We verify the table consistency here.
    let m = parse("declare function f(): number;\nconst x = f();");
    assert_eq!(
        classify_decl_var_path(first_var_decl(&m), false),
        DeclVarPath::LibraryMode
    );
}

#[test]
fn classify_decl_var_path_executable_lit_is_toplevel_const() {
    let m = parse("const x = 42;");
    assert_eq!(
        classify_decl_var_path(first_var_decl(&m), true),
        DeclVarPath::ToplevelConst
    );
}

#[test]
fn classify_decl_var_path_executable_side_effect_is_capture() {
    let m = parse("declare function f(): number;\nconst x = f();");
    assert_eq!(
        classify_decl_var_path(first_var_decl(&m), true),
        DeclVarPath::FnMainBodyCapture
    );
}

#[test]
fn classify_decl_var_path_executable_await_is_capture() {
    let m = parse("declare const p: Promise<number>;\nconst x = await p;");
    assert_eq!(
        classify_decl_var_path(first_var_decl(&m), true),
        DeclVarPath::FnMainBodyCapture
    );
}

// === is_executable_mode (AST variant exhaustiveness + Boundary) ==========================

#[test]
fn is_executable_mode_empty_module_is_library() {
    let m = parse("");
    assert!(!is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a0_declarations_only_is_library() {
    let m = parse(
        "function helper() { return 1; }\n\
         interface Box<T> { value: T; }\n\
         type Id = number;\n",
    );
    assert!(!is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a1_stmt_expr_is_executable() {
    let m = parse("console.log('hi');");
    assert!(is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a2_lit_init_only_is_library() {
    let m = parse("const x: number = 42;");
    assert!(!is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a3_side_effect_init_is_executable() {
    let m = parse("declare function f(): number;\nconst x = f();");
    assert!(is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a3_await_init_is_executable() {
    let m = parse("declare const p: Promise<number>;\nconst x = await p;");
    assert!(is_executable_mode(&m));
}

#[test]
fn is_executable_mode_regex_init_is_executable() {
    // T2 deep-review fix (cross-predicate consequence of L2-3 Lit::Regex narrow):
    // regex literal init is now classified as SideEffect (= `Regex::new()` runtime
    // call, not Rust-const-compatible), so has_side_effect_init returns true and
    // is_executable_mode triggers. This locks in the post-narrow exec-mode
    // behavior so any future widening of classify_init_kind that re-includes
    // Regex into Lit (= ToplevelConst routing → broken Rust emission) fails the
    // test.
    let m = parse("const r = /ab/g;");
    assert!(is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a5a_empty_only_is_library() {
    let m = parse(";");
    assert!(!is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a5b_debugger_only_is_library() {
    // Debugger does not contribute to executable_mode; T4-2 will reject the
    // module via transform_module_item independently of this predicate.
    let m = parse("debugger;");
    assert!(!is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a4_control_flow_only_is_library() {
    let m = parse("if (true) { }");
    assert!(!is_executable_mode(&m));
}

#[test]
fn is_executable_mode_a6_mixed_a1_a2_is_executable() {
    let m = parse("const x: number = 0;\nconsole.log(x);");
    assert!(is_executable_mode(&m));
}

#[test]
fn is_executable_mode_module_decl_only_is_library() {
    // Imports / exports do not contribute to executable_mode (Axis E orthogonal).
    let m = parse("import { x } from './m';\nexport const y = 1;");
    assert!(!is_executable_mode(&m));
}

// === detect_user_main (B-axis exhaustive) ================================================

#[test]
fn detect_user_main_b0_no_main() {
    let m = parse("function helper() { return 1; }");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_b1_function_decl_sync() {
    let m = parse("function main(): void { }");
    assert_eq!(detect_user_main(&m), UserMainKind::FnSync);
}

#[test]
fn detect_user_main_b1_const_arrow_sync() {
    let m = parse("const main = (): void => { };");
    assert_eq!(detect_user_main(&m), UserMainKind::FnSync);
}

#[test]
fn detect_user_main_b1_const_fn_expr_sync() {
    let m = parse("const main = function(): void { };");
    assert_eq!(detect_user_main(&m), UserMainKind::FnSync);
}

#[test]
fn detect_user_main_b2_function_decl_async() {
    let m = parse("async function main(): Promise<void> { }");
    assert_eq!(detect_user_main(&m), UserMainKind::FnAsync);
}

#[test]
fn detect_user_main_b2_const_arrow_async() {
    let m = parse("const main = async (): Promise<void> => { };");
    assert_eq!(detect_user_main(&m), UserMainKind::FnAsync);
}

#[test]
fn detect_user_main_b2_const_fn_expr_async() {
    let m = parse("const main = async function(): Promise<void> { };");
    assert_eq!(detect_user_main(&m), UserMainKind::FnAsync);
}

#[test]
fn detect_user_main_b3_const_non_callable() {
    let m = parse("const main = 42;");
    assert_eq!(detect_user_main(&m), UserMainKind::NonFn);
}

#[test]
fn detect_user_main_b3_class() {
    let m = parse("class main { }");
    assert_eq!(detect_user_main(&m), UserMainKind::NonFn);
}

#[test]
fn detect_user_main_b3_interface() {
    let m = parse("interface main { x: number; }");
    assert_eq!(detect_user_main(&m), UserMainKind::NonFn);
}

#[test]
fn detect_user_main_b3_type_alias() {
    let m = parse("type main = number;");
    assert_eq!(detect_user_main(&m), UserMainKind::NonFn);
}

#[test]
fn detect_user_main_b3_enum() {
    let m = parse("enum main { A, B }");
    assert_eq!(detect_user_main(&m), UserMainKind::NonFn);
}

#[test]
fn detect_user_main_b3_namespace() {
    let m = parse("namespace main { export const x = 1; }");
    assert_eq!(detect_user_main(&m), UserMainKind::NonFn);
}

#[test]
fn detect_user_main_b4_function_ts_main() {
    let m = parse("function __ts_main(): void { }");
    assert_eq!(detect_user_main(&m), UserMainKind::Collision);
}

// ---- Ambient (`declare`-marked) decls: B0 (no rename target) ----
//
// Ambient declarations introduce no Rust runtime construct, so they are not
// B-axis triggers per the PRD's "function main with body" definition (B1 / B2
// require a body; B3 requires a runtime construct). Ambient `__ts_main` is
// rejected separately by the namespace lint (`scan_for_ts_namespace_collisions`)
// regardless of detect_user_main's output.

#[test]
fn detect_user_main_declare_function_main_is_b0() {
    let m = parse("declare function main(): void;");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_declare_async_function_main_is_b0() {
    let m = parse("declare function main(): Promise<void>;");
    // `declare` strips the `async` keyword in TS source (declarations cannot be
    // async-marked); SWC parses the body-less form as is_async=false. Either
    // way, ambient = B0.
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_declare_const_main_is_b0() {
    let m = parse("declare const main: () => void;");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_declare_class_main_is_b0() {
    let m = parse("declare class main { }");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_declare_enum_main_is_b0() {
    let m = parse("declare enum main { A }");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_declare_namespace_main_is_b0() {
    let m = parse("declare namespace main { }");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_export_declare_function_main_is_b0() {
    // export-wrapped ambient: same B0 classification.
    let m = parse("export declare function main(): void;");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_declare_function_ts_main_is_b0_at_dispatch_level() {
    // Ambient `__ts_main` is rejected by the namespace lint upstream; here we
    // verify dispatch-level classification only. Per `is_ambient_decl`'s
    // docstring "Collision precedence note", the dispatch tree never sees the
    // source (lint rejects first), so detect_user_main returning B0 is
    // consistent with namespace-lint semantics.
    let m = parse("declare function __ts_main(): void;");
    assert_eq!(detect_user_main(&m), UserMainKind::None);
}

#[test]
fn detect_user_main_interface_main_is_b3_regardless_of_declare_keyword() {
    // Interface is always type-only; `declare` keyword is redundant. Both
    // forms classify as B3 NonFn (TS-namespace `main` symbol exists, no
    // runtime collision because Rust value/type namespaces are disjoint).
    let m = parse("declare interface main { x: number; }");
    assert_eq!(detect_user_main(&m), UserMainKind::NonFn);
}

#[test]
fn detect_user_main_b4_const_ts_main() {
    let m = parse("const __ts_main = 1;");
    assert_eq!(detect_user_main(&m), UserMainKind::Collision);
}

#[test]
fn detect_user_main_b4_class_ts_main() {
    let m = parse("class __ts_main { }");
    assert_eq!(detect_user_main(&m), UserMainKind::Collision);
}

#[test]
fn detect_user_main_export_function_main() {
    let m = parse("export function main(): void { }");
    assert_eq!(detect_user_main(&m), UserMainKind::FnSync);
}

#[test]
fn detect_user_main_export_default_named_main() {
    let m = parse("export default function main(): void { }");
    assert_eq!(detect_user_main(&m), UserMainKind::FnSync);
}

#[test]
fn detect_user_main_collision_takes_precedence_over_main() {
    // Source has both `function main()` (B1 FnSync) and `function __ts_main()`
    // (B4 Collision). Collision must win regardless of source order.
    let m = parse(
        "function main(): void { }\n\
         function __ts_main(): void { }\n",
    );
    assert_eq!(detect_user_main(&m), UserMainKind::Collision);
}

#[test]
fn detect_user_main_collision_takes_precedence_when_collision_first() {
    // Reverse source order: Collision still wins.
    let m = parse(
        "function __ts_main(): void { }\n\
         function main(): void { }\n",
    );
    assert_eq!(detect_user_main(&m), UserMainKind::Collision);
}

#[test]
fn detect_user_main_other_ts_prefix_is_not_b4() {
    // `__ts_helper` is rejected by the namespace lint (T1-2), but is **not** a
    // B-axis Collision (the Collision arm is specifically about __ts_main, the
    // rename target).
    let m = parse(
        "function __ts_helper(): void { }\n\
         function main(): void { }\n",
    );
    assert_eq!(detect_user_main(&m), UserMainKind::FnSync);
}

// === classify_dispatch_arm (Decision Table all 13 arms) ==================================

#[test]
fn classify_dispatch_arm_all_reachable_arms() {
    // 13 reachable arms (= 12 leaves + 1 Collision), each verified with a
    // representative tuple value. The 4 unreachable tuples (false, *, true)
    // for non-Collision UserMainKind are tested separately via #[should_panic].
    let cases: &[(bool, UserMainKind, bool, DispatchArm)] = &[
        // Collision arm absorbs (_, Collision, _) — 4 representative tuples.
        (
            false,
            UserMainKind::Collision,
            false,
            DispatchArm::Collision,
        ),
        (false, UserMainKind::Collision, true, DispatchArm::Collision),
        (true, UserMainKind::Collision, false, DispatchArm::Collision),
        (true, UserMainKind::Collision, true, DispatchArm::Collision),
        // Library mode (false, *, false).
        (false, UserMainKind::None, false, DispatchArm::LibraryNone),
        (
            false,
            UserMainKind::FnSync,
            false,
            DispatchArm::LibraryFnSyncDirect,
        ),
        (
            false,
            UserMainKind::FnAsync,
            false,
            DispatchArm::LibraryFnAsyncDirect,
        ),
        (false, UserMainKind::NonFn, false, DispatchArm::LibraryNonFn),
        // Executable mode + no top-await.
        (true, UserMainKind::None, false, DispatchArm::ExecNoneSync),
        (
            true,
            UserMainKind::FnSync,
            false,
            DispatchArm::ExecFnSyncRename,
        ),
        (
            true,
            UserMainKind::FnAsync,
            false,
            DispatchArm::ExecFnAsyncRename,
        ),
        (true, UserMainKind::NonFn, false, DispatchArm::ExecNonFnSync),
        // Executable mode + top-await.
        (true, UserMainKind::None, true, DispatchArm::ExecNoneAsync),
        (
            true,
            UserMainKind::FnSync,
            true,
            DispatchArm::ExecFnSyncRenameAsync,
        ),
        (
            true,
            UserMainKind::FnAsync,
            true,
            DispatchArm::ExecFnAsyncRenameAsync,
        ),
        (true, UserMainKind::NonFn, true, DispatchArm::ExecNonFnAsync),
    ];
    for (exec, kind, await_, expected) in cases {
        let actual = classify_dispatch_arm(*exec, *kind, *await_);
        assert_eq!(
            actual, *expected,
            "tuple ({exec}, {kind:?}, {await_}): expected {expected:?}, got {actual:?}",
        );
    }
}

#[test]
#[should_panic(expected = "Library mode + has_top_level_await=true is structurally impossible")]
fn classify_dispatch_arm_library_mode_top_await_panics_none() {
    let _ = classify_dispatch_arm(false, UserMainKind::None, true);
}

#[test]
#[should_panic(expected = "Library mode + has_top_level_await=true is structurally impossible")]
fn classify_dispatch_arm_library_mode_top_await_panics_fn_sync() {
    let _ = classify_dispatch_arm(false, UserMainKind::FnSync, true);
}

#[test]
#[should_panic(expected = "Library mode + has_top_level_await=true is structurally impossible")]
fn classify_dispatch_arm_library_mode_top_await_panics_fn_async() {
    let _ = classify_dispatch_arm(false, UserMainKind::FnAsync, true);
}

#[test]
#[should_panic(expected = "Library mode + has_top_level_await=true is structurally impossible")]
fn classify_dispatch_arm_library_mode_top_await_panics_non_fn() {
    let _ = classify_dispatch_arm(false, UserMainKind::NonFn, true);
}

// === collect_top_level_executions (representative cells) ================================
//
// The full 80-cell coverage lives in `tests/i224_helper_test.rs::
// test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`. The unit tests below
// exercise each MainStmt variant + a representative axis combination per
// dispatch arm to lock in the helper's structural invariants.

#[test]
fn collect_cell_1_library_no_main() {
    // (A0, B0, C0): library mode, declarations only.
    let (stmts, kind, awaitf) = collect("function helper(): number { return 7; }");
    assert!(
        stmts.is_empty(),
        "library mode produces no main_stmts: {stmts:?}"
    );
    assert_eq!(kind, UserMainKind::None);
    assert!(!awaitf);
}

#[test]
fn collect_cell_3_library_user_sync_main_directly() {
    // (A0, B1, C0): user sync main, declarations only.
    let (stmts, kind, awaitf) = collect("function main(): void { }");
    assert!(stmts.is_empty());
    assert_eq!(kind, UserMainKind::FnSync);
    assert!(!awaitf);
}

#[test]
fn collect_cell_5_library_user_async_main_directly() {
    // (A0, B2, C0): user async main, declarations only.
    let (stmts, kind, awaitf) = collect("async function main(): Promise<void> { }");
    assert!(stmts.is_empty());
    assert_eq!(kind, UserMainKind::FnAsync);
    assert!(!awaitf);
}

#[test]
fn collect_cell_11_exec_no_main_stmt_expr() {
    // (A1, B0, C0): top-level Stmt::Expr, no user main.
    let (stmts, kind, awaitf) = collect("console.log('hi');");
    assert_eq!(stmts.len(), 1);
    assert!(matches!(stmts[0], MainStmt::Expr(_)));
    assert_eq!(kind, UserMainKind::None);
    assert!(!awaitf);
}

#[test]
fn collect_cell_12_exec_no_main_top_await() {
    // (A1, B0, C1): top-level Await Stmt::Expr.
    let (stmts, kind, awaitf) = collect("declare const p: Promise<number>;\nawait p;");
    assert_eq!(stmts.len(), 1);
    assert!(
        matches!(stmts[0], MainStmt::ExprAwait(_)),
        "expected ExprAwait, got {:?}",
        stmts[0]
    );
    assert_eq!(kind, UserMainKind::None);
    assert!(awaitf);
}

#[test]
fn collect_cell_31_exec_decl_var_side_effect_capture() {
    // (A3, B0, C0): Decl::Var with side-effect init captured into MainStmt::Let.
    let (stmts, kind, awaitf) = collect("declare function f(): number;\nconst c = f();");
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        MainStmt::Let { name, .. } => assert_eq!(name, "c"),
        other => panic!("expected MainStmt::Let, got {other:?}"),
    }
    assert_eq!(kind, UserMainKind::None);
    assert!(!awaitf);
}

#[test]
fn collect_cell_32_exec_decl_var_await_init_capture() {
    // (A3, B0, C1): Decl::Var with await init captured into MainStmt::LetAwait.
    let (stmts, kind, awaitf) =
        collect("declare function f(): Promise<number>;\nconst c = await f();");
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        MainStmt::LetAwait { name, .. } => assert_eq!(name, "c"),
        other => panic!("expected MainStmt::LetAwait, got {other:?}"),
    }
    assert_eq!(kind, UserMainKind::None);
    assert!(awaitf);
}

#[test]
fn collect_cell_71_exec_a6_mixed_source_order_preservation() {
    // (A6, B0, C0): mixed top-level — Lit Decl::Var hoisted to top-level (not in
    // main_stmts), side-effect Decl::Var + Stmt::Expr captured in source order.
    let src = "declare function compute(): number;\n\
               console.log('a');\n\
               const X: number = 1;\n\
               const c = compute();\n\
               console.log('b');\n";
    let (stmts, kind, awaitf) = collect(src);
    // X (Lit init) goes to ToplevelConst path = NOT in main_stmts.
    // console.log('a'), c = compute(), console.log('b') = 3 entries in source order.
    assert_eq!(stmts.len(), 3, "expected 3 main_stmts, got {stmts:?}");
    assert!(
        matches!(stmts[0], MainStmt::Expr(_)),
        "stmts[0] = {:?}",
        stmts[0]
    );
    assert!(
        matches!(stmts[1], MainStmt::Let { ref name, .. } if name == "c"),
        "stmts[1] = {:?}",
        stmts[1]
    );
    assert!(
        matches!(stmts[2], MainStmt::Expr(_)),
        "stmts[2] = {:?}",
        stmts[2]
    );
    assert_eq!(kind, UserMainKind::None);
    assert!(!awaitf);
}

#[test]
fn collect_cell_13_exec_user_sync_main_with_stmt_expr() {
    // (A1, B1, C0): user sync main + top-level Stmt::Expr.
    let src = "function main(): void { }\nconsole.log('hi');";
    let (stmts, kind, awaitf) = collect(src);
    assert_eq!(stmts.len(), 1);
    assert!(matches!(stmts[0], MainStmt::Expr(_)));
    assert_eq!(kind, UserMainKind::FnSync);
    assert!(!awaitf);
}

#[test]
fn collect_cell_19_exec_collision_user_main_with_stmt_expr() {
    // (A1, B4, C0): user `__ts_main` collision detected; main_stmts still
    // captured for collecting-mode partial output (the dispatch-tree Collision
    // arm will reject this in T3, but the helper itself just records facts).
    let src = "function __ts_main(): void { }\nconsole.log('hi');";
    let (stmts, kind, _awaitf) = collect(src);
    assert_eq!(kind, UserMainKind::Collision);
    // `console.log('hi')` is captured (Collision priority is enforced at the
    // dispatch-tree level, not at the capture level).
    assert_eq!(stmts.len(), 1);
    assert!(matches!(stmts[0], MainStmt::Expr(_)));
}
