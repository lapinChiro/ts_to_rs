use super::*;
use crate::parser::parse_typescript;
use crate::registry::build_registry;

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
