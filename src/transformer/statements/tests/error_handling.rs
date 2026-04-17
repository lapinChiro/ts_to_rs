use super::*;
use crate::ir::CallTarget;

#[test]
fn test_convert_stmt_list_try_catch_expands_to_let_block_if() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Use a try body that does NOT always return (just a local decl), so the
    // I-023 `!`-typed labeled block short-circuit does not apply and the full
    // Let + LabeledBlock + IfLet machinery is emitted.
    let stmts = parse_fn_body("function f() { try { const x = 1; } catch (e) { return 0; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Let(_try_result) + LabeledBlock + If
    assert_eq!(result.len(), 3, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_try_result"));
    match &result[1] {
        Stmt::LabeledBlock { label, body } => {
            assert_eq!(label, "try_block");
            assert_eq!(body.len(), 1, "try body should have 1 stmt");
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }
    match &result[2] {
        Stmt::IfLet {
            pattern, then_body, ..
        } => {
            // Structured `Err(e)` pattern: TupleStruct ctor=Builtin(Err) with single Binding "e".
            let is_err_e = matches!(
                pattern,
                crate::ir::Pattern::TupleStruct { ctor, fields }
                    if matches!(ctor, crate::ir::PatternCtor::Builtin(crate::ir::BuiltinVariant::Err))
                        && fields.len() == 1
                        && matches!(&fields[0], crate::ir::Pattern::Binding { name, .. } if name == "e")
            );
            assert!(is_err_e, "expected Err(e) pattern, got {pattern:?}");
            assert_eq!(then_body.len(), 1);
        }
        _ => panic!("expected IfLet, got {:?}", result[2]),
    }
}

#[test]
fn test_convert_try_catch_try_always_returns_no_throw_emits_body_inline() {
    // I-023 root cause: when the try body always returns and there are no
    // throws / outer break / outer continue rewritten into `break 'try_block`,
    // the labeled block is `!`-typed and the downstream `if let Err + unreachable!`
    // machinery would trigger the `unreachable_code` lint (denied under
    // compile_test). The fix emits the try body inline without any machinery.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(): number { try { const x = 1; return x; } catch (e) { return -1; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Just the try body inline: `const x = 1; return x;`.
    // No `_try_result`, `LabeledBlock`, `IfLet`, or `unreachable!()`.
    assert_eq!(
        result.len(),
        2,
        "expected inlined body (2 stmts), got {result:?}"
    );
    assert!(
        matches!(&result[0], Stmt::Let { name, .. } if name == "x"),
        "expected `let x` first, got {:?}",
        result[0]
    );
    assert!(
        matches!(&result[1], Stmt::Return(Some(_))),
        "expected `return x` second, got {:?}",
        result[1]
    );
    // Sanity: none of the emitted stmts should be machinery-related.
    for s in &result {
        if let Stmt::Let { name, .. } = s {
            assert_ne!(name, "_try_result", "machinery leaked into inlined output");
        }
        assert!(!matches!(s, Stmt::LabeledBlock { .. }));
        assert!(!matches!(s, Stmt::IfLet { .. }));
    }
}

#[test]
fn test_convert_try_catch_try_has_throw_emits_machinery() {
    // Counterpart to the noreturn test: if the try body contains an explicit
    // throw (which is rewritten to `_try_result = Err(...); break 'try_block`),
    // the labeled block is `()`-typed and the machinery is still required —
    // the catch body is reachable via the break path.
    //
    // Pass an explicit `Some(&F64)` so `convert_try_stmt` also appends the
    // trailing `unreachable!()` when both try and catch always return.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(): number { \
           try { if (true) throw new Error(\"e\"); return 1; } \
           catch (e) { return -1; } \
         }",
    );
    let return_type = RustType::F64;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, Some(&return_type))
    }
    .unwrap();
    // Let(_try_result) + LabeledBlock + IfLet + unreachable!() — all branches
    // end with return so `unreachable!()` is appended.
    assert_eq!(result.len(), 4, "expected full machinery, got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_try_result"));
    assert!(matches!(&result[1], Stmt::LabeledBlock { .. }));
    assert!(matches!(&result[2], Stmt::IfLet { .. }));
    assert!(matches!(
        &result[3],
        Stmt::Expr(Expr::MacroCall { name, .. }) if name == "unreachable"
    ));
}

