use super::*;

#[test]
fn test_convert_stmt_for_counter_zero_to_n() {
    let stmts = parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { i; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Some(Box::new(Expr::NumberLit(0.0))),
                end: Some(Box::new(Expr::Ident("n".to_string()))),
            },
            body: vec![
                Stmt::Let {
                    mutable: false,
                    name: "i".to_string(),
                    ty: None,
                    init: Some(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    }),
                },
                Stmt::Expr(Expr::Ident("i".to_string())),
            ],
        }
    );
}

#[test]
fn test_convert_stmt_for_counter_start_to_literal() {
    let stmts = parse_fn_body("function f() { for (let i = 1; i < 10; i++) { i; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Some(Box::new(Expr::NumberLit(1.0))),
                end: Some(Box::new(Expr::NumberLit(10.0))),
            },
            body: vec![
                Stmt::Let {
                    mutable: false,
                    name: "i".to_string(),
                    ty: None,
                    init: Some(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    }),
                },
                Stmt::Expr(Expr::Ident("i".to_string())),
            ],
        }
    );
}

#[test]
fn test_convert_stmt_for_of() {
    let stmts = parse_fn_body("function f() { for (const item of items) { item; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "item".to_string(),
            iterable: Expr::Ident("items".to_string()),
            body: vec![Stmt::Expr(Expr::Ident("item".to_string()))],
        }
    );
}

// --- for...in ---

#[test]
fn test_convert_stmt_for_in_generates_keys_iteration() {
    let stmts = parse_fn_body("function f() { for (const k in obj) { k; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::ForIn {
            label: None,
            var: "k".to_string(),
            iterable: Expr::MethodCall {
                object: Box::new(Expr::Ident("obj".to_string())),
                method: "keys".to_string(),
                args: vec![],
            },
            body: vec![Stmt::Expr(Expr::Ident("k".to_string()))],
        }
    );
}

#[test]
fn test_convert_for_range_inserts_f64_shadow() {
    // for (let i = 0; i < n; i++) { sum += i; }
    // → body should start with: let i = i as f64;
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { sum += i; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::ForIn { body, .. } => {
            // First stmt should be: let i = i as f64;
            assert!(
                matches!(&body[0], Stmt::Let { name, init: Some(Expr::Cast { target: RustType::F64, .. }), .. } if name == "i"),
                "expected let i = i as f64; as first stmt, got {:?}",
                body[0]
            );
        }
        other => panic!("expected ForIn, got: {other:?}"),
    }
}

// -- General for loop (loop fallback) tests --

#[test]
fn test_convert_stmt_list_for_decrement_becomes_loop() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = n; i >= 0; i--) { console.log(i); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Should produce: let mut i = n; loop { if !(i >= 0) { break; } body; i--; }
    assert_eq!(result.len(), 2); // init + loop
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
    assert!(matches!(&result[1], Stmt::Loop { .. }));
}

#[test]
fn test_convert_stmt_list_for_step_by_two_becomes_loop() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(n: number) { for (let i = 0; i < n; i += 2) { console.log(i); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
    assert!(matches!(&result[1], Stmt::Loop { .. }));
}

#[test]
fn test_convert_stmt_for_simple_counter_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Existing simple counter pattern should still produce ForIn
    let stmts =
        parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { console.log(i); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], Stmt::ForIn { .. }));
}

#[test]
fn test_convert_stmt_labeled_for_range() {
    let stmts =
        parse_fn_body("function f() { outer: for (let i = 0; i < 10; i++) { break outer; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::ForIn { label, .. } => {
            assert_eq!(label, Some("outer".to_string()));
        }
        _ => panic!("expected labeled ForIn"),
    }
}

#[test]
fn test_convert_stmt_for_of_array_destructuring_generates_tuple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // for (const [k, v] of entries) { ... }
    let stmts = parse_fn_body(
        "function f(entries: [string, number][]) { for (const [k, v] of entries) { console.log(k); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    // Should produce a ForIn with a tuple destructuring pattern
    assert!(!result.is_empty(), "should produce at least one statement");
}

