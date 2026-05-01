mod basics;
mod i_205;
mod inheritance;
mod param_prop;

use std::collections::HashMap;

use super::*;
use crate::ir::{Expr, Item, Method, Param, RustType, Stmt, StructField, Visibility};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;
use swc_ecma_ast::{Decl, ModuleItem};

/// Helper: parse TS source and extract the first ClassDecl.
fn parse_class_decl(source: &str) -> ast::ClassDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Class(decl))) => decl.clone(),
        _ => panic!("expected ClassDecl"),
    }
}
