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
use crate::registry::TypeRegistry;
use crate::transformer::context::TransformContext;
// Re-export for tests that use `super::*`
#[cfg(test)]
use crate::ir::{ClosureBody, Param};
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
/// Converts an SWC [`ast::Expr`] into an IR [`Expr`].
///
/// The expected type is read from `FileTypeResolution.expected_type()`.
/// For Option<T> unwrapping, an internal override mechanism is used to
/// avoid infinite recursion (same span would return Option<T> again).
pub fn convert_expr(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    convert_expr_with_expected(expr, tctx, reg, None, type_env, synthetic)
}

/// Converts an expression with an explicit expected type override.
///
/// Private helper: only used by `convert_expr` for Option<T> unwrap recursion
/// (to avoid infinite loop from reading the same Option<T> span).
fn convert_expr_with_expected(
    expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    expected_override: Option<&RustType>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let expected = expected_override.or_else(|| {
        tctx.type_resolution
            .expected_type(Span::from_swc(expr.span()))
    });
    // Option<T> expected: unified wrapping logic.
    // All Option wrapping decisions are made here to prevent double-wrapping (Some(Some(x))).
    if let Some(RustType::Option(inner)) = expected {
        // null / undefined → None
        if matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
            || matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
        {
            return Ok(Expr::Ident("None".to_string()));
        }
        // Literals → Some(convert(expr, inner_T))
        if matches!(expr, ast::Expr::Lit(_)) {
            let inner_result =
                convert_expr_with_expected(expr, tctx, reg, Some(inner), type_env, synthetic)?;
            return Ok(Expr::FnCall {
                name: "Some".to_string(),
                args: vec![inner_result],
            });
        }
        // Non-literal expressions: check if the expression already produces Option<T>.
        // If so, fall through to normal conversion (tctx has expected types for sub-exprs).
        let expr_type = type_resolution::get_expr_type(tctx, expr);
        if matches!(expr_type, Some(RustType::Option(_))) || ast_produces_option(expr) {
            // Already produces Option — skip wrapping, fall through to normal conversion
        } else {
            // Expression produces T or unknown → convert with inner type, then wrap in Some()
            let inner_result =
                convert_expr_with_expected(expr, tctx, reg, Some(inner), type_env, synthetic)?;
            // Guard: skip wrapping if conversion already produced None or Some(...)
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
        ast::Expr::Lit(lit) => convert_lit(lit, expected, tctx, reg),
        ast::Expr::Bin(bin) => convert_bin_expr(bin, tctx, reg, expected, type_env, synthetic),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl, tctx, reg, type_env, synthetic),
        ast::Expr::Paren(paren) => convert_expr(&paren.expr, tctx, reg, type_env, synthetic),
        ast::Expr::Member(member) => convert_member_expr(member, tctx, reg, type_env, synthetic),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign, tctx, reg, type_env, synthetic),
        ast::Expr::Update(up) => convert_update_expr(up),
        ast::Expr::Arrow(arrow) => convert_arrow_expr(
            arrow,
            tctx,
            reg,
            false,
            &mut Vec::new(),
            type_env,
            synthetic,
        ),
        ast::Expr::Fn(fn_expr) => convert_fn_expr(fn_expr, tctx, reg, type_env, synthetic),
        ast::Expr::Call(call) => convert_call_expr(call, tctx, reg, type_env, synthetic),
        ast::Expr::New(new_expr) => convert_new_expr(new_expr, tctx, reg, type_env, synthetic),
        ast::Expr::Array(array_lit) => {
            convert_array_lit(array_lit, tctx, reg, expected, type_env, synthetic)
        }
        ast::Expr::Object(obj_lit) => {
            convert_object_lit(obj_lit, tctx, reg, expected, type_env, synthetic)
        }
        ast::Expr::Cond(cond) => convert_cond_expr(cond, tctx, reg, type_env, synthetic),
        ast::Expr::Unary(unary) => convert_unary_expr(unary, tctx, reg, type_env, synthetic),
        ast::Expr::TsAs(ts_as) => {
            match convert_ts_type(&ts_as.type_ann, synthetic, reg) {
                Ok(target_ty)
                    if matches!(target_ty, RustType::F64 | RustType::Bool) =>
                {
                    let inner = convert_expr(&ts_as.expr, tctx, reg, type_env, synthetic)?;
                    Ok(Expr::Cast {
                        expr: Box::new(inner),
                        target: target_ty,
                    })
                }
                _ => convert_expr(&ts_as.expr, tctx, reg, type_env, synthetic),
            }
        }
        ast::Expr::OptChain(opt_chain) => {
            convert_opt_chain_expr(opt_chain, tctx, reg, type_env, synthetic)
        }
        ast::Expr::Await(await_expr) => {
            // Cat A: await inner type depends on Promise resolution
            let inner = convert_expr(&await_expr.arg, tctx, reg, type_env, synthetic)?;
            Ok(Expr::Await(Box::new(inner)))
        }
        // Non-null assertion (expr!) — TS type-level only, no runtime effect. Strip assertion.
        ast::Expr::TsNonNull(ts_non_null) => {
            convert_expr(&ts_non_null.expr, tctx, reg, type_env, synthetic)
        }
        _ => Err(anyhow!("unsupported expression: {:?}", expr)),
    }?;

    // Trait type coercion: when expected type is Box<dyn Trait> and the expression
    // produces a concrete (non-Box) value, wrap it in Box::new().
    if let Some(expected) = expected {
        if needs_trait_box_coercion(expected, expr, tctx, reg) {
            return Ok(Expr::FnCall {
                name: "Box::new".to_string(),
                args: vec![result],
            });
        }
    }

    Ok(result)
}

