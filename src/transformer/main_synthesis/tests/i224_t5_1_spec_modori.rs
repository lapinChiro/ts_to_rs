//! I-224 T5-1 Spec stage 逆戻り 2026-05-08 — `NonTrigger` partition split into
//! `NonTriggerDef` (Arrow / Fn / Class) and `NonTriggerData` (Object / Array)
//! plus the corresponding `classify_per_decl_path` routing extension that
//! lifts `(true, NonTriggerData)` from `LibraryMode` (silent drop via
//! `convert_var_decl_module_level`'s `_ => continue` fallback) to
//! `FnMainBodyCapture` (= `let` binding inside the synthesized fn main body).
//!
//! **Lesson source**: cell-12 / cell-24 e2e empirical probe in T5-1
//! (`tests/e2e/scripts/i-224/cell-12-stmt-expr-with-non-fn-main.ts` and
//! `cell-24-decl-var-with-non-fn-main.ts`) revealed that
//! `const v: T = { ... };` followed by a top-level `console.log(v.x)` produced
//! Rust output where `v` was silently dropped (= the existing
//! `convert_var_decl_module_level` path fell through `_ => continue` for
//! Object / Array literal init), leaving the `println!("{}", v.value)` body
//! referencing an undefined `v` and producing E0425 "cannot find value `v`
//! in this scope" downstream. This is a Tier 1 silent semantic loss
//! (= the runtime-visible TS binding is dropped while the consumer code is
//! kept). Per `conversion-correctness-priority.md` and
//! `ideal-implementation-primacy.md`, the structural fix (NonTriggerData →
//! `FnMainBodyCapture` in executable mode) was applied as the cleanest
//! resolution.
//!
//! Section comments mirror the I-228 sub-module convention (lesson source
//! anchor + structural lock-in goal) so future regressions map back to the
//! Spec-stage 逆戻り decision.

use super::*;

// ===== InitKind partition lock-in (single-decl coverage) =======================
//
// `/check_job deep deep` 2026-05-08 migration: routing assertions previously
// used the legacy `classify_decl_var_path` (VarDecl-level aggregating
// classifier). After the deep review removed the legacy classifier entirely
// in favor of the per-declarator `classify_per_decl_path`, these tests
// assert only the InitKind partition (per-init kind classification, not
// routing). The routing decision table is locked in by the
// `classify_per_decl_path_*` tests in the next section, which exercise the
// actual production routing helper.

#[test]
fn classify_init_kind_object_literal_with_consumer_is_non_trigger_data() {
    // cell-12 fixture pattern: `const v: main = { value: 42 };` followed by
    // `console.log(v.value);`. The InitKind partition is NonTriggerData; the
    // routing under per-decl is `FnMainBodyCapture` (= verified by the
    // `classify_per_decl_path_executable_object_is_fn_main_body_capture` test
    // in the routing section below).
    let m = parse(
        "interface main { value: number; }\n\
         const v: main = { value: 42 };\n\
         console.log(v.value);",
    );
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::NonTriggerData);
}

#[test]
fn classify_init_kind_array_literal_with_consumer_is_non_trigger_data() {
    // Symmetric Array literal counterpart. The InitKind partition is
    // NonTriggerData regardless of executable-mode context.
    let m = parse("const xs = [1, 2, 3];\nconsole.log(xs);");
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::NonTriggerData);
}

#[test]
fn classify_init_kind_ts_as_object_with_consumer_is_non_trigger_data() {
    // Type-only wrapper recursive walk: `const x = { a: 1, b: 2 } as const;`
    // classified as NonTriggerData via the inner Object literal — the wrapper
    // is purely a TS-level annotation with no runtime effect.
    let m = parse(
        "const x = { a: 1, b: 2 } as const;\n\
         console.log(x.a);",
    );
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::NonTriggerData);
}

#[test]
fn classify_init_kind_arrow_with_consumer_is_non_trigger_def() {
    // NonTriggerDef partition lock-in for Arrow declarators with downstream
    // usage. Per-decl routing keeps this as LibraryMode (= top-level
    // Item::Fn) even in executable mode, verified by
    // `classify_per_decl_path_executable_arrow_is_library_mode` below.
    let m = parse(
        "const f = (x: number) => x + 1;\n\
         console.log(f(2));",
    );
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::NonTriggerDef);
}

#[test]
fn classify_init_kind_class_expression_with_consumer_is_non_trigger_def() {
    // Class expression NonTriggerDef partition lock-in. Routing under
    // per-decl is LibraryMode (= top-level class shape), even though Class
    // expressions are silently dropped by the existing
    // `convert_var_decl_module_level` `_ => continue` fall-through (= a
    // separate pre-existing I-016 owner concern, orthogonal to T5-1
    // NonTriggerData split).
    let m = parse(
        "const C = class { foo(): number { return 1; } };\n\
         console.log(new C().foo());",
    );
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::NonTriggerDef);
}

