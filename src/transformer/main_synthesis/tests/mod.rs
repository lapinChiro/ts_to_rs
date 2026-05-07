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
fn classify_init_kind_arrow_init_is_function_def() {
    // T3-4 spec extension: `const f = () => { ... };` is a function definition,
    // not a runtime side effect. classify_init_kind returns NonTriggerDef so
    // `has_side_effect_init` returns false (= no executable-mode trigger) and
    // `classify_per_decl_path` routes to `LibraryMode` (= Item::Fn emission via
    // convert_var_decl_module_level, not fn main body capture). Lock in the
    // partition so a future widening that re-classifies Arrow as SideEffect (=
    // would falsely trigger the rename gate for library-mode B-axis cells)
    // fails the test.
    //
    // **T5-1 Spec stage 逆戻り 2026-05-08 split**: previously classified as
    // `NonTrigger` (the merged variant); now `NonTriggerDef` to keep the
    // function-definition emission path (= top-level `Item::Fn`) distinct
    // from data literals (`NonTriggerData` → fn main body capture in
    // executable mode).
    let m = parse("const f = () => 1;");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerDef
    );
}

#[test]
fn classify_init_kind_async_arrow_init_is_function_def() {
    // The async modifier of an arrow init is irrelevant for the InitKind
    // partition: the body's await happens in the closure's own async context,
    // not at module-load time, so the init does not carry a top-level await.
    // (= `expr_contains_await_recursive` correctly skips function boundaries.)
    let m = parse("declare const p: Promise<number>;\nconst f = async () => await p;");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerDef
    );
}

#[test]
fn classify_init_kind_fn_expr_init_is_function_def() {
    // B1c form: `const main = function() { ... };` — same partition as Arrow.
    // The detect_user_main path uses this shape to recognize B1c user main;
    // classify_init_kind treats it identically to Arrow for trigger purposes.
    let m = parse("const f = function(): number { return 7; };");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerDef
    );
}

#[test]
fn has_side_effect_init_arrow_is_false() {
    // T3-4 e2e regression lock-in: Arrow / FnExpr inits MUST NOT trigger
    // executable mode. The original Spec gap was that Arrow init was
    // classified as SideEffect, causing library-mode modules with arrow
    // declarations + B-axis user main to falsely enter executable mode →
    // rename gate fired → no synthesized fn main → cargo run fails. This
    // test locks in the fix at the predicate level.
    let m = parse("const f = () => 1;");
    assert!(!has_side_effect_init(first_var_decl(&m)));
}

#[test]
fn has_side_effect_init_fn_expr_is_false() {
    let m = parse("const f = function(): number { return 7; };");
    assert!(!has_side_effect_init(first_var_decl(&m)));
}

#[test]
fn classify_per_decl_path_executable_function_def_is_library_mode() {
    // NonTriggerDef + executable mode → LibraryMode (= existing
    // convert_var_decl_module_level path emits Item::Fn). The Cartesian
    // product table in classify_per_decl_path's docstring is locked in here.
    let m = parse("const f = () => 1;");
    let var = first_var_decl(&m);
    let decl = var.decls.first().expect("single declarator");
    assert_eq!(
        classify_per_decl_path(decl, true, var.declare),
        DeclVarPath::LibraryMode
    );
}

// === NonTrigger split: NonTriggerDef vs NonTriggerData =================================
//
// I-224 T5-1 Spec stage 逆戻り 2026-05-08: the original `NonTrigger` variant
// merged function/class definitions (`Arrow` / `Fn` / `Class`) with aggregate
// data literals (`Object` / `Array`). The split into `NonTriggerDef` /
// `NonTriggerData` resolves the cell-12 / cell-24 silent-drop Tier 1 semantic
// loss: data literals carry runtime-visible bindings that subsequent top-level
// statements may reference, so they MUST be captured into fn main body in
// executable mode (= `NonTriggerData` → `FnMainBodyCapture`); function/class
// definitions cannot be expressed as fn-main-body `let` bindings, so they
// keep the top-level `Item::Fn` / class emission (= `NonTriggerDef` →
// `LibraryMode`).
//
// In library mode (no executable trigger), Object / Array literals are still
// silently dropped by `convert_var_decl_module_level`'s `_ => continue`
// fallback (= pre-existing I-016 owner; resolved by a follow-up PRD scope,
// not I-224 T5-1).

