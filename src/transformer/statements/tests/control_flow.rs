use super::*;

// I-153 T0: `ast::Stmt::Block` support (flatten into parent scope).
// TS allows block statements at any statement position (e.g., `case 1: { stmts }`,
// `function f() { { stmts } }`). Currently `convert_stmt` lacks a Block arm and
// rejects with `unsupported statement: Block(...)`. The fix flattens the block
// contents into the parent via `convert_stmt_list`, preserving semantics for
// valid TS (Rust's enclosing match arm / function body provides block scope).

#[test]
fn test_convert_block_stmt_flattens_into_parent() {
    // Bare block at function body level.
    let stmts = parse_fn_body("function f() { { const x = 1; return x; } }");
    let result = {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Expect 2 flattened statements (Let + Return), not a single wrapper.
    assert_eq!(
        result.len(),
        2,
        "block should flatten to parent, got {result:?}"
    );
    assert!(matches!(result[0], Stmt::Let { .. }));
    assert!(matches!(result[1], Stmt::Return(_)));
}

#[test]
fn test_convert_block_stmt_in_switch_case_flattens() {
    // Block inside switch case body (the common `no-case-declarations` pattern).
    let stmts = parse_fn_body(
        r#"function f(x: number): string {
            switch (x) {
                case 1: {
                    const y = x;
                    return "one";
                }
                default:
                    return "other";
            }
        }"#,
    );
    let result = {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Expect a Match (switch) with the block contents flattened into its arm body.
    assert_eq!(result.len(), 1, "expected single Match, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2, "expected case + default arms, got {arms:?}");
            // First arm body should have >= 2 stmts (Let + Return), flattened from block.
            assert!(
                arms[0].body.len() >= 2,
                "case body should contain flattened block contents, got {:?}",
                arms[0].body
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_block_stmt_nested_break_preserved_for_i153_walker() {
    // Block containing a nested bare break: verifies the block flatten
    // preserves the break stmt so I-153's walker can rewrite it later.
    let stmts = parse_fn_body(
        r#"function f(x: number, cond: boolean) {
            for (;;) {
                switch (x) {
                    case 1: {
                        if (cond) break;
                        return;
                    }
                    default:
                        return;
                }
            }
        }"#,
    );
    let result = {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Ensure conversion succeeded (no unsupported error) and produced a Loop wrapping Match.
    // `for (;;)` without init/test/update converts to Stmt::Loop (not ForIn).
    // The nested bare `break` inside block inside if inside case body is preserved
    // for I-153 walker (T2/T3) to rewrite later — T0 only verifies conversion succeeds.
    assert_eq!(result.len(), 1);
    let loop_body = match &result[0] {
        Stmt::Loop { body, .. } => body,
        other => panic!("expected Loop, got {other:?}"),
    };
    assert!(
        loop_body
            .iter()
            .any(|s| matches!(s, Stmt::Match { .. } | Stmt::LabeledBlock { .. })),
        "loop-body should contain Match or LabeledBlock(Match), got {loop_body:?}"
    );
    // The block inside `case 1: { if (cond) break; return; }` should flatten such that
    // the `if`/`return` become direct match arm body stmts.
    if let Some(Stmt::Match { arms, .. }) =
        loop_body.iter().find(|s| matches!(s, Stmt::Match { .. }))
    {
        let case1_arm = &arms[0];
        assert!(
            case1_arm.body.len() >= 2,
            "case 1 arm body should have if + return (flattened from block), got {:?}",
            case1_arm.body
        );
        assert!(
            matches!(case1_arm.body[0], Stmt::If { .. }),
            "case 1 arm body[0] should be If, got {:?}",
            case1_arm.body[0]
        );
    }
}

/// I-144 T6-3 H-3 lock-in: `!x` early-return on `Option<Union>` where the
/// union mixes primitive and Named (object) variants. The Named variant must
/// emit a guard-less arm (JS always-truthy semantics for object references).
///
/// This integration test runs the full pipeline (TypeResolver + Transformer)
/// and inspects the emitted IR directly, bypassing the E2E Rust compile step
/// because the separate call-arg Union coercion gap (non-literal → Union
/// variant wrap) blocks a full E2E run for this cell. The IR-level lock-in
/// is sufficient because the H-3 fix lives entirely inside
/// `build_union_variant_truthy_arms` — the consolidated match is materialized
/// well before any call site and is independent of call-arg coercion.
#[test]
fn test_try_generate_option_truthy_complement_match_h3_mixed_union_emits_guard_only_for_primitives()
{
    use crate::ir::{CallTarget, MatchArm, PatternCtor};

    let source = r#"
interface Tag {
    label: string;
}
function describe(x: string | Tag | null): string {
    if (!x) return "none";
    if (typeof x === "string") return "s:" + x;
    return "tag:" + x.label;
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let source_reg = crate::registry::build_registry(&module);
    let mg = crate::pipeline::ModuleGraph::empty();
    let mut synthetic = SyntheticTypeRegistry::new();
    let parsed = crate::pipeline::ParsedFile {
        path: std::path::PathBuf::from("test.ts"),
        source: source.to_string(),
        module: module.clone(),
    };
    let mut resolver =
        crate::pipeline::type_resolver::TypeResolver::new(&source_reg, &mut synthetic);
    let res = resolver.resolve_file(&parsed);
    let tctx = TransformContext::new(&mg, &source_reg, &res, Path::new("test.ts"));

    // Extract the function `describe` and convert it.
    let mut synthetic2 = synthetic;
    let items = Transformer::for_module(&tctx, &mut synthetic2)
        .transform_module(&module)
        .unwrap();
    let describe = items
        .iter()
        .find_map(|i| {
            if let crate::ir::Item::Fn {
                name,
                body,
                return_type,
                ..
            } = i
            {
                if name == "describe" {
                    return Some((body.clone(), return_type.clone()));
                }
            }
            None
        })
        .expect("describe function not found");

    // First body stmt must be the consolidated match `let x = match x { ... }`
    // produced by `try_generate_option_truthy_complement_match`.
    let Stmt::Let {
        init: Some(Expr::Match { expr: _, ref arms }),
        ..
    } = describe.0[0]
    else {
        panic!(
            "expected first stmt to be `let x = match x {{ ... }}`, got {:?}",
            describe.0[0]
        );
    };

    // Expect 3 arms: String variant (with `!is_empty` guard), Tag variant
    // (guard-less, JS always-truthy), and the `_ => return ...` exit arm.
    assert_eq!(
        arms.len(),
        3,
        "expected 3 arms (String guard / Tag guard-less / exit), got {arms:?}"
    );

    let is_primitive_variant_arm = |arm: &MatchArm, expected_variant: &str| -> bool {
        // Pattern: Some(Enum::Variant(__ts_union_inner))
        let Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
            fields,
        } = &arm.patterns[0]
        else {
            return false;
        };
        let Some(Pattern::TupleStruct {
            ctor: PatternCtor::UserEnumVariant { variant, .. },
            ..
        }) = fields.first()
        else {
            return false;
        };
        variant == expected_variant && arm.guard.is_some()
    };

    let is_guard_less_named_arm = |arm: &MatchArm, expected_variant: &str| -> bool {
        let Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
            fields,
        } = &arm.patterns[0]
        else {
            return false;
        };
        let Some(Pattern::TupleStruct {
            ctor: PatternCtor::UserEnumVariant { variant, .. },
            ..
        }) = fields.first()
        else {
            return false;
        };
        variant == expected_variant && arm.guard.is_none()
    };

    let is_exit_arm = |arm: &MatchArm| -> bool {
        matches!(arm.patterns[0], Pattern::Wildcard) && arm.guard.is_none()
    };

    assert!(
        is_primitive_variant_arm(&arms[0], "String"),
        "arm 0 must be Some(Union::String(_)) WITH guard, got {:?}",
        arms[0]
    );
    assert!(
        is_guard_less_named_arm(&arms[1], "Tag"),
        "arm 1 must be Some(Union::Tag(_)) WITHOUT guard (JS always-truthy), got {:?}",
        arms[1]
    );
    assert!(
        is_exit_arm(&arms[2]),
        "arm 2 must be `_ => <exit>`, got {:?}",
        arms[2]
    );

    // Verify the Tag arm body re-emits `Tag` variant constructor (guard-less
    // branch must still re-wrap to preserve the union type in the outer `x`).
    let Stmt::TailExpr(Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor { ref variant, .. },
        ..
    }) = arms[1].body[0]
    else {
        panic!(
            "Tag arm body must be TailExpr(Union::Tag(...)), got {:?}",
            arms[1].body
        );
    };
    assert_eq!(variant, "Tag", "Tag arm body must reconstruct Tag variant");
}

/// Locks in [`ir_body_always_exits`] coverage across all control-flow
/// statement kinds. Previously the implementation omitted `Stmt::Match`,
/// silently treating a body whose tail was `match x { a => return, b =>
/// return }` as non-exit — this would cause the consolidated match in
/// `try_generate_option_truthy_complement_match` to be skipped for
/// future cells that emit a match-based exit. Round 2 review finding R2-I1.
#[test]
fn test_ir_body_always_exits_covers_all_exit_shapes() {
    use crate::ir::MatchArm;
    use crate::transformer::statements::control_flow::ir_body_always_exits;

    // Positive: return / break / continue are exits.
    assert!(ir_body_always_exits(&[Stmt::Return(Some(
        Expr::NumberLit(1.0)
    ))]));
    assert!(ir_body_always_exits(&[Stmt::Break {
        label: None,
        value: None,
    }]));
    assert!(ir_body_always_exits(&[Stmt::Continue { label: None }]));

    // Negative: empty body and non-exit tail.
    assert!(!ir_body_always_exits(&[]));
    assert!(!ir_body_always_exits(&[Stmt::Expr(Expr::NumberLit(1.0))]));

    // If: always-exit only when both branches exit.
    let if_both_exit = Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(None)],
        else_body: Some(vec![Stmt::Return(None)]),
    };
    assert!(ir_body_always_exits(&[if_both_exit]));
    let if_only_then_exit = Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(None)],
        else_body: Some(vec![Stmt::Expr(Expr::NumberLit(1.0))]),
    };
    assert!(!ir_body_always_exits(&[if_only_then_exit]));
    let if_no_else = Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(None)],
        else_body: None,
    };
    assert!(!ir_body_always_exits(&[if_no_else]));

    // Match: always-exit iff non-empty AND every arm exits.
    let match_all_exit = Stmt::Match {
        expr: Expr::Ident("x".to_string()),
        arms: vec![
            MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![Stmt::Return(None)],
            },
            MatchArm {
                patterns: vec![Pattern::Literal(Expr::NumberLit(1.0))],
                guard: None,
                body: vec![Stmt::Break {
                    label: None,
                    value: None,
                }],
            },
        ],
    };
    assert!(ir_body_always_exits(&[match_all_exit]));

    let match_one_non_exit = Stmt::Match {
        expr: Expr::Ident("x".to_string()),
        arms: vec![
            MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![Stmt::Return(None)],
            },
            MatchArm {
                patterns: vec![Pattern::Literal(Expr::NumberLit(1.0))],
                guard: None,
                body: vec![Stmt::Expr(Expr::NumberLit(1.0))],
            },
        ],
    };
    assert!(!ir_body_always_exits(&[match_one_non_exit]));

    let match_empty = Stmt::Match {
        expr: Expr::Ident("x".to_string()),
        arms: vec![],
    };
    assert!(
        !ir_body_always_exits(&[match_empty]),
        "empty match cannot be always-exit"
    );
}

