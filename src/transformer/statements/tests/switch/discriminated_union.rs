//! Discriminated-union switch (`switch(s.kind)` where `s: Shape`) →
//! `match &s { Shape::Circle { radius } => ... }` including DU field
//! access promoted to pattern bindings (single field / multiple fields).

use super::*;

/// Helper to build a `Shape` DU registry with `Circle { radius }` and
/// `Square { width, height }`.
fn build_shape_registry() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64).into()],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![
            ("width".to_string(), RustType::F64).into(),
            ("height".to_string(), RustType::F64).into(),
        ],
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
fn test_convert_switch_discriminated_union_to_enum_match() {
    let source = r#"
        function main(): void {
            const s: Shape = { kind: "circle", radius: 5 };
            switch (s.kind) {
                case "circle":
                    console.log("circle");
                    break;
                case "square":
                    console.log("square");
                    break;
            }
        }
    "#;

    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64).into()],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![("side".to_string(), RustType::F64).into()],
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

    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected function declaration"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(body_stmts, None)
    }
    .unwrap();

    // Find the match statement
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");

    if let Stmt::Match { expr, arms } = match_stmt {
        // Match on `s` (the enum variable), not `s.kind`
        assert_eq!(*expr, Expr::Ref(Box::new(Expr::Ident("s".to_string()))));
        // First arm should be Pattern::Struct with path ["Shape", "Circle"]
        assert!(
            arms[0].patterns.iter().any(|p| matches!(
                p,
                Pattern::Struct { ctor, .. }
                    if matches!(
                        ctor,
                        crate::ir::PatternCtor::UserEnumVariant { enum_ty, variant }
                            if enum_ty.as_str() == "Shape" && variant == "Circle"
                    )
            )),
            "expected Pattern::Struct for Shape::Circle, got: {:?}",
            arms[0].patterns
        );
    } else {
        panic!("expected Match");
    }
}

#[test]
fn test_convert_du_switch_field_access_single_field_becomes_binding() {
    let source = r#"
        function get_radius(s: Shape): number {
            switch (s.kind) {
                case "circle":
                    return s.radius;
                case "square":
                    return 0;
            }
        }
    "#;
    let reg = build_shape_registry();
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected function declaration"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic)
            .convert_stmt_list(body_stmts, Some(&RustType::F64))
    }
    .unwrap();

    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");

    if let Stmt::Match { arms, .. } = match_stmt {
        // Circle arm should have "radius" binding in struct fields
        let circle_arm = &arms[0];
        assert!(
            circle_arm.patterns.iter().any(|p| matches!(
                p,
                Pattern::Struct { fields, .. }
                    if fields.len() == 1 && fields[0].0 == "radius"
            )),
            "expected radius binding in Circle arm, got: {:?}",
            circle_arm.patterns
        );
        // Circle arm body should reference `radius.clone()` (match on &s binds by ref)
        assert!(
            circle_arm.body.iter().any(|s| {
                matches!(s, Stmt::Return(Some(Expr::MethodCall { object, method, .. }))
                    if matches!(object.as_ref(), Expr::Ident(name) if name == "radius")
                    && method == "clone")
            }),
            "expected return of `radius.clone()`, got: {:?}",
            circle_arm.body
        );
        // Square arm should have no bindings (no field access)
        let square_arm = &arms[1];
        assert!(
            square_arm.patterns.iter().any(|p| matches!(
                p,
                Pattern::Struct { fields, .. } if fields.is_empty()
            )),
            "expected no bindings in Square arm, got: {:?}",
            square_arm.patterns
        );
    } else {
        panic!("expected Match");
    }
}

#[test]
fn test_convert_du_switch_field_access_multiple_fields_become_bindings() {
    let source = r#"
        function area(s: Shape): number {
            switch (s.kind) {
                case "circle":
                    return 0;
                case "square":
                    return s.width * s.height;
            }
        }
    "#;
    let reg = build_shape_registry();
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected function declaration"),
    };
    let body_stmts = &fn_decl.function.body.as_ref().unwrap().stmts;
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic)
            .convert_stmt_list(body_stmts, Some(&RustType::F64))
    }
    .unwrap();

    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");

    if let Stmt::Match { arms, .. } = match_stmt {
        // Square arm should have width and height in bindings
        let square_arm = &arms[1];
        let has_bindings = square_arm.patterns.iter().any(|p| {
            if let Pattern::Struct { fields, .. } = p {
                fields.iter().any(|(n, _)| n == "width")
                    && fields.iter().any(|(n, _)| n == "height")
            } else {
                false
            }
        });
        assert!(
            has_bindings,
            "expected width, height bindings in Square arm, got: {:?}",
            square_arm.patterns
        );
    } else {
        panic!("expected Match");
    }
}
