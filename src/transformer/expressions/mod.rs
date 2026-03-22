//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType};
use crate::transformer::context::TransformContext;
// Re-export for tests that use `super::*`
#[cfg(test)]
use crate::ir::{ClosureBody, Param};
use crate::pipeline::SyntheticTypeRegistry;
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
pub(crate) mod patterns;
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
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    ctx: &ExprContext,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
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
            let inner_result = convert_expr(
                expr,
                tctx,
                reg,
                &ExprContext::with_expected(inner),
                type_env,
                synthetic,
            )?;
            return Ok(Expr::FnCall {
                name: "Some".to_string(),
                args: vec![inner_result],
            });
        }
    }

    let result = match expr {
        ast::Expr::Ident(ident) => {
            let name = ident.sym.to_string();
            match name.as_str() {
                "undefined" => Ok(Expr::Ident("None".to_string())),
                "NaN" => Ok(Expr::Ident("f64::NAN".to_string())),
                "Infinity" => Ok(Expr::Ident("f64::INFINITY".to_string())),
                _ => Ok(Expr::Ident(name)),
            }
        }
        ast::Expr::Lit(lit) => convert_lit(lit, expected, tctx, reg),
        ast::Expr::Bin(bin) => convert_bin_expr(bin, tctx, reg, expected, type_env, synthetic),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl, tctx, reg, type_env, synthetic),
        ast::Expr::Paren(paren) => convert_expr(&paren.expr, tctx, reg, ctx, type_env, synthetic),
        ast::Expr::Member(member) => convert_member_expr(member, tctx, reg, type_env, synthetic),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign, tctx, reg, type_env, synthetic),
        ast::Expr::Update(up) => convert_update_expr(up),
        ast::Expr::Arrow(arrow) => {
            convert_arrow_expr(arrow, tctx, reg, false, &mut Vec::new(), type_env, synthetic)
        }
        ast::Expr::Fn(fn_expr) => convert_fn_expr(fn_expr, tctx, reg, type_env, synthetic),
        ast::Expr::Call(call) => convert_call_expr(call, tctx, reg, type_env, synthetic),
        ast::Expr::New(new_expr) => convert_new_expr(new_expr, tctx, reg, type_env, synthetic),
        ast::Expr::Array(array_lit) => {
            convert_array_lit(array_lit, tctx, reg, expected, type_env, synthetic)
        }
        ast::Expr::Object(obj_lit) => {
            convert_object_lit(obj_lit, tctx, reg, expected, type_env, synthetic)
        }
        ast::Expr::Cond(cond) => convert_cond_expr(cond, tctx, reg, expected, type_env, synthetic),
        ast::Expr::Unary(unary) => convert_unary_expr(unary, tctx, reg, type_env, synthetic),
        ast::Expr::TsAs(ts_as) => convert_ts_as_expr(ts_as, tctx, reg, expected, type_env, synthetic),
        ast::Expr::OptChain(opt_chain) => {
            convert_opt_chain_expr(opt_chain, tctx, reg, type_env, synthetic)
        }
        ast::Expr::Await(await_expr) => {
            // Cat A: await inner type depends on Promise resolution
            let inner = convert_expr(
                &await_expr.arg,
                tctx,
                reg,
                &ExprContext::none(),
                type_env,
                synthetic,
            )?;
            Ok(Expr::Await(Box::new(inner)))
        }
        // Non-null assertion (expr!) — TS type-level only, no runtime effect. Strip assertion.
        ast::Expr::TsNonNull(ts_non_null) => {
            convert_expr(&ts_non_null.expr, tctx, reg, ctx, type_env, synthetic)
        }
        _ => Err(anyhow!("unsupported expression: {:?}", expr)),
    }?;

    // Trait type coercion: when expected type is Box<dyn Trait> and the expression
    // produces a concrete (non-Box) value, wrap it in Box::new().
    if let Some(expected) = expected {
        if needs_trait_box_coercion(expected, expr, type_env, tctx, reg) {
            return Ok(Expr::FnCall {
                name: "Box::new".to_string(),
                args: vec![result],
            });
        }
    }

    Ok(result)
}

