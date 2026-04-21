use super::*;
use crate::ir::ClosureBody;

// --- contains_throw recursion tests ---

/// Helper: check if the function's return type is Result
fn fn_returns_result(source: &str) -> bool {
    let fn_decl = parse_fn_decl(source);
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_fn_decl(&fn_decl, Visibility::Public, false)
        .unwrap()
        .0;
    let item = items.last().unwrap();
    matches!(
        item,
        Item::Fn {
            return_type: Some(RustType::Result { .. }),
            ..
        }
    )
}

#[test]
fn test_contains_throw_in_for_loop_wraps_result() {
    assert!(fn_returns_result(
        "function f(n: number) { for (let i = 0; i < n; i++) { throw new Error(\"x\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_while_loop_wraps_result() {
    assert!(fn_returns_result(
        "function f() { while (true) { throw new Error(\"x\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_do_while_wraps_result() {
    assert!(fn_returns_result(
        "function f() { do { throw new Error(\"x\"); } while (true); }"
    ));
}

#[test]
fn test_contains_throw_in_switch_detected() {
    // switch is not yet supported by convert_stmt, so test contains_throw directly
    let fn_decl =
        parse_fn_decl("function f(x: number) { switch(x) { case 1: throw new Error(\"x\"); } }");
    let block = fn_decl.function.body.as_ref().unwrap();
    assert!(
        contains_throw(&block.stmts),
        "should detect throw inside switch case"
    );
}

#[test]
fn test_contains_throw_in_for_of_wraps_result() {
    assert!(fn_returns_result(
        "function f(arr: string[]) { for (const x of arr) { throw new Error(\"x\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_try_block_excluded() {
    assert!(!fn_returns_result(
        "function f() { try { throw new Error(\"x\"); } catch(e) {} }"
    ));
}

#[test]
fn test_contains_throw_in_catch_block_wraps_result() {
    assert!(fn_returns_result(
        "function f() { try { } catch(e) { throw new Error(\"rethrow\"); } }"
    ));
}

#[test]
fn test_contains_throw_in_labeled_wraps_result() {
    assert!(fn_returns_result(
        "function f() { outer: while(true) { throw new Error(\"x\"); } }"
    ));
}

// --- convert_last_return_to_tail tests ---

#[test]
fn test_convert_last_return_to_tail_converts_final_return() {
    let mut body = vec![
        Stmt::Expr(Expr::Ident("setup".to_string())),
        Stmt::Return(Some(Expr::Ident("x".to_string()))),
    ];
    convert_last_return_to_tail(&mut body);
    assert_eq!(body.len(), 2);
    assert_eq!(body[1], Stmt::TailExpr(Expr::Ident("x".to_string())));
}

#[test]
fn test_convert_last_return_to_tail_preserves_non_final_return() {
    let mut body = vec![
        Stmt::Return(Some(Expr::Ident("early".to_string()))),
        Stmt::Expr(Expr::Ident("x".to_string())),
    ];
    convert_last_return_to_tail(&mut body);
    // Non-final return should remain unchanged
    assert_eq!(
        body[0],
        Stmt::Return(Some(Expr::Ident("early".to_string())))
    );
}

#[test]
fn test_convert_last_return_to_tail_skips_return_none() {
    let mut body = vec![Stmt::Return(None)];
    convert_last_return_to_tail(&mut body);
    // Return(None) cannot be a tail expression — should remain unchanged
    assert_eq!(body[0], Stmt::Return(None));
}

#[test]
fn test_convert_last_return_to_tail_empty_body_noop() {
    let mut body: Vec<Stmt> = vec![];
    convert_last_return_to_tail(&mut body);
    assert!(body.is_empty());
}

// --- mark_mut_params_from_body tests ---

/// I-255: `x++` (converted to `x += 1`) should detect parameter mutation
#[test]
fn test_param_rebinding_for_assign_op() {
    // Body: x = x + 1 (IR representation of x++)
    let body = vec![Stmt::Expr(Expr::Assign {
        target: Box::new(Expr::Ident("x".to_string())),
        value: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        }),
    })];
    let params = vec![Param {
        name: "x".to_string(),
        ty: Some(RustType::F64),
    }];
    let rebindings = mark_mut_params_from_body(&body, &params, &std::collections::HashSet::new());
    assert_eq!(
        rebindings.len(),
        1,
        "x++ (Assign) should trigger parameter rebinding"
    );
    assert!(matches!(
        &rebindings[0],
        Stmt::Let { mutable: true, name, .. } if name == "x"
    ));
}

/// I-258: mutation inside a closure should detect parameter mutation
#[test]
fn test_param_rebinding_for_closure_mutation() {
    // Body: let f = || { items.push(1) };
    let body = vec![Stmt::Let {
        mutable: false,
        name: "f".to_string(),
        ty: None,
        init: Some(Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Block(vec![Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident("items".to_string())),
                method: "push".to_string(),
                args: vec![Expr::NumberLit(1.0)],
            })]),
        }),
    }];
    let params = vec![Param {
        name: "items".to_string(),
        ty: Some(RustType::Vec(Box::new(RustType::F64))),
    }];
    let rebindings = mark_mut_params_from_body(&body, &params, &std::collections::HashSet::new());
    assert_eq!(
        rebindings.len(),
        1,
        "closure mutation (items.push) should trigger parameter rebinding"
    );
    assert!(matches!(
        &rebindings[0],
        Stmt::Let { mutable: true, name, .. } if name == "items"
    ));
}