#[test]
fn test_convert_stmt_if_no_else() {
    let stmts = parse_fn_body("function f() { if (true) { return 1; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
            else_body: None,
        }
    );
}

#[test]
fn test_convert_stmt_if_else() {
    let stmts = parse_fn_body("function f() { if (true) { return 1; } else { return 2; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
            else_body: Some(vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]),
        }
    );
}

#[test]
fn test_convert_stmt_while() {
    let stmts = parse_fn_body("function f() { while (x > 0) { x = x - 1; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::While {
            label: None,
            condition: Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            },
            body: vec![Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident("x".to_string())),
                value: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Sub,
                    right: Box::new(Expr::NumberLit(1.0)),
                }),
            })],
        }
    );
}

/// Helper: extract (LabeledBlock body, break_check) from a do-while Loop with continue.
/// Asserts: Loop { label, body: [LabeledBlock("__ts_do_while", body), If { break 'label }] }
fn assert_do_while_with_continue(stmt: &Stmt) -> (&Vec<Stmt>, &Stmt) {
    match stmt {
        Stmt::Loop { label, body } => {
            assert!(
                label.is_some(),
                "do-while with continue should have a loop label"
            );
            assert!(
                body.len() >= 2,
                "do-while loop body should have LabeledBlock + break check, got {body:?}"
            );
            let labeled_block = &body[0];
            let break_check = body.last().unwrap();
            match labeled_block {
                Stmt::LabeledBlock {
                    label: block_label,
                    body,
                } => {
                    assert_eq!(
                        block_label, "__ts_do_while",
                        "block label should be 'do_while'"
                    );
                    // Verify break check targets the loop label
                    match break_check {
                        Stmt::If {
                            condition,
                            then_body,
                            else_body,
                        } => {
                            assert!(
                                matches!(condition, Expr::UnaryOp { op: UnOp::Not, .. }),
                                "condition should be negated"
                            );
                            assert_eq!(
                                then_body,
                                &vec![Stmt::Break {
                                    label: label.clone(),
                                    value: None
                                }]
                            );
                            assert!(else_body.is_none());
                        }
                        _ => panic!("expected break check If, got: {break_check:?}"),
                    }
                    (body, break_check)
                }
                _ => panic!("expected LabeledBlock as first stmt, got: {labeled_block:?}"),
            }
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
}

#[test]
fn test_do_while_basic_no_continue_no_labeled_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { do { x = x - 1; } while (x > 0); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Loop { label, body } => {
            assert_eq!(*label, None, "no loop label when no continue");
            // No LabeledBlock — body is [assignment, break_check]
            assert_eq!(body.len(), 2, "body should have stmt + break check");
            assert!(
                matches!(&body[0], Stmt::Expr(Expr::Assign { .. })),
                "expected assignment, got: {:?}",
                body[0]
            );
            match &body[1] {
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    assert!(matches!(condition, Expr::UnaryOp { op: UnOp::Not, .. }));
                    assert_eq!(
                        then_body,
                        &vec![Stmt::Break {
                            label: None,
                            value: None
                        }]
                    );
                    assert!(else_body.is_none());
                }
                _ => panic!("expected break check If"),
            }
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
}

#[test]
fn test_do_while_continue_rewritten_to_break_label() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // continue inside an if inside do-while body
    let stmts =
        parse_fn_body("function f() { do { if (skip) { continue; } x = x + 1; } while (x < 10); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    let (body, _) = assert_do_while_with_continue(&result[0]);
    // First stmt should be If with break 'do_while in then_body
    match &body[0] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("__ts_do_while".to_string()),
                    value: None,
                }],
                "continue should be rewritten to break 'do_while"
            );
        }
        other => panic!("expected If stmt, got: {other:?}"),
    }
}