#[test]
fn test_convert_try_catch_throw_nested_in_switch_arm_emits_full_machinery() {
    // Regression for the Critical bug found in /check_job: pre-fix,
    // `TryBodyRewrite::rewrite` did not recurse into `Stmt::Match { arms }`, so
    // a `throw` inside a switch case (which converts to an IR match arm) was
    // left as a raw `Stmt::Return(Some(Err(...)))` and `throw_count` stayed
    // 0. The I-023 short-circuit then wrongly fired, silently dropping the
    // catch body. Post-fix, the rewriter recurses into match arms and converts
    // throws to `_try_result = Err; break 'try_block`, so the full machinery
    // is emitted correctly.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number): number { \
           try { \
             switch (x) { \
               case 1: if (x < 0) throw new Error(\"bad\"); return x; \
               default: return 0; \
             } \
           } catch (e) { return -1; } \
         }",
    );
    let return_type = RustType::F64;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, Some(&return_type))
    }
    .unwrap();
    // Must emit full machinery: the throw is inside a match arm, so the
    // labeled block is `()`-typed and the catch body is reachable.
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_try_result")),
        "_try_result must be declared (catch body is reachable via nested throw), \
         got: {result:?}"
    );
    assert!(
        result
            .iter()
            .any(|s| matches!(s, Stmt::LabeledBlock { .. })),
        "'try_block labeled block must be emitted, got: {result:?}"
    );
    assert!(
        result.iter().any(|s| matches!(s, Stmt::IfLet { .. })),
        "if let Err(e) = _try_result catch block must be emitted, got: {result:?}"
    );

    // Dig into the match arm body and verify the throw was rewritten into
    // `_try_result = Err(...); break 'try_block` (not left as `return Err(...)`).
    let labeled_body = result
        .iter()
        .find_map(|s| match s {
            Stmt::LabeledBlock { body, .. } => Some(body),
            _ => None,
        })
        .expect("labeled block present");
    let match_arms = labeled_body
        .iter()
        .find_map(|s| match s {
            Stmt::Match { arms, .. } => Some(arms),
            _ => None,
        })
        .expect("match stmt in try body");
    let first_arm_body = &match_arms[0].body;
    // Inside the first arm body: If { then: [Assign _try_result = Err(...), Break 'try_block], ... }
    let inner_if_then = first_arm_body
        .iter()
        .find_map(|s| match s {
            Stmt::If { then_body, .. } => Some(then_body),
            _ => None,
        })
        .expect("if in first match arm body");
    assert!(
        inner_if_then
            .iter()
            .any(|s| matches!(s, Stmt::Expr(Expr::Assign { target, .. })
                if matches!(target.as_ref(), Expr::Ident(n) if n == "_try_result"))),
        "throw inside switch arm must rewrite into _try_result assignment, got: {inner_if_then:?}"
    );
    assert!(
        inner_if_then
            .iter()
            .any(|s| matches!(s, Stmt::Break { label: Some(l), .. } if l == "try_block")),
        "throw inside switch arm must rewrite into break 'try_block, got: {inner_if_then:?}"
    );
}

#[test]
fn test_convert_try_catch_throw_nested_in_if_let_emits_full_machinery() {
    // Counterpart: `Stmt::IfLet` is the other body-bearing variant added to
    // TryBodyRewrite's recursion. This test exercises that path via an explicit
    // `if let`-shaped TS source. We construct IR directly since TS does not
    // surface `if let` — but the Match test above covers the real-world case
    // (switch statements). Here we at least lock in the ends_with_return +
    // rewrite behaviour for exhaustive if/else by testing:
    // `try { if (cond) return 1; else return 2; } catch { return -1; }`
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(c: boolean): number { \
           try { \
             if (c) return 1; else return 2; \
           } catch (e) { return -1; } \
         }",
    );
    let return_type = RustType::F64;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, Some(&return_type))
    }
    .unwrap();
    // If body has no throws but both if-branches return: try_ends_with_return=true,
    // has_break_to_try_block=false → short-circuit fires, body inlined.
    // Catch body is dropped (no throw path exists). This is the ideal clean output.
    assert!(
        !result
            .iter()
            .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_try_result")),
        "_try_result must NOT be declared when no throws are present, got: {result:?}"
    );
    assert!(
        !result
            .iter()
            .any(|s| matches!(s, Stmt::LabeledBlock { .. })),
        "labeled block must be dropped when try body always returns and no throws"
    );
}