// ===== `has_side_effect_init` invariant: NonTriggerData does not trigger ========

#[test]
fn has_side_effect_init_non_trigger_data_object_remains_false() {
    // **Critical invariant**: NonTriggerData MUST NOT trigger executable
    // mode by itself. A pure data module like `const Phase = { S: 1, B: 2 };`
    // (no Stmt::Expr, no SideEffect Var) stays in library mode → silent drop
    // (pre-existing I-016 owner). The T5-1 fix only changes the routing
    // **when** other sources have already triggered executable mode; the
    // trigger-detection predicate stays orthogonal to the routing.
    //
    // Regression scenario (= what this test prevents): a future widening
    // that classifies Object literal as SideEffect would falsely trigger
    // exec mode for pure-data modules, synthesizing an empty fn main and
    // breaking library-mode build outputs.
    let m = parse("const x = { a: 1, b: 2 };");
    assert!(!has_side_effect_init(first_var_decl(&m)));
}

#[test]
fn has_side_effect_init_non_trigger_data_array_remains_false() {
    let m = parse("const xs = [1, 2, 3];");
    assert!(!has_side_effect_init(first_var_decl(&m)));
}

#[test]
fn has_side_effect_init_non_trigger_data_ts_as_const_remains_false() {
    // The original typeof_const fixture pattern. Lock in the regression
    // boundary post-T5-1 split: TsAs(Object) is NonTriggerData but NOT a
    // trigger.
    let m = parse("const Phase = { S: 1, B: 2, R: 3 } as const;");
    assert!(!has_side_effect_init(first_var_decl(&m)));
}

// ===== Per-declarator routing (`classify_per_decl_path`) lock-in ================
//
// **/check_job deep review structural fix 2026-05-08**: replaces the
// VarDecl-level routing (`classify_decl_var_path`) with per-declarator
// routing (`classify_per_decl_path`) for mixed Def+Data multi-declarator
// VarDecls. Each declarator's emission path is decided independently,
// enabling Arrow → top-level `Item::Fn` AND Object → fn-main-body `let`
// binding to coexist in the same VarDecl (= the architecturally cleanest
// resolution of the L3 review insight).

#[test]
fn classify_per_decl_path_executable_arrow_is_library_mode() {
    // Single-declarator Arrow in executable mode: per-decl routing returns
    // LibraryMode (= top-level Item::Fn via convert_arrow_var_decl). Symmetric
    // with `classify_per_decl_path_executable_function_def_is_library_mode`
    // (= the migrated test in mod.rs) but at the per-declarator granularity.
    let m = parse("const f = () => 1;\nconsole.log(f());");
    let var = first_var_decl(&m);
    let decl = var.decls.first().expect("single declarator");
    assert_eq!(
        classify_per_decl_path(decl, true, var.declare),
        DeclVarPath::LibraryMode
    );
}

#[test]
fn classify_per_decl_path_executable_object_is_fn_main_body_capture() {
    // Single-declarator Object literal in executable mode: per-decl routing
    // returns FnMainBodyCapture (= MainStmt::Let inside fn main body).
    let m = parse(
        "interface Foo { a: number; }\n\
         const x: Foo = { a: 1 };\n\
         console.log(x.a);",
    );
    let var = first_var_decl(&m);
    let decl = var.decls.first().expect("single declarator");
    assert_eq!(
        classify_per_decl_path(decl, true, var.declare),
        DeclVarPath::FnMainBodyCapture
    );
}

#[test]
fn classify_per_decl_path_mixed_def_and_data_routes_independently() {
    // `const f = () => 1, x = { a: 1 };` mixed Def+Data multi-declarator.
    // Per-decl routing: Arrow → LibraryMode (top-level Item::Fn), Object →
    // FnMainBodyCapture (fn main body let). The two declarators route
    // **independently** despite sharing the same VarDecl — this is the
    // architectural improvement over the pre-fix VarDecl-level routing
    // (which forced both into FnMainBodyCapture via NonTriggerData
    // precedence, losing Arrow's top-level Item::Fn emission).
    let m = parse("const f = () => 1, x = { a: 1 };\nconsole.log(x.a);");
    let var = first_var_decl(&m);
    let arrow_decl = var.decls.first().expect("first declarator (Arrow)");
    let object_decl = var.decls.get(1).expect("second declarator (Object)");
    assert_eq!(
        classify_per_decl_path(arrow_decl, true, var.declare),
        DeclVarPath::LibraryMode,
        "Arrow declarator must keep LibraryMode (top-level Item::Fn) routing"
    );
    assert_eq!(
        classify_per_decl_path(object_decl, true, var.declare),
        DeclVarPath::FnMainBodyCapture,
        "Object declarator must take FnMainBodyCapture (fn main body let) routing"
    );
}