#[test]
fn classify_init_kind_object_literal_is_non_trigger_data() {
    // `const x = { a: 1, b: 2 };` — Object literal is not a runtime side
    // effect at module-load (= no observable I/O / mutation), but carries a
    // runtime-visible binding. Classified as NonTriggerData so executable-mode
    // fixtures capture it into fn main body (cell-12 / cell-24 fix).
    let m = parse("const x = { a: 1, b: 2 };");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerData
    );
}

#[test]
fn classify_init_kind_array_literal_is_non_trigger_data() {
    // `const x = [1, 2, 3];` — same NonTriggerData partition as Object literal.
    let m = parse("const x = [1, 2, 3];");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerData
    );
}

#[test]
fn classify_init_kind_class_expression_init_is_non_trigger_def() {
    // `const C = class { ... };` — class expression definition. The class
    // body is NOT executed at declaration time; the constructor runs only
    // on `new C(...)` invocation. Symmetric with Arrow / FnExpr in the
    // NonTriggerDef partition (top-level class shape, never captured into
    // fn main body).
    let m = parse("const C = class { foo(): void {} };");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerDef
    );
}

// === NonTrigger expansion: type-only wrapper recursive walk ==========================
//
// `Expr::TsAs` / `TsConstAssertion` / `TsTypeAssertion` / `TsNonNull` /
// `TsInstantiation` / `TsSatisfies` / `Paren` are AST decorators with no
// runtime semantics. `expr_init_kind` recurses on the inner expression so
// the wrapper inherits the inner's classification. These tests lock in the
// recursive-walk behavior for the most common wrapper shapes encountered in
// production TypeScript fixtures (e.g., `as const` patterns).

#[test]
fn classify_init_kind_ts_as_object_is_non_trigger_data() {
    // `const x = { a: 1 } as const;` — TsAs wraps Object literal. The
    // recursive walk classifies the inner Object as NonTriggerData and the
    // outer TsAs inherits.
    //
    // **Lesson source**: this exact pattern (`const Phase = { ... } as
    // const;`) caused the typeof_const e2e fixture to fail before the T3-4
    // NonTrigger expansion — the Spec extension was driven by this case.
    // The T5-1 split preserves the recursive-walk inheritance: a wrapper
    // around Object literal is still data, not function definition.
    let m = parse("const x = { a: 1, b: 2 } as const;");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerData
    );
}

#[test]
fn classify_init_kind_ts_const_assertion_array_is_non_trigger_data() {
    // `const x = [1, 2, 3] as const;` — TsConstAssertion wraps Array. Inner
    // Array is NonTriggerData; the outer wrapper inherits.
    let m = parse("const x = [1, 2, 3] as const;");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerData
    );
}

#[test]
fn classify_init_kind_paren_lit_is_lit() {
    // `const x = (1);` — Paren wraps Lit::Num. Recursive walk inherits the
    // inner Lit classification (= NOT NonTrigger; preserves the more
    // specific Lit partition for Rust-const-compatible inits). This locks
    // in that the recursive walk doesn't over-collapse Lit to NonTrigger.
    let m = parse("const x = (1);");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::Lit);
}

#[test]
fn classify_init_kind_ts_non_null_arrow_is_non_trigger_def() {
    // `const f = (() => 1)!;` — TsNonNull wraps Paren wraps Arrow. The
    // recursive walk traverses both wrappers; inner Arrow → NonTriggerDef.
    let m = parse("const f = ((): number => 1)!;");
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerDef
    );
}

#[test]
fn classify_init_kind_nested_wrappers_with_side_effect_inner_is_side_effect() {
    // `const x = ((compute()) as number);` — Paren wraps TsAs wraps Paren
    // wraps Call (SideEffect). Recursive walk traverses every wrapper and
    // surfaces the inner SideEffect classification through to the outer.
    // This is the structural correctness probe: type-only wrappers do not
    // hide side-effect-bearing inits from the trigger partition.
    let m = parse("declare function compute(): number;\nconst x = ((compute()) as number);");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::SideEffect);
}

#[test]
fn classify_init_kind_ts_satisfies_object_is_non_trigger_data() {
    // `const x = { a: 1 } satisfies { a: number };` — TsSatisfies wraps
    // Object literal. Recursive walk inherits NonTriggerData.
    let m = parse(
        "interface Shape { a: number }\n\
         const x = { a: 1 } satisfies Shape;",
    );
    assert_eq!(
        classify_init_kind(first_var_decl(&m)),
        InitKind::NonTriggerData
    );
}

