//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType};
// Re-export for tests that use `super::*`
#[cfg(test)]
use crate::ir::{ClosureBody, Param};
use crate::registry::TypeRegistry;
use crate::transformer::TypeEnv;

mod assignments;
mod binary;
mod calls;
mod data_literals;
mod functions;
mod literals;
mod member_access;
mod methods;
mod patterns;
mod type_resolution;
use assignments::{convert_assign_expr, convert_update_expr};
pub(crate) use binary::convert_binary_op;
use binary::{convert_bin_expr, convert_unary_expr};
use calls::{convert_call_expr, convert_new_expr};
use data_literals::{convert_array_lit, convert_object_lit};
pub use functions::convert_arrow_expr;
pub(crate) use functions::convert_arrow_expr_with_return_type;
use functions::convert_fn_expr;
use literals::convert_lit;
use member_access::{convert_member_expr, convert_opt_chain_expr};
use type_resolution::convert_ts_as_expr;
pub use type_resolution::resolve_expr_type;

/// Expression conversion context. Holds a type hint from the enclosing scope.
#[derive(Debug, Clone)]
pub struct ExprContext<'a> {
    /// Expected type from outer context (variable annotation, function parameter, etc.)
    pub expected: Option<&'a RustType>,
}

impl<'a> ExprContext<'a> {
    /// No type information available.
    pub fn none() -> Self {
        Self { expected: None }
    }

    /// With an expected type.
    pub fn with_expected(expected: &'a RustType) -> Self {
        Self {
            expected: Some(expected),
        }
    }
}

/// Converts an SWC [`ast::Expr`] into an IR [`Expr`], with an optional expected type.
///
/// The `expected` type is used for:
/// - Object literals: determines the struct name from `RustType::Named`
/// - String literals: adds `.to_string()` when `RustType::String` is expected
/// - Array literals: propagates element type from `RustType::Vec`
///
/// # Errors
///
/// Returns an error for unsupported expression types.
pub fn convert_expr(
    expr: &ast::Expr,
    reg: &TypeRegistry,
    ctx: &ExprContext,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let expected = ctx.expected;
    // Option<T> expected: handle null/undefined → None, literals → Some(lit)
    if let Some(RustType::Option(inner)) = expected {
        // null / undefined → None
        if matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
            || matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
        {
            return Ok(Expr::Ident("None".to_string()));
        }
        // Wrap non-null literals in Some() (needed for array elements like vec![Some(1.0), None])
        if matches!(expr, ast::Expr::Lit(_)) {
            let inner_result =
                convert_expr(expr, reg, &ExprContext::with_expected(inner), type_env)?;
            return Ok(Expr::FnCall {
                name: "Some".to_string(),
                args: vec![inner_result],
            });
        }
    }

    match expr {
        ast::Expr::Ident(ident) => {
            let name = ident.sym.to_string();
            match name.as_str() {
                "undefined" => Ok(Expr::Ident("None".to_string())),
                "NaN" => Ok(Expr::Ident("f64::NAN".to_string())),
                "Infinity" => Ok(Expr::Ident("f64::INFINITY".to_string())),
                _ => Ok(Expr::Ident(name)),
            }
        }
        ast::Expr::Lit(lit) => convert_lit(lit, expected, reg),
        ast::Expr::Bin(bin) => convert_bin_expr(bin, reg, expected, type_env),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl, reg, type_env),
        ast::Expr::Paren(paren) => convert_expr(&paren.expr, reg, ctx, type_env),
        ast::Expr::Member(member) => convert_member_expr(member, reg, type_env),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign, reg, type_env),
        ast::Expr::Update(up) => convert_update_expr(up),
        ast::Expr::Arrow(arrow) => convert_arrow_expr(arrow, reg, false, &mut Vec::new(), type_env),
        ast::Expr::Fn(fn_expr) => convert_fn_expr(fn_expr, reg, type_env),
        ast::Expr::Call(call) => convert_call_expr(call, reg, type_env),
        ast::Expr::New(new_expr) => convert_new_expr(new_expr, reg, type_env),
        ast::Expr::Array(array_lit) => convert_array_lit(array_lit, reg, expected, type_env),
        ast::Expr::Object(obj_lit) => convert_object_lit(obj_lit, reg, expected, type_env),
        ast::Expr::Cond(cond) => convert_cond_expr(cond, reg, expected, type_env),
        ast::Expr::Unary(unary) => convert_unary_expr(unary, reg, type_env),
        ast::Expr::TsAs(ts_as) => convert_ts_as_expr(ts_as, reg, expected, type_env),
        ast::Expr::OptChain(opt_chain) => convert_opt_chain_expr(opt_chain, reg, type_env),
        ast::Expr::Await(await_expr) => {
            // Cat A: await inner type depends on Promise resolution
            let inner = convert_expr(&await_expr.arg, reg, &ExprContext::none(), type_env)?;
            Ok(Expr::Await(Box::new(inner)))
        }
        // Non-null assertion (expr!) — TS type-level only, no runtime effect. Strip assertion.
        ast::Expr::TsNonNull(ts_non_null) => convert_expr(&ts_non_null.expr, reg, ctx, type_env),
        _ => Err(anyhow!("unsupported expression: {:?}", expr)),
    }
}

/// Converts a template literal to `Expr::FormatMacro`.
///
/// `` `Hello ${name}` `` becomes `format!("Hello {}", name)`.
fn convert_template_literal(
    tpl: &ast::Tpl,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let mut template = String::new();
    let mut args = Vec::new();

    for (i, quasi) in tpl.quasis.iter().enumerate() {
        // raw text of the quasi (the string parts between expressions)
        template.push_str(&quasi.raw);
        if i < tpl.exprs.len() {
            template.push_str("{}");
            // Cat A: template interpolation — format!() accepts any Display type
            let arg = convert_expr(&tpl.exprs[i], reg, &ExprContext::none(), type_env)?;
            args.push(arg);
        }
    }

    Ok(Expr::FormatMacro { template, args })
}

/// Converts an SWC conditional (ternary) expression to `Expr::If`.
///
/// `condition ? consequent : alternate` → `if condition { consequent } else { alternate }`
fn convert_cond_expr(
    cond: &ast::CondExpr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let ctx = &ExprContext { expected };
    // Cat A: ternary condition is always boolean
    let condition = convert_expr(&cond.test, reg, &ExprContext::none(), type_env)?;
    let then_expr = convert_expr(&cond.cons, reg, ctx, type_env)?;
    let else_expr = convert_expr(&cond.alt, reg, ctx, type_env)?;
    Ok(Expr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    })
}

#[cfg(test)]
mod tests;
