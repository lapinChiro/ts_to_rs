use super::*;

// --- typeof const value ---

#[test]
fn test_convert_typeof_const_array_returns_named() {
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

    let module = parse_typescript("type T = typeof TYPES;").unwrap();
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

// --- indexed access [number] key ---

#[test]
fn test_indexed_access_number_key_const_string_array() {
    let module = parse_typescript(
        "const TYPES = ['gzip', 'deflate'] as const;\ntype T = (typeof TYPES)[number];",
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
            "indexed access [number] on const array should succeed: {:?}",
            result.err()
        );
        let ty = result.unwrap();
        assert!(
            matches!(&ty, RustType::Named { .. }),
            "expected Named type for synthetic enum, got {ty:?}"
        );
        assert!(
            !synthetic.all_items().is_empty(),
            "should generate a synthetic string enum"
        );
    } else {
        panic!("expected type alias as second declaration");
    }
}

// --- indexed access [keyof typeof X] key ---

#[test]
fn test_indexed_access_keyof_typeof_string_values() {
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

// --- keyof typeof X type operator ---

#[test]
fn test_keyof_typeof_const_object() {
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

// --- lookup_field_type for ConstValue ---

#[test]
fn test_indexed_access_string_key_const_object() {
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
