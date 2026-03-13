//! Function declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC function declarations into the IR [`Item::Fn`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, Param, RustType, Stmt, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::statements::convert_stmt_list;
use crate::transformer::types::{convert_ts_type, extract_type_params};
use crate::transformer::{extract_pat_ident_name, extract_prop_name};

/// Converts an SWC [`ast::FnDecl`] into an IR [`Item::Fn`].
///
/// Extracts the function name, parameters (with type annotations),
/// return type, and body statements.
///
/// # Errors
///
/// Returns an error if parameter patterns are unsupported, type annotations
/// are missing, or body statements fail to convert.
pub fn convert_fn_decl(
    fn_decl: &ast::FnDecl,
    vis: Visibility,
    reg: &TypeRegistry,
    resilient: bool,
) -> Result<(Item, Vec<String>)> {
    let name = fn_decl.ident.sym.to_string();
    let mut fallback_warnings = Vec::new();

    let mut params = Vec::new();
    let mut destructuring_stmts = Vec::new();
    for param in &fn_decl.function.params {
        let (p, stmts) = convert_param(&param.pat, resilient, &mut fallback_warnings)?;
        params.push(p);
        destructuring_stmts.extend(stmts);
    }

    let is_async = fn_decl.function.is_async;

    let return_type = fn_decl
        .function
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type_with_fallback(&ann.type_ann, resilient, &mut fallback_warnings))
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

    let body_stmts = match &fn_decl.function.body {
        Some(block) => convert_stmt_list(&block.stmts, reg, return_type.as_ref())?,
        None => Vec::new(),
    };
    // Prepend destructuring expansion statements
    let body = if destructuring_stmts.is_empty() {
        body_stmts
    } else {
        let mut combined = destructuring_stmts;
        combined.extend(body_stmts);
        combined
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

    Ok((
        Item::Fn {
            vis,
            is_async,
            name,
            type_params,
            params,
            return_type,
            body,
        },
        fallback_warnings,
    ))
}

/// Converts a TypeScript type to an IR type, falling back to [`RustType::Any`] when
/// `resilient` is true and the type is unsupported.
///
/// When falling back, appends the error message to `fallback_warnings` for reporting.
pub(crate) fn convert_ts_type_with_fallback(
    ts_type: &swc_ecma_ast::TsType,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
) -> Result<RustType> {
    match convert_ts_type(ts_type) {
        Ok(ty) => Ok(ty),
        Err(e) => {
            if resilient {
                fallback_warnings.push(e.to_string());
                Ok(RustType::Any)
            } else {
                Err(e)
            }
        }
    }
}

/// Converts a function parameter pattern into an IR [`Param`] and optional expansion statements.
///
/// For simple identifier parameters, returns the param with no expansion.
/// For object destructuring parameters (`{ x, y }: Point`), returns a synthetic
/// parameter (named from the type annotation) and `let` statements to expand the fields.
///
/// When `resilient` is true, unsupported type annotations fall back to [`RustType::Any`].
fn convert_param(
    pat: &ast::Pat,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
) -> Result<(Param, Vec<Stmt>)> {
    match pat {
        ast::Pat::Ident(ident) => {
            let name = ident.id.sym.to_string();
            let ty = ident
                .type_ann
                .as_ref()
                .ok_or_else(|| anyhow!("parameter '{}' has no type annotation", name))?;
            let rust_type =
                convert_ts_type_with_fallback(&ty.type_ann, resilient, fallback_warnings)?;
            Ok((
                Param {
                    name,
                    ty: Some(rust_type),
                },
                vec![],
            ))
        }
        ast::Pat::Object(obj_pat) => convert_object_destructuring_param(obj_pat),
        ast::Pat::Assign(assign) => convert_default_param(assign, resilient, fallback_warnings),
        _ => Err(anyhow!("unsupported parameter pattern")),
    }
}