/// I-335: user-defined `&mut self` method call should detect parameter mutation
#[test]
fn test_param_rebinding_for_user_defined_mut_method() {
    // Body: counter.increment()
    let body = vec![Stmt::Expr(Expr::MethodCall {
        object: Box::new(Expr::Ident("counter".to_string())),
        method: "increment".to_string(),
        args: vec![],
    })];
    let params = vec![Param {
        name: "counter".to_string(),
        ty: None,
    }];
    // With empty extra_mut_methods, should NOT detect
    let rebindings = mark_mut_params_from_body(&body, &params, &std::collections::HashSet::new());
    assert_eq!(
        rebindings.len(),
        0,
        "without extra_mut_methods, increment() should not trigger rebinding"
    );

    // With "increment" in extra_mut_methods, SHOULD detect
    let mut extra = std::collections::HashSet::new();
    extra.insert("increment".to_string());
    let rebindings = mark_mut_params_from_body(&body, &params, &extra);
    assert_eq!(
        rebindings.len(),
        1,
        "with extra_mut_methods containing 'increment', should trigger rebinding"
    );
    assert!(matches!(
        &rebindings[0],
        Stmt::Let { mutable: true, name, .. } if name == "counter"
    ));
}

/// I-335: nested field access through `&mut self` method should mark root variable
#[test]
fn test_param_rebinding_for_nested_field_mut_method() {
    // Body: obj.inner.increment() where inner is a field and increment is &mut self
    let body = vec![Stmt::Expr(Expr::MethodCall {
        object: Box::new(Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "inner".to_string(),
        }),
        method: "increment".to_string(),
        args: vec![],
    })];
    let params = vec![Param {
        name: "obj".to_string(),
        ty: None,
    }];
    let mut extra = std::collections::HashSet::new();
    extra.insert("increment".to_string());
    let rebindings = mark_mut_params_from_body(&body, &params, &extra);
    assert_eq!(
        rebindings.len(),
        1,
        "obj.inner.increment() should mark root 'obj' for rebinding"
    );
    assert!(matches!(
        &rebindings[0],
        Stmt::Let { mutable: true, name, .. } if name == "obj"
    ));
}

/// I-258: mutation inside control flow inside body should detect parameter mutation
#[test]
fn test_param_rebinding_for_control_flow_mutation() {
    // Body: if (cond) { count = count + 1; }
    let body = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Expr(Expr::Assign {
            target: Box::new(Expr::Ident("count".to_string())),
            value: Box::new(Expr::NumberLit(1.0)),
        })],
        else_body: None,
    }];
    let params = vec![Param {
        name: "count".to_string(),
        ty: Some(RustType::F64),
    }];
    let rebindings = mark_mut_params_from_body(&body, &params, &std::collections::HashSet::new());
    assert_eq!(
        rebindings.len(),
        1,
        "mutation inside if-body should trigger parameter rebinding"
    );
}

