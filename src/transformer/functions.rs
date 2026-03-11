//! Function declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC function declarations into the IR [`Item::Fn`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Item, Param, Visibility};
use crate::transformer::statements::convert_stmt;
use crate::transformer::types::convert_ts_type;

/// Converts an SWC [`ast::FnDecl`] into an IR [`Item::Fn`].
///
/// Extracts the function name, parameters (with type annotations),
/// return type, and body statements.
///
/// # Errors
///
/// Returns an error if parameter patterns are unsupported, type annotations
/// are missing, or body statements fail to convert.
pub fn convert_fn_decl(fn_decl: &ast::FnDecl) -> Result<Item> {
    let name = fn_decl.ident.sym.to_string();

    let mut params = Vec::new();
    for param in &fn_decl.function.params {
        let p = convert_param(&param.pat)?;
        params.push(p);
    }

    let return_type = fn_decl
        .function
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann))
        .transpose()?;

    let body = match &fn_decl.function.body {
        Some(block) => {
            let mut stmts = Vec::new();
            for stmt in &block.stmts {
                stmts.push(convert_stmt(stmt)?);
            }
            stmts
        }
        None => Vec::new(),
    };

    Ok(Item::Fn {
        vis: Visibility::Public,
        name,
        params,
        return_type,
        body,
    })
}

/// Converts a function parameter pattern into an IR [`Param`].
fn convert_param(pat: &ast::Pat) -> Result<Param> {
    match pat {
        ast::Pat::Ident(ident) => {
            let name = ident.id.sym.to_string();
            let ty = ident
                .type_ann
                .as_ref()
                .ok_or_else(|| anyhow!("parameter '{}' has no type annotation", name))?;
            let rust_type = convert_ts_type(&ty.type_ann)?;
            Ok(Param {
                name,
                ty: rust_type,
            })
        }
        _ => Err(anyhow!("unsupported parameter pattern")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, Item, Param, RustType, Stmt, Visibility};
    use crate::parser::parse_typescript;
    use swc_ecma_ast::{Decl, ModuleItem};

    /// Helper: parse TS source and extract the first FnDecl.
    fn parse_fn_decl(source: &str) -> ast::FnDecl {
        let module = parse_typescript(source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => fn_decl.clone(),
            _ => panic!("expected function declaration"),
        }
    }

    #[test]
    fn test_convert_fn_decl_add() {
        let fn_decl = parse_fn_decl("function add(a: number, b: number): number { return a + b; }");
        let item = convert_fn_decl(&fn_decl).unwrap();
        assert_eq!(
            item,
            Item::Fn {
                vis: Visibility::Public,
                name: "add".to_string(),
                params: vec![
                    Param {
                        name: "a".to_string(),
                        ty: RustType::F64,
                    },
                    Param {
                        name: "b".to_string(),
                        ty: RustType::F64,
                    },
                ],
                return_type: Some(RustType::F64),
                body: vec![Stmt::Return(Some(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("a".to_string())),
                    op: "+".to_string(),
                    right: Box::new(Expr::Ident("b".to_string())),
                }))],
            }
        );
    }

    #[test]
    fn test_convert_fn_decl_no_return_type() {
        let fn_decl = parse_fn_decl("function greet(name: string) { return name; }");
        let item = convert_fn_decl(&fn_decl).unwrap();
        match item {
            Item::Fn {
                name, return_type, ..
            } => {
                assert_eq!(name, "greet");
                assert_eq!(return_type, None);
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_no_params() {
        let fn_decl = parse_fn_decl("function noop(): boolean { return true; }");
        let item = convert_fn_decl(&fn_decl).unwrap();
        match item {
            Item::Fn { params, body, .. } => {
                assert!(params.is_empty());
                assert_eq!(body, vec![Stmt::Return(Some(Expr::BoolLit(true)))]);
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_with_local_vars() {
        let fn_decl = parse_fn_decl(
            "function calc(x: number): number { const result = x + 1; return result; }",
        );
        let item = convert_fn_decl(&fn_decl).unwrap();
        match item {
            Item::Fn { body, .. } => {
                assert_eq!(body.len(), 2);
                // first statement is a let binding
                match &body[0] {
                    Stmt::Let {
                        mutable,
                        name,
                        init,
                        ..
                    } => {
                        assert!(!mutable);
                        assert_eq!(name, "result");
                        assert!(init.is_some());
                    }
                    _ => panic!("expected Stmt::Let"),
                }
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_missing_param_type_annotation() {
        let fn_decl = parse_fn_decl("function bad(x) { return x; }");
        let result = convert_fn_decl(&fn_decl);
        assert!(result.is_err());
    }
}
