use super::*;
use crate::ir::CallTarget;

#[test]
fn test_convert_stmt_return_expr() {
    let stmts = parse_fn_body("function f() { return 42; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
}

#[test]
fn test_convert_stmt_return_no_value() {
    let stmts = parse_fn_body("function f() { return; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(result, Stmt::Return(None));
}

#[test]
fn test_convert_stmt_const_decl() {
    let stmts = parse_fn_body("function f() { const x = 1; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(1.0)),
        }
    );
}

#[test]
fn test_convert_stmt_let_decl_initially_immutable() {
    // `let` declarations start immutable; `mark_mutated_vars` adds mut when needed.
    let stmts = parse_fn_body("function f() { let x = 1; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(1.0)),
        }
    );
}

#[test]
fn test_convert_stmt_const_with_type_annotation() {
    let stmts = parse_fn_body("function f() { const x: number = 1; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(
        result,
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(RustType::F64),
            init: Some(Expr::NumberLit(1.0)),
        }
    );
}

#[test]
fn test_convert_stmt_expression_statement() {
    let stmts = parse_fn_body("function f() { foo; }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    assert_eq!(result, Stmt::Expr(Expr::Ident("foo".to_string())));
}

#[test]
fn test_convert_stmt_break_no_label() {
    let stmts = parse_fn_body("function f() { while (true) { break; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::While { body, .. } => {
            assert_eq!(
                body[0],
                Stmt::Break {
                    label: None,
                    value: None
                }
            );
        }
        _ => panic!("expected While"),
    }
}

#[test]
fn test_convert_stmt_continue_no_label() {
    let stmts = parse_fn_body("function f() { while (true) { continue; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::While { body, .. } => {
            assert_eq!(body[0], Stmt::Continue { label: None });
        }
        _ => panic!("expected While"),
    }
}

#[test]
fn test_convert_stmt_break_with_label() {
    let stmts = parse_fn_body("function f() { outer: while (true) { break outer; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::While { label, body, .. } => {
            assert_eq!(label, Some("outer".to_string()));
            assert_eq!(
                body[0],
                Stmt::Break {
                    label: Some("outer".to_string()),
                    value: None,
                }
            );
        }
        _ => panic!("expected labeled While"),
    }
}

#[test]
fn test_convert_stmt_continue_with_label() {
    let stmts = parse_fn_body("function f() { outer: for (const x of items) { continue outer; } }");
    let result = convert_single_stmt(&stmts[0], &TypeRegistry::new(), None);
    match result {
        Stmt::ForIn { label, body, .. } => {
            assert_eq!(label, Some("outer".to_string()));
            assert_eq!(
                body[0],
                Stmt::Continue {
                    label: Some("outer".to_string())
                }
            );
        }
        _ => panic!("expected labeled ForIn"),
    }
}

#[test]
fn test_convert_stmt_nested_fn_decl_generates_closure_let() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function outer() { function inner(x: number): number { return x; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Let {
            name,
            mutable,
            init: Some(Expr::Closure { params, .. }),
            ..
        } => {
            assert_eq!(name, "inner");
            assert!(!mutable);
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
        }
        other => panic!("expected Let with Closure, got: {other:?}"),
    }
}

// --- Spread array expansion tests (SWC AST level) ---

#[test]
fn test_convert_stmt_spread_let_single_spread_optimizes_to_clone() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const x = [...arr] → let x = arr.clone();
    let stmts = parse_fn_body("function f(arr: number[]) { const x = [...arr]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(&result[0], Stmt::Let { name, init: Some(Expr::MethodCall { method, .. }), .. }
            if name == "x" && method == "clone"),
        "expected let x = arr.clone(), got: {result:?}"
    );
}