#[test]
fn test_do_while_break_rewritten_when_continue_present() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Both continue and break inside do-while body
    // continue → break 'do_while, break → break 'do_while_loop
    let stmts = parse_fn_body(
        "function f() { do { if (skip) { continue; } if (done) { break; } x += 1; } while (x < 10); }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // Loop should have label "__ts_do_while_loop" (auto-generated)
    match &result[0] {
        Stmt::Loop { label, .. } => {
            assert_eq!(label, &Some("__ts_do_while_loop".to_string()));
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
    let (body, _) = assert_do_while_with_continue(&result[0]);
    // body[0] = if (skip) { break 'do_while } (rewritten continue)
    match &body[0] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("__ts_do_while".to_string()),
                    value: None,
                }],
                "continue should be rewritten to break 'do_while"
            );
        }
        other => panic!("expected If stmt for continue, got: {other:?}"),
    }
    // body[1] = if (done) { break 'do_while_loop } (rewritten break)
    match &body[1] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("__ts_do_while_loop".to_string()),
                    value: None,
                }],
                "break should be rewritten to break 'do_while_loop"
            );
        }
        other => panic!("expected If stmt for break, got: {other:?}"),
    }
}

#[test]
fn test_do_while_nested_loop_continue_not_rewritten() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // continue inside a nested for-of loop — targets the for-of, not do-while
    // So the do-while should NOT have a LabeledBlock
    let stmts =
        parse_fn_body("function f() { do { for (const x of items) { continue; } } while (cond); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Loop { label, body } => {
            assert_eq!(
                *label, None,
                "no loop label when no continue targets do-while"
            );
            // Body should be [for-in, break_check] (no LabeledBlock)
            assert_eq!(body.len(), 2);
            match &body[0] {
                Stmt::ForIn { body: for_body, .. } => {
                    assert_eq!(
                        *for_body,
                        vec![Stmt::Continue { label: None }],
                        "continue inside nested loop should remain unchanged"
                    );
                }
                other => panic!("expected ForIn stmt, got: {other:?}"),
            }
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
}

