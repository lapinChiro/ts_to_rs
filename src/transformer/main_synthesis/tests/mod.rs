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
    // not a runtime side effect. classify_init_kind returns FunctionDef so that
    // `has_side_effect_init` returns false (= no executable-mode trigger) and
    // `classify_decl_var_path` routes to `LibraryMode` (= Item::Fn emission via
    // convert_var_decl_module_level, not fn main body capture). Lock in the
    // partition so a future widening that re-classifies Arrow as SideEffect (=
    // would falsely trigger the rename gate for library-mode B-axis cells)
    // fails the test.
    let m = parse("const f = () => 1;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
}

#[test]
fn classify_init_kind_async_arrow_init_is_function_def() {
    // The async modifier of an arrow init is irrelevant for the InitKind
    // partition: the body's await happens in the closure's own async context,
    // not at module-load time, so the init does not carry a top-level await.
    // (= `expr_contains_await_recursive` correctly skips function boundaries.)
    let m = parse("declare const p: Promise<number>;\nconst f = async () => await p;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
}

#[test]
fn classify_init_kind_fn_expr_init_is_function_def() {
    // B1c form: `const main = function() { ... };` — same partition as Arrow.
    // The detect_user_main path uses this shape to recognize B1c user main;
    // classify_init_kind treats it identically to Arrow for trigger purposes.
    let m = parse("const f = function(): number { return 7; };");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
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
fn classify_decl_var_path_executable_function_def_is_library_mode() {
    // FunctionDef + executable mode → LibraryMode (= existing
    // convert_var_decl_module_level path emits Item::Fn). The Cartesian
    // product table in classify_decl_var_path's docstring is locked in here.
    let m = parse("const f = () => 1;");
    assert_eq!(
        classify_decl_var_path(first_var_decl(&m), true),
        DeclVarPath::LibraryMode
    );
}

// === NonTrigger expansion: aggregate literals + class expression =====================
//
// `Expr::Object` / `Expr::Array` / `Expr::Class` are part of the NonTrigger
// partition along with Arrow / Fn (function definitions). Aggregate literals
// are silently dropped by `convert_var_decl_module_level` (I-016 owner —
// pre-existing gap), so classifying them as NonTrigger has no Rust-emission
// consequence; the partition prevents the rename gate from firing falsely on
// library-mode modules whose only non-Lit Decl::Var items are pure data
// declarations like `const Phase = { Stringify: 1, ... };`.

#[test]
fn classify_init_kind_object_literal_is_non_trigger() {
    // `const x = { a: 1, b: 2 };` — Object literal is not a runtime side
    // effect at module-load (= no observable I/O / mutation). T3-4 NonTrigger
    // expansion locks this in to prevent rename-gate over-trigger for
    // library-mode modules with pure data declarations (e.g., enum-like
    // const objects).
    let m = parse("const x = { a: 1, b: 2 };");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
}

#[test]
fn classify_init_kind_array_literal_is_non_trigger() {
    // `const x = [1, 2, 3];` — same NonTrigger partition as Object literal.
    let m = parse("const x = [1, 2, 3];");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
}

#[test]
fn classify_init_kind_class_expression_init_is_non_trigger() {
    // `const C = class { ... };` — class expression definition. The class
    // body is NOT executed at declaration time; the constructor runs only
    // on `new C(...)` invocation. Symmetric with Arrow / FnExpr in the
    // NonTrigger partition.
    let m = parse("const C = class { foo(): void {} };");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
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
fn classify_init_kind_ts_as_object_is_non_trigger() {
    // `const x = { a: 1 } as const;` — TsAs wraps Object literal. The
    // recursive walk classifies the inner Object as NonTrigger and the
    // outer TsAs inherits.
    //
    // **Lesson source**: this exact pattern (`const Phase = { ... } as
    // const;`) caused the typeof_const e2e fixture to fail before the T3-4
    // NonTrigger expansion — the Spec extension was driven by this case.
    let m = parse("const x = { a: 1, b: 2 } as const;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
}

#[test]
fn classify_init_kind_ts_const_assertion_array_is_non_trigger() {
    // `const x = [1, 2, 3] as const;` — TsConstAssertion wraps Array.
    let m = parse("const x = [1, 2, 3] as const;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
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
fn classify_init_kind_ts_non_null_arrow_is_non_trigger() {
    // `const f = (() => 1)!;` — TsNonNull wraps Paren wraps Arrow. The
    // recursive walk traverses both wrappers.
    let m = parse("const f = ((): number => 1)!;");
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
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
fn classify_init_kind_ts_satisfies_object_is_non_trigger() {
    // `const x = { a: 1 } satisfies { a: number };` — TsSatisfies wraps
    // Object literal. Recursive walk inherits NonTrigger.
    let m = parse(
        "interface Shape { a: number }\n\
         const x = { a: 1 } satisfies Shape;",
    );
    assert_eq!(classify_init_kind(first_var_decl(&m)), InitKind::NonTrigger);
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

// I-224 Spec stage 逆戻り Iteration v8 fix tests (extracted to keep file-line check OK).
mod i228_spec_modori;