#[test]
fn test_convert_stmt_spread_let_mixed_segments_expands_to_stmts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const x = [...arr, 1] → let mut x = Vec::new(); x.extend(...); x.push(1.0);
    let stmts = parse_fn_body("function f(arr: number[]) { const x = [...arr, 1]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 3, "expected 3 statements, got: {result:?}");
    // First: let mut x = Vec::new();
    assert!(matches!(
        &result[0],
        Stmt::Let { mutable: true, name, init: Some(Expr::FnCall { target, .. }), .. }
        if name == "x" && matches!(target, CallTarget::ExternalPath(ref __s) if __s.iter().map(String::as_str).eq(["Vec", "new"].iter().copied()))
    ));
    // Second: x.extend(...)
    assert!(matches!(
        &result[1],
        Stmt::Expr(Expr::MethodCall { method, .. }) if method == "extend"
    ));
    // Third: x.push(1.0)
    assert!(matches!(
        &result[2],
        Stmt::Expr(Expr::MethodCall { method, .. }) if method == "push"
    ));
}

#[test]
fn test_convert_stmt_spread_return_single_spread_optimizes_to_clone() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // return [...arr] → return arr.clone();
    let stmts = parse_fn_body("function f(arr: number[]) { return [...arr]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(&result[0], Stmt::Return(Some(Expr::MethodCall { method, .. })) if method == "clone"),
        "expected return arr.clone(), got: {result:?}"
    );
}

#[test]
fn test_convert_stmt_spread_return_mixed_segments_expands_to_stmts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // return [...arr, 1] → let mut __spread_vec = Vec::new(); ...; return __spread_vec;
    let stmts = parse_fn_body("function f(arr: number[]) { return [...arr, 1]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert!(
        result.len() >= 4,
        "expected at least 4 statements, got: {result:?}"
    );
    assert!(matches!(
        &result[0],
        Stmt::Let { mutable: true, name, .. } if name == "__spread_vec"
    ));
    assert!(matches!(
        result.last().unwrap(),
        Stmt::Return(Some(Expr::Ident(n))) if n == "__spread_vec"
    ));
}

#[test]
fn test_convert_stmt_spread_non_spread_array_uses_normal_path() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const x = [1, 2] → let x = vec![1.0, 2.0]; (normal path, no expansion)
    let stmts = parse_fn_body("function f() { const x = [1, 2]; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    assert!(
        matches!(&result[0], Stmt::Let { name, init: Some(Expr::Vec { .. }), .. } if name == "x")
    );
}

#[test]
fn test_convert_stmt_local_interface_skipped() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // interface inside function body should not error, just be skipped
    let stmts = parse_fn_body("function f() { interface Foo { x: number; } const a: number = 1; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Should have 1 statement (const a), interface is skipped
    assert_eq!(
        result.len(),
        1,
        "expected 1 stmt (interface skipped), got {result:?}"
    );
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "a"));
}

#[test]
fn test_convert_stmt_local_type_alias_skipped() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // type alias inside function body should not error, just be skipped
    let stmts = parse_fn_body("function f() { type ID = number; const b: number = 2; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(
        result.len(),
        1,
        "expected 1 stmt (type alias skipped), got {result:?}"
    );
    assert!(matches!(&result[0], Stmt::Let { name, .. } if name == "b"));
}

// --- const mutability body scan ---

#[test]
fn test_const_field_assignment_in_body_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const with number type (primitive, not object) but field assignment in body → let mut
    // is_object_type returns false for number, so body scan must detect the mutation
    let stmts = parse_fn_body("function f() { const p: number = 0; p.x = 1; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // First stmt should be `let mut p = ...`
    match &result[0] {
        Stmt::Let { mutable, name, .. } => {
            assert_eq!(name, "p");
            assert!(
                *mutable,
                "const with field assignment should become let mut"
            );
        }
        other => panic!("expected Let, got {other:?}"),
    }
}

#[test]
fn test_const_mutating_method_in_body_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const arr WITHOUT type annotation, with push() call in body → let mut
    // This tests the body-scan path (not the is_object_type path)
    let stmts = parse_fn_body("function f() { const arr = [1, 2, 3]; arr.push(4); }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    match &result[0] {
        Stmt::Let { mutable, name, .. } => {
            assert_eq!(name, "arr");
            assert!(
                *mutable,
                "const with mutating method call should become let mut"
            );
        }
        other => panic!("expected Let, got {other:?}"),
    }
}