#[test]
fn test_do_while_nested_do_while_continues_independent() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Nested do-while: each continue targets its own do-while
    let stmts =
        parse_fn_body("function f() { do { do { continue; } while (a); continue; } while (b); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    let (outer_body, _) = assert_do_while_with_continue(&result[0]);
    // Outer body: [inner_do_while_loop, break 'do_while (rewritten continue)]
    assert_eq!(
        outer_body.len(),
        2,
        "outer body should have inner loop + rewritten continue"
    );

    // Inner do-while should also have LabeledBlock structure
    let (inner_body, _) = assert_do_while_with_continue(&outer_body[0]);
    // Inner continue should be rewritten to break 'do_while (inner's block label)
    assert_eq!(
        inner_body,
        &vec![Stmt::Break {
            label: Some("__ts_do_while".to_string()),
            value: None,
        }],
        "inner continue should be rewritten to break 'do_while"
    );

    // Outer continue should also be rewritten
    assert_eq!(
        outer_body[1],
        Stmt::Break {
            label: Some("__ts_do_while".to_string()),
            value: None,
        },
        "outer continue should be rewritten to break 'do_while"
    );
}

#[test]
fn test_do_while_labeled_in_labeled_stmt() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // labeled do-while: continue outer → break 'do_while
    let stmts = parse_fn_body(
        "function f() { outer: do { if (skip) { continue outer; } x += 1; } while (x < 10); }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // Should be Loop with label "outer"
    match &result[0] {
        Stmt::Loop { label, .. } => {
            assert_eq!(label, &Some("outer".to_string()));
        }
        other => panic!("expected Loop, got: {other:?}"),
    }
    let (body, _) = assert_do_while_with_continue(&result[0]);
    // continue outer should be rewritten to break 'do_while
    match &body[0] {
        Stmt::If { then_body, .. } => {
            assert_eq!(
                then_body,
                &vec![Stmt::Break {
                    label: Some("__ts_do_while".to_string()),
                    value: None,
                }],
                "continue outer should be rewritten to break 'do_while"
            );
        }
        other => panic!("expected If stmt, got: {other:?}"),
    }
}

