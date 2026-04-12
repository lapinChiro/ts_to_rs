//! Return value wrapping for divergent callable interface return types.
//!
//! When a callable interface has overloads with different return types,
//! the inner function returns a synthetic union enum. Each return expression
//! in the arrow body must be wrapped in the appropriate enum variant.
//!
//! Phase 7 で delegate impl から使用予定。現時点では test からのみ呼ばれる。
#![allow(dead_code)]

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType};
use crate::pipeline::synthetic_registry::variant_name_for_type;
use crate::registry::MethodSignature;

/// Context for wrapping return expressions in a synthetic union enum variant.
#[derive(Debug, Clone)]
pub(crate) struct ReturnWrapContext {
    /// Name of the synthetic union enum (e.g., `"CookieOrOptionString"`)
    pub enum_name: String,
    /// Mapping from return type to variant name.
    pub variant_by_type: Vec<(RustType, String)>,
}

/// Builds a `ReturnWrapContext` from the call signatures of a callable interface.
///
/// Returns `None` if all overloads have the same return type (no wrapping needed).
pub(crate) fn build_return_wrap_context(
    call_sigs: &[MethodSignature],
    enum_name: &str,
) -> Option<ReturnWrapContext> {
    // Collect unique return types
    let return_types: Vec<RustType> = call_sigs
        .iter()
        .filter_map(|s| s.return_type.clone())
        .map(|ty| ty.unwrap_promise())
        .collect();

    let mut unique = Vec::new();
    for ty in &return_types {
        if !unique.contains(ty) {
            unique.push(ty.clone());
        }
    }

    // No divergence → no wrap needed
    if unique.len() <= 1 {
        return None;
    }

    let variant_by_type: Vec<(RustType, String)> = unique
        .iter()
        .map(|ty| (ty.clone(), variant_name_for_type(ty)))
        .collect();

    Some(ReturnWrapContext {
        enum_name: enum_name.to_string(),
        variant_by_type,
    })
}

impl ReturnWrapContext {
    /// Finds the variant name for the given return type.
    ///
    /// Tries exact match first, then Option<T> narrowing (T matches Option<T> variant).
    fn variant_for(&self, ty: &RustType) -> Option<&str> {
        // Exact match
        if let Some((_, name)) = self.variant_by_type.iter().find(|(t, _)| t == ty) {
            return Some(name);
        }

        // Option narrowing: T can match Option<T>
        for (vty, name) in &self.variant_by_type {
            if let RustType::Option(inner) = vty {
                if inner.as_ref() == ty {
                    return Some(name);
                }
            }
        }

        None
    }

    /// Finds the unique Option<_> variant for polymorphic None wrapping.
    ///
    /// Returns `Some(variant_name)` if exactly one variant is `Option<_>`.
    /// Returns `None` if zero or multiple Option variants exist.
    fn unique_option_variant(&self) -> Option<&str> {
        let options: Vec<&str> = self
            .variant_by_type
            .iter()
            .filter(|(ty, _)| matches!(ty, RustType::Option(_)))
            .map(|(_, name)| name.as_str())
            .collect();
        if options.len() == 1 {
            Some(options[0])
        } else {
            None
        }
    }
}

