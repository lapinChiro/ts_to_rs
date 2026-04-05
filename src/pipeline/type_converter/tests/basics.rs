use super::*;
use crate::pipeline::SyntheticTypeRegistry;

// ── extract_type_params monomorphization ──

#[test]
fn test_extract_type_params_monomorphizes_number_constraint() {
    // `T extends number` → T is monomorphized (removed from params, placed in subs)
    let source = "interface Foo<T extends number> { x: T; }";
    let decl = parse_interface(source);
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let (params, subs) = extract_type_params(decl.type_params.as_deref(), &mut synthetic, &reg);
    // T should be monomorphized away (f64 is not a valid trait bound)
    assert!(
        params.is_empty(),
        "T extends number should be monomorphized"
    );
    assert_eq!(subs.get("T"), Some(&RustType::F64));
}

#[test]
fn test_extract_type_params_keeps_unconstrained() {
    // `T` with no constraint → stays as a type param
    let source = "interface Foo<T> { x: T; }";
    let decl = parse_interface(source);
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let (params, subs) = extract_type_params(decl.type_params.as_deref(), &mut synthetic, &reg);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "T");
    assert!(subs.is_empty());
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