#[test]
fn classify_per_decl_path_library_mode_disables_capture() {
    // Library mode (`is_executable_mode = false`): per-decl routing always
    // returns LibraryMode regardless of init kind. Mirrors the VarDecl-level
    // (false, _) → LibraryMode arm.
    let m = parse("const x = { a: 1 };");
    let var = first_var_decl(&m);
    let decl = var.decls.first().expect("single declarator");
    assert_eq!(
        classify_per_decl_path(decl, false, var.declare),
        DeclVarPath::LibraryMode
    );
}

#[test]
fn classify_per_decl_path_declare_marked_is_library_mode() {
    // Ambient (`declare const x: T;`): no init expression to capture, all
    // declarators route to LibraryMode regardless of executable mode.
    // (The TS parser allows `declare` only without init, so this exercises
    // the ambient guard at the per-decl helper.)
    let m = parse("declare const x: number;");
    let var = first_var_decl_or_declare(&m);
    let decl = var.decls.first().expect("single declarator");
    assert_eq!(
        classify_per_decl_path(decl, true, var.declare),
        DeclVarPath::LibraryMode
    );
}

#[test]
fn classify_per_decl_path_no_init_declarator_is_library_mode() {
    // Multi-declarator `let a = 1, b;` second declarator has no init →
    // LibraryMode (no fn-main-body capture).
    let m = parse("let a = 1, b: number;\nconsole.log(a);");
    let var = first_var_decl(&m);
    let no_init_decl = var.decls.get(1).expect("second declarator (no init)");
    assert_eq!(
        classify_per_decl_path(no_init_decl, true, var.declare),
        DeclVarPath::LibraryMode
    );
}

/// Local helper: returns the first VarDecl regardless of the `declare` flag,
/// since `first_var_decl` (in mod.rs) skips declare-marked / no-init Vars to
/// keep the module-level partition tests focused on concrete inits.
fn first_var_decl_or_declare(module: &swc_ecma_ast::Module) -> &swc_ecma_ast::VarDecl {
    use swc_ecma_ast::{Decl, ModuleItem, Stmt};
    for item in &module.body {
        if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) = item {
            return var;
        }
    }
    panic!("expected at least one Decl::Var in module body");
}

// ===== Destructuring var decl Tier 2 honest reject (`/check_problem` 2026-05-08) ====
//
// Module-level destructuring var decls (`const { a } = compute();` /
// `const [a] = arr;`) with executable-mode trigger were previously silently
// dropped by `capture_var_decl_into_main_stmts`'s `else { continue; }` guard
// against `Pat::Ident`. Per `conversion-correctness-priority.md` Tier 1
// silent semantic loss is the highest-priority defect class; the
// `/check_problem` review surfaced this as Tier 2 honest error so user
// intent visibility is preserved (= proper destructuring pattern conversion
// is deferred to a separate architectural concern, but silent loss is
// eliminated).

#[test]
fn destructuring_object_pattern_with_capture_init_returns_unsupported_error() {
    // `const { a } = compute();` at module level + `Stmt::Expr` exec trigger:
    // the Var Decl's Object init route via `try_capture_module_item_into_main_stmts`
    // hits the destructuring guard in `capture_var_decl_into_main_stmts` and
    // returns `UnsupportedSyntaxError` instead of silently dropping.
    let source = "declare function compute(): { a: number };\n\
                  const { a } = compute();\n\
                  console.log(a);";
    let m = parse(source);
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let mut main_stmts = Vec::new();
    let var = first_var_decl(&m);
    let result = t.capture_var_decl_into_main_stmts(var, true, &mut main_stmts);
    assert!(
        result.is_err(),
        "destructuring object pattern with capture-bound init must return UnsupportedSyntaxError"
    );
    let err_str = format!("{:#}", result.unwrap_err());
    assert!(
        err_str.contains("destructuring") || err_str.to_lowercase().contains("destructuring"),
        "error message must mention destructuring, got: {err_str}"
    );
}

#[test]
fn destructuring_array_pattern_with_capture_init_returns_unsupported_error() {
    // Symmetric Array pattern counterpart: `const [a, b] = compute();`.
    let source = "declare function compute(): [number, number];\n\
                  const [a, b] = compute();\n\
                  console.log(a, b);";
    let m = parse(source);
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let mut main_stmts = Vec::new();
    let var = first_var_decl(&m);
    let result = t.capture_var_decl_into_main_stmts(var, true, &mut main_stmts);
    assert!(
        result.is_err(),
        "destructuring array pattern with capture-bound init must return UnsupportedSyntaxError"
    );
}