#[test]
fn test_do_while_labeled_continue_targeting_outer_loop_not_rewritten() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // continue outer targets the while loop, not the do-while
    // So the do-while should NOT have a LabeledBlock
    let stmts = parse_fn_body(
        "function f() { outer: while (true) { do { continue outer; } while (x > 0); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    // Outer is While with label "outer"
    match &result[0] {
        Stmt::While { label, body, .. } => {
            assert_eq!(label, &Some("outer".to_string()));
            // Inner is do-while Loop without LabeledBlock
            match &body[0] {
                Stmt::Loop {
                    label: inner_label,
                    body: inner_body,
                } => {
                    assert_eq!(*inner_label, None, "do-while should have no label");
                    // continue outer should remain as Continue { label: Some("outer") }
                    assert_eq!(
                        inner_body[0],
                        Stmt::Continue {
                            label: Some("outer".to_string()),
                        },
                        "continue outer targeting while loop should not be rewritten"
                    );
                }
                other => panic!("expected Loop, got: {other:?}"),
            }
        }
        other => panic!("expected While, got: {other:?}"),
    }
}

#[test]
fn test_labeled_for_in_stmt() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f() { outer: for (const key in obj) { console.log(key); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::ForIn { label, var, .. } => {
            assert_eq!(label, &Some("outer".to_string()));
            assert_eq!(var, "key");
        }
        other => panic!("expected ForIn, got: {other:?}"),
    }
}

// --- Conditional assignment tests ---

