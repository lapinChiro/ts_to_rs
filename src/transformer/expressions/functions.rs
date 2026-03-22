//! Function and arrow expression conversion to IR closures.
//!
//! Converts function expressions (`function(x) { ... }`) and arrow expressions
//! (`(x) => ...`) into [`Expr::Closure`] IR nodes.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{ClosureBody, Expr, Param, RustType, Stmt};
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::functions::{convert_last_return_to_tail, convert_ts_type_with_fallback};
use crate::transformer::statements::convert_stmt;
use crate::transformer::TypeEnv;

use super::{convert_expr, ExprContext};
use crate::transformer::context::TransformContext;

/// Converts a single parameter pattern into IR [`Param`] and expansion statements.
///
/// Shared logic for both function expressions and arrow expressions.
/// The `convert_type` closure abstracts over `convert_ts_type` vs
/// `convert_ts_type_with_fallback`, allowing callers to control resilient mode.
/// `context` is used in error messages (e.g. "function expression", "arrow").
fn convert_function_param_pat(
    pat: &ast::Pat,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    params: &mut Vec<Param>,
    expansion_stmts: &mut Vec<Stmt>,
    context: &str,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<()> {
    match pat {
        ast::Pat::Ident(ident) => {
            let name = ident.id.sym.to_string();
            let rust_type = ident
                .type_ann
                .as_ref()
                .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                .transpose()?;
            params.push(Param {
                name,
                ty: rust_type,
            });
        }
        ast::Pat::Object(obj_pat) => {
            let (param, stmts) = crate::transformer::functions::convert_object_destructuring_param(
                obj_pat, tctx, reg, synthetic,
            )?;
            params.push(param);
            expansion_stmts.extend(stmts);
        }
        ast::Pat::Assign(assign) => match assign.left.as_ref() {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let inner_type = ident
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                    .transpose()?
                    .ok_or_else(|| anyhow!("default parameter requires a type annotation"))?;
                let option_type = RustType::Option(Box::new(inner_type));
                let (default_expr, use_unwrap_or_default) =
                    crate::transformer::functions::convert_default_value(&assign.right, synthetic)?;
                let unwrap_call = if use_unwrap_or_default {
                    Expr::MethodCall {
                        object: Box::new(Expr::Ident(name.clone())),
                        method: "unwrap_or_default".to_string(),
                        args: vec![],
                    }
                } else {
                    Expr::MethodCall {
                        object: Box::new(Expr::Ident(name.clone())),
                        method: "unwrap_or".to_string(),
                        args: vec![default_expr.unwrap()],
                    }
                };
                expansion_stmts.push(Stmt::Let {
                    mutable: false,
                    name: name.clone(),
                    ty: None,
                    init: Some(unwrap_call),
                });
                params.push(Param {
                    name,
                    ty: Some(option_type),
                });
            }
            _ => return Err(anyhow!("unsupported {context} default parameter")),
        },
        ast::Pat::Rest(rest) => {
            if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                let name = ident.id.sym.to_string();
                let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                let rust_type = type_ann
                    .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                    .transpose()?;
                params.push(Param {
                    name,
                    ty: rust_type,
                });
            } else {
                return Err(anyhow!("unsupported {context} rest parameter"));
            }
        }
        _ => return Err(anyhow!("unsupported {context} parameter pattern")),
    }
    Ok(())
}

/// Converts a function expression to `Expr::Closure`.
///
/// Function expressions (`function(x) { ... }` or `function name(x) { ... }`)
/// are treated identically to arrow functions — the optional name is ignored.
pub(super) fn convert_fn_expr(
    fn_expr: &ast::FnExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let func = &fn_expr.function;

    // Convert parameters — reuse the same logic as arrow functions
    let mut params = Vec::new();
    let mut expansion_stmts = Vec::new();
    for param in &func.params {
        convert_function_param_pat(
            &param.pat,
            tctx,
            reg,
            &mut params,
            &mut expansion_stmts,
            "function expression",
            synthetic,
        )?;
    }

    let return_type = func
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
        .transpose()?;

    // void → None
    let return_type = return_type.and_then(|ty| {
        if matches!(ty, RustType::Unit) {
            None
        } else {
            Some(ty)
        }
    });

    let body = match &func.body {
        Some(block) => {
            let mut inner_env = type_env.clone();
            let mut stmts = expansion_stmts;
            for stmt in &block.stmts {
                stmts.extend(convert_stmt(
                    stmt,
                    tctx,
                    reg,
                    return_type.as_ref(),
                    &mut inner_env,
                    synthetic,
                )?);
            }
            convert_last_return_to_tail(&mut stmts);
            ClosureBody::Block(stmts)
        }
        None => ClosureBody::Block(expansion_stmts),
    };

    Ok(Expr::Closure {
        params,
        return_type,
        body,
    })
}

/// Converts an arrow expression into an IR [`Expr::Closure`].
///
/// - Expression body: `(x: number) => x + 1` → `|x: f64| x + 1`
/// - Block body: `(x: number) => { return x + 1; }` → `|x: f64| { x + 1 }`
///
/// When `resilient` is true, unsupported types fall back to [`RustType::Any`] and
/// the error message is appended to `fallback_warnings`.
/// `override_return_type` allows callers to inject a return type from an external source
/// (e.g., variable type annotation `const f: FnType = () => ...`). When provided and the
/// arrow has no explicit return type annotation, this type is used for the body conversion.
pub fn convert_arrow_expr(
    arrow: &ast::ArrowExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    convert_arrow_expr_with_return_type(
        arrow,
        tctx,
        reg,
        resilient,
        fallback_warnings,
        type_env,
        None,
        None,
        synthetic,
    )
}