/// Converts a parameter with a default value into an `Option<T>` parameter
/// with an `unwrap_or` / `unwrap_or_default` expansion statement.
///
/// Example: `(x: number = 0)` → param `x: Option<f64>` + `let x = x.unwrap_or(0.0);`
fn convert_default_param(
    assign: &ast::AssignPat,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
) -> Result<(Param, Vec<Stmt>)> {
    // Recursively convert the inner parameter (left side)
    let (inner_param, mut stmts) = convert_param(&assign.left, resilient, fallback_warnings)?;
    let param_name = inner_param.name.clone();

    // Wrap the type in Option<T>
    let inner_type = inner_param
        .ty
        .ok_or_else(|| anyhow!("default parameter requires a type annotation"))?;
    let option_type = RustType::Option(Box::new(inner_type));

    // Convert default value to IR expression
    let (default_expr, use_unwrap_or_default) = convert_default_value(&assign.right)?;

    // Generate expansion statement: `let x = x.unwrap_or(value);` or `let x = x.unwrap_or_default();`
    let unwrap_call = if use_unwrap_or_default {
        Expr::MethodCall {
            object: Box::new(Expr::Ident(param_name.clone())),
            method: "unwrap_or_default".to_string(),
            args: vec![],
        }
    } else {
        Expr::MethodCall {
            object: Box::new(Expr::Ident(param_name.clone())),
            method: "unwrap_or".to_string(),
            args: vec![default_expr.unwrap()],
        }
    };

    stmts.insert(
        0,
        Stmt::Let {
            mutable: false,
            name: param_name.clone(),
            ty: None,
            init: Some(unwrap_call),
        },
    );

    Ok((
        Param {
            name: param_name,
            ty: Some(option_type),
        },
        stmts,
    ))
}

/// Converts a default value expression to an IR [`Expr`].
///
/// Returns `(Some(expr), false)` for literal values (use `unwrap_or`),
/// or `(None, true)` for empty objects (use `unwrap_or_default`).
fn convert_default_value(expr: &ast::Expr) -> Result<(Option<Expr>, bool)> {
    match expr {
        ast::Expr::Lit(lit) => match lit {
            ast::Lit::Num(n) => Ok((Some(Expr::NumberLit(n.value)), false)),
            ast::Lit::Str(s) => Ok((
                Some(Expr::MethodCall {
                    object: Box::new(Expr::StringLit(s.value.to_string_lossy().into_owned())),
                    method: "to_string".to_string(),
                    args: vec![],
                }),
                false,
            )),
            ast::Lit::Bool(b) => Ok((Some(Expr::BoolLit(b.value)), false)),
            _ => Err(anyhow!("unsupported default parameter value")),
        },
        ast::Expr::Object(obj) if obj.props.is_empty() => {
            // `= {}` → unwrap_or_default()
            Ok((None, true))
        }
        _ => Err(anyhow!("unsupported default parameter value")),
    }
}

/// Converts an object destructuring parameter pattern into a synthetic [`Param`]
/// and expansion statements.
///
/// Example: `{ x, y }: Point` → param `point: Point` + `let x = point.x; let y = point.y;`
fn convert_object_destructuring_param(obj_pat: &ast::ObjectPat) -> Result<(Param, Vec<Stmt>)> {
    let type_ann = obj_pat
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("object destructuring parameter requires a type annotation"))?;
    let rust_type = convert_ts_type(&type_ann.type_ann)?;

    // Generate parameter name from type name (PascalCase → snake_case)
    let param_name = match &rust_type {
        RustType::Named { name, .. } => pascal_to_snake(name),
        _ => "param".to_string(),
    };

    let param = Param {
        name: param_name.clone(),
        ty: Some(rust_type),
    };

    let mut stmts = Vec::new();
    for prop in &obj_pat.props {
        match prop {
            ast::ObjectPatProp::Assign(assign) => {
                // { x } — shorthand
                let field_name = assign.key.sym.to_string();
                stmts.push(Stmt::Let {
                    mutable: false,
                    name: field_name.clone(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident(param_name.clone())),
                        field: field_name,
                    }),
                });
            }
            ast::ObjectPatProp::KeyValue(kv) => {
                // { x: newX } — rename
                let field_name = extract_prop_name(&kv.key)
                    .map_err(|_| anyhow!("unsupported destructuring key"))?;
                let binding_name = extract_pat_ident_name(kv.value.as_ref())
                    .map_err(|_| anyhow!("unsupported destructuring value pattern"))?;
                let binding_name = pascal_to_snake(&binding_name);
                stmts.push(Stmt::Let {
                    mutable: false,
                    name: binding_name,
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident(param_name.clone())),
                        field: field_name,
                    }),
                });
            }
            ast::ObjectPatProp::Rest(_) => {
                return Err(anyhow!(
                    "rest pattern in destructuring parameter is not supported"
                ));
            }
        }
    }

    Ok((param, stmts))
}

