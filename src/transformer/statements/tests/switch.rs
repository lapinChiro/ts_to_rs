use super::*;

#[test]
fn test_convert_switch_single_case_break_generates_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body("function f(x: number) { switch(x) { case 1: doA(); break; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert_eq!(arms[0].patterns.len(), 1);
            assert!(arms[0]
                .patterns
                .iter()
                .all(|p| matches!(p, crate::ir::MatchPattern::Literal(_))));
            assert!(!arms[0].body.is_empty());
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_empty_fallthrough_merges_patterns() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts =
        parse_fn_body("function f(x: number) { switch(x) { case 1: case 2: doAB(); break; } }");
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert_eq!(
                arms[0].patterns.len(),
                2,
                "expected 2 patterns for merged cases"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_default_generates_wildcard() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); break; default: doB(); } }",
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
            assert!(arms[0]
                .patterns
                .iter()
                .all(|p| matches!(p, crate::ir::MatchPattern::Literal(_))));
            assert!(
                arms[1]
                    .patterns
                    .iter()
                    .any(|p| matches!(p, crate::ir::MatchPattern::Wildcard)),
                "last arm should be wildcard"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_fallthrough_generates_labeled_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // break-less fall-through: case 1 falls into case 2
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); case 2: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    // Fall-through path generates a LabeledBlock with flag pattern
    match &result[0] {
        Stmt::LabeledBlock { label, body } => {
            assert_eq!(label, "switch");
            // Should contain: let mut _fall = false; + if chains
            let has_fall_flag = body
                .iter()
                .any(|s| matches!(s, Stmt::Let { name, .. } if name == "_fall"));
            assert!(has_fall_flag, "expected _fall flag, got {body:?}");
        }
        other => panic!("expected LabeledBlock for fall-through, got {other:?}"),
    }
}

#[test]
fn test_convert_switch_return_terminated_case_generates_clean_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // case ending with return should be treated as terminated → clean match, not fall-through
    let stmts = parse_fn_body(
        "function f(x: number): string { switch(x) { case 1: return \"one\"; case 2: return \"two\"; } }",
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
            // Both arms should have return statements
            assert!(matches!(arms[0].body.last(), Some(Stmt::Return(_))));
            assert!(matches!(arms[1].body.last(), Some(Stmt::Return(_))));
        }
        other => panic!("expected Match (not LabeledBlock), got {other:?}"),
    }
}

#[test]
fn test_convert_switch_throw_terminated_case_generates_clean_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { case 1: doA(); throw new Error(\"fail\"); case 2: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2, "expected 2 arms, got {arms:?}");
        }
        other => panic!("expected Match (not LabeledBlock), got {other:?}"),
    }
}

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
                    crate::ir::MatchPattern::Literal(Expr::StringLit(s)) if s == "hello"
                )),
                "expected string pattern 'hello', got {:?}",
                arms[0].patterns
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

// --- Switch non-literal case ---

