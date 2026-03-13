//! Function declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC function declarations into the IR [`Item::Fn`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, Param, RustType, Stmt, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::statements::convert_stmt_list;
use crate::transformer::types::{convert_ts_type, extract_type_params};

/// Converts an SWC [`ast::FnDecl`] into an IR [`Item::Fn`].
///
/// Extracts the function name, parameters (with type annotations),
/// return type, and body statements.
///
/// # Errors
///
/// Returns an error if parameter patterns are unsupported, type annotations
/// are missing, or body statements fail to convert.
pub fn convert_fn_decl(fn_decl: &ast::FnDecl, vis: Visibility, reg: &TypeRegistry) -> Result<Item> {
    let name = fn_decl.ident.sym.to_string();

    let mut params = Vec::new();
    for param in &fn_decl.function.params {
        let p = convert_param(&param.pat)?;
        params.push(p);
    }

    let is_async = fn_decl.function.is_async;

    let return_type = fn_decl
        .function
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann))
        .transpose()?;

    // void → None (Rust omits `-> ()`)
    let return_type = return_type.and_then(|ty| {
        if matches!(ty, RustType::Unit) {
            None
        } else {
            Some(ty)
        }
    });

    // Unwrap Promise<T> → T for async functions (before body conversion
    // so that return type context propagates correctly)
    let return_type = if is_async {
        return_type.and_then(unwrap_promise_type)
    } else {
        return_type
    };

    let body = match &fn_decl.function.body {
        Some(block) => convert_stmt_list(&block.stmts, reg, return_type.as_ref())?,
        None => Vec::new(),
    };

    let type_params = extract_type_params(fn_decl.function.type_params.as_deref());

    // If the function body contains `throw`, wrap return type in Result and returns in Ok()
    let has_throw = fn_decl
        .function
        .body
        .as_ref()
        .is_some_and(|block| contains_throw(&block.stmts));

    let (return_type, body) = if has_throw {
        let ok_type = return_type.unwrap_or_else(|| RustType::Named {
            name: "()".to_string(),
            type_args: vec![],
        });
        let result_type = RustType::Result {
            ok: Box::new(ok_type),
            err: Box::new(RustType::String),
        };
        let wrapped_body = wrap_returns_in_ok(body);
        (Some(result_type), wrapped_body)
    } else {
        (return_type, body)
    };

    Ok(Item::Fn {
        vis,
        is_async,
        name,
        type_params,
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
                ty: Some(rust_type),
            })
        }
        _ => Err(anyhow!("unsupported parameter pattern")),
    }
}

/// Unwraps `Promise<T>` to `T` for async function return types.
///
/// If the type is `Named { name: "Promise", type_args: [T] }`, returns `Some(T)`.
/// Otherwise returns the type unchanged.
fn unwrap_promise_type(ty: RustType) -> Option<RustType> {
    match ty {
        RustType::Named {
            ref name,
            ref type_args,
        } if name == "Promise" && type_args.len() == 1 => Some(type_args[0].clone()),
        other => Some(other),
    }
}

/// Checks whether a list of SWC statements contains a `throw` statement (shallow scan).
fn contains_throw(stmts: &[ast::Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        ast::Stmt::Throw(_) => true,
        ast::Stmt::If(if_stmt) => {
            let then_has = match if_stmt.cons.as_ref() {
                ast::Stmt::Block(block) => contains_throw(&block.stmts),
                ast::Stmt::Throw(_) => true,
                _ => false,
            };
            let else_has = if_stmt.alt.as_ref().is_some_and(|alt| match alt.as_ref() {
                ast::Stmt::Block(block) => contains_throw(&block.stmts),
                ast::Stmt::Throw(_) => true,
                _ => false,
            });
            then_has || else_has
        }
        ast::Stmt::Block(block) => contains_throw(&block.stmts),
        _ => false,
    })
}

/// Wraps `return expr` statements in `Ok(expr)` for functions that use `Result`.
///
/// `throw` statements are already converted to `return Err(...)` by `convert_stmt`,
/// so only non-Err returns need wrapping.
fn wrap_returns_in_ok(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts.into_iter().map(wrap_stmt_return).collect()
}

