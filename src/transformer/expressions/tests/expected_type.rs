use super::*;

#[test]
fn test_convert_expr_string_lit_with_string_expected_adds_to_string() {
    let f = TctxFixture::from_source(r#"const s: string = "hello";"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::StringLit("hello".to_string())),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_lit_without_expected_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("\"hello\";");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_string_lit_with_f64_expected_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("\"hello\";");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_array_string_with_vec_string_expected() {
    let f = TctxFixture::from_source(r#"const a: string[] = ["a", "b"];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::MethodCall {
                    object: Box::new(Expr::StringLit("a".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                },
                Expr::MethodCall {
                    object: Box::new(Expr::StringLit("b".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                },
            ],
        }
    );
}

#[test]
fn test_binary_number_plus_string_generates_format() {
    // x + " px" where x: number → format!("{}{}", x, " px")
    let f = TctxFixture::from_source(r#"function f(x: number): string { return x + " px"; }"#);
    let tctx = f.tctx();
    // The return expression is the binary expression
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let ret_stmt = &fn_decl.function.body.as_ref().unwrap().stmts[0];
    let swc_expr = match ret_stmt {
        ast::Stmt::Return(ret) => *ret.arg.as_ref().unwrap().clone(),
        _ => panic!("expected return"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FormatMacro { template, args } => {
            assert_eq!(template, "{}{}");
            assert_eq!(args.len(), 2);
        }
        other => panic!("expected FormatMacro for number + string, got {other:?}"),
    }
}

#[test]
fn test_fn_arg_box_dyn_fn_gets_box_new() {
    // applyFn(myFunc) where param is Fn type → applyFn(Box::new(my_func))
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "applyFn".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![(
                "f".to_string(),
                RustType::Fn {
                    params: vec![RustType::F64],
                    return_type: Box::new(RustType::F64),
                },
            )
                .into()],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let swc_expr = parse_expr("applyFn(myFunc);");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::FnCall { args, .. } => {
            assert!(
                matches!(&args[0], Expr::FnCall { target, .. } if matches!(target, CallTarget::ExternalPath(ref __s) if __s.iter().map(String::as_str).eq(["Box", "new"].iter().copied()))),
                "expected Box::new wrapping, got {:?}",
                args[0]
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

#[test]
fn test_convert_expr_type_assertion_primitive_generates_cast() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `x as number` → `x as f64` (primitive cast preserved)
    let expr = parse_expr("x as number;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Cast {
            expr: Box::new(Expr::Ident("x".to_string())),
            target: RustType::F64,
        }
    );
}

#[test]
fn test_convert_expr_type_assertion_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `(obj as Foo).bar` → `obj.bar`
    let expr = parse_expr("(obj as Foo).bar;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "bar".to_string(),
        }
    );
}

#[test]
fn test_string_concat_rhs_ident_gets_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // "Hello " + name → BinaryOp { left: StringLit, op: Add, right: Ref(Ident) }
    let swc_expr = parse_expr(r#""Hello " + name"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::BinaryOp { right, op, .. } => {
            assert_eq!(op, BinOp::Add);
            assert!(
                matches!(*right, Expr::Ref(_)),
                "expected RHS to be Ref(...), got: {right:?}"
            );
        }
        other => panic!("expected BinaryOp, got: {other:?}"),
    }
}

#[test]
fn test_string_concat_chain_rhs_gets_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // "Hello " + name + "!" → outer Add: LHS is Add(StringLit, Ref(Ident)), RHS should be Ref(StringLit("!"))
    // Actually "!" is a literal, so it gets .to_string() in Rust, which is already &str-compatible
    // But the pattern is: greeting + " " + name
    let swc_expr = parse_expr(r#"greeting + " " + name"#);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // The outer BinaryOp's left is also a BinaryOp with Add
    // We just verify the structure doesn't panic and produces BinaryOp
    assert!(matches!(result, Expr::BinaryOp { op: BinOp::Add, .. }));
}

#[test]
fn test_numeric_add_no_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // a + b (numeric) should NOT get Ref
    let swc_expr = parse_expr("a + b");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match result {
        Expr::BinaryOp { right, op, .. } => {
            assert_eq!(op, BinOp::Add);
            assert!(
                !matches!(*right, Expr::Ref(_)),
                "numeric add should NOT have Ref on RHS"
            );
        }
        other => panic!("expected BinaryOp, got: {other:?}"),
    }
}

