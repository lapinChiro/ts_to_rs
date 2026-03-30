use super::*;

#[test]
fn test_convert_lit_string_to_enum_variant_when_expected_is_string_literal_union() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    string_values.insert("down".to_string(), "Down".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"const d: Direction = "up";"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("Direction::Up".to_string()));
}

#[test]
fn test_convert_lit_string_no_match_falls_back_to_string_lit() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"const d: Direction = "unknown";"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::StringLit("unknown".to_string()));
}

#[test]
fn test_convert_bin_expr_enum_var_eq_string_literal_converts_rhs() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    string_values.insert("down".to_string(), "Down".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"function f(d: Direction) { d == "up"; }"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("d".to_string())),
            op: BinOp::Eq,
            right: Box::new(Expr::Ident("Direction::Up".to_string())),
        }
    );
}

#[test]
fn test_convert_bin_expr_string_literal_ne_enum_var_converts_lhs() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"function f(d: Direction) { "up" != d; }"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("Direction::Up".to_string())),
            op: BinOp::NotEq,
            right: Box::new(Expr::Ident("d".to_string())),
        }
    );
}

#[test]
fn test_convert_call_args_string_literal_to_enum_variant() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    string_values.insert("down".to_string(), "Down".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );
    reg.register(
        "move_dir".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![(
                "d".to_string(),
                RustType::Named {
                    name: "Direction".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: None,
            has_rest: false,
        },
    );

    let source = r#"move_dir("up");"#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "move_dir".to_string(),
            args: vec![Expr::Ident("Direction::Up".to_string())],
        }
    );
}

#[test]
fn test_convert_object_lit_discriminated_union_to_enum_variant() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![("side".to_string(), RustType::F64)],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );

    let f = TctxFixture::from_source_with_reg(
        r#"const s: Shape = { kind: "circle", radius: 5 };"#,
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Shape::Circle".to_string(),
            fields: vec![("radius".to_string(), Expr::NumberLit(5.0))],
            base: None,
        }
    );
}

#[test]
fn test_convert_object_lit_discriminated_union_unit_variant() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("active".to_string(), "Active".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert("Active".to_string(), vec![]);
    reg.register(
        "Status".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Active".to_string()],
            string_values,
            tag_field: Some("type".to_string()),
            variant_fields,
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"const s: Status = { type: "active" };"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Unit variant: no fields → Ident
    assert_eq!(result, Expr::Ident("Status::Active".to_string()));
}

#[test]
fn test_convert_member_expr_discriminant_field_to_method_call() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );

    let f = TctxFixture::from_source_with_reg("function f(s: Shape) { s.kind; }", reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "kind".to_string(),
            args: vec![],
        }
    );
}

fn build_shape_registry_for_expr() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![("side".to_string(), RustType::F64)],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );
    reg
}

#[test]
fn test_convert_du_standalone_field_access_generates_match_expr() {
    let reg = build_shape_registry_for_expr();

    // s.radius → match expression
    let f = TctxFixture::from_source_with_reg("function f(s: Shape) { const x = s.radius; }", reg);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_var_init(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();

    // Should be Expr::Match
    if let Expr::Match { expr, arms } = &result {
        // Match on &s
        assert_eq!(**expr, Expr::Ref(Box::new(Expr::Ident("s".to_string()))));
        // One arm for Circle (which has radius) + wildcard
        assert!(
            arms.len() >= 2,
            "expected at least 2 arms, got: {}",
            arms.len()
        );
        // First arm should bind radius
        assert!(
            arms[0].patterns.iter().any(|p| {
                matches!(p, MatchPattern::EnumVariant { path, bindings }
                    if path == "Shape::Circle" && bindings == &["radius"])
            }),
            "expected Circle arm with radius binding, got: {:?}",
            arms[0].patterns
        );
    } else {
        panic!("expected Expr::Match, got: {result:?}");
    }
}
