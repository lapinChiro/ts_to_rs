//! String discriminant: `switch(s)` where `s: string` produces
//! `Pattern::Literal(StringLit(_))` patterns (no guard rewrite — unlike
//! numeric discriminants which hit the I-315 `Wildcard + guard` path).

use super::*;

#[test]
fn test_convert_switch_string_discriminant_generates_string_patterns() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(s: string) { switch(s) { case \"hello\": doA(); break; case \"world\": doB(); break; } }",
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
            // Patterns should be StringLit
            assert!(
                arms[0].patterns.iter().any(|p| matches!(
                    p,
                    crate::ir::Pattern::Literal(Expr::StringLit(s)) if s == "hello"
                )),
                "expected string pattern 'hello', got {:?}",
                arms[0].patterns
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}