#[test]
fn has_side_effect_init_object_literal_is_false() {
    // T3-4 e2e regression lock-in (typeof_const fixture): Object literal
    // init MUST NOT trigger executable mode for library-mode modules with
    // pure data declarations. This complements `has_side_effect_init_*`
    // tests for Arrow / FnExpr.
    let m = parse("const x = { a: 1 };");
    assert!(!has_side_effect_init(first_var_decl(&m)));
}

#[test]
fn has_side_effect_init_ts_as_const_is_false() {
    // `const Phase = { ... } as const;` — the exact typeof_const fixture
    // pattern. Locks in that TsAs(Object) does not trigger exec mode.
    let m = parse("const Phase = { S: 1, B: 2, R: 3 } as const;");
    assert!(!has_side_effect_init(first_var_decl(&m)));
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
#[should_panic(expected = "TS Decl::Var requires init in at least one declarator")]
fn classify_init_kind_panics_on_no_init_var() {
    // Defensive precondition test: classify_init_kind documents that callers must
    // filter no-init Var (`declare const x: T;`) via has_side_effect_init /
    // classify_per_decl_path defensive guards before calling. Bypassing those
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

// === classify_per_decl_path (Decision Table) ==============================================
//
// `/check_job deep deep` 2026-05-08 migration: the legacy VarDecl-level
// `classify_decl_var_path` was removed entirely. Tests below lock in the
// per-declarator routing decision table for single-decl scenarios (= the
// only shape the legacy classifier was tested against). Multi-decl mixed
// routing tests live in `i224_t5_1_spec_modori.rs::classify_per_decl_path_*`.

fn first_decl(m: &Module) -> (&swc_ecma_ast::VarDeclarator, bool) {
    let var = first_var_decl(m);
    let decl = var
        .decls
        .first()
        .expect("test fixture must have at least one declarator");
    (decl, var.declare)
}

#[test]
fn classify_per_decl_path_library_lit() {
    let m = parse("const x = 42;");
    let (decl, declare) = first_decl(&m);
    assert_eq!(
        classify_per_decl_path(decl, false, declare),
        DeclVarPath::LibraryMode
    );
}

#[test]
fn classify_per_decl_path_library_side_effect_unreachable_via_predicate() {
    // The (false, SideEffect) cell of the table maps to LibraryMode in code
    // even though `is_executable_mode` would return true for such a module —
    // reaching this combination requires bypassing the predicate (e.g., direct
    // unit test). We verify the table consistency here.
    let m = parse("declare function f(): number;\nconst x = f();");
    let (decl, declare) = first_decl(&m);
    assert_eq!(
        classify_per_decl_path(decl, false, declare),
        DeclVarPath::LibraryMode
    );
}

#[test]
fn classify_per_decl_path_executable_lit_is_toplevel_const() {
    let m = parse("const x = 42;");
    let (decl, declare) = first_decl(&m);
    assert_eq!(
        classify_per_decl_path(decl, true, declare),
        DeclVarPath::ToplevelConst
    );
}

#[test]
fn classify_per_decl_path_executable_side_effect_is_capture() {
    let m = parse("declare function f(): number;\nconst x = f();");
    let (decl, declare) = first_decl(&m);
    assert_eq!(
        classify_per_decl_path(decl, true, declare),
        DeclVarPath::FnMainBodyCapture
    );
}

#[test]
fn classify_per_decl_path_executable_await_is_capture() {
    let m = parse("declare const p: Promise<number>;\nconst x = await p;");
    let (decl, declare) = first_decl(&m);
    assert_eq!(
        classify_per_decl_path(decl, true, declare),
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

// I-224 Spec stage 逆戻り Iteration v8 fix tests (extracted to keep file-line check OK).
mod i228_spec_modori;

// I-224 T5-1 Spec stage 逆戻り 2026-05-08 — `NonTrigger` partition split into
// `NonTriggerDef` + `NonTriggerData` for the cell-12 / cell-24 silent-drop
// Tier 1 semantic loss fix.
mod i224_t5_1_spec_modori;

// `synthesize_fn_main` dispatch tree tests (T3-1) — extracted by `/check_job deep deep`
// 2026-05-08 to keep mod.rs under the 1000-line file-line check threshold.
mod synthesize_fn_main;
