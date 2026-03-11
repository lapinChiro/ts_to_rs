//! Statement conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC statement nodes into the IR [`Stmt`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::Stmt;
use crate::transformer::expressions::convert_expr;
use crate::transformer::types::convert_ts_type;

/// Converts an SWC [`ast::Stmt`] into an IR [`Stmt`].
///
/// # Supported conversions
///
/// - Variable declarations (`const` → `let`, `let` → `let mut`)
/// - Return statements
/// - If/else statements
/// - Expression statements
///
/// # Errors
///
/// Returns an error for unsupported statement types.
pub fn convert_stmt(stmt: &ast::Stmt) -> Result<Stmt> {
    match stmt {
        ast::Stmt::Return(ret) => {
            let expr = ret.arg.as_ref().map(|e| convert_expr(e)).transpose()?;
            Ok(Stmt::Return(expr))
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => convert_var_decl(var_decl),
        ast::Stmt::If(if_stmt) => convert_if_stmt(if_stmt),
        ast::Stmt::Expr(expr_stmt) => {
            let expr = convert_expr(&expr_stmt.expr)?;
            Ok(Stmt::Expr(expr))
        }
        _ => Err(anyhow!("unsupported statement: {:?}", stmt)),
    }
}

/// Converts a variable declaration to an IR `Stmt::Let`.
///
/// - `const` → immutable (`let`)
/// - `let` / `var` → mutable (`let mut`)
fn convert_var_decl(var_decl: &ast::VarDecl) -> Result<Stmt> {
    // We only handle single-declarator variable declarations
    if var_decl.decls.len() != 1 {
        return Err(anyhow!(
            "multiple variable declarators in one statement are not supported"
        ));
    }
    let declarator = &var_decl.decls[0];

    let name = match &declarator.name {
        ast::Pat::Ident(ident) => ident.id.sym.to_string(),
        _ => return Err(anyhow!("unsupported variable binding pattern")),
    };

    let mutable = !matches!(var_decl.kind, ast::VarDeclKind::Const);

    let ty = match &declarator.name {
        ast::Pat::Ident(ident) => ident
            .type_ann
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann))
            .transpose()?,
        _ => None,
    };

    let init = declarator
        .init
        .as_ref()
        .map(|e| convert_expr(e))
        .transpose()?;

    Ok(Stmt::Let {
        mutable,
        name,
        ty,
        init,
    })
}

/// Converts an if statement to an IR `Stmt::If`.
fn convert_if_stmt(if_stmt: &ast::IfStmt) -> Result<Stmt> {
    let condition = convert_expr(&if_stmt.test)?;

    let then_body = convert_block_or_stmt(&if_stmt.cons)?;

    let else_body = if_stmt
        .alt
        .as_ref()
        .map(|alt| convert_block_or_stmt(alt))
        .transpose()?;

    Ok(Stmt::If {
        condition,
        then_body,
        else_body,
    })
}

/// Converts a block statement or single statement into a `Vec<Stmt>`.
fn convert_block_or_stmt(stmt: &ast::Stmt) -> Result<Vec<Stmt>> {
    match stmt {
        ast::Stmt::Block(block) => {
            let mut stmts = Vec::new();
            for s in &block.stmts {
                stmts.push(convert_stmt(s)?);
            }
            Ok(stmts)
        }
        other => {
            let s = convert_stmt(other)?;
            Ok(vec![s])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, RustType, Stmt};
    use crate::parser::parse_typescript;
    use swc_ecma_ast::{Decl, ModuleItem};

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

    #[test]
    fn test_convert_stmt_return_expr() {
        let stmts = parse_fn_body("function f() { return 42; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
    }

    #[test]
    fn test_convert_stmt_return_no_value() {
        let stmts = parse_fn_body("function f() { return; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(result, Stmt::Return(None));
    }

    #[test]
    fn test_convert_stmt_const_decl() {
        let stmts = parse_fn_body("function f() { const x = 1; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_let_decl_mutable() {
        let stmts = parse_fn_body("function f() { let x = 1; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: true,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_const_with_type_annotation() {
        let stmts = parse_fn_body("function f() { const x: number = 1; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: Some(RustType::F64),
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_if_no_else() {
        let stmts = parse_fn_body("function f() { if (true) { return 1; } }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(
            result,
            Stmt::If {
                condition: Expr::BoolLit(true),
                then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
                else_body: None,
            }
        );
    }

    #[test]
    fn test_convert_stmt_if_else() {
        let stmts = parse_fn_body("function f() { if (true) { return 1; } else { return 2; } }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(
            result,
            Stmt::If {
                condition: Expr::BoolLit(true),
                then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
                else_body: Some(vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]),
            }
        );
    }

    #[test]
    fn test_convert_stmt_expression_statement() {
        let stmts = parse_fn_body("function f() { foo; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(result, Stmt::Expr(Expr::Ident("foo".to_string())));
    }
}