#[test]
fn test_cond_assign_if_option_type_generates_if_let_some() {
    // if (x = getOpt()) { use(x); }
    // When getOpt returns Option<f64>, should generate: if let Some(x) = get_opt() { ... }
    let source =
        "function f(): void { let x: number | null = null; if (x = getOpt()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getOpt".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::Option(Box::new(RustType::F64))),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // Should produce IfLet with Some(x) pattern
    let expected_pat = crate::ir::Pattern::some_binding("x");
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::IfLet { pattern, .. } if *pattern == expected_pat)),
        "expected IfLet with Some(x), got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_if_f64_type_generates_let_and_if_neq_zero() {
    // if (x = getNum()) { use(x); }
    // When getNum returns f64, should generate: let x = get_num(); if x != 0.0 { ... }
    let source = "function f(): void { let x: number = 0; if (x = getNum()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getNum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // Should contain: Let + If with condition x != 0.0
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::Let { name, .. } if name == "x")),
        "expected Let binding for x, got: {:?}",
        result
    );
    assert!(
        result.iter().any(|s| matches!(s, Stmt::If { .. })),
        "expected If statement, got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_while_option_type_generates_while_let_some() {
    // while (x = getOpt()) { use(x); }
    // When getOpt returns Option<f64>, should generate: while let Some(x) = get_opt() { ... }
    let source = "function f(): void { let x: number | null = null; while (x = getOpt()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getOpt".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::Option(Box::new(RustType::F64))),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    let expected_pat = crate::ir::Pattern::some_binding("x");
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::WhileLet { pattern, .. } if *pattern == expected_pat)),
        "expected WhileLet with Some(x), got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_while_f64_type_generates_loop_with_break() {
    // while (x = getNum()) { use(x); }
    // When getNum returns f64, should generate: loop { let x = ...; if x == 0.0 { break; } ... }
    let source =
        "function f(): void { let x: number = 0; while (x = getNum()) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "getNum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    assert!(
        result.iter().any(|s| matches!(s, Stmt::Loop { .. })),
        "expected Loop statement, got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_if_comparison_extracts_assignment() {
    // if ((x = compute()) > 0) { use(x); }
    // Should generate: let x = compute(); if x > 0.0 { ... }
    let source =
        "function f(): void { let x: number = 0; if ((x = compute()) > 0) { console.log(x); } }";
    let mut reg = TypeRegistry::new();
    reg.register(
        "compute".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::Let { name, .. } if name == "x")),
        "expected Let binding for x, got: {:?}",
        result
    );
    assert!(
        result.iter().any(|s| matches!(s, Stmt::If { .. })),
        "expected If with comparison, got: {:?}",
        result
    );
}

#[test]
fn test_cond_assign_normal_if_unchanged() {
    // if (x > 0) { ... } — no assignment, should pass through unchanged
    let source = "function f(): void { let x: number = 1; if (x > 0) { console.log(x); } }";
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // The result should contain an If statement (not a conditional assignment)
    assert!(
        result.iter().any(|s| matches!(s, Stmt::If { .. })),
        "expected If statement, got: {:?}",
        result
    );
}

// ------------------------------------------------------------------
// I-169 T6-2 follow-up structural snapshot (D-5): narrow match
// suppression for closure-reassigned Option<T> variable.
// ------------------------------------------------------------------

#[test]
fn narrowing_match_suppressed_when_closure_reassign_present() {
    // Matrix cell #1 / C2: `if (x === null) return -1;` where `x` is
    // closure-reassigned should emit `if x.is_none() { return -1; }` NOT
    // the match-shadow form `let x = match x { None => return, Some(x) => x };`.
    let source = r#"
        function f(): number {
            let x: number | null = 5;
            if (x === null) return -1;
            const reset = () => { x = null; };
            reset();
            return x + 1;
        }
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // Find the narrow guard output. Expected: Stmt::If { condition: MethodCall(x, "is_none"), ... }.
    let guard_stmt = result.iter().find(|s| matches!(s, Stmt::If { .. }));
    assert!(
        guard_stmt.is_some(),
        "narrow guard must be emitted as an If stmt, got {result:?}"
    );
    match guard_stmt.unwrap() {
        Stmt::If {
            condition,
            else_body,
            ..
        } => {
            assert!(
                else_body.is_none(),
                "narrow guard suppress form must have no else branch"
            );
            // condition should be `x.is_none()` (MethodCall).
            match condition {
                Expr::MethodCall {
                    object,
                    method,
                    args,
                } => {
                    assert_eq!(method, "is_none");
                    assert!(args.is_empty());
                    assert!(matches!(object.as_ref(), Expr::Ident(name) if name == "x"));
                }
                other => panic!("expected MethodCall(is_none), got {other:?}"),
            }
        }
        _ => unreachable!(),
    }

    // Verify no shadow-let (`let x = match x { ... }`) was emitted — the
    // suppression path is mutually exclusive with match-shadow.
    assert!(
        !result.iter().any(|s| matches!(
            s,
            Stmt::Let { init: Some(Expr::Match { .. }), name, .. } if name == "x"
        )),
        "match-shadow `let x = match x {{ ... }}` must be suppressed, got {result:?}"
    );
}
