//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType};
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
use crate::pipeline::SyntheticTypeRegistry;
use crate::transformer::context::TransformContext;
// Re-export for tests that use `super::*`
#[cfg(test)]
use crate::ir::{ClosureBody, Param};
use crate::transformer::Transformer;
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
pub(crate) use type_resolution::get_expr_type;

impl<'a> Transformer<'a> {
    /// Converts an SWC [`ast::Expr`] into an IR [`Expr`].
    ///
    /// The expected type is read from `FileTypeResolution.expected_type()`.
    pub(crate) fn convert_expr(&mut self, expr: &ast::Expr) -> Result<Expr> {
        self.convert_expr_with_expected(expr, None)
    }

    /// Converts an expression with an explicit expected type override.
    ///
    /// Private helper: only used by `convert_expr` for Option<T> unwrap recursion
    /// (to avoid infinite loop from reading the same Option<T> span).
    fn convert_expr_with_expected(
        &mut self,
        expr: &ast::Expr,
        expected_override: Option<&RustType>,
    ) -> Result<Expr> {
        let reg = self.reg();
        let expected = expected_override.or_else(|| {
            self.tctx
                .type_resolution
                .expected_type(Span::from_swc(expr.span()))
        });
        // Option<T> expected: unified wrapping logic.
        if let Some(RustType::Option(inner)) = expected {
            // null / undefined → None
            if matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
                || matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
            {
                return Ok(Expr::Ident("None".to_string()));
            }
            // Literals → Some(convert(expr, inner_T))
            if matches!(expr, ast::Expr::Lit(_)) {
                let inner_result = convert_expr_with_expected(
                    expr,
                    self.tctx,
                    Some(inner),
                    &self.type_env,
                    self.synthetic,
                )?;
                return Ok(Expr::FnCall {
                    name: "Some".to_string(),
                    args: vec![inner_result],
                });
            }
            let expr_type = type_resolution::get_expr_type(self.tctx, expr);
            if matches!(expr_type, Some(RustType::Option(_))) {
                // Already produces Option — skip wrapping, fall through
            } else {
                let inner_result = convert_expr_with_expected(
                    expr,
                    self.tctx,
                    Some(inner),
                    &self.type_env,
                    self.synthetic,
                )?;
                if matches!(&inner_result, Expr::Ident(name) if name == "None")
                    || matches!(&inner_result, Expr::FnCall { name, .. } if name == "Some")
                {
                    return Ok(inner_result);
                }
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
            ast::Expr::Lit(lit) => convert_lit(lit, expected, self.tctx),
            ast::Expr::Bin(bin) => {
                convert_bin_expr(bin, self.tctx, expected, &self.type_env, self.synthetic)
            }
            ast::Expr::Tpl(tpl) => {
                convert_template_literal(tpl, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::Paren(paren) => {
                convert_expr(&paren.expr, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::Member(member) => {
                convert_member_expr(member, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
            ast::Expr::Assign(assign) => {
                convert_assign_expr(assign, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::Update(up) => convert_update_expr(up),
            ast::Expr::Arrow(arrow) => convert_arrow_expr(
                arrow,
                self.tctx,
                false,
                &mut Vec::new(),
                &self.type_env,
                self.synthetic,
            ),
            ast::Expr::Fn(fn_expr) => {
                convert_fn_expr(fn_expr, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::Call(call) => {
                convert_call_expr(call, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::New(new_expr) => {
                convert_new_expr(new_expr, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::Array(array_lit) => {
                convert_array_lit(array_lit, self.tctx, expected, &self.type_env, self.synthetic)
            }
            ast::Expr::Object(obj_lit) => {
                convert_object_lit(obj_lit, self.tctx, expected, &self.type_env, self.synthetic)
            }
            ast::Expr::Cond(cond) => {
                convert_cond_expr(cond, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::Unary(unary) => {
                convert_unary_expr(unary, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::TsAs(ts_as) => match convert_ts_type(&ts_as.type_ann, self.synthetic, reg) {
                Ok(target_ty) if matches!(target_ty, RustType::F64 | RustType::Bool) => {
                    let inner =
                        convert_expr(&ts_as.expr, self.tctx, &self.type_env, self.synthetic)?;
                    Ok(Expr::Cast {
                        expr: Box::new(inner),
                        target: target_ty,
                    })
                }
                _ => convert_expr(&ts_as.expr, self.tctx, &self.type_env, self.synthetic),
            },
            ast::Expr::OptChain(opt_chain) => {
                convert_opt_chain_expr(opt_chain, self.tctx, &self.type_env, self.synthetic)
            }
            ast::Expr::Await(await_expr) => {
                let inner =
                    convert_expr(&await_expr.arg, self.tctx, &self.type_env, self.synthetic)?;
                Ok(Expr::Await(Box::new(inner)))
            }
            ast::Expr::TsNonNull(ts_non_null) => {
                convert_expr(&ts_non_null.expr, self.tctx, &self.type_env, self.synthetic)
            }
            _ => Err(anyhow!("unsupported expression: {:?}", expr)),
        }?;

        // Trait type coercion
        if let Some(expected) = expected {
            if needs_trait_box_coercion(expected, expr, self.tctx) {
                return Ok(Expr::FnCall {
                    name: "Box::new".to_string(),
                    args: vec![result],
                });
            }
        }

        Ok(result)
    }
}

/// Wrapper: delegates to [`Transformer::convert_expr`].
pub fn convert_expr(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    convert_expr_with_expected(expr, tctx, None, type_env, synthetic)
}

/// Wrapper: delegates to Transformer's convert_expr_with_expected.
fn convert_expr_with_expected(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    expected_override: Option<&RustType>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let env = type_env.clone();
    Transformer {
        tctx,
        type_env: env,
        synthetic,
    }
    .convert_expr_with_expected(expr, expected_override)
}

/// Returns true when the expected type is a trait type (`Box<dyn Trait>`)
/// and the source expression produces a concrete (non-Box) value that needs wrapping.
fn needs_trait_box_coercion(
    expected: &RustType,
    src_expr: &ast::Expr,
    tctx: &TransformContext<'_>,
) -> bool {
    let reg = tctx.type_registry;
    // Check if expected type is Box<dyn Trait> or a Named trait type
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
        RustType::Named { name, .. } if reg.is_trait_type(name) => name.as_str(),
        _ => return false,
    };

    let Some(expr_type) = type_resolution::get_expr_type(tctx, src_expr) else {
        return false;
    };
    if matches!(expr_type, RustType::Any) {
        return false;
    }

    if matches!(
        expr_type,
        RustType::Named { name, type_args }
            if name == "Box" && type_args.first().is_some_and(|a| matches!(a, RustType::DynTrait(t) if t == trait_name))
    ) {
        return false;
    }

    if let RustType::Named {
        name: expr_name,
        type_args: expr_args,
    } = expr_type
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
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let mut template = String::new();
    let mut args = Vec::new();

    for (i, quasi) in tpl.quasis.iter().enumerate() {
        template.push_str(&quasi.raw);
        if i < tpl.exprs.len() {
            template.push_str("{}");
            let arg = convert_expr(&tpl.exprs[i], tctx, type_env, synthetic)?;
            args.push(arg);
        }
    }

    Ok(Expr::FormatMacro { template, args })
}

/// Converts an SWC conditional (ternary) expression to `Expr::If` or `Expr::IfLet`.
fn convert_cond_expr(
    cond: &ast::CondExpr,
    tctx: &TransformContext<'_>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    // Try narrowing: extract guard from the ternary condition
    if let Some(guard) = patterns::extract_narrowing_guard(&cond.test) {
        if let Some((pattern, is_swap)) = guard.if_let_pattern(type_env, tctx) {
            let (matched_ast, unmatched_ast) = if is_swap {
                (cond.alt.as_ref(), cond.cons.as_ref())
            } else {
                (cond.cons.as_ref(), cond.alt.as_ref())
            };

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
            let matched_expr = convert_expr(matched_ast, tctx, &narrowed_env, synthetic)?;
            let unmatched_expr = convert_expr(unmatched_ast, tctx, type_env, synthetic)?;

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
    let condition = convert_expr(&cond.test, tctx, type_env, synthetic)?;
    let then_expr = convert_expr(&cond.cons, tctx, type_env, synthetic)?;
    let else_expr = convert_expr(&cond.alt, tctx, type_env, synthetic)?;
    Ok(Expr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    })
}

/// Returns true if the AST expression inherently produces `Option` when
/// expected type is `Option<T>` — based on AST structure, not type resolution.
///
/// Detects:
/// - Optional chaining (`?.`) — always produces Option
/// - Ternary with null/undefined branch (`x ? y : null`) — produces Option
///
#[cfg(test)]
mod tests;
