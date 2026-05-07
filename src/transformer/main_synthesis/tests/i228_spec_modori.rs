//! I-224 Spec stage 逆戻り Iteration v8 tests for the 4 I-228 sub-entries
//! (nested top-level await / Lit::Regex narrow / ExportDecl-wrapped Decl::Var
//! side-effect / multi-declarator iter ANY-rule) plus the Layer 3 extension
//! findings (Object computed property key, Class shape outer-context await).
//!
//! Section comments group tests by the originating sub-entry / extension so
//! a future regression maps back to the fix scope. Extracted from `tests.rs`
//! to keep individual files under the 1000-line file-line check threshold.

use super::*;

// ===== I-228 Spec stage 逆戻り batch fix tests (2026-05-07) =====================
//
// Spec stage 逆戻り (`spec-first-prd.md` 「Spec への逆戻り」procedure) で 4
// sub-entries (I-228 main + I-228-b/c/d) の Spec gap を全 fix。本セクションの
// tests は revised spec の structural lock-in (= future regression を fail で
// 検出) を提供する。

// ---- I-228 main: nested top-level await detection (recursive walker) ----

#[test]
fn has_top_level_await_nested_in_call_args() {
    // `console.log(await getNum())` — outer Stmt::Expr is Call (not Await).
    // Pre-fix returned false (= AST shape direct miss); post-fix recursive walker
    // returns true.
    let m = parse(
        "async function getNum(): Promise<number> { return 42; }\n\
         function show(n: number): void { }\n\
         show(await getNum());\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_decl_var_init() {
    // `const c = double(await getNum())` — outer Decl::Var.init is Call (not Await).
    let m = parse(
        "async function getNum(): Promise<number> { return 42; }\n\
         function double(n: number): number { return n * 2; }\n\
         const c: number = double(await getNum());\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_unary() {
    // `const c = -await x` — outer is Unary, inner is Await.
    let m = parse(
        "declare const x: Promise<number>;\n\
         const c: number = -await x;\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_paren() {
    // `(await x);` — outer is Paren, inner is Await.
    let m = parse(
        "declare const x: Promise<number>;\n\
         (await x);\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_ts_as() {
    // `await x as number` — TsAs wraps Await.
    let m = parse(
        "declare const x: Promise<number>;\n\
         (await x as number);\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_bin_op() {
    // `await x + 1` — Bin's left operand is Await.
    let m = parse(
        "declare const x: Promise<number>;\n\
         (await x + 1);\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_array_element() {
    // `[await x, 2]` — Array element is Await.
    let m = parse(
        "declare const x: Promise<number>;\n\
         [await x, 2];\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_object_keyvalue_value() {
    // `({ k: await x });` — Object KeyValue's value is Await.
    let m = parse(
        "declare const x: Promise<number>;\n\
         ({ k: await x });\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_object_keyvalue_computed_key() {
    // `({ [await x]: 1 });` — Object KeyValue's key is computed with Await.
    // Computed keys are evaluated at the outer (enclosing) context, so this is
    // a top-level await trigger. (`/check_job` Layer 3 finding 2026-05-07:
    // initial walker missed computed keys; subsequent fix added recursion.)
    let m = parse(
        "declare const x: Promise<number>;\n\
         ({ [await x]: 1 });\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_object_method_computed_key() {
    // `({ [await x](): void {} });` — Object Method's key is computed with Await.
    // The method body is a boundary (= nested async context), but the computed
    // key IS evaluated in the outer context, so this triggers top-level await.
    let m = parse(
        "declare const x: Promise<number>;\n\
         ({ [await x](): void {} });\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_skips_object_method_body() {
    // Method body is a boundary: `await` inside the body does NOT trigger
    // top-level await. Static key + body await = no detection.
    let m = parse(
        "declare const x: Promise<number>;\n\
         ({ async k(): Promise<void> { await x; } });\n",
    );
    assert!(!has_top_level_await(&m));
}

#[test]
fn has_top_level_await_nested_in_object_spread() {
    // `({ ...await x });` — Object Spread of await result.
    let m = parse(
        "declare const x: Promise<{ a: number }>;\n\
         ({ ...await x });\n",
    );
    assert!(has_top_level_await(&m));
}

// ---- Class shape: super_class / decorators / member computed keys ----
//
// I-228 main scope extension (2026-05-07): Decl::Class / Expr::Class were
// previously treated as full boundaries by the walker, missing await reachable
// in **outer-context** sub-Exprs (super_class call args, decorators, member
// computed keys). Method bodies remain a boundary (= separate async context).

#[test]
fn has_top_level_await_decl_class_super_class_await() {
    // `class C extends makeBase(await getBaseTag()) {}` — bare top-level Class
    // declaration with await inside the super_class expression. The class
    // definition runs makeBase at module-load time in the outer async context.
    let m = parse(
        "async function getBaseTag(): Promise<number> { return 1; }\n\
         function makeBase(t: number) { return class { tag = t; }; }\n\
         class C extends makeBase(await getBaseTag()) {}\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn is_executable_mode_decl_class_super_class_await_is_executable() {
    // Same fixture as above — super_class await also triggers executable mode
    // (= dispatch tree consistency: has_top_level_await=true requires
    // is_executable_mode=true to avoid the structurally-unreachable arm panic).
    let m = parse(
        "async function getBaseTag(): Promise<number> { return 1; }\n\
         function makeBase(t: number) { return class { tag = t; }; }\n\
         class C extends makeBase(await getBaseTag()) {}\n",
    );
    assert!(is_executable_mode(&m));
}

#[test]
fn has_top_level_await_class_skips_method_body() {
    // Method body is a boundary: `await` inside the method body does NOT trigger
    // top-level await. (= cf. nested function body skip test above.)
    let m = parse(
        "declare const x: Promise<number>;\n\
         class C { async m(): Promise<void> { await x; } }\n",
    );
    assert!(!has_top_level_await(&m));
}

#[test]
fn has_top_level_await_export_class_super_class_await() {
    // ExportDecl-wrapped Class with super_class await — same outer-context
    // detection as bare Decl::Class case.
    let m = parse(
        "async function getTag(): Promise<number> { return 1; }\n\
         function makeBase(t: number) { return class { tag = t; }; }\n\
         export class C extends makeBase(await getTag()) {}\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_class_member_computed_key_await() {
    // `class C { [await x](): void {} }` — member's computed property key
    // contains await at the outer (class-definition) context.
    let m = parse(
        "declare const x: Promise<string>;\n\
         class C { [await x](): void {} }\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn has_top_level_await_class_skips_property_initializer() {
    // `class C { p: number = await x; }` — invalid TS (`await` rejected in class
    // field initializer regardless of `is_static`: non-static runs in instance
    // construction sync context, static runs in class-definition sync context).
    // Walker conservatively skips ClassProp.value to avoid over-detection on
    // SWC-parseable but tsc-invalid sources. Test asserts `false` to lock in the
    // skip behavior.
    let m = parse(
        "declare const x: Promise<number>;\n\
         class C { p: number = await x; }\n",
    );
    assert!(!has_top_level_await(&m));
}

#[test]
fn has_top_level_await_class_simple_no_await_is_false() {
    // Sanity: class without any outer-context await stays library mode.
    let m = parse(
        "class Base {}\n\
         class C extends Base { p: number = 1; }\n",
    );
    assert!(!has_top_level_await(&m));
    assert!(!is_executable_mode(&m));
}

#[test]
fn has_top_level_await_skips_nested_function_body() {
    // Boundary check: `await` inside a function body is NOT a top-level await
    // (= different async context). The recursive walker stops at Fn / Arrow /
    // Class boundaries.
    let m = parse(
        "declare const x: Promise<number>;\n\
         async function helper(): Promise<number> { return await x; }\n\
         helper();\n",
    );
    assert!(!has_top_level_await(&m));
}

#[test]
fn has_top_level_await_skips_arrow_body() {
    // Same boundary check for arrow function.
    let m = parse(
        "declare const x: Promise<number>;\n\
         const helper = async (): Promise<number> => await x;\n\
         helper();\n",
    );
    assert!(!has_top_level_await(&m));
}

// ---- I-228-c: ExportDecl-wrapped Decl::Var with side-effect init ----

#[test]
fn is_executable_mode_export_decl_side_effect_init_is_executable() {
    // `export const c = compute();` — ExportDecl-wrapped Decl::Var with
    // side-effect init. Pre-fix returned false (= ModuleDecl skipped); post-fix
    // recognizes as Axis A3 trigger.
    let m = parse(
        "function compute(): number { return 7; }\n\
         export const c: number = compute();\n",
    );
    assert!(is_executable_mode(&m));
}

#[test]
fn is_executable_mode_export_decl_lit_init_is_library() {
    // `export const X = 1;` — Lit init is library-mode-orthogonal (= existing
    // path emits `pub const X: f64 = 1.0;` correctly).
    let m = parse("export const X: number = 1;\n");
    assert!(!is_executable_mode(&m));
}

#[test]
fn has_top_level_await_export_decl_await_init() {
    // `export const c = await fetch();` — ExportDecl-wrapped await init.
    let m = parse(
        "declare function fetchData(): Promise<number>;\n\
         export const c: number = await fetchData();\n",
    );
    assert!(has_top_level_await(&m));
}

#[test]
fn is_executable_mode_export_decl_fn_is_library() {
    // `export function f() {}` — Fn declaration via ExportDecl is library-mode-
    // orthogonal (= no side effect).
    let m = parse("export function f(): void { }\n");
    assert!(!is_executable_mode(&m));
}

// ---- I-228-d: multi-declarator VarDecl with mixed init ----

#[test]
fn classify_init_kind_multi_declarator_mixed_lit_and_side_effect_is_side_effect() {
    // `const a = 1, b = compute();` — first Lit, second SideEffect.
    // ANY-rule precedence (I-228-d fix): SideEffect since ANY non-Lit triggers.
    let m = parse(
        "function compute(): number { return 7; }\n\
         const a: number = 1, b: number = compute();\n",
    );
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::SideEffect);
}

#[test]
fn classify_init_kind_multi_declarator_all_lit_is_lit() {
    // `const a = 1, b = 2;` — all Lit, classify as Lit.
    let m = parse("const a: number = 1, b: number = 2;\n");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::Lit);
}

#[test]
fn classify_init_kind_multi_declarator_with_await_is_await_init() {
    // `const a = 1, b = await fetch();` — second declarator has await.
    // ANY-rule: AwaitInit (highest precedence).
    let m = parse(
        "declare function fetchData(): Promise<number>;\n\
         const a: number = 1, b: number = await fetchData();\n",
    );
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::AwaitInit);
}

#[test]
fn is_executable_mode_multi_declarator_first_lit_second_side_effect_is_executable() {
    // `const a = 1, b = compute();` — second declarator triggers exec mode.
    // Pre-fix: first-only check classified as Lit → library mode (wrong).
    // Post-fix: ANY-rule classifies as SideEffect → exec mode (correct).
    let m = parse(
        "function compute(): number { return 7; }\n\
         const a: number = 1, b: number = compute();\n",
    );
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