// --- Closure mutable capture ---

#[test]
fn test_closure_mutating_outer_var_closure_binding_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const inc = () => { count += 1; } where closure captures mutably
    // → inc should be `let mut` because calling FnMut requires mutable binding
    let stmts = parse_fn_body(
        "function f() { let count: number = 0; const inc = (): void => { count += 1; }; }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Second stmt: let mut inc = || { ... } (closure binding needs mut for FnMut)
    match &result[1] {
        Stmt::Let { mutable, name, .. } => {
            assert_eq!(name, "inc");
            assert!(*mutable, "closure that captures mutably should be let mut");
        }
        other => panic!("expected Let for inc, got {other:?}"),
    }
}

#[test]
fn test_convert_stmt_empty_stmt_produces_no_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // function f(): void { ; } — the empty statement should produce no IR
    let stmts = parse_fn_body("function f(): void { ; }");
    assert_eq!(stmts.len(), 1, "should parse one empty statement");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(&stmts[0], None)
    }
    .unwrap();
    assert!(
        result.is_empty(),
        "empty statement should produce no IR statements"
    );
}

#[test]
fn test_convert_var_decl_trait_type_generates_box_dyn() {
    // const g: Greeter = ... → let g: Box<dyn Greeter> = ...
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String).into()],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_interface(vec![], vec![], methods, vec![]),
    );
    let stmts = parse_fn_body("function _f(): void { const g: Greeter = null as any; }");
    let stmt = &stmts[0];
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(stmt, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Let { name, ty, .. } => {
            assert_eq!(name, "g");
            assert_eq!(
                *ty,
                Some(RustType::Named {
                    name: "Box".to_string(),
                    type_args: vec![RustType::DynTrait("Greeter".to_string())],
                })
            );
        }
        other => panic!("expected Let, got {:?}", other),
    }
}

// --- mark_mutated_vars comprehensive tests ---

#[test]
fn test_direct_reassignment_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { let x: number = 1; x = 2; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert!(
        matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"),
        "direct reassignment should mark variable as let mut"
    );
}

#[test]
fn test_index_assignment_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f() { let arr: number[] = [1, 2]; arr[0] = 99; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert!(
        matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "arr"),
        "index assignment should mark variable as let mut"
    );
}

#[test]
fn test_no_mutation_remains_immutable() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(): number { let x: number = 1; return x; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert!(
        matches!(&result[0], Stmt::Let { mutable: false, name, .. } if name == "x"),
        "variable without mutation should remain immutable"
    );
}

#[test]
fn test_nested_field_assignment_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // obj.a.b = 1 — nested field assignment should still mark obj as mutable
    let stmts = parse_fn_body("function f() { let obj: number = 0; obj.a.b = 1; }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert!(
        matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "obj"),
        "nested field assignment should mark root variable as let mut: {result:?}"
    );
}

#[test]
fn test_mutation_inside_if_body_detected() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f(cond: boolean) { let x: number = 0; if (cond) { x = 1; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert!(
        matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"),
        "mutation inside if body should mark variable as let mut"
    );
}

#[test]
fn test_mutation_inside_while_body_detected() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f() { let count: number = 0; while (count < 10) { count = count + 1; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert!(
        matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "count"),
        "mutation inside while body should mark variable as let mut"
    );
}

#[test]
fn test_closure_with_field_mutation_becomes_let_mut() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Closure that performs field assignment should mark the closure binding as let mut
    let stmts = parse_fn_body(
        "function f() { let obj: number = 0; const mutator = (): void => { obj.x = 1; }; }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // obj should be mutable (captured and mutated)
    assert!(
        matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "obj"),
        "variable captured and mutated in closure should be let mut: {result:?}"
    );
    // mutator should be mutable (FnMut closure)
    assert!(
        matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "mutator"),
        "closure that mutates captured variable should be let mut: {result:?}"
    );
}