#[test]
fn destructuring_pattern_in_library_mode_passes_through_silently() {
    // Library mode (`is_executable_mode = false`): the destructuring Var
    // Decl never enters the capture path (per-decl routing returns
    // LibraryMode), so the existing `convert_var_decl_module_level`
    // `_ => continue` fallback handles it (= pre-existing I-016 silent drop
    // owner, follow-up scope). The capture helper itself short-circuits via
    // the routing filter and returns Ok(()) without inspecting the pattern.
    let source = "declare function compute(): { a: number };\n\
                  const { a } = compute();";
    let m = parse(source);
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let mut main_stmts = Vec::new();
    let var = first_var_decl(&m);
    let result = t.capture_var_decl_into_main_stmts(var, false, &mut main_stmts);
    assert!(
        result.is_ok(),
        "library mode destructuring var decl must NOT return error (pre-existing \
         I-016 silent drop scope, not in T5-1 Tier 2 reject path)"
    );
    assert!(
        main_stmts.is_empty(),
        "library mode must not capture into main_stmts"
    );
}

// ===== Multi-declarator ANY-rule precedence with NonTriggerData =================
//
// **Note (deep review 2026-05-08)**: the ANY-rule precedence in
// `classify_init_kind` aggregates per-declarator init kinds at the VarDecl
// level for backward compatibility (still used by tests). The per-decl
// routing in `classify_per_decl_path` ignores this aggregation and decides
// each declarator independently. These tests lock in the aggregation
// semantic; per-decl routing tests above lock in the independent semantic.

#[test]
fn classify_init_kind_mixed_non_trigger_def_and_data_is_data() {
    // `const f = () => 1, x = { a: 1 };` — multi-declarator with Arrow (def)
    // + Object (data). Per the new precedence (NonTriggerData >
    // NonTriggerDef), the whole VarDecl classifies as NonTriggerData so the
    // executable-mode routing captures it.
    //
    // **Behavioral note (T5-1 review 2026-05-08, accurate description)**:
    // under the current VarDecl-level routing, `capture_var_decl_into_main_stmts`
    // iterates per-declarator and emits `MainStmt::Let` for **every**
    // declarator including the Arrow one. The Arrow's `Item::Fn` top-level
    // emission path (= `convert_arrow_var_decl` via
    // `convert_var_decl_module_level`) is therefore **NOT** taken in the
    // mixed Def+Data case — the Arrow becomes a Rust closure literal `let f =
    // || 1.0;` inside fn main instead of a top-level `fn f() -> f64 { 1.0 }`.
    //
    // Both forms compile and execute correctly when the consumer is also
    // captured into fn main (= the runtime-visible binding contract holds),
    // but the architectural difference (closure-in-fn-main vs top-level
    // Item::Fn) is a **Review insight** for follow-up: ideal per-declarator
    // routing would emit Arrow as Item::Fn AND Object as MainStmt::Let
    // independently. Hono codebase reachability for this mixed pattern = 0
    // (verified by `grep -rE "^\s*const\s+\w+\s*=\s*\(...\)...,\s*\w+\s*=\s*\{"
    // /tmp/hono-src` 2026-05-08), so the architectural gap is theoretical
    // and deferred to a future scope.
    let m = parse("const f = () => 1, x = { a: 1 };\nconsole.log(x.a);");
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::NonTriggerData);
}

#[test]
fn classify_init_kind_mixed_lit_and_non_trigger_data_is_data() {
    // `const a = 1, b = { x: 2 };` — Lit + Object. Precedence: NonTriggerData
    // wins over Lit. The whole VarDecl captures into fn main when in exec mode.
    let m = parse("const a = 1, b = { x: 2 };\nconsole.log(a, b.x);");
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::NonTriggerData);
}

#[test]
fn classify_init_kind_mixed_side_effect_and_non_trigger_data_is_side_effect() {
    // `const a = compute(), b = { x: 1 };` — SideEffect + NonTriggerData.
    // Precedence: SideEffect > NonTriggerData. Verifies the precedence chain
    // is honest (SideEffect's existing FnMainBodyCapture routing dominates).
    let m = parse(
        "declare function compute(): number;\n\
         const a = compute(), b = { x: 1 };\n\
         console.log(a, b.x);",
    );
    let var = first_var_decl(&m);
    assert_eq!(classify_init_kind(var), InitKind::SideEffect);
}
