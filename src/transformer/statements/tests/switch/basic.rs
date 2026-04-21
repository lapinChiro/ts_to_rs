//! Basic switch rewrite: single case / empty fallthrough / default /
//! break-less fallthrough (→ `LabeledBlock` + `_fall` flag) /
//! return-terminated / throw-terminated cases.
//!
//! All tests use a numeric discriminant so arm patterns are
//! `Wildcard + guard` (per I-315: f64 cannot be used as a pattern
//! literal).

use super::*;

#[test]
fn test_convert_switch_single_case_break_generates_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(x: number) { switch(x) { case 1: doA(); break; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            // f64 cases use guard instead of literal pattern (I-315)
            assert!(
                arms[0]
                    .patterns
                    .iter()
                    .all(|p| matches!(p, crate::ir::Pattern::Wildcard)),
                "numeric case should use wildcard + guard, not literal pattern"
            );
            assert!(arms[0].guard.is_some(), "numeric case should have a guard");
            assert!(!arms[0].body.is_empty());
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_empty_fallthrough_merges_patterns() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f(x: number) { switch(x) { case 1: case 2: doAB(); break; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            // f64 cases are merged via combined guard (x == 1.0 || x == 2.0)
            assert!(
                arms[0].guard.is_some(),
                "merged numeric cases should have a combined guard"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_default_generates_wildcard() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); break; default: doB(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // f64 case uses guard (I-315)
            assert!(arms[0].guard.is_some(), "numeric case should have a guard");
            assert!(
                arms[1]
                    .patterns
                    .iter()
                    .any(|p| matches!(p, crate::ir::Pattern::Wildcard)),
                "last arm should be wildcard"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_fallthrough_generates_labeled_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // break-less fall-through: case 1 falls into case 2
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); case 2: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    // Fall-through path generates a LabeledBlock with flag pattern.
    // I-154 rename: "switch" → "__ts_switch".
    match &result[0] {
        Stmt::LabeledBlock { label, body } => {
            assert_eq!(label, "__ts_switch");
            // Should contain: let mut _fall = false; + if chains
            let has_fall_flag = body
                .iter()
                .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_fall"));
            assert!(has_fall_flag, "expected _fall flag, got {body:?}");
        }
        other => panic!("expected LabeledBlock for fall-through, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_return_terminated_case_generates_clean_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // case ending with return should be treated as terminated → clean match, not fall-through
    let stmts = parse_fn_body(
        "function f(x: number): string { switch(x) { case 1: return \"one\"; case 2: return \"two\"; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // Both arms should have return statements
            assert!(matches!(arms[0].body.last(), Some(Stmt::Return(_))));
            assert!(matches!(arms[1].body.last(), Some(Stmt::Return(_))));
        }
        other => panic!("expected Match (not LabeledBlock), got {other:?}"),
    }
}

#[test]
fn test_convert_switch_throw_terminated_case_generates_clean_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); throw new Error(\"fail\"); case 2: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2, "expected 2 arms, got {arms:?}");
        }
        other => panic!("expected Match (not LabeledBlock), got {other:?}"),
    }
}
