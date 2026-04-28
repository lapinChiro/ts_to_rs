use super::super::*;
use crate::registry::MethodKind;

#[test]
fn test_call_with_missing_default_arg_appends_none() {
    // greet("World") where greet has params: (name: String, greeting: Option<String>)
    // Should produce: greet("World".to_string(), None)
    let reg = greet_registry();
    let f = TctxFixture::from_source_with_reg(r#"greet("World");"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "greet"));
            assert_eq!(
                args.len(),
                2,
                "expected 2 args (with None appended), got {args:?}"
            );
            // Second arg should be None (BuiltinVariantValue(None))
            assert_eq!(
                args[1],
                Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)
            );
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_call_with_option_arg_wraps_some() {
    // greet("World", "Hi") where greeting is Option<String>
    // Should produce: greet("World".to_string(), Some("Hi".to_string()))
    let reg = greet_registry();
    let f = TctxFixture::from_source_with_reg(r#"greet("World", "Hi");"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "greet"));
            assert_eq!(args.len(), 2);
            // Second arg should be Some(...)
            assert!(
                matches!(&args[1], Expr::FnCall { target, args: inner } if matches!(target, CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some)) && inner.len() == 1),
                "expected Some(...), got: {:?}",
                args[1]
            );
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_typeenv_fn_provides_param_expected() {
    // f("hello") where f: (s: string) => boolean is declared
    // → "hello" should become "hello".to_string() because expected=String
    let source = r#"
        function f(s: string): boolean { return true; }
        f("hello");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();

    let swc_expr = extract_expr_stmt(f.module(), 1);
    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
        used_marker_names: std::collections::HashSet::new(),
    }
    .convert_expr(&swc_expr)
    .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "f"));
            assert_eq!(args.len(), 1);
            // The string literal should have .to_string() because param type is String
            assert!(
                matches!(
                    &args[0],
                    Expr::MethodCall { method, .. } if method == "to_string"
                ),
                "arg should be .to_string() call, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {:?}", result),
    }
}

#[test]
fn test_convert_call_expr_no_typeenv_fn_no_expected() {
    // f("hello") where type info is unavailable → "hello" stays as StringLit (no .to_string())
    let swc_expr = parse_expr("f(\"hello\");");
    let f = TctxFixture::new();
    let tctx = f.tctx();

    let result = Transformer {
        tctx: &tctx,

        synthetic: &mut SyntheticTypeRegistry::new(),
        mut_method_names: std::collections::HashSet::new(),
        used_marker_names: std::collections::HashSet::new(),
    }
    .convert_expr(&swc_expr)
    .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "f"));
            assert_eq!(args.len(), 1);
            assert!(
                matches!(&args[0], Expr::StringLit(s) if s == "hello"),
                "arg should be plain StringLit, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {:?}", result),
    }
}

#[test]
fn test_convert_call_expr_rest_param_packs_args_into_vec() {
    // sum(1, 2, 3) where sum(...nums: number[]) → sum(vec![1.0, 2.0, 3.0])
    let swc_expr = parse_expr("sum(1, 2, 3);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "sum"));
            assert_eq!(args.len(), 1, "all args should be packed into one vec");
            match &args[0] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 3);
                }
                other => panic!("expected Vec, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_mixed_regular_and_rest() {
    // log("hello", 1, 2) where log(prefix: string, ...nums: number[])
    // → log("hello".to_string(), vec![1.0, 2.0])
    let swc_expr = parse_expr(r#"log("hello", 1, 2);"#);
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "log".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![
                ("prefix".to_string(), RustType::String).into(),
                ("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into(),
            ],
            return_type: None,
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "log"));
            assert_eq!(args.len(), 2, "prefix + packed rest");
            match &args[1] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 2);
                }
                other => panic!("expected Vec for rest args, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_no_rest_args() {
    // sum() where sum(...nums: number[]) → sum(vec![])
    let swc_expr = parse_expr("sum();");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "sum"));
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 0, "no rest args → empty vec");
                }
                other => panic!("expected empty Vec, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_spread_single_array() {
    // sum(...arr) where sum(...nums: number[]) → sum(arr)
    let swc_expr = parse_expr("sum(...arr);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "sum"));
            assert_eq!(args.len(), 1);
            // Should pass arr directly, not wrap in vec!
            assert!(
                matches!(&args[0], Expr::Ident(name) if name == "arr"),
                "spread arg should be passed directly, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_mixed_literal_and_spread() {
    // sum(1, ...arr) where sum(...nums: number[]) → sum([vec![1.0], arr].concat())
    let swc_expr = parse_expr("sum(1, ...arr);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64))).into()],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "sum"));
            assert_eq!(args.len(), 1);
            // Should be [vec![1.0], arr].concat()
            match &args[0] {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method, "concat");
                }
                other => panic!("expected MethodCall(.concat()), got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_method_call_string_arg_gets_to_string_with_registry() {
    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("name".to_string(), RustType::String).into()],
            return_type: None,
            has_rest: false,
            type_params: vec![],
            kind: MethodKind::Method,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_struct(vec![], methods, vec![]),
    );

    let source = r#"
        const g: Greeter = new Greeter();
        g.greet("world");
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Should have .to_string() on the string arg
    if let Expr::MethodCall { args, .. } = &result {
        assert!(
            matches!(
                &args[0],
                Expr::MethodCall { method, .. } if method == "to_string"
            ),
            "expected .to_string() on method arg, got: {:?}",
            args[0]
        );
    } else {
        panic!("expected MethodCall, got: {result:?}");
    }
}
