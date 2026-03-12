//! Statement conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC statement nodes into the IR [`Stmt`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Stmt};
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
        ast::Stmt::Throw(throw_stmt) => convert_throw_stmt(throw_stmt),
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

/// Converts a `throw` statement into `return Err(...)`.
///
/// - `throw new Error("msg")` → `return Err("msg".to_string())`
/// - `throw "msg"` → `return Err("msg".to_string())`
/// - Other throw expressions → `return Err(expr.to_string())`
fn convert_throw_stmt(throw_stmt: &ast::ThrowStmt) -> Result<Stmt> {
    let err_arg = extract_error_message(&throw_stmt.arg);
    let err_expr = Expr::MethodCall {
        object: Box::new(err_arg),
        method: "to_string".to_string(),
        args: vec![],
    };
    Ok(Stmt::Return(Some(Expr::FnCall {
        name: "Err".to_string(),
        args: vec![err_expr],
    })))
}

/// Extracts the error message expression from a `throw` argument.
///
/// - `new Error("msg")` → `StringLit("msg")`
/// - `"msg"` → `StringLit("msg")`
/// - Other → converts as generic expression
fn extract_error_message(expr: &ast::Expr) -> Expr {
    match expr {
        ast::Expr::New(new_expr) => {
            // `throw new Error("msg")` → extract "msg"
            if let Some(args) = &new_expr.args {
                if let Some(first) = args.first() {
                    if let Ok(e) = convert_expr(&first.expr) {
                        return e;
                    }
                }
            }
            Expr::StringLit("unknown error".to_string())
        }
        other => {
            convert_expr(other).unwrap_or_else(|_| Expr::StringLit("unknown error".to_string()))
        }
    }
}

/// Converts a list of SWC statements into IR statements, expanding `try/catch` blocks inline.
///
/// `try { stmts... } catch (e) { ... }` is expanded to just the try body statements.
/// The catch block is dropped (throw statements in the try body are already converted to `return Err(...)`).
pub fn convert_stmt_list(stmts: &[ast::Stmt]) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();
    for stmt in stmts {
        match stmt {
            ast::Stmt::Try(try_stmt) => {
                // Expand try body inline
                for s in &try_stmt.block.stmts {
                    result.push(convert_stmt(s)?);
                }
                // catch block is dropped — throw is already Err(), and ? propagation
                // requires function call support which is not yet available
            }
            other => {
                result.push(convert_stmt(other)?);
            }
        }
    }
    Ok(result)
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
    fn test_convert_stmt_list_try_catch_expands_try_body() {
        let stmts = parse_fn_body(
            "function f() { try { const x = 1; return x; } catch (e) { return 0; } }",
        );
        // try/catch is expanded: try body is inlined, catch is dropped
        let result = convert_stmt_list(&stmts).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
        assert_eq!(result[1], Stmt::Return(Some(Expr::Ident("x".to_string()))));
    }

    #[test]
    fn test_convert_stmt_list_try_catch_empty_catch() {
        let stmts = parse_fn_body("function f() { try { const x = 1; } catch (e) { } }");
        let result = convert_stmt_list(&stmts).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_throw_new_error_string() {
        let stmts = parse_fn_body("function f() { throw new Error(\"something went wrong\"); }");
        let result = convert_stmt(&stmts[0]).unwrap();
        // throw new Error("msg") → return Err("msg".to_string())
        assert_eq!(
            result,
            Stmt::Return(Some(Expr::FnCall {
                name: "Err".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::StringLit("something went wrong".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }],
            }))
        );
    }

    #[test]
    fn test_convert_stmt_throw_string_literal() {
        let stmts = parse_fn_body("function f() { throw \"error msg\"; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        // throw "msg" → return Err("msg".to_string())
        assert_eq!(
            result,
            Stmt::Return(Some(Expr::FnCall {
                name: "Err".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::StringLit("error msg".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }],
            }))
        );
    }

    #[test]
    fn test_convert_stmt_expression_statement() {
        let stmts = parse_fn_body("function f() { foo; }");
        let result = convert_stmt(&stmts[0]).unwrap();
        assert_eq!(result, Stmt::Expr(Expr::Ident("foo".to_string())));
    }
}
