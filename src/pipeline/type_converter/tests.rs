use super::*;
use crate::parser::parse_typescript;
use crate::registry::{build_registry, TypeDef};

/// Helper: parse a type annotation from a variable declaration.
fn parse_type_annotation(source: &str) -> swc_ecma_ast::Module {
    parse_typescript(source).unwrap()
}

#[test]
fn test_convert_string_type() {
    let module = parse_type_annotation("const x: string = '';");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), RustType::String);
            assert!(
                synthetic.all_items().is_empty(),
                "string type should not create synthetic types"
            );
        }
    }
}

#[test]
fn test_convert_number_type() {
    let module = parse_type_annotation("const x: number = 0;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), RustType::F64);
        }
    }
}

#[test]
fn test_convert_union_type_registers_synthetic() {
    let module = parse_type_annotation("const x: string | number = '';");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            // The union should have created a synthetic enum
            assert!(
                !synthetic.all_items().is_empty(),
                "union type should create a synthetic enum"
            );
        }
    }
}

#[test]
fn test_convert_nullable_type() {
    let module = parse_type_annotation("const x: string | null = null;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            assert_eq!(
                result.unwrap(),
                RustType::Option(Box::new(RustType::String))
            );
            assert!(
                synthetic.all_items().is_empty(),
                "nullable union should not create synthetic enum (just Option)"
            );
        }
    }
}

// --- typeof const value (T3) ---

#[test]
fn test_convert_typeof_const_array_returns_named() {
    // typeof on a registered ConstValue should return RustType::Named
    let mut reg = crate::registry::TypeRegistry::new();
    reg.register(
        "TYPES".to_string(),
        TypeDef::ConstValue {
            fields: vec![],
            elements: vec![
                crate::registry::ConstElement {
                    ty: RustType::String,
                    string_literal_value: Some("a".to_string()),
                },
                crate::registry::ConstElement {
                    ty: RustType::String,
                    string_literal_value: Some("b".to_string()),
                },
            ],
            type_ref_name: None,
        },
    );
    let mut synthetic = SyntheticTypeRegistry::new();

    // Build a typeof expression AST node
    let module = parse_typescript("type T = typeof TYPES;").unwrap();
    // The type alias body should be TsTypeQuery
    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.first()
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(result.is_ok(), "typeof on ConstValue should succeed");
        assert_eq!(
            result.unwrap(),
            RustType::Named {
                name: "TYPES".to_string(),
                type_args: vec![],
            }
        );
    } else {
        panic!("expected type alias declaration");
    }
}

// --- indexed access [number] key (T4) ---

#[test]
fn test_indexed_access_number_key_const_string_array() {
    // (typeof X)[number] on a string array ConstValue → synthetic string enum
    let module = parse_typescript(
        "const TYPES = ['gzip', 'deflate'] as const;\ntype T = (typeof TYPES)[number];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    // Get the second declaration (the type alias)
    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "indexed access [number] on const array should succeed: {:?}",
            result.err()
        );
        // Should be a Named type pointing to a synthetic string enum
        let ty = result.unwrap();
        assert!(
            matches!(&ty, RustType::Named { .. }),
            "expected Named type for synthetic enum, got {ty:?}"
        );
        // Synthetic registry should have the enum
        assert!(
            !synthetic.all_items().is_empty(),
            "should generate a synthetic string enum"
        );
    } else {
        panic!("expected type alias as second declaration");
    }
}

// --- indexed access [keyof typeof X] key (T5) ---

#[test]
fn test_indexed_access_keyof_typeof_string_values() {
    // (typeof X)[keyof typeof X] on an object with string values → string enum
    let module = parse_typescript(
        "const MIMES = { aac: 'audio/aac', avi: 'video/avi' } as const;\ntype T = (typeof MIMES)[keyof typeof MIMES];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "indexed access [keyof typeof X] should succeed: {:?}",
            result.err()
        );
        let ty = result.unwrap();
        assert!(
            matches!(&ty, RustType::Named { .. }),
            "expected Named type for synthetic string enum, got {ty:?}"
        );
    } else {
        panic!("expected type alias as second declaration");
    }
}

#[test]
fn test_indexed_access_keyof_typeof_number_values() {
    // (typeof X)[keyof typeof X] on an object with number values → f64
    let module = parse_typescript(
        "const PHASE = { A: 1, B: 2 } as const;\ntype T = (typeof PHASE)[keyof typeof PHASE];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "indexed access [keyof typeof X] with number values should succeed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), RustType::F64);
    } else {
        panic!("expected type alias as second declaration");
    }
}

// --- keyof typeof X type operator (T6) ---

#[test]
fn test_keyof_typeof_const_object() {
    // keyof typeof X → string enum of field names
    let module =
        parse_typescript("const OBJ = { a: 1, b: 2, c: 3 } as const;\ntype K = keyof typeof OBJ;")
            .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "keyof typeof should succeed: {:?}",
            result.err()
        );
        let ty = result.unwrap();
        assert!(
            matches!(&ty, RustType::Named { .. }),
            "expected Named type for key string enum, got {ty:?}"
        );
        assert!(
            !synthetic.all_items().is_empty(),
            "should generate a synthetic string enum for keys"
        );
    } else {
        panic!("expected type alias as second declaration");
    }
}

// --- lookup_field_type for ConstValue (T7) ---

#[test]
fn test_indexed_access_string_key_const_object() {
    // (typeof X)['fieldName'] → field type from ConstValue
    let module = parse_typescript(
        "const OBJ = { x: 'hello', y: 42 } as const;\ntype T = (typeof OBJ)['x'];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "string key indexed access on ConstValue should succeed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), RustType::String);
    } else {
        panic!("expected type alias as second declaration");
    }
}