/// Wraps a leaf return expression in the appropriate union variant.
///
/// `ast_arg` is the original SWC AST expression for error span reporting.
pub(crate) fn wrap_leaf(
    ir_expr: Expr,
    ast_arg: &ast::Expr,
    ctx: &ReturnWrapContext,
) -> Result<Expr> {
    // Polymorphic None: `return null/undefined/None`
    if is_none_expr(&ir_expr) {
        return match ctx.unique_option_variant() {
            Some(variant) => Ok(Expr::FnCall {
                target: crate::ir::CallTarget::Free(format!("{}::{}", ctx.enum_name, variant)),
                args: vec![Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)],
            }),
            None => Err(anyhow!(
                "ambiguous polymorphic None at byte {}..{}: multiple Option<_> variants in {}",
                ast_arg.span().lo.0,
                ast_arg.span().hi.0,
                ctx.enum_name,
            )),
        };
    }

    // Try to infer the variant from the expression's type
    // For now, use a simple heuristic: if there's only one non-Option variant, use it
    // More sophisticated type inference would require TypeResolver integration
    if ctx.variant_by_type.len() == 2 {
        // Binary case: try each variant
        // If the expr is a string literal, it likely matches the String variant
        if let Some(variant) = infer_variant_from_expr(&ir_expr, ctx) {
            return Ok(wrap_in_variant(&ir_expr, &ctx.enum_name, variant));
        }
    }

    // Fallback: if only one non-Option variant, use it
    let non_option_variants: Vec<&str> = ctx
        .variant_by_type
        .iter()
        .filter(|(ty, _)| !matches!(ty, RustType::Option(_)))
        .map(|(_, name)| name.as_str())
        .collect();

    if non_option_variants.len() == 1 {
        return Ok(wrap_in_variant(
            &ir_expr,
            &ctx.enum_name,
            non_option_variants[0],
        ));
    }

    // Cannot determine variant — hard error (INV-3)
    Err(anyhow!(
        "cannot determine return variant at byte {}..{} for union {}",
        ast_arg.span().lo.0,
        ast_arg.span().hi.0,
        ctx.enum_name,
    ))
}

/// Wraps an expression in a union variant constructor.
fn wrap_in_variant(expr: &Expr, enum_name: &str, variant: &str) -> Expr {
    Expr::FnCall {
        target: crate::ir::CallTarget::Free(format!("{enum_name}::{variant}")),
        args: vec![expr.clone()],
    }
}

/// Returns true if the expression represents None/null/undefined.
fn is_none_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)
    )
}