// --- append_implicit_none_if_needed tests ---

fn none_expr() -> Expr {
    Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)
}

#[test]
fn implicit_none_appended_for_if_without_else() {
    let mut body = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
        else_body: None,
    }];
    append_implicit_none_if_needed(&mut body, Some(&RustType::Option(Box::new(RustType::F64))));
    assert_eq!(body.len(), 2);
    assert_eq!(body[1], Stmt::TailExpr(none_expr()));
}

#[test]
fn implicit_none_appended_for_if_with_else_where_branches_fall_through() {
    // Both branches have inner ifs that may not return — the key cell-i025 case.
    let mut body = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
            else_body: None,
        }],
        else_body: Some(vec![Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Return(Some(Expr::NumberLit(2.0)))],
            else_body: None,
        }]),
    }];
    append_implicit_none_if_needed(&mut body, Some(&RustType::Option(Box::new(RustType::F64))));
    assert_eq!(
        body.len(),
        2,
        "None should be appended after if-else with fall-through branches"
    );
    assert_eq!(body[1], Stmt::TailExpr(none_expr()));
}

#[test]
fn implicit_none_not_appended_when_all_paths_return() {
    let mut body = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
        else_body: Some(vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]),
    }];
    append_implicit_none_if_needed(&mut body, Some(&RustType::Option(Box::new(RustType::F64))));
    assert_eq!(
        body.len(),
        1,
        "should NOT append None when all paths return"
    );
}

#[test]
fn implicit_none_not_appended_when_tail_expr_present() {
    let mut body = vec![Stmt::TailExpr(Expr::MethodCall {
        object: Box::new(Expr::Ident("x".to_string())),
        method: "clone".to_string(),
        args: vec![],
    })];
    append_implicit_none_if_needed(&mut body, Some(&RustType::Option(Box::new(RustType::F64))));
    assert_eq!(body.len(), 1, "should NOT append None when TailExpr exists");
}

#[test]
fn implicit_none_appended_for_empty_body() {
    let mut body: Vec<Stmt> = vec![];
    append_implicit_none_if_needed(&mut body, Some(&RustType::Option(Box::new(RustType::F64))));
    assert_eq!(body.len(), 1, "empty Option body should get None");
    assert_eq!(body[0], Stmt::TailExpr(none_expr()));
}

#[test]
fn implicit_none_skipped_for_non_option_return() {
    let mut body = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
        else_body: None,
    }];
    append_implicit_none_if_needed(&mut body, Some(&RustType::F64));
    assert_eq!(
        body.len(),
        1,
        "should NOT append None for non-Option return type"
    );
}

#[test]
fn implicit_none_skipped_for_no_return_type() {
    let mut body = vec![Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
        else_body: None,
    }];
    append_implicit_none_if_needed(&mut body, None);
    assert_eq!(
        body.len(),
        1,
        "should NOT append None when return type is absent"
    );
}

#[test]
fn implicit_none_appended_for_while_loop() {
    let mut body = vec![Stmt::While {
        label: None,
        condition: Expr::BoolLit(true),
        body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
    }];
    append_implicit_none_if_needed(&mut body, Some(&RustType::Option(Box::new(RustType::F64))));
    assert_eq!(body.len(), 2, "while loop should get trailing None");
    assert_eq!(body[1], Stmt::TailExpr(none_expr()));
}

#[test]
fn implicit_none_appended_for_let_binding_at_end() {
    let mut body = vec![Stmt::Let {
        mutable: false,
        name: "x".to_string(),
        ty: None,
        init: Some(Expr::NumberLit(1.0)),
    }];
    append_implicit_none_if_needed(&mut body, Some(&RustType::Option(Box::new(RustType::F64))));
    assert_eq!(body.len(), 2, "let binding at end should get trailing None");
    assert_eq!(body[1], Stmt::TailExpr(none_expr()));
}