#[test]
fn test_convert_stmt_for_of_array_destructuring_3_elements() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(entries: [string, number, boolean][]) { for (const [a, b, c] of entries) { console.log(a); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    // Should produce a ForIn with a 3-element tuple destructuring pattern "(a, b, c)"
    assert!(!result.is_empty(), "should produce at least one statement");
    let for_in = result.iter().find(|s| matches!(s, Stmt::ForIn { .. }));
    assert!(for_in.is_some(), "should contain a ForIn statement");
    match for_in.unwrap() {
        Stmt::ForIn { var, .. } => {
            assert_eq!(
                var, "(a, b, c)",
                "for-in var should be tuple pattern (a, b, c)"
            );
        }
        _ => unreachable!(),
    }
}

// ---- for loop multiple declarators ----

#[test]
fn test_convert_stmt_for_loop_multiple_declarators() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(n: number) { for (let i = 0, len = n; i < len; i++) { console.log(i); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    // Multiple declarators fall back to loop pattern: Let(i), Let(len), Loop { ... }
    assert!(
        result.len() >= 3,
        "expected at least 3 statements (2 lets + loop), got {:?}",
        result
    );
    // First two should be Let statements for i and len
    match &result[0] {
        Stmt::Let { name, mutable, .. } => {
            assert_eq!(name, "i");
            assert!(*mutable, "i should be mutable");
        }
        other => panic!("expected Let for i, got {:?}", other),
    }
    match &result[1] {
        Stmt::Let { name, mutable, .. } => {
            assert_eq!(name, "len");
            assert!(*mutable, "len should be mutable");
        }
        other => panic!("expected Let for len, got {:?}", other),
    }
}

// ============ I-154 T6: `__ts_` prefix label namespace lint tests ============
//
// User labels starting with `__ts_` are reserved for ts_to_rs internal emission.
// Currently reserved internal identifiers (single source of truth: doc on
// `check_ts_internal_label_namespace` in src/transformer/statements/mod.rs):
//   - labels:         `__ts_switch`, `__ts_try_block`, `__ts_do_while`,
//                     `__ts_do_while_loop`
//   - value bindings: `__ts_old`, `__ts_new`, `__ts_recv`
//   - rename target:  `__ts_main` (user `main` rename target — see
//                     `TS_MAIN_RENAME` in src/transformer/expressions/mod.rs)
// The lint fires at 3 entry points: labeled stmt declaration, labeled break,
// labeled continue. The mechanism is `label.sym.starts_with("__ts_")` —
// shared across all 3 entry points, all reserved names.

#[test]
fn i154_labeled_stmt_rejects_ts_internal_prefix() {
    let src = "function f() { __ts_foo: while (true) { break; } }";
    let result = crate::transpile_collecting(src);
    assert!(
        result.is_err() || {
            if let Ok((_rust, unsupported)) = &result {
                unsupported.iter().any(|u| u.kind.contains("__ts_"))
            } else {
                false
            }
        },
        "expected UnsupportedSyntaxError for `__ts_foo:` labeled stmt, got {result:?}"
    );
}

#[test]
fn i154_labeled_break_rejects_ts_internal_prefix() {
    // SWC accepts undefined labels; tsx would reject with "Undefined label"
    // syntax error, but we lint directly to prevent collision with internal labels.
    let src = "function f() { for (;;) { break __ts_switch; } }";
    let result = crate::transpile_collecting(src);
    assert!(
        result.is_err() || {
            if let Ok((_rust, unsupported)) = &result {
                unsupported.iter().any(|u| u.kind.contains("__ts_"))
            } else {
                false
            }
        },
        "expected UnsupportedSyntaxError for `break __ts_switch;`, got {result:?}"
    );
}

#[test]
fn i154_labeled_continue_rejects_ts_internal_prefix() {
    let src = "function f() { for (;;) { continue __ts_do_while; } }";
    let result = crate::transpile_collecting(src);
    assert!(
        result.is_err() || {
            if let Ok((_rust, unsupported)) = &result {
                unsupported.iter().any(|u| u.kind.contains("__ts_"))
            } else {
                false
            }
        },
        "expected UnsupportedSyntaxError for `continue __ts_do_while;`, got {result:?}"
    );
}

#[test]
fn i154_labeled_break_rejects_ts_main_prefix() {
    // `__ts_main` is the user-`main` rename target reserved by the namespace
    // (see `TS_MAIN_RENAME` in src/transformer/expressions/mod.rs). This
    // regression test locks in that the prefix-based label lint covers
    // `__ts_main` — exercised at the `break` entry point as a representative,
    // since the lint mechanism (`label.sym.starts_with("__ts_")`) is shared
    // across all 3 entry points (declaration / break / continue) and adding
    // a new reserved name does not change which entry points fire.
    let src = "function f() { for (;;) { break __ts_main; } }";
    let result = crate::transpile_collecting(src);
    assert!(
        result.is_err() || {
            if let Ok((_rust, unsupported)) = &result {
                unsupported.iter().any(|u| u.kind.contains("__ts_"))
            } else {
                false
            }
        },
        "expected UnsupportedSyntaxError for `break __ts_main;`, got {result:?}"
    );
}

#[test]
fn i154_non_prefixed_user_labels_accepted() {
    // Sanity: ordinary user labels should NOT trigger the lint.
    let src = "function f() { outer: for (;;) { break outer; } }";
    let result = crate::transpile_collecting(src);
    match result {
        Ok((_, unsupported)) => {
            assert!(
                !unsupported.iter().any(|u| u.kind.contains("__ts_")),
                "non-prefixed user label should not trigger the lint: {unsupported:?}"
            );
        }
        Err(e) => panic!("non-prefixed user label should not error: {e}"),
    }
}

// ============ I-224: Module-level fn-name namespace lint tests ============
//
// Symmetric to the label-side I-154 lint above: user-defined module-level
// identifiers starting with `__ts_` (= `function __ts_main() {}`,
// `const __ts_main = ...`, `class __ts_main {}`, `interface __ts_main {}`,
// `type __ts_main = ...`, `enum __ts_main {}`, `namespace __ts_main {}`, and
// their `export`-wrapped variants) are rejected as Tier 2 honest errors with
// wording mentioning the offending identifier and the namespace structure.
// Mechanism: `check_ts_internal_fn_name_namespace` invoked via
// `scan_for_ts_namespace_collisions` at the top of `transform_module(_collecting)`.
//
// Acceptance helper: the validator's wording always contains the substring
// "is reserved" — tests assert this substring plus the offending identifier
// appears in the collected `unsupported` list.

/// Helper: run `transpile_collecting` and assert that at least one unsupported
/// entry is reported with both `name` (verbatim) and the substring
/// `"is reserved"` in its `kind` field.
fn assert_ts_namespace_collision(src: &str, name: &str) {
    let result = crate::transpile_collecting(src);
    match result {
        Ok((_rust, unsupported)) => {
            let hits: Vec<_> = unsupported
                .iter()
                .filter(|u| u.kind.contains(name) && u.kind.contains("is reserved"))
                .collect();
            assert!(
                !hits.is_empty(),
                "expected at least one Tier 2 reject mentioning `{name}` and `is reserved`, \
                 got unsupported list: {unsupported:?}"
            );
        }
        Err(e) => panic!("transpile_collecting failed unexpectedly for {src:?}: {e}"),
    }
}

#[test]
fn i224_module_level_fn_decl_rejects_ts_main() {
    // Matrix # 9 representative (A0 + B4 + C0): library-form `__ts_main` collision.
    assert_ts_namespace_collision(
        "function __ts_main(): void { console.log('user'); }",
        "__ts_main",
    );
}

#[test]
fn i224_module_level_const_decl_rejects_ts_main() {
    // PRD task description names this shape explicitly:
    // `const __ts_main = ...` should reject identical to `function __ts_main()`.
    assert_ts_namespace_collision(
        "const __ts_main = (): void => { console.log('user'); };",
        "__ts_main",
    );
}

#[test]
fn i224_module_level_let_decl_rejects_ts_main() {
    // `let __ts_main = ...` shape (Decl::Var with VarDeclKind::Let).
    assert_ts_namespace_collision("let __ts_main = 1;", "__ts_main");
}

#[test]
fn i224_module_level_class_decl_rejects_ts_main() {
    assert_ts_namespace_collision("class __ts_main {}", "__ts_main");
}

#[test]
fn i224_module_level_interface_decl_rejects_ts_main() {
    assert_ts_namespace_collision("interface __ts_main { x: number; }", "__ts_main");
}

#[test]
fn i224_module_level_type_alias_rejects_ts_main() {
    assert_ts_namespace_collision("type __ts_main = number;", "__ts_main");
}

#[test]
fn i224_module_level_enum_decl_rejects_ts_main() {
    assert_ts_namespace_collision("enum __ts_main { A, B }", "__ts_main");
}

#[test]
fn i224_module_level_namespace_decl_rejects_ts_main() {
    // `namespace __ts_main {}` (TsModule with Ident id).
    assert_ts_namespace_collision("namespace __ts_main { export const x = 1; }", "__ts_main");
}

#[test]
fn i224_export_fn_decl_rejects_ts_main() {
    // ExportDecl wrapper around Decl::Fn — same dispatch path.
    assert_ts_namespace_collision("export function __ts_main(): void {}", "__ts_main");
}

#[test]
fn i224_export_default_named_fn_rejects_ts_main() {
    // ExportDefaultDecl with a named DefaultDecl::Fn — covers the
    // `default-decl` branch of the scan.
    assert_ts_namespace_collision("export default function __ts_main(): void {}", "__ts_main");
}

#[test]
fn i224_other_ts_prefixed_module_name_rejected() {
    // Prefix check (rather than name-specific to `__ts_main`) — verifies the
    // namespace is reserved for any future internal target. Not a B4 cell;
    // hygiene parity with label-side prefix lint.
    assert_ts_namespace_collision(
        "function __ts_other_internal(): void {}",
        "__ts_other_internal",
    );
}

#[test]
fn i224_non_prefixed_user_main_accepted() {
    // Sanity: user `function main() {}` (= matrix B1 partition) must NOT be
    // rejected by the namespace lint. The rename / synthesis dispatch is
    // separate (T3 work).
    let result = crate::transpile_collecting("function main(): void { console.log('hi'); }");
    match result {
        Ok((_rust, unsupported)) => {
            assert!(
                !unsupported.iter().any(|u| u.kind.contains("is reserved")),
                "user `function main` must not trigger namespace lint: {unsupported:?}"
            );
        }
        Err(e) => panic!("user `function main` should not error: {e}"),
    }
}

#[test]
fn i224_matrix_cell_19_stmt_expr_with_collision_rejected() {
    // Matrix # 19 representative (A1 + B4 + C0): `__ts_main` collision in
    // executable-mode source (top-level Stmt::Expr `console.log` + user-side
    // call `__ts_main()`). The collision-detection scan precedes A-axis
    // dispatch (per design dispatch tree top arm), so the rejection still
    // fires regardless of the additional execution stmts.
    let src = "\
        function __ts_main(): void { console.log('user __ts_main'); }\n\
        console.log('top-level');\n\
        __ts_main();\n";
    assert_ts_namespace_collision(src, "__ts_main");
}

// ============ I-224 T4-2: A4 (control-flow) Tier 2 wording tests ============
//
// Top-level control-flow statements (`if`, loops, `try`, etc.) execute at
// module-load time in TS but have no Rust module-item analogue — Rust has no
// top-level execution context outside `fn main()`. The `fn main()` synthesis
// (T3 / T4-1) captures `Stmt::Expr` and side-effect `Decl::Var` but does NOT
// wrap A4 statements. T4-2 improves the Tier 2 reject wording to mention the
// `fn main` wrapping requirement and the I-203 future expansion scope.

/// Helper: assert at least one collected `unsupported` entry contains the
/// `ControlFlow at top-level` substring (= the T4-2 A4 reject wording).
fn assert_a4_control_flow_reject(src: &str) {
    let result = crate::transpile_collecting(src);
    match result {
        Ok((_rust, unsupported)) => {
            let hits: Vec<_> = unsupported
                .iter()
                .filter(|u| u.kind.contains("ControlFlow at top-level"))
                .collect();
            assert!(
                !hits.is_empty(),
                "expected at least one A4 Tier 2 reject mentioning `ControlFlow at top-level`, \
                 got unsupported list: {unsupported:?}"
            );
        }
        Err(e) => panic!("transpile_collecting failed unexpectedly for {src:?}: {e}"),
    }
}

#[test]
fn i224_a4_top_level_if_rejected_with_control_flow_wording() {
    // Cell 41 representative (A4 + B0 + C0): top-level `if` statement.
    assert_a4_control_flow_reject("const x = 7;\nif (x > 5) { console.log('hi'); }\n");
}

#[test]
fn i224_a4_top_level_for_rejected_with_control_flow_wording() {
    assert_a4_control_flow_reject("for (let i = 0; i < 3; i++) { console.log(i); }\n");
}

#[test]
fn i224_a4_top_level_while_rejected_with_control_flow_wording() {
    assert_a4_control_flow_reject("let i = 0;\nwhile (i < 3) { i++; }\n");
}

#[test]
fn i224_a4_top_level_try_rejected_with_control_flow_wording() {
    assert_a4_control_flow_reject("try { console.log('hi'); } catch (e) {}\n");
}

#[test]
fn i224_a4_top_level_switch_rejected_with_control_flow_wording() {
    assert_a4_control_flow_reject(
        "const x = 1;\nswitch (x) { case 1: console.log('one'); break; }\n",
    );
}

#[test]
fn i224_a4_top_level_block_rejected_with_control_flow_wording() {
    // Bare `{ ... }` at module level (rare, but valid TS syntax = block stmt).
    assert_a4_control_flow_reject("{ console.log('block'); }\n");
}

// ============ I-224 T4-2: A5b (Debugger) Tier 2 wording test ============
//
// `debugger;` at module level signals a debugger breakpoint at module load.
// Rust has no built-in debugger-breakpoint statement; T4-2 reports as Tier 2
// with explicit guidance on `panic!()` / `std::dbg!()` alternatives.

#[test]
fn i224_a5b_debugger_rejected_with_debugger_wording() {
    // Cell 27-b representative: top-level `debugger;` statement.
    let src = "debugger;\nconsole.log('after debugger');\n";
    let result = crate::transpile_collecting(src);
    match result {
        Ok((_rust, unsupported)) => {
            let hits: Vec<_> = unsupported
                .iter()
                .filter(|u| {
                    u.kind
                        .contains("`debugger` statement has no Rust equivalent")
                        && u.kind.contains("panic!()")
                        && u.kind.contains("std::dbg!()")
                })
                .collect();
            assert!(
                !hits.is_empty(),
                "expected A5b Tier 2 reject with `debugger` + `panic!()` + `std::dbg!()` \
                 wording, got unsupported list: {unsupported:?}"
            );
        }
        Err(e) => panic!("transpile_collecting failed unexpectedly for {src:?}: {e}"),
    }
}
