mod control_flow;
mod destructuring;
mod error_handling;
mod expected_types;
mod helpers;
mod loops;
mod switch;
mod variables;

use std::collections::HashMap;

use super::*;
use crate::ir::{BinOp, Expr, Pattern, RustType, Stmt, UnOp};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{MethodSignature, TypeDef, TypeRegistry};
use crate::transformer::context::TransformContext;
use crate::transformer::expressions::member_access::{
    build_safe_index_expr_unwrapped, convert_index_to_usize,
};
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;
use std::path::Path;
use swc_ecma_ast::{Decl, ModuleItem};

/// Helper: convert a single statement and assert exactly one IR statement is produced.
fn convert_single_stmt(
    stmt: &ast::Stmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Stmt {
    let (mg, res) = TctxFixture::empty_context_parts();
    let tctx = TransformContext::new(&mg, reg, &res, Path::new("test.ts"));
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);
    let mut stmts = t.convert_stmt(stmt, return_type).unwrap();
    assert_eq!(stmts.len(), 1, "expected single statement, got {stmts:?}");
    stmts.remove(0)
}

/// Helper: convert a single statement from source, running TypeResolver first.
///
/// Unlike `convert_single_stmt`, this runs TypeResolver to populate expected types.
/// Use for tests that depend on type annotation-based expected type propagation.
fn convert_single_stmt_resolved(
    source: &str,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Stmt {
    let module = parse_typescript(source).expect("parse failed");
    let mut source_reg = crate::registry::build_registry(&module);
    source_reg.merge(reg);
    let mg = crate::pipeline::ModuleGraph::empty();
    let mut synthetic = SyntheticTypeRegistry::new();
    let parsed = crate::pipeline::ParsedFile {
        path: std::path::PathBuf::from("test.ts"),
        source: source.to_string(),
        module: module.clone(),
    };
    let mut resolver =
        crate::pipeline::type_resolver::TypeResolver::new(&source_reg, &mut synthetic);
    let res = resolver.resolve_file(&parsed);
    let tctx = TransformContext::new(&mg, &source_reg, &res, Path::new("test.ts"));

    // Extract the statement from the parsed module
    let stmt = match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Var(_))) => match &module.body[0] {
            ModuleItem::Stmt(s) => s,
            _ => unreachable!(),
        },
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => {
            // For function declarations, extract the return statement from body
            let body = fn_decl.function.body.as_ref().expect("no function body");
            let stmt = &body.stmts[0];
            let mut stmts = {
                let mut synthetic = SyntheticTypeRegistry::new();
                Transformer::for_module(&tctx, &mut synthetic).convert_stmt(stmt, return_type)
            }
            .unwrap();
            assert_eq!(stmts.len(), 1, "expected single statement, got {stmts:?}");
            return stmts.remove(0);
        }
        ModuleItem::Stmt(s) => s,
        _ => panic!("expected statement"),
    };

    let mut stmts = {
        let mut synthetic = SyntheticTypeRegistry::new();
        Transformer::for_module(&tctx, &mut synthetic).convert_stmt(stmt, return_type)
    }
    .unwrap();
    assert_eq!(stmts.len(), 1, "expected single statement, got {stmts:?}");
    stmts.remove(0)
}

/// Helper: parse TS source containing a function and return its body statements.
fn parse_fn_body(source: &str) -> Vec<ast::Stmt> {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => fn_decl
            .function
            .body
            .as_ref()
            .expect("no function body")
            .stmts
            .clone(),
        _ => panic!("expected function declaration"),
    }
}