/// Recursively wraps return expressions in `Ok(...)`.
fn wrap_stmt_return(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Return(Some(expr)) => {
            // Don't wrap if already an Err(...) call
            if matches!(&expr, Expr::FnCall { name, .. } if name == "Err") {
                Stmt::Return(Some(expr))
            } else {
                Stmt::Return(Some(Expr::FnCall {
                    name: "Ok".to_string(),
                    args: vec![expr],
                }))
            }
        }
        Stmt::Return(None) => Stmt::Return(Some(Expr::FnCall {
            name: "Ok".to_string(),
            args: vec![Expr::Ident("()".to_string())],
        })),
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => Stmt::If {
            condition,
            then_body: wrap_returns_in_ok(then_body),
            else_body: else_body.map(wrap_returns_in_ok),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, Item, Param, RustType, Stmt, Visibility};
    use crate::parser::parse_typescript;
    use crate::registry::TypeRegistry;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        assert_eq!(
            item,
            Item::Fn {
                vis: Visibility::Public,
                is_async: false,
                name: "add".to_string(),
                type_params: vec![],
                params: vec![
                    Param {
                        name: "a".to_string(),
                        ty: Some(RustType::F64),
                    },
                    Param {
                        name: "b".to_string(),
                        ty: Some(RustType::F64),
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
    fn test_convert_fn_decl_generic_single_param() {
        let fn_decl = parse_fn_decl("function identity<T>(x: T): T { return x; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn { type_params, .. } => {
                assert_eq!(type_params, vec!["T".to_string()]);
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_generic_multiple_params() {
        let fn_decl = parse_fn_decl("function pair<A, B>(a: A, b: B): A { return a; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn { type_params, .. } => {
                assert_eq!(type_params, vec!["A".to_string(), "B".to_string()]);
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_throw_wraps_return_type_in_result() {
        let fn_decl =
            parse_fn_decl("function validate(x: number): string { if (x < 0) { throw new Error(\"negative\"); } return \"ok\"; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn { return_type, .. } => {
                assert_eq!(
                    return_type,
                    Some(RustType::Result {
                        ok: Box::new(RustType::String),
                        err: Box::new(RustType::String),
                    })
                );
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_throw_wraps_return_in_ok() {
        let fn_decl =
            parse_fn_decl("function validate(x: number): string { if (x < 0) { throw new Error(\"negative\"); } return \"ok\"; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn { body, .. } => {
                // The last statement should be return Ok("ok".to_string())
                let last = body.last().unwrap();
                assert_eq!(
                    *last,
                    Stmt::Return(Some(Expr::FnCall {
                        name: "Ok".to_string(),
                        args: vec![Expr::MethodCall {
                            object: Box::new(Expr::StringLit("ok".to_string())),
                            method: "to_string".to_string(),
                            args: vec![],
                        }],
                    }))
                );
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_throw_no_return_type_becomes_result_unit() {
        let fn_decl = parse_fn_decl("function fail() { throw new Error(\"boom\"); }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn { return_type, .. } => {
                assert_eq!(
                    return_type,
                    Some(RustType::Result {
                        ok: Box::new(RustType::Named {
                            name: "()".to_string(),
                            type_args: vec![],
                        }),
                        err: Box::new(RustType::String),
                    })
                );
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_missing_param_type_annotation() {
        let fn_decl = parse_fn_decl("function bad(x) { return x; }");
        let result = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new());
        assert!(result.is_err());
    }

    // -- async function tests --

    #[test]
    fn test_convert_fn_decl_async_is_async() {
        let fn_decl = parse_fn_decl("async function fetchData(): Promise<number> { return 42; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn {
                is_async,
                return_type,
                ..
            } => {
                assert!(is_async);
                // Promise<number> should unwrap to f64
                assert_eq!(return_type, Some(RustType::F64));
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_async_no_return_type() {
        let fn_decl = parse_fn_decl("async function doWork() { return; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn {
                is_async,
                return_type,
                ..
            } => {
                assert!(is_async);
                assert_eq!(return_type, None);
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_sync_is_not_async() {
        let fn_decl = parse_fn_decl("function add(a: number, b: number): number { return a + b; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        match item {
            Item::Fn { is_async, .. } => {
                assert!(!is_async);
            }
            _ => panic!("expected Item::Fn"),
        }
    }
}
