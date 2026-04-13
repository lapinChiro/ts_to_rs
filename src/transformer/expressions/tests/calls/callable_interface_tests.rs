use super::super::*;
use crate::ir::CallTarget;

/// P10.2: Callable interface call dispatch produces MethodCall (not Free).
///
/// Verifies that `try_convert_callable_trait_call` correctly routes
/// callable interface calls through the 2-stage ConstValue → Struct lookup
/// and generates `Expr::MethodCall { method: "call_N", .. }`.
#[test]
fn test_callable_interface_call_dispatches_to_method_call() {
    let source = r#"
        interface GetValue {
            (key: string): string;
        }
        const getValue: GetValue = (key: string): string => {
            return key;
        };
        getValue("test");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    // Extract the call expression: getValue("test")
    let module = f.module();
    let call_stmt = module.body.last().unwrap();
    let call_expr = match call_stmt {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(call_expr)
        .unwrap();
    // Should be MethodCall (not FnCall with CallTarget::Free)
    match &result {
        Expr::MethodCall { object, method, .. } => {
            assert!(matches!(object.as_ref(), Expr::Ident(name) if name == "getValue"));
            assert_eq!(method, "call_0");
        }
        other => panic!("expected MethodCall, got {other:?}"),
    }
}

/// P10.2: Multi-overload arity selection at call site.
///
/// 2-arg call dispatches to call_1 (not call_0).
#[test]
fn test_callable_interface_multi_overload_selects_by_arity() {
    let source = r#"
        interface GetCookie {
            (c: string): string;
            (c: string, key: string): number;
        }
        const getCookie: GetCookie = (c: string, key?: string): string => {
            return c;
        };
        getCookie("ctx", "name");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let module = f.module();
    let call_stmt = module.body.last().unwrap();
    let call_expr = match call_stmt {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(call_expr)
        .unwrap();
    match &result {
        Expr::MethodCall { object, method, .. } => {
            assert!(matches!(object.as_ref(), Expr::Ident(name) if name == "getCookie"));
            assert_eq!(method, "call_1", "2-arg call should dispatch to call_1");
        }
        other => panic!("expected MethodCall, got {other:?}"),
    }
}

/// P10.2: Non-callable const falls through to CallTarget::Free.
///
/// Verifies that consts NOT annotated with a callable interface type
/// are not intercepted by `try_convert_callable_trait_call`.
#[test]
fn test_non_callable_const_falls_through_to_free() {
    let source = r#"
        function greet(name: string): string {
            return name;
        }
        greet("world");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let module = f.module();
    let call_stmt = module.body.last().unwrap();
    let call_expr = match call_stmt {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(call_expr)
        .unwrap();
    // Regular function should produce FnCall with Free target (not MethodCall)
    match &result {
        Expr::FnCall {
            target: CallTarget::Free(name),
            ..
        } => {
            assert_eq!(name, "greet");
        }
        other => panic!("expected FnCall(Free), got {other:?}"),
    }
}

/// P10.2 symmetry: `classify_callable_interface` returns identical results
/// whether reached from the conversion path (via RustType::Named) or the
/// call path (via ConstValue → type_ref_name → Struct).
///
/// Both paths ultimately pass the same `TypeDef::Struct` to
/// `classify_callable_interface`, guaranteeing symmetric classification.
#[test]
fn test_classify_symmetry_conversion_and_call_paths() {
    let source = r#"
        interface GetCookie {
            (c: string): string;
            (c: string, key: string): number;
        }
        const getCookie: GetCookie = (c: string, key?: string): string => {
            return c;
        };
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let reg = tctx.type_registry;

    // Call path: ConstValue → type_ref_name → Struct → classify
    let type_ref_name = match reg.get("getCookie") {
        Some(crate::registry::TypeDef::ConstValue { type_ref_name, .. }) => type_ref_name
            .clone()
            .expect("type_ref_name should be Some for callable interface const"),
        other => panic!("expected ConstValue, got {other:?}"),
    };
    let call_side_def = reg.get(&type_ref_name).expect("interface def should exist");
    let call_side_kind = crate::registry::collection::classify_callable_interface(call_side_def);

    // Conversion path: directly look up the interface name
    let conversion_side_def = reg.get("GetCookie").expect("interface def should exist");
    let conversion_side_kind =
        crate::registry::collection::classify_callable_interface(conversion_side_def);

    // Both must classify as MultiOverload with the same signatures
    use crate::registry::collection::CallableInterfaceKind;
    match (&call_side_kind, &conversion_side_kind) {
        (CallableInterfaceKind::MultiOverload(a), CallableInterfaceKind::MultiOverload(b)) => {
            assert_eq!(a.len(), b.len(), "overload count must match");
            assert_eq!(a, b, "overload signatures must be identical");
        }
        _ => panic!(
            "expected MultiOverload on both paths, got call={call_side_kind:?}, conversion={conversion_side_kind:?}"
        ),
    }
}

/// ConstValue with `type_ref_name: None` (e.g., `as const` object) falls through.
///
/// Branch coverage: `ConstValue { type_ref_name: None }` → `type_ref_name.clone()?` returns None.
#[test]
fn test_const_value_without_type_ref_falls_through() {
    let source = r#"
        const config = { key: "value" } as const;
        config;
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let module = f.module();
    // Last statement: `config;`
    let stmt = module.body.last().unwrap();
    let expr = match stmt {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(expr)
        .unwrap();
    // `as const` objects are Ident references, not callable — should NOT produce MethodCall
    assert!(
        !matches!(&result, Expr::MethodCall { .. }),
        "as const object should not be treated as callable interface"
    );
}

/// Generic callable interface call dispatch via unit test (not just fixture).
#[test]
fn test_generic_callable_interface_call_dispatch() {
    let source = r#"
        interface Mapper<T, U> {
            (input: T): U;
        }
        const strToNum: Mapper<string, number> = (input: string): number => {
            return 42;
        };
        strToNum("hello");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let module = f.module();
    let call_stmt = module.body.last().unwrap();
    let call_expr = match call_stmt {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(call_expr)
        .unwrap();
    match &result {
        Expr::MethodCall { object, method, .. } => {
            assert!(matches!(object.as_ref(), Expr::Ident(name) if name == "strToNum"));
            assert_eq!(method, "call_0");
        }
        other => panic!("expected MethodCall, got {other:?}"),
    }
}

/// 0-arg callable interface call dispatches correctly.
#[test]
fn test_callable_interface_zero_arg_call() {
    let source = r#"
        interface GetDefault {
            (): string;
        }
        const getDefault: GetDefault = (): string => {
            return "default";
        };
        getDefault();
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let module = f.module();
    let call_stmt = module.body.last().unwrap();
    let call_expr = match call_stmt {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(call_expr)
        .unwrap();
    match &result {
        Expr::MethodCall {
            object,
            method,
            args,
        } => {
            assert!(matches!(object.as_ref(), Expr::Ident(name) if name == "getDefault"));
            assert_eq!(method, "call_0");
            assert!(args.is_empty(), "0-arg call should produce empty args");
        }
        other => panic!("expected MethodCall, got {other:?}"),
    }
}

/// Multi-overload: 1-arg call dispatches to call_0 (complement to existing 2-arg → call_1 test).
#[test]
fn test_callable_interface_multi_overload_1arg_selects_call_0() {
    let source = r#"
        interface GetCookie {
            (c: string): string;
            (c: string, key: string): number;
        }
        const getCookie: GetCookie = (c: string, key?: string): string => {
            return c;
        };
        getCookie("ctx");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let module = f.module();
    let call_stmt = module.body.last().unwrap();
    let call_expr = match call_stmt {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => &*expr_stmt.expr,
        _ => panic!("expected expression statement"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(call_expr)
        .unwrap();
    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "call_0", "1-arg call should dispatch to call_0");
        }
        other => panic!("expected MethodCall, got {other:?}"),
    }
}