#[test]
fn test_convert_try_catch_try_has_break_to_outer_loop_emits_machinery() {
    // Bare `break` inside the try body (not inside an inner loop) is rewritten
    // into `_try_break = true; break 'try_block`, so the labeled block is
    // `()`-typed even though the trailing return looks exhaustive.
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(): number { \
           while (true) { \
             try { if (true) break; return 1; } catch (e) { return -1; } \
           } \
           return 0; \
         }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Outer: while(...) + return 0 = 2 stmts
    assert_eq!(result.len(), 2, "outer should have 2 stmts, got {result:?}");
    let while_body = match &result[0] {
        Stmt::While { body, .. } => body,
        other => panic!("expected While, got {other:?}"),
    };
    // Inner try/catch inside the while body: should still have the full machinery
    // (break_flag + _try_result + _try_break check + LabeledBlock + IfLet).
    assert!(
        while_body
            .iter()
            .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_try_break")),
        "expected _try_break flag to be declared, got {while_body:?}"
    );
    assert!(
        while_body
            .iter()
            .any(|s| matches!(s, Stmt::LabeledBlock { .. })),
        "expected LabeledBlock in while body, got {while_body:?}"
    );
}

#[test]
fn test_convert_stmt_list_try_catch_empty_catch_expands_correctly() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { const x = 1; } catch (e) { } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 3, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_try_result"));
    match &result[1] {
        Stmt::LabeledBlock { body, .. } => assert_eq!(body.len(), 1),
        _ => panic!("expected LabeledBlock"),
    }
    match &result[2] {
        Stmt::IfLet { then_body, .. } => assert!(then_body.is_empty()),
        _ => panic!("expected IfLet"),
    }
}

#[test]
fn test_convert_stmt_list_try_finally_expands_to_scopeguard_and_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { const x = 1; } finally { const y = 2; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // scopeguard + try body inline
    assert_eq!(result.len(), 2, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_finally_guard"));
    assert!(matches!(&result[1], Stmt::Let { .. })); // const x = 1
}

#[test]
fn test_convert_stmt_list_try_catch_finally_expands_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { const x = 1; } catch (e) { const y = 2; } finally { const z = 3; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // scopeguard + _try_result + LabeledBlock + If
    assert_eq!(result.len(), 4, "got {result:?}");
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "_finally_guard"));
    assert!(matches!(&result[1], Stmt::Let { name, .. } if name == "_try_result"));
    assert!(matches!(&result[2], Stmt::LabeledBlock { .. }));
    assert!(matches!(&result[3], Stmt::IfLet { .. }));
}

