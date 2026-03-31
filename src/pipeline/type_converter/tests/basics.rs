use super::*;

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

// --- sanitize_rust_type_name ---

#[test]
fn test_sanitize_rust_type_name_prefixes_prelude_types() {
    use super::super::sanitize_rust_type_name;
    assert_eq!(sanitize_rust_type_name("Result"), "TsResult");
    assert_eq!(sanitize_rust_type_name("Option"), "TsOption");
    assert_eq!(sanitize_rust_type_name("String"), "TsString");
    assert_eq!(sanitize_rust_type_name("Vec"), "TsVec");
    assert_eq!(sanitize_rust_type_name("Box"), "TsBox");
    assert_eq!(sanitize_rust_type_name("Some"), "TsSome");
    assert_eq!(sanitize_rust_type_name("None"), "TsNone");
    assert_eq!(sanitize_rust_type_name("Ok"), "TsOk");
    assert_eq!(sanitize_rust_type_name("Err"), "TsErr");
    assert_eq!(sanitize_rust_type_name("Self"), "TsSelf");
}

#[test]
fn test_sanitize_rust_type_name_preserves_non_prelude() {
    use super::super::sanitize_rust_type_name;
    assert_eq!(sanitize_rust_type_name("MyType"), "MyType");
    assert_eq!(sanitize_rust_type_name("Context"), "Context");
    assert_eq!(sanitize_rust_type_name("User"), "User");
    assert_eq!(sanitize_rust_type_name("ResultType"), "ResultType");
}
