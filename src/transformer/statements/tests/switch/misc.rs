//! Cross-cutting switch rewrite behaviors: discriminant type
//! propagation (string-enum case literal → `Direction::Up` pattern) and
//! `default` source position independence (wildcard always ends up as
//! the last arm, regardless of where it appears in the source).

use super::*;

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
            // Case "up" should become Direction::Up (UnitStruct with path segments)
            assert!(
                arms[0].patterns.iter().any(|p| matches!(
                    p,
                    Pattern::UnitStruct { ctor }
                        if matches!(
                            ctor,
                            crate::ir::PatternCtor::UserEnumVariant { enum_ty, variant }
                                if enum_ty.as_str() == "Direction" && variant == "Up"
                        )
                )),
                "expected Direction::Up pattern, got {:?}",
                arms[0].patterns
            );
            // Case "down" should become Direction::Down
            assert!(
                arms[1].patterns.iter().any(|p| matches!(
                    p,
                    Pattern::UnitStruct { ctor }
                        if matches!(
                            ctor,
                            crate::ir::PatternCtor::UserEnumVariant { enum_ty, variant }
                                if enum_ty.as_str() == "Direction" && variant == "Down"
                        )
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
            // First arm should be the numeric case (wildcard + guard, I-315)
            assert!(
                arms[0].guard.is_some(),
                "numeric case arm should have a guard"
            );
            // Last arm should be wildcard
            assert!(
                arms.last()
                    .unwrap()
                    .patterns
                    .iter()
                    .any(|p| matches!(p, crate::ir::Pattern::Wildcard)),
                "last arm must be wildcard regardless of source position"
            );
        }
        other => panic!("expected Match, got {other:?}"),
    }
}