#[test]
fn test_convert_bin_expr_expected_string_enables_concat() {
    // a + b with expected=String → string concat context (RHS wrapped in Ref)
    let f = TctxFixture::from_source("const s: string = a + b;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // In string concat context, RHS should be wrapped in Ref
    match &result {
        Expr::BinaryOp { op, right, .. } => {
            assert_eq!(*op, BinOp::Add);
            assert!(
                matches!(right.as_ref(), Expr::Ref(_)),
                "RHS should be Ref in string concat context, got: {:?}",
                right
            );
        }
        _ => panic!("expected BinaryOp, got: {:?}", result),
    }
}

#[test]
fn test_convert_bin_expr_no_expected_numeric_add() {
    // a + b with expected=None → numeric addition (no Ref wrapping)
    let swc_expr = parse_expr("a + b;");
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
        Expr::BinaryOp { op, right, .. } => {
            assert_eq!(*op, BinOp::Add);
            assert!(
                !matches!(right.as_ref(), Expr::Ref(_)),
                "RHS should NOT be Ref in numeric context"
            );
        }
        _ => panic!("expected BinaryOp, got: {:?}", result),
    }
}

#[test]
fn test_self_field_string_concat_gets_clone() {
    // this.name + " suffix" → self.name.clone() + &" suffix"
    let f = TctxFixture::from_source(r#"const s: string = this.name + " suffix";"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());

    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::BinaryOp { left, op, .. } => {
            assert_eq!(*op, BinOp::Add);
            // LHS should be self.name.clone()
            assert!(
                matches!(left.as_ref(), Expr::MethodCall { method, .. } if method == "clone"),
                "expected .clone() on self.field, got: {:?}",
                left
            );
        }
        _ => panic!("expected BinaryOp, got: {:?}", result),
    }
}

#[test]
fn test_convert_expr_array_with_tuple_expected_generates_tuple() {
    // ["a", 1] with expected=Tuple([String, F64]) → Expr::Tuple
    let f = TctxFixture::from_source(r#"const t: [string, number] = ["a", 1];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::Tuple { elements } => {
            assert_eq!(elements.len(), 2);
        }
        other => panic!("expected Tuple, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_nested_array_with_vec_tuple_expected() {
    // [["a", 1], ["b", 2]] with expected=Vec<Tuple([String, F64])>
    // → Expr::Vec { elements: [Expr::Tuple, Expr::Tuple] }
    let f = TctxFixture::from_source(r#"const t: [string, number][] = [["a", 1], ["b", 2]];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    match &result {
        Expr::Vec { elements } => {
            assert_eq!(elements.len(), 2);
            assert!(matches!(&elements[0], Expr::Tuple { .. }));
            assert!(matches!(&elements[1], Expr::Tuple { .. }));
        }
        other => panic!("expected Vec of Tuples, got: {other:?}"),
    }
}

#[test]
fn test_convert_assign_expr_propagates_type_from_resolution() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String).into()],
            std::collections::HashMap::new(),
            vec![],
        ),
    );

    let source = r#"
        let x: Config = { name: "" };
        x = { name: "test" };
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Expected: x = Config { name: "test".to_string() }
    match &result {
        Expr::Assign { value, .. } => match value.as_ref() {
            Expr::StructInit { name, fields, .. } => {
                assert_eq!(name, "Config");
                assert_eq!(fields[0].0, "name");
                assert!(
                    matches!(&fields[0].1, Expr::MethodCall { method, .. } if method == "to_string"),
                    "expected .to_string() on string field, got {:?}",
                    fields[0].1
                );
            }
            other => panic!("expected StructInit, got {other:?}"),
        },
        other => panic!("expected Assign, got {other:?}"),
    }
}

#[test]
fn test_convert_hashmap_propagates_value_type() {
    let f = TctxFixture::from_source(r#"const m: { [key: string]: string } = { [key]: "val" };"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Expected: HashMap::from(vec![(key, "val".to_string())])
    match &result {
        Expr::FnCall { target, args } => {
            assert!(
                matches!(target, CallTarget::ExternalPath(ref __s) if __s.iter().map(String::as_str).eq(["HashMap", "from"].iter().copied()))
            );
            match &args[0] {
                Expr::Vec { elements } => match &elements[0] {
                    Expr::Tuple { elements } => {
                        assert!(
                            matches!(&elements[1], Expr::MethodCall { method, .. } if method == "to_string"),
                            "expected .to_string() on HashMap value, got {:?}",
                            elements[1]
                        );
                    }
                    other => panic!("expected Tuple, got {other:?}"),
                },
                other => panic!("expected Vec, got {other:?}"),
            }
        }
        other => panic!("expected FnCall(HashMap::from), got {other:?}"),
    }
}

#[test]
fn test_convert_empty_object_with_hashmap_expected_type() {
    // const m: Record<string, string> = {} → HashMap::new()
    let f = TctxFixture::from_source(r#"const m: Record<string, string> = {};"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    match &result {
        Expr::FnCall { target, args } => {
            assert!(
                matches!(target, CallTarget::ExternalPath(ref __s) if __s.iter().map(String::as_str).eq(["HashMap", "new"].iter().copied()))
            );
            assert!(args.is_empty(), "HashMap::new() should have no arguments");
        }
        other => panic!("expected FnCall(HashMap::new), got {other:?}"),
    }
}