/// Tries to infer the correct variant from the expression shape.
fn infer_variant_from_expr<'a>(expr: &Expr, ctx: &'a ReturnWrapContext) -> Option<&'a str> {
    match expr {
        // String literal → String variant
        Expr::StringLit(_) => ctx.variant_for(&RustType::String),
        // Number literal → F64 variant
        Expr::NumberLit(_) | Expr::IntLit(_) => ctx.variant_for(&RustType::F64),
        // Bool literal → Bool variant
        Expr::BoolLit(_) => ctx.variant_for(&RustType::Bool),
        // Some(...) → find Option variant
        Expr::FnCall {
            target: crate::ir::CallTarget::Free(name),
            ..
        } if name == "Some" => ctx.unique_option_variant(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::BuiltinVariant;
    use crate::registry::ParamDef;

    fn make_sig(return_type: Option<RustType>) -> MethodSignature {
        MethodSignature {
            params: vec![ParamDef {
                name: "x".to_string(),
                ty: RustType::String,
                optional: false,
                has_default: false,
            }],
            return_type,
            ..Default::default()
        }
    }

    // --- build_return_wrap_context ---

    #[test]
    fn build_context_returns_none_for_identical_returns() {
        let sigs = vec![
            make_sig(Some(RustType::String)),
            make_sig(Some(RustType::String)),
        ];
        assert!(build_return_wrap_context(&sigs, "Unused").is_none());
    }

    #[test]
    fn build_context_collects_unique_variants() {
        let cookie = RustType::Named {
            name: "Cookie".to_string(),
            type_args: vec![],
        };
        let sigs = vec![
            make_sig(Some(cookie.clone())),
            make_sig(Some(RustType::Option(Box::new(RustType::String)))),
        ];
        let ctx = build_return_wrap_context(&sigs, "CookieOrOptionString").unwrap();
        assert_eq!(ctx.enum_name, "CookieOrOptionString");
        assert_eq!(ctx.variant_by_type.len(), 2);
        assert_eq!(ctx.variant_by_type[0].1, "Cookie");
        assert_eq!(ctx.variant_by_type[1].1, "OptionString");
    }

    #[test]
    fn build_context_dedupes_identical_returns() {
        let sigs = vec![
            make_sig(Some(RustType::String)),
            make_sig(Some(RustType::String)),
            make_sig(Some(RustType::F64)),
        ];
        let ctx = build_return_wrap_context(&sigs, "F64OrString").unwrap();
        assert_eq!(ctx.variant_by_type.len(), 2);
    }

    #[test]
    fn build_context_unwraps_promise_in_variants() {
        let promise_string = RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![RustType::String],
        };
        let promise_f64 = RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![RustType::F64],
        };
        let sigs = vec![make_sig(Some(promise_string)), make_sig(Some(promise_f64))];
        let ctx = build_return_wrap_context(&sigs, "F64OrString").unwrap();
        // Should unwrap Promise<String> → String, Promise<f64> → f64
        assert_eq!(ctx.variant_by_type.len(), 2);
        assert!(ctx
            .variant_by_type
            .iter()
            .any(|(ty, _)| *ty == RustType::String));
        assert!(ctx
            .variant_by_type
            .iter()
            .any(|(ty, _)| *ty == RustType::F64));
    }

    // --- variant_for ---

    #[test]
    fn variant_for_exact_match() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (RustType::F64, "F64".to_string()),
            ],
        };
        assert_eq!(ctx.variant_for(&RustType::String), Some("String"));
        assert_eq!(ctx.variant_for(&RustType::F64), Some("F64"));
    }

    #[test]
    fn variant_for_option_narrowing() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (
                    RustType::Option(Box::new(RustType::String)),
                    "OptionString".to_string(),
                ),
            ],
        };
        // String matches both exact String and Option<String> → exact match wins
        assert_eq!(ctx.variant_for(&RustType::String), Some("String"));
    }

    #[test]
    fn variant_for_returns_none_when_no_match() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![(RustType::String, "String".to_string())],
        };
        assert_eq!(ctx.variant_for(&RustType::Bool), None);
    }

    // --- unique_option_variant ---

    #[test]
    fn unique_option_variant_picks_single() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (
                    RustType::Option(Box::new(RustType::String)),
                    "OptionString".to_string(),
                ),
            ],
        };
        assert_eq!(ctx.unique_option_variant(), Some("OptionString"));
    }

    #[test]
    fn unique_option_variant_none_when_zero() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (RustType::F64, "F64".to_string()),
            ],
        };
        assert_eq!(ctx.unique_option_variant(), None);
    }

    #[test]
    fn unique_option_variant_none_when_multiple() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (
                    RustType::Option(Box::new(RustType::String)),
                    "OptionString".to_string(),
                ),
                (
                    RustType::Option(Box::new(RustType::F64)),
                    "OptionF64".to_string(),
                ),
            ],
        };
        assert_eq!(ctx.unique_option_variant(), None);
    }

    // --- wrap_leaf ---

    #[test]
    fn wrap_leaf_wraps_in_variant() {
        let ctx = ReturnWrapContext {
            enum_name: "F64OrString".to_string(),
            variant_by_type: vec![
                (RustType::F64, "F64".to_string()),
                (RustType::String, "String".to_string()),
            ],
        };
        let dummy_ast = ast::Expr::Lit(ast::Lit::Num(ast::Number {
            span: swc_common::DUMMY_SP,
            value: 0.0,
            raw: None,
        }));
        let result = wrap_leaf(Expr::NumberLit(42.0), &dummy_ast, &ctx).unwrap();
        match result {
            Expr::FnCall { target, args } => {
                assert!(
                    matches!(&target, crate::ir::CallTarget::Free(name) if name == "F64OrString::F64")
                );
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected FnCall, got {result:?}"),
        }
    }

    #[test]
    fn wrap_leaf_none_uses_unique_option_variant() {
        let ctx = ReturnWrapContext {
            enum_name: "CookieOrOptionString".to_string(),
            variant_by_type: vec![
                (
                    RustType::Named {
                        name: "Cookie".to_string(),
                        type_args: vec![],
                    },
                    "Cookie".to_string(),
                ),
                (
                    RustType::Option(Box::new(RustType::String)),
                    "OptionString".to_string(),
                ),
            ],
        };
        let dummy_ast = ast::Expr::Lit(ast::Lit::Null(ast::Null {
            span: swc_common::DUMMY_SP,
        }));
        let result = wrap_leaf(
            Expr::BuiltinVariantValue(BuiltinVariant::None),
            &dummy_ast,
            &ctx,
        )
        .unwrap();
        match result {
            Expr::FnCall { target, .. } => {
                assert!(
                    matches!(&target, crate::ir::CallTarget::Free(name) if name == "CookieOrOptionString::OptionString")
                );
            }
            _ => panic!("expected FnCall, got {result:?}"),
        }
    }
}