/// Returns true when the expected type is `Box<dyn Trait>` and the source expression
/// produces a concrete (non-Box) value that needs wrapping.
fn needs_trait_box_coercion(
    expected: &RustType,
    src_expr: &ast::Expr,
    type_env: &TypeEnv,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> bool {
    // Check if expected type is Box<dyn Trait>
    let trait_name = match expected {
        RustType::Named { name, type_args }
            if name == "Box"
                && type_args.len() == 1
                && matches!(&type_args[0], RustType::DynTrait(_)) =>
        {
            if let RustType::DynTrait(t) = &type_args[0] {
                t.as_str()
            } else {
                return false;
            }
        }
        _ => return false,
    };

    // Resolve the expression's actual type. If unknown or Any, skip coercion (safe default).
    let Some(expr_type) = type_resolution::resolve_expr_type(src_expr, type_env, tctx, reg) else {
        return false;
    };
    if matches!(expr_type, RustType::Any) {
        return false;
    }

    // If the expression already produces Box<dyn Trait>, no wrapping needed
    if matches!(
        &expr_type,
        RustType::Named { name, type_args }
            if name == "Box" && type_args.first().is_some_and(|a| matches!(a, RustType::DynTrait(t) if t == trait_name))
    ) {
        return false;
    }

    // If the expression's type is the trait type itself (e.g., a function returning `Greeter`
    // which will be transformed to `Box<dyn Greeter>`), no wrapping needed — the generated
    // code already returns Box<dyn Trait>.
    if let RustType::Named {
        name: expr_name,
        type_args: expr_args,
    } = &expr_type
    {
        if expr_args.is_empty() && expr_name == trait_name && reg.is_trait_type(expr_name) {
            return false;
        }
    }

    true
}

/// Converts a template literal to `Expr::FormatMacro`.
///
/// `` `Hello ${name}` `` becomes `format!("Hello {}", name)`.
fn convert_template_literal(
    tpl: &ast::Tpl,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let mut template = String::new();
    let mut args = Vec::new();

    for (i, quasi) in tpl.quasis.iter().enumerate() {
        // raw text of the quasi (the string parts between expressions)
        template.push_str(&quasi.raw);
        if i < tpl.exprs.len() {
            template.push_str("{}");
            // Cat A: template interpolation — format!() accepts any Display type
            let arg = convert_expr(
                &tpl.exprs[i],
                tctx,
                reg,
                &ExprContext::none(),
                type_env,
                synthetic,
            )?;
            args.push(arg);
        }
    }

    Ok(Expr::FormatMacro { template, args })
}

/// Converts an SWC conditional (ternary) expression to `Expr::If` or `Expr::IfLet`.
///
/// `condition ? consequent : alternate` → `if condition { consequent } else { alternate }`
///
/// When the condition is a narrowing guard (typeof/instanceof/null-check/truthy),
/// generates `Expr::IfLet` with the narrowed variable available in the then branch.
fn convert_cond_expr(
    cond: &ast::CondExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let ctx = &ExprContext { expected };

    // Try narrowing: extract guard from the ternary condition
    if let Some(guard) = patterns::extract_narrowing_guard(&cond.test) {
        if let Some((pattern, is_swap)) = guard.if_let_pattern(type_env, tctx, reg) {
            // Determine which TS branch corresponds to the if-let match (pattern matched).
            // For === guards: consequent is the matched branch.
            // For !== guards (is_swap): alternate is the matched branch.
            let (matched_ast, unmatched_ast) = if is_swap {
                (cond.alt.as_ref(), cond.cons.as_ref())
            } else {
                (cond.cons.as_ref(), cond.alt.as_ref())
            };

            // Convert the matched branch with narrowed type environment.
            // The narrowing corresponds to what the if-let pattern extracts
            // (e.g., String from StringOrF64::String(x), T from Some(x)).
            let mut narrowed_env = type_env.clone();
            narrowed_env.push_scope();
            if let Some(original) = type_env.get(guard.var_name()).cloned() {
                let narrowed = if is_swap {
                    guard.narrowed_type_for_else(&original)
                } else {
                    guard.narrowed_type_for_then(&original)
                };
                if let Some(narrowed) = narrowed {
                    narrowed_env.insert(guard.var_name().to_string(), narrowed);
                }
            }
            let matched_expr = convert_expr(matched_ast, tctx, reg, ctx, &narrowed_env, synthetic)?;

            // Convert the unmatched branch with original type environment
            let unmatched_expr = convert_expr(unmatched_ast, tctx, reg, ctx, type_env, synthetic)?;

            let expr_ir = Expr::Ident(guard.var_name().to_string());
            return Ok(Expr::IfLet {
                pattern,
                expr: Box::new(expr_ir),
                then_expr: Box::new(matched_expr),
                else_expr: Box::new(unmatched_expr),
            });
        }
    }

    // Fallback: regular if expression
    let condition = convert_expr(&cond.test, tctx, reg, &ExprContext::none(), type_env, synthetic)?;
    let then_expr = convert_expr(&cond.cons, tctx, reg, ctx, type_env, synthetic)?;
    let else_expr = convert_expr(&cond.alt, tctx, reg, ctx, type_env, synthetic)?;
    Ok(Expr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    })
}

#[cfg(test)]
mod tests;
