use super::super::*;
use crate::ir::{CallTarget, ClosureBody};

#[test]
fn test_convert_call_expr_paren_ident_unwraps_to_fn_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (foo)(1) → foo(1.0)
    let expr = parse_expr("(foo)(1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "foo"));
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_paren_member_unwraps_to_method_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (obj.method)() → obj.method()
    let expr = parse_expr("(obj.method)();");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "method");
        }
        other => panic!("expected MethodCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_chained_call_does_not_error() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // f(x)(y) — chained call should not error
    let expr = parse_expr("f(1)(2);");
    let result =
        Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new()).convert_expr(&expr);
    assert!(
        result.is_ok(),
        "chained call should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_opt_chain_method_call_propagates_param_types() {
    use crate::registry::TypeDef;

    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("name".to_string(), RustType::String).into()],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_struct(vec![], methods, vec![]),
    );

    let source = r#"
        const obj: Greeter | undefined = undefined;
        obj?.greet("hello");
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Expected: obj.as_ref().map(|_v| _v.greet("hello".to_string()))
    // The "hello" arg should have .to_string() because greet's param type is String
    match &result {
        Expr::MethodCall { method, args, .. } if method == "map" => {
            // The closure body should contain a method call with to_string on the arg
            match &args[0] {
                Expr::Closure { body, .. } => match body {
                    ClosureBody::Expr(expr) => match expr.as_ref() {
                        Expr::MethodCall {
                            args: inner_args, ..
                        } => {
                            assert!(
                                matches!(&inner_args[0], Expr::MethodCall { method, .. } if method == "to_string"),
                                "expected .to_string() on string arg, got {:?}",
                                inner_args[0]
                            );
                        }
                        other => panic!("expected MethodCall inside closure, got {other:?}"),
                    },
                    _ => panic!("expected ClosureBody::Expr"),
                },
                other => panic!("expected Closure, got {other:?}"),
            }
        }
        other => panic!("expected MethodCall(map), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// I-375: `convert_call_expr` / `convert_new_expr` `type_ref` classification
// ---------------------------------------------------------------------------

/// `foo(x)` where `foo` is a plain function registered in `TypeRegistry`
/// must classify as `CallTarget::Path { type_ref: None }`. The walker will
/// then skip it — free functions are not type references.
#[test]
fn test_convert_expr_call_ident_function_in_registry_gets_type_ref_none() {
    use crate::registry::{ParamDef, TypeDef};
    let mut reg = TypeRegistry::new();
    reg.register(
        "foo".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![ParamDef {
                name: "x".to_string(),
                ty: RustType::F64,
                optional: false,
                has_default: false,
            }],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::from_source_with_reg("foo(1);", reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert!(
                matches!(target, CallTarget::Free(name) if name == "foo"),
                "function call must classify as Free, got {target:?}"
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// When an Ident callee name happens to be registered as a struct/class (a
/// degenerate TS situation — e.g. a callable interface), the Transformer must
/// still classify it as a type reference so the walker can wire the graph.
#[test]
fn test_convert_expr_call_ident_struct_in_registry_gets_type_ref_some() {
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Callable".to_string(),
        TypeDef::new_struct(vec![], std::collections::HashMap::new(), vec![]),
    );
    let f = TctxFixture::from_source_with_reg("Callable(1);", reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert!(
                matches!(
                    target,
                    CallTarget::UserTupleCtor(ty) if ty.as_str() == "Callable"
                ),
                "struct-typed Ident callee must classify as UserTupleCtor, got {target:?}"
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// An unknown identifier (not registered anywhere) must default to
/// `type_ref: None`. This is the common case for imported functions or
/// identifiers whose types the resolver couldn't determine.
#[test]
fn test_convert_expr_call_ident_unknown_name_gets_type_ref_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("unknownFn(x);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert!(
                matches!(target, CallTarget::Free(name) if name == "unknownFn"),
                "unknown ident must classify as Free, got {target:?}"
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// `new Foo(x)` must become `CallTarget::Path { segments: ["Foo", "new"],
/// type_ref: Some("Foo") }`. The `type_ref` is critical for the walker to
/// register `Foo` as a referenced type, and is what enables lowercase class
/// names to survive (I-375 correctness fix).
#[test]
fn test_convert_new_expr_sets_type_ref_to_class_name() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new Foo(1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => {
            assert_eq!(
                target,
                &CallTarget::UserAssocFn {
                    ty: crate::ir::UserTypeRef::new("Foo"),
                    method: "new".to_string()
                },
                "new Foo(...) must produce assoc(\"Foo\", \"new\") with type_ref Some(\"Foo\")"
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// The lowercase-class correctness scenario, verified directly at the
/// Transformer layer: `new myClass(...)` must still record
/// `type_ref: Some("myClass")` even though `myClass` does not follow Rust
/// PascalCase convention. This proves the structural fix is independent of
/// naming conventions.
#[test]
fn test_convert_new_expr_lowercase_class_records_type_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new myClass(1);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { target, .. } => match target {
            CallTarget::UserAssocFn { ty, method } => {
                assert_eq!(ty.as_str(), "myClass");
                assert_eq!(method, "new");
            }
            _ => panic!("expected UserAssocFn, got {target:?}"),
        },
        other => panic!("expected FnCall, got {other:?}"),
    }
}

/// `super(args)` in a class constructor context must produce
/// `CallTarget::Super`, never a `Path { segments: ["super"] }`.
#[test]
fn test_convert_callee_super_produces_super_variant() {
    // We parse a class constructor body so that the swc parser accepts super().
    let source = r#"class Child extends Parent { constructor(x: number) { super(x); } }"#;
    let fx = TctxFixture::from_source_with_reg(source, TypeRegistry::new());
    let tctx = fx.tctx();
    // Walk the module to find the super call inside the constructor body.
    let module = fx.module();
    let mut found = false;
    for item in &module.body {
        if let swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
            swc_ecma_ast::Decl::Class(class_decl),
        )) = item
        {
            for member in &class_decl.class.body {
                if let swc_ecma_ast::ClassMember::Constructor(ctor) = member {
                    if let Some(body) = &ctor.body {
                        for stmt in &body.stmts {
                            if let swc_ecma_ast::Stmt::Expr(expr_stmt) = stmt {
                                if let swc_ecma_ast::Expr::Call(call) = expr_stmt.expr.as_ref() {
                                    let result = Transformer::for_module(
                                        &tctx,
                                        &mut SyntheticTypeRegistry::new(),
                                    )
                                    .convert_call_expr(call)
                                    .unwrap();
                                    assert!(
                                        matches!(
                                            result,
                                            Expr::FnCall {
                                                target: CallTarget::Super,
                                                ..
                                            }
                                        ),
                                        "expected CallTarget::Super, got {result:?}"
                                    );
                                    found = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(found, "super() call not found in the parsed module");
}