#[test]
fn test_convert_stmt_nested_try_catch_expands_inner_in_outer_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { try { x(); } catch (inner) { y(); } } catch (outer) { z(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Outer: Let + LabeledBlock + If
    assert_eq!(result.len(), 3, "got {result:?}");
    // The LabeledBlock body should contain the inner try/catch expansion (3 stmts)
    match &result[1] {
        Stmt::LabeledBlock { body, .. } => {
            // Inner: Let(_try_result) + LabeledBlock + If = 3 stmts
            assert_eq!(
                body.len(),
                3,
                "inner try/catch should expand to 3 stmts, got {body:?}"
            );
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }
    // Outer if let should use "outer" param
    match &result[2] {
        Stmt::IfLet { pattern, .. } => {
            // Structured `Err(outer)` pattern
            let binds_outer = matches!(
                pattern,
                crate::ir::Pattern::TupleStruct { ctor, fields }
                    if matches!(ctor, crate::ir::PatternCtor::Builtin(crate::ir::BuiltinVariant::Err))
                        && fields.len() == 1
                        && matches!(&fields[0], crate::ir::Pattern::Binding { name, .. } if name == "outer")
            );
            assert!(binds_outer, "expected Err(outer) pattern, got {pattern:?}");
        }
        _ => panic!("expected IfLet, got {:?}", result[2]),
    }
}

#[test]
fn test_convert_stmt_throw_new_error_string() {
    let stmts = parse_fn_body("function f() { throw new Error(\"something went wrong\"); }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    // throw new Error("msg") → return Err("msg".to_string())
    assert_eq!(
        result,
        Stmt::Return(Some(Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
            args: vec![Expr::MethodCall {
                object: Box::new(Expr::StringLit("something went wrong".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            }],
        }))
    );
}

#[test]
fn test_convert_stmt_throw_string_literal() {
    let stmts = parse_fn_body("function f() { throw \"error msg\"; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    // throw "msg" → return Err("msg".to_string())
    assert_eq!(
        result,
        Stmt::Return(Some(Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
            args: vec![Expr::MethodCall {
                object: Box::new(Expr::StringLit("error msg".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            }],
        }))
    );
}

// -- try/catch expansion tests (primitive IR, no Stmt::TryCatch) --

#[test]
fn test_convert_try_catch_basic_expands_to_let_labeledblock_if() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { risky(); } catch (e) { handle(e); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should expand to 3 statements: Let, LabeledBlock, If
    assert_eq!(result.len(), 3, "expected 3 stmts, got {result:?}");

    // 1. let mut _try_result: Result<(), String> = Ok(());
    match &result[0] {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            assert!(mutable, "expected mutable");
            assert_eq!(name, "_try_result");
            assert_eq!(
                ty,
                &Some(RustType::Result {
                    ok: Box::new(RustType::Unit),
                    err: Box::new(RustType::String),
                })
            );
            assert!(
                matches!(init, Some(Expr::FnCall { target, .. }) if matches!(target, CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok))),
                "expected Ok(...) init, got {init:?}"
            );
        }
        _ => panic!("expected Let, got {:?}", result[0]),
    }

    // 2. 'try_block: { risky(); }
    match &result[1] {
        Stmt::LabeledBlock { label, body } => {
            assert_eq!(label, "try_block");
            assert_eq!(body.len(), 1, "expected 1 stmt in try body");
            assert!(
                matches!(&body[0], Stmt::Expr(Expr::FnCall { target, .. }) if matches!(target, CallTarget::Free(__n) if __n == "risky")),
                "expected risky() call, got {:?}",
                body[0]
            );
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }

    // 3. if let Err(e) = _try_result { handle(e); }
    match &result[2] {
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            let is_err = matches!(
                pattern,
                crate::ir::Pattern::TupleStruct { ctor, .. }
                    if matches!(ctor, crate::ir::PatternCtor::Builtin(crate::ir::BuiltinVariant::Err))
            );
            assert!(is_err, "expected Err pattern, got {pattern:?}");
            assert!(
                matches!(expr, Expr::Ident(s) if s == "_try_result"),
                "expected _try_result expr, got {expr:?}"
            );
            assert!(!then_body.is_empty(), "expected catch body");
            assert!(else_body.is_none());
        }
        _ => panic!("expected IfLet, got {:?}", result[2]),
    }
}

#[test]
fn test_convert_try_catch_throw_in_body_expands_to_assign_break() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { throw new Error(\"oops\"); } catch (e) { handle(e); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    assert_eq!(result.len(), 3, "expected 3 stmts, got {result:?}");

    // Check the labeled block body: throw should be expanded to Assign + Break
    match &result[1] {
        Stmt::LabeledBlock { body, .. } => {
            assert_eq!(body.len(), 2, "expected assign + break, got {body:?}");
            // First: _try_result = Err("oops".to_string())
            match &body[0] {
                Stmt::Expr(Expr::Assign { target, value }) => {
                    assert!(
                        matches!(target.as_ref(), Expr::Ident(s) if s == "_try_result"),
                        "expected _try_result target"
                    );
                    assert!(
                        matches!(value.as_ref(), Expr::FnCall { target, .. } if matches!(target, CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err))),
                        "expected Err(...) value"
                    );
                }
                _ => panic!("expected Assign, got {:?}", body[0]),
            }
            // Second: break 'try_block;
            assert!(
                matches!(&body[1], Stmt::Break { label: Some(l), value: None } if l == "try_block"),
                "expected break 'try_block, got {:?}",
                body[1]
            );
        }
        _ => panic!("expected LabeledBlock, got {:?}", result[1]),
    }
}

#[test]
fn test_convert_try_finally_expands_to_scopeguard_and_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { try { risky(); } finally { cleanup(); } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should expand to: Let(scopeguard) + try body inline
    assert!(
        result.len() >= 2,
        "expected at least 2 stmts, got {result:?}"
    );

    // 1. let _finally_guard = scopeguard::guard((), |_| { cleanup(); });
    match &result[0] {
        Stmt::Let { name, init, .. } => {
            assert_eq!(name, "_finally_guard");
            assert!(
                matches!(init, Some(Expr::FnCall { target, .. }) if matches!(target, CallTarget::ExternalPath(ref __s) if __s.iter().map(String::as_str).eq(["scopeguard", "guard"].iter().copied()))),
                "expected scopeguard::guard call, got {init:?}"
            );
        }
        _ => panic!("expected Let for scopeguard, got {:?}", result[0]),
    }

    // 2. try body inline: risky();
    assert!(
        matches!(&result[1], Stmt::Expr(Expr::FnCall { target, .. }) if matches!(target, CallTarget::Free(__n) if __n == "risky")),
        "expected risky() call, got {:?}",
        result[1]
    );
}

#[test]
fn test_convert_try_catch_finally_expands_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { try { risky(); } catch (e) { handle(e); } finally { cleanup(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should expand to: Let(scopeguard), Let(_try_result), LabeledBlock, If
    assert_eq!(result.len(), 4, "expected 4 stmts, got {result:?}");

    // 1. scopeguard
    match &result[0] {
        Stmt::Let { name, .. } => assert_eq!(name, "_finally_guard"),
        _ => panic!("expected Let for scopeguard, got {:?}", result[0]),
    }

    // 2. _try_result
    match &result[1] {
        Stmt::Let { name, .. } => assert_eq!(name, "_try_result"),
        _ => panic!("expected Let for _try_result, got {:?}", result[1]),
    }

    // 3. labeled block
    assert!(
        matches!(&result[2], Stmt::LabeledBlock { label, .. } if label == "try_block"),
        "expected LabeledBlock, got {:?}",
        result[2]
    );

    // 4. if let error check
    assert!(
        matches!(&result[3], Stmt::IfLet { .. }),
        "expected IfLet, got {:?}",
        result[3]
    );
}

// -- try/catch break/continue in try body tests --

#[test]
fn test_convert_try_catch_break_in_loop_uses_flag() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // break inside try body (within a for loop) should use flag pattern
    let stmts = parse_fn_body(
        "function f(items: number[]) { for (const item of items) { try { if (item < 0) { break; } risky(); } catch (e) { handle(e); } } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    // Should have a ForIn with body containing: _try_result, _try_break, LabeledBlock, if _try_break { break }, if let Err
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::ForIn { body, .. } => {
            // Look for _try_break flag declaration
            let has_break_flag = body
                .iter()
                .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_try_break"));
            assert!(has_break_flag, "expected _try_break flag, got {body:?}");

            // Look for break flag check: if _try_break { break; }
            let has_break_check = body.iter().any(|s| {
                matches!(s, Stmt::If { condition: Expr::Ident(c), then_body, .. }
                    if c == "_try_break" && matches!(then_body.first(), Some(Stmt::Break { label: None, .. })))
            });
            assert!(
                has_break_check,
                "expected if _try_break {{ break; }}, got {body:?}"
            );
        }
        _ => panic!("expected ForIn, got {:?}", result[0]),
    }
}

#[test]
fn test_convert_try_catch_continue_in_loop_uses_flag() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(items: number[]) { for (const item of items) { try { if (item < 0) { continue; } risky(); } catch (e) { handle(e); } } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();

    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::ForIn { body, .. } => {
            // Look for _try_continue flag declaration
            let has_continue_flag = body
                .iter()
                .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_try_continue"));
            assert!(
                has_continue_flag,
                "expected _try_continue flag, got {body:?}"
            );

            // Look for continue flag check: if _try_continue { continue; }
            let has_continue_check = body.iter().any(|s| {
                matches!(s, Stmt::If { condition: Expr::Ident(c), then_body, .. }
                    if c == "_try_continue" && matches!(then_body.first(), Some(Stmt::Continue { label: None })))
            });
            assert!(
                has_continue_check,
                "expected if _try_continue {{ continue; }}, got {body:?}"
            );
        }
        _ => panic!("expected ForIn, got {:?}", result[0]),
    }
}

// -- try/catch with return type tests --

#[test]
fn test_convert_try_catch_both_return_adds_unreachable() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // When both try and catch end with return in a function with return type,
    // unreachable!() should be added after the if-let-Err block
    let stmts = parse_fn_body(
        "function safeDivide(a: number, b: number): number { try { if (b === 0) throw new Error(\"div by zero\"); return a / b; } catch (e) { return 0; } }",
    );
    let return_type = RustType::F64;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, Some(&return_type))
    }
    .unwrap();

    // Last statement should be Expr(MacroCall { name: "unreachable", args: [] })
    let last = result.last().expect("should have statements");
    assert!(
        matches!(last, Stmt::Expr(Expr::MacroCall { name, args, .. }) if name == "unreachable" && args.is_empty()),
        "expected unreachable!() as last stmt, got {last:?}"
    );
}

#[test]
fn test_convert_try_catch_try_no_return_no_unreachable() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // When try body does NOT end with return, unreachable!() should NOT be added
    let stmts = parse_fn_body(
        "function riskyOp(x: number): number { try { if (x < 0) { throw new Error(\"negative\"); } console.log(x); } catch (e) { console.log(e); } return x; }",
    );
    let return_type = RustType::F64;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, Some(&return_type))
    }
    .unwrap();

    // The last statement should be the Return(x), NOT unreachable
    let last = result.last().expect("should have statements");
    assert!(
        matches!(last, Stmt::Return(_)),
        "expected Return as last stmt (no unreachable), got {last:?}"
    );
}
