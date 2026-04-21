//! I-169 T6-2 follow-up structural snapshot (D-5): narrow match
//! suppression for closure-reassigned `Option<T>` variable.
//!
//! When the narrowing analyzer detects a closure reassigning an outer
//! narrow-candidate variable, the narrow materialization (match-shadow
//! `let x = match x { None => return, Some(x) => x };`) is suppressed in
//! favor of the preserved-Option form (`if x.is_none() { return ...; }`)
//! so that subsequent `x = null;` inside the closure body remains
//! typechecked.

use super::*;

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