#[test]
fn test_switch_nonliteral_case_generates_guard() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // case A (variable reference) should generate a match guard, not a pattern binding
    let stmts = parse_fn_body(
        "function f(x: number) { const A: number = 1; switch(x) { case A: doA(); break; default: doB(); } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    // Find the Match statement (second stmt after the const)
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");
    match match_stmt {
        Stmt::Match { arms, .. } => {
            // First arm (case A) should have a guard
            assert!(
                arms[0].guard.is_some(),
                "non-literal case should have a guard, got {:?}",
                arms[0]
            );
            assert!(
                arms[0]
                    .patterns
                    .iter()
                    .any(|p| matches!(p, crate::ir::MatchPattern::Wildcard)),
                "non-literal case should use wildcard pattern"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_switch_nonliteral_fallthrough_cases_combined_guard() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // case A: case B: ... should combine into a single guard with ||
    let stmts = parse_fn_body(
        "function f(x: number) { const A: number = 1; const B: number = 2; switch(x) { case A: case B: doAB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");
    match match_stmt {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert!(
                arms[0].guard.is_some(),
                "combined non-literal cases should have a guard"
            );
            // Guard should be a LogicalOr of two equality checks
            match &arms[0].guard {
                Some(Expr::BinaryOp {
                    op: BinOp::LogicalOr,
                    ..
                }) => {} // OK
                other => panic!("expected LogicalOr guard, got {other:?}"),
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_switch_mixed_literal_nonliteral_separate_arms() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Literal cases should have no guard, non-literal cases should have guards
    let stmts = parse_fn_body(
        "function f(x: number) { const A: number = 10; switch(x) { case 1: doA(); break; case A: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    let match_stmt = result
        .iter()
        .find(|s| matches!(s, Stmt::Match { .. }))
        .expect("expected a Match statement");
    match match_stmt {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // First arm (case 1) - literal, no guard
            assert!(
                arms[0].guard.is_none(),
                "literal case should have no guard, got {:?}",
                arms[0]
            );
            // Second arm (case A) - non-literal, has guard
            assert!(
                arms[1].guard.is_some(),
                "non-literal case should have a guard, got {:?}",
                arms[1]
            );
        }
        _ => unreachable!(),
    }
}

// --- discriminated union switch → enum match ---

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
        // First arm should be EnumVariant pattern
        assert!(
            arms[0].patterns.iter().any(
                |p| matches!(p, MatchPattern::EnumVariant { path, .. } if path == "Shape::Circle")
            ),
            "expected EnumVariant pattern for circle, got: {:?}",
            arms[0].patterns
        );
    } else {
        panic!("expected Match");
    }
}

// --- discriminated union field access in switch arms ---

/// Helper to build a Shape discriminated union registry.
fn build_shape_registry() -> TypeRegistry {
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
        vec![
            ("width".to_string(), RustType::F64),
            ("height".to_string(), RustType::F64),
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
        // Circle arm should have "radius" in bindings
        let circle_arm = &arms[0];
        assert!(
            circle_arm.patterns.iter().any(
                |p| matches!(p, MatchPattern::EnumVariant { bindings, .. } if bindings == &["radius"])
            ),
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
            square_arm.patterns.iter().any(
                |p| matches!(p, MatchPattern::EnumVariant { bindings, .. } if bindings.is_empty())
            ),
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
            if let MatchPattern::EnumVariant { bindings, .. } = p {
                bindings.contains(&"width".to_string()) && bindings.contains(&"height".to_string())
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

// --- Expected type propagation (Category B improvements) ---

/// Switch case values should propagate discriminant type for string enum matching.
/// `switch(dir) { case "up": ... }` where dir: Direction → case becomes `Direction::Up`
#[test]
fn test_convert_switch_case_propagates_discriminant_type_for_string_enum() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values: HashMap::from([
                ("up".to_string(), "Up".to_string()),
                ("down".to_string(), "Down".to_string()),
            ]),
            tag_field: None,
            variant_fields: HashMap::new(),
        },
    );

    let source = r#"function f(dir: Direction) { switch(dir) { case "up": doA(); break; case "down": doB(); break; } }"#;
    let f = TctxFixture::from_source_with_reg(source, reg);
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
    assert_eq!(result.len(), 1, "expected 1 stmt, got {result:?}");
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
            // Case "up" should become Direction::Up
            assert!(
                arms[0].patterns.iter().any(|p| matches!(
                    p,
                    MatchPattern::Literal(Expr::Ident(s)) if s == "Direction::Up"
                )),
                "expected Direction::Up pattern, got {:?}",
                arms[0].patterns
            );
            // Case "down" should become Direction::Down
            assert!(
                arms[1].patterns.iter().any(|p| matches!(
                    p,
                    MatchPattern::Literal(Expr::Ident(s)) if s == "Direction::Down"
                )),
                "expected Direction::Down pattern, got {:?}",
                arms[1].patterns
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}

#[test]
fn test_switch_default_before_case_moves_to_last_arm() {
    // default appears BEFORE case 1 — wildcard should still be the LAST arm
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let stmts = parse_fn_body(
        "function f(x: number) { switch(x) { default: doA(); break; case 1: doB(); break; } }",
    );
    let result = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt_list(&stmts, None)
    }
    .unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Stmt::Match { arms, .. } => {
            assert_eq!(arms.len(), 2, "should have 2 arms: case + default");
            // First arm should be the literal case
            assert!(
                arms[0]
                    .patterns
                    .iter()
                    .all(|p| !matches!(p, crate::ir::MatchPattern::Wildcard)),
                "first arm should NOT be wildcard"
            );
            // Last arm should be wildcard
            assert!(
                arms.last()
                    .unwrap()
                    .patterns
                    .iter()
                    .any(|p| matches!(p, crate::ir::MatchPattern::Wildcard)),
                "last arm must be wildcard regardless of source position"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}
