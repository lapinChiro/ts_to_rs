mod collections;
mod interfaces;
mod intersections;
mod primitives;
mod structural_transforms;
mod type_aliases;
mod unions;

use super::*;
use crate::ir::{StructField, TypeParam};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use swc_ecma_ast::{Decl, ModuleItem, Stmt};

/// Helper: parse TS source and extract the first TsInterfaceDecl.
fn parse_interface(source: &str) -> TsInterfaceDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(decl))) => *decl.clone(),
        _ => panic!("expected TsInterfaceDecl"),
    }
}

/// Helper: parse TS source and extract the first TsTypeAliasDecl.
fn parse_type_alias(source: &str) -> TsTypeAliasDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(decl))) => *decl.clone(),
        _ => panic!("expected TsTypeAliasDecl"),
    }
}

/// Helper: parse a type annotation from `interface T { x: <TYPE>; }` and return the SWC type node.
fn parse_type_ann(type_str: &str) -> Box<swc_ecma_ast::TsType> {
    let source = format!("interface T {{ x: {type_str}; }}");
    let decl = parse_interface(&source);
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    prop.type_ann.as_ref().unwrap().type_ann.clone()
}

/// Helper: create a TypeRegistry with Point struct (x: f64, y: f64, z: f64).
fn reg_with_point() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
                ("z".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg
}