/// Converts a PascalCase name to snake_case.
///
/// Example: `"HonoOptions"` → `"hono_options"`, `"Point"` → `"point"`
fn pascal_to_snake(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap_or(ch));
        } else {
            result.push(ch);
        }
    }
    result
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let result = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false);
        assert!(result.is_err());
    }

    // -- async function tests --

    #[test]
    fn test_convert_fn_decl_async_is_async() {
        let fn_decl = parse_fn_decl("async function fetchData(): Promise<number> { return 42; }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
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
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
        match item {
            Item::Fn { is_async, .. } => {
                assert!(!is_async);
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_object_destructuring_param_generates_expansion() {
        let fn_decl = parse_fn_decl("function foo({ x, y }: Point): void { console.log(x); }");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;

        match item {
            Item::Fn { params, body, .. } => {
                // Parameter should be renamed to snake_case of the type
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "point");
                assert_eq!(
                    params[0].ty,
                    Some(RustType::Named {
                        name: "Point".to_string(),
                        type_args: vec![],
                    })
                );
                // Body should start with expansion statements
                assert!(body.len() >= 2);
                assert_eq!(
                    body[0],
                    Stmt::Let {
                        mutable: false,
                        name: "x".to_string(),
                        ty: None,
                        init: Some(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("point".to_string())),
                            field: "x".to_string(),
                        }),
                    }
                );
                assert_eq!(
                    body[1],
                    Stmt::Let {
                        mutable: false,
                        name: "y".to_string(),
                        ty: None,
                        init: Some(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("point".to_string())),
                            field: "y".to_string(),
                        }),
                    }
                );
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_object_destructuring_rename() {
        let fn_decl = parse_fn_decl("function foo({ x: newX, y: newY }: Point): void {}");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;

        match item {
            Item::Fn { body, .. } => {
                assert_eq!(
                    body[0],
                    Stmt::Let {
                        mutable: false,
                        name: "new_x".to_string(),
                        ty: None,
                        init: Some(Expr::FieldAccess {
                            object: Box::new(Expr::Ident("point".to_string())),
                            field: "x".to_string(),
                        }),
                    }
                );
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_destructuring_with_normal_params() {
        let fn_decl = parse_fn_decl("function foo(name: string, { x, y }: Point): void {}");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;

        match item {
            Item::Fn { params, body, .. } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].name, "name");
                assert_eq!(params[1].name, "point");
                // Expansion statements in body
                assert_eq!(body.len(), 2);
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_destructuring_no_type_annotation_fails() {
        let fn_decl = parse_fn_decl("function foo({ x, y }): void {}");
        let result = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_fn_decl_default_number_param_wraps_in_option() {
        let fn_decl = parse_fn_decl("function foo(x: number = 0): void {}");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
        match item {
            Item::Fn { params, body, .. } => {
                // Parameter type should be Option<f64>
                assert_eq!(
                    params[0].ty,
                    Some(RustType::Option(Box::new(RustType::F64)))
                );
                // Body should start with `let x = x.unwrap_or(0.0);`
                assert!(
                    !body.is_empty(),
                    "body should contain unwrap_or expansion statement"
                );
                match &body[0] {
                    Stmt::Let {
                        name,
                        init,
                        mutable,
                        ..
                    } => {
                        assert_eq!(name, "x");
                        assert!(!mutable);
                        // init should be a method call: x.unwrap_or(0.0)
                        match init.as_ref().unwrap() {
                            Expr::MethodCall {
                                object,
                                method,
                                args,
                            } => {
                                assert_eq!(method, "unwrap_or");
                                assert!(matches!(object.as_ref(), Expr::Ident(n) if n == "x"));
                                assert_eq!(args.len(), 1);
                                assert!(
                                    matches!(&args[0], Expr::NumberLit(n) if *n == 0.0),
                                    "expected NumberLit(0.0), got {:?}",
                                    &args[0]
                                );
                            }
                            other => panic!("expected MethodCall, got {other:?}"),
                        }
                    }
                    other => panic!("expected Let statement, got {other:?}"),
                }
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_default_string_param_wraps_in_option() {
        let fn_decl = parse_fn_decl("function foo(name: string = \"hello\"): void {}");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
        match item {
            Item::Fn { params, body, .. } => {
                assert_eq!(
                    params[0].ty,
                    Some(RustType::Option(Box::new(RustType::String)))
                );
                match &body[0] {
                    Stmt::Let { name, init, .. } => {
                        assert_eq!(name, "name");
                        match init.as_ref().unwrap() {
                            Expr::MethodCall { method, args, .. } => {
                                assert_eq!(method, "unwrap_or");
                                // arg should be "hello".to_string()
                                assert_eq!(args.len(), 1);
                                match &args[0] {
                                    Expr::MethodCall { object, method, .. } => {
                                        assert_eq!(method, "to_string");
                                        assert!(
                                            matches!(object.as_ref(), Expr::StringLit(s) if s == "hello")
                                        );
                                    }
                                    other => panic!("expected MethodCall, got {other:?}"),
                                }
                            }
                            other => panic!("expected MethodCall, got {other:?}"),
                        }
                    }
                    other => panic!("expected Let, got {other:?}"),
                }
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_default_bool_param_wraps_in_option() {
        let fn_decl = parse_fn_decl("function foo(flag: boolean = true): void {}");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
        match item {
            Item::Fn { params, body, .. } => {
                assert_eq!(
                    params[0].ty,
                    Some(RustType::Option(Box::new(RustType::Bool)))
                );
                match &body[0] {
                    Stmt::Let { init, .. } => match init.as_ref().unwrap() {
                        Expr::MethodCall { method, args, .. } => {
                            assert_eq!(method, "unwrap_or");
                            assert!(matches!(&args[0], Expr::BoolLit(true)));
                        }
                        other => panic!("expected MethodCall, got {other:?}"),
                    },
                    other => panic!("expected Let, got {other:?}"),
                }
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_default_empty_object_uses_unwrap_or_default() {
        let fn_decl = parse_fn_decl("function foo(options: Config = {}): void {}");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
        match item {
            Item::Fn { params, body, .. } => {
                assert_eq!(
                    params[0].ty,
                    Some(RustType::Option(Box::new(RustType::Named {
                        name: "Config".to_string(),
                        type_args: vec![],
                    })))
                );
                match &body[0] {
                    Stmt::Let { init, .. } => match init.as_ref().unwrap() {
                        Expr::MethodCall { method, args, .. } => {
                            assert_eq!(method, "unwrap_or_default");
                            assert!(args.is_empty());
                        }
                        other => panic!("expected MethodCall, got {other:?}"),
                    },
                    other => panic!("expected Let, got {other:?}"),
                }
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_default_param_mixed_with_normal() {
        let fn_decl = parse_fn_decl("function foo(a: number, b: number = 10): void {}");
        let item = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false)
            .unwrap()
            .0;
        match item {
            Item::Fn { params, body, .. } => {
                // First param: normal
                assert_eq!(params[0].name, "a");
                assert_eq!(params[0].ty, Some(RustType::F64));
                // Second param: Option<f64>
                assert_eq!(params[1].name, "b");
                assert_eq!(
                    params[1].ty,
                    Some(RustType::Option(Box::new(RustType::F64)))
                );
                // Body should have unwrap_or expansion for b
                match &body[0] {
                    Stmt::Let { name, .. } => assert_eq!(name, "b"),
                    other => panic!("expected Let, got {other:?}"),
                }
            }
            _ => panic!("expected Item::Fn"),
        }
    }

    #[test]
    fn test_convert_fn_decl_default_unsupported_value_errors() {
        // new Map() is an unsupported default value
        let fn_decl = parse_fn_decl("function foo(m: Map = new Map()): void {}");
        let result = convert_fn_decl(&fn_decl, Visibility::Public, &TypeRegistry::new(), false);
        assert!(result.is_err());
    }
}