/// Returns true when the expected type is a trait type (`Box<dyn Trait>`)
/// and the source expression produces a concrete (non-Box) value that needs wrapping.
///
/// TypeResolver applies `wrap_trait_for_position` so expected types are normally
/// `Box<dyn Trait>`, but `Named(trait)` is also accepted for robustness.
fn needs_trait_box_coercion(
    expected: &RustType,
    src_expr: &ast::Expr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> bool {
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
        // TypeResolver sets Named(trait) as expected; treat it the same as Box<dyn Trait>
        RustType::Named { name, .. } if reg.is_trait_type(name) => name.as_str(),
        _ => return false,
    };

    // Resolve the expression's actual type. If unknown or Any, skip coercion (safe default).
    let Some(expr_type) = type_resolution::get_expr_type(tctx, src_expr) else {
        return false;
    };
    if matches!(expr_type, RustType::Any) {
        return false;
    }

    // If the expression already produces Box<dyn Trait>, no wrapping needed
    if matches!(
        expr_type,
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
            let arg = convert_expr(&tpl.exprs[i], tctx, reg, type_env, synthetic)?;
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
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
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
            let matched_expr = convert_expr(matched_ast, tctx, reg, &narrowed_env, synthetic)?;

            // Convert the unmatched branch with original type environment
            let unmatched_expr = convert_expr(unmatched_ast, tctx, reg, type_env, synthetic)?;

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
    let condition = convert_expr(&cond.test, tctx, reg, type_env, synthetic)?;
    let then_expr = convert_expr(&cond.cons, tctx, reg, type_env, synthetic)?;
    let else_expr = convert_expr(&cond.alt, tctx, reg, type_env, synthetic)?;
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
/// This is a temporary workaround: `get_expr_type` does not return `Option<T>`
/// for Cond/OptChain expressions. Phase 3 task 3-7 will replace this with
/// `tctx.type_resolution.expr_type()` after TypeResolver is enhanced to set
/// correct expr_types for these patterns.
fn ast_produces_option(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::OptChain(_) => true,
        ast::Expr::Paren(p) => ast_produces_option(&p.expr),
        ast::Expr::Cond(cond) => {
            // A ternary produces Option if either branch is null/undefined,
            // OR if either branch itself produces Option (recursive check for nested ternaries).
            // e.g., `x ? (y ? "a" : null) : "b"` — the cons branch produces Option,
            // so TypeResolver will wrap the alt in Some(), making the whole expression Option.
            is_null_or_undefined(&cond.cons)
                || is_null_or_undefined(&cond.alt)
                || ast_produces_option(&cond.cons)
                || ast_produces_option(&cond.alt)
        }
        _ => false,
    }
}

/// Returns true if the expression is a `null` literal or `undefined` identifier.
fn is_null_or_undefined(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
        || matches!(expr, ast::Expr::Ident(id) if id.sym.as_ref() == "undefined")
}

#[cfg(test)]
mod tests;
