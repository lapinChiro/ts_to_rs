mod destructuring;
mod fn_decl;
mod helpers;
mod params;

use std::collections::HashMap;

use super::*;
use crate::ir::{BinOp, Expr, Item, Param, RustType, Stmt, StructField, TypeParam, Visibility};
use crate::parser::parse_typescript;
use crate::registry::{MethodSignature, TypeDef, TypeRegistry};
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;
use swc_ecma_ast::{Decl, ModuleItem};

/// Helper: parse TS source and extract the first FnDecl.
fn parse_fn_decl(source: &str) -> ast::FnDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => fn_decl.clone(),
        _ => panic!("expected function declaration"),
    }
}