/// Inner implementation of arrow expression conversion with optional type overrides.
///
/// `override_return_type` allows callers to inject a return type from an external source
/// (e.g., variable type annotation `const f: FnType = () => ...`).
/// `override_param_types` allows callers to inject parameter types from an external source
/// (e.g., variable type annotation `const f: (x: number) => void = (x) => ...`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn convert_arrow_expr_with_return_type(
    arrow: &ast::ArrowExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    type_env: &TypeEnv,
    override_return_type: Option<&RustType>,
    override_param_types: Option<&[RustType]>,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let mut params = Vec::new();
    let mut expansion_stmts = Vec::new();
    {
        for (i, param) in arrow.params.iter().enumerate() {
            match param {
                // Ident needs arrow-specific override_param_types fallback
                ast::Pat::Ident(ident) => {
                    let name = ident.id.sym.to_string();
                    let rust_type = ident
                        .type_ann
                        .as_ref()
                        .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                        .transpose()?;
                    // If no direct annotation, try override from variable type annotation
                    let rust_type = rust_type
                        .or_else(|| override_param_types.and_then(|types| types.get(i).cloned()));
                    params.push(Param {
                        name,
                        ty: rust_type,
                    });
                }
                // Array destructuring is arrow-only
                ast::Pat::Array(arr_pat) => {
                    let names: Vec<String> = arr_pat
                        .elems
                        .iter()
                        .map(|elem| match elem {
                            Some(ast::Pat::Ident(ident)) => Ok(ident.id.sym.to_string()),
                            Some(_) => {
                                Err(anyhow!("unsupported arrow array destructuring element"))
                            }
                            None => Ok("_".to_string()),
                        })
                        .collect::<Result<_>>()?;
                    let tuple_name = format!("({})", names.join(", "));
                    let rust_type = arr_pat
                        .type_ann
                        .as_ref()
                        .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                        .transpose()?;
                    params.push(Param {
                        name: tuple_name,
                        ty: rust_type,
                    });
                }
                // Object, Assign, Rest, and catch-all are shared with fn_expr
                other => convert_function_param_pat(
                    other,
                    tctx,
                    reg,
                    &mut params,
                    &mut expansion_stmts,
                    "arrow",
                    synthetic,
                )?,
            }
        }
    }

    // Arrow's explicit return type annotation takes priority;
    // fall back to override_return_type from variable type annotation
    let return_type = arrow
        .return_type
        .as_ref()
        .map(|ann| {
            convert_ts_type_with_fallback(
                &ann.type_ann,
                resilient,
                fallback_warnings,
                synthetic,
                tctx,
                reg,
            )
        })
        .transpose()?
        .or_else(|| override_return_type.cloned());

    let body = if expansion_stmts.is_empty() {
        match arrow.body.as_ref() {
            ast::BlockStmtOrExpr::Expr(expr) => {
                let ret_ctx = match return_type.as_ref() {
                    Some(ty) => ExprContext::with_expected(ty),
                    // Cat C: return type propagated when available
                    None => ExprContext::none(),
                };
                let ir_expr = convert_expr(expr, tctx, reg, &ret_ctx, type_env, synthetic)?;
                ClosureBody::Expr(Box::new(ir_expr))
            }
            ast::BlockStmtOrExpr::BlockStmt(block) => {
                let mut inner_env = type_env.clone();
                let mut stmts = Vec::new();
                for stmt in &block.stmts {
                    stmts.extend(convert_stmt(
                        stmt,
                        tctx,
                        reg,
                        return_type.as_ref(),
                        &mut inner_env,
                        synthetic,
                    )?);
                }
                convert_last_return_to_tail(&mut stmts);
                ClosureBody::Block(stmts)
            }
        }
    } else {
        // When we have expansion stmts, the body must be a Block
        let mut body_stmts = expansion_stmts;
        match arrow.body.as_ref() {
            ast::BlockStmtOrExpr::Expr(expr) => {
                let ret_ctx = match return_type.as_ref() {
                    Some(ty) => ExprContext::with_expected(ty),
                    // Cat C: return type propagated when available
                    None => ExprContext::none(),
                };
                let ir_expr = convert_expr(expr, tctx, reg, &ret_ctx, type_env, synthetic)?;
                body_stmts.push(Stmt::Return(Some(ir_expr)));
            }
            ast::BlockStmtOrExpr::BlockStmt(block) => {
                let mut inner_env = type_env.clone();
                for stmt in &block.stmts {
                    body_stmts.extend(convert_stmt(
                        stmt,
                        tctx,
                        reg,
                        return_type.as_ref(),
                        &mut inner_env,
                        synthetic,
                    )?);
                }
                convert_last_return_to_tail(&mut body_stmts);
            }
        }
        ClosureBody::Block(body_stmts)
    };

    Ok(Expr::Closure {
        params,
        return_type,
        body,
    })
}
