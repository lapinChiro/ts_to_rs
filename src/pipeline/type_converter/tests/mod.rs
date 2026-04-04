mod basics;
mod collections;
mod const_typeof;
mod indexed_access_mapped;
mod interfaces;
mod intersections;
mod primitives;
mod structural_transforms;
mod type_alias_forms;
mod type_aliases;
mod unions;
mod unions_discriminated;

use super::*;
use crate::ir::{StructField, TypeParam};
use crate::parser::parse_typescript;
use crate::registry::{build_registry, TypeDef, TypeRegistry};
use swc_ecma_ast::{Decl, ModuleItem, Stmt, TsTypeElement};

/// Helper: parse TS source and extract the first TsInterfaceDecl.
pub(crate) fn parse_interface(source: &str) -> swc_ecma_ast::TsInterfaceDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(decl))) => *decl.clone(),
        _ => panic!("expected TsInterfaceDecl"),
    }
}

/// Helper: parse TS source and extract the first TsTypeAliasDecl.
pub(crate) fn parse_type_alias(source: &str) -> swc_ecma_ast::TsTypeAliasDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(decl))) => *decl.clone(),
        _ => panic!("expected TsTypeAliasDecl"),
    }
}

/// Helper: parse a type annotation from `interface T { x: <TYPE>; }` and return the SWC type node.
pub(crate) fn parse_type_ann(type_str: &str) -> Box<swc_ecma_ast::TsType> {
    let source = format!("interface T {{ x: {type_str}; }}");
    let decl = parse_interface(&source);
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    prop.type_ann.as_ref().unwrap().type_ann.clone()
}

/// Helper: create a TypeRegistry with Point struct (x: f64, y: f64, z: f64).
pub(crate) fn reg_with_point() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
                ("z".to_string(), RustType::F64).into(),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg
}

/// Helper: parse a type annotation from a variable declaration.
pub(crate) fn parse_type_annotation(source: &str) -> swc_ecma_ast::Module {
    parse_typescript(source).unwrap()
}

/// Extracts a `TsTypeAliasDecl` from the module body at `index`.
pub(crate) fn extract_type_alias(
    module: &swc_ecma_ast::Module,
    index: usize,
) -> &swc_ecma_ast::TsTypeAliasDecl {
    match module.body.get(index) {
        Some(ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(alias)))) => alias,
        _ => panic!("expected type alias declaration at index {index}"),
    }
}
