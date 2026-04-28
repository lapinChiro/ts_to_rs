//! Leaf return expression wrapping (`wrap_leaf`) + private inference / coercion helpers.
//!
//! Determines the appropriate synthetic union variant for each leaf return expression
//! using a 5-tier priority cascade (already-wrapped guard → polymorphic None →
//! literal inference → TypeResolver type → single non-Option fallback → hard error).

use anyhow::{anyhow, Result};

use crate::ir::{Expr, RustType};

use super::context::ReturnWrapContext;

/// Wraps a leaf return expression in the appropriate union variant.
///
/// Variant determination priority:
/// 1. Polymorphic None (None/null/undefined → unique Option variant)
/// 2. Literal inference (string/number/bool literals)
/// 3. TypeResolver resolved type (from pre-collected `ReturnLeafType`)
/// 4. Single non-Option variant fallback
/// 5. Hard error
///
/// `expr_type` is the resolved type from TypeResolver (pre-collected by
/// `collect_return_leaf_types`). `span` is the source byte range for error reporting.
pub(crate) fn wrap_leaf(
    ir_expr: Expr,
    expr_type: Option<&RustType>,
    span: Option<(u32, u32)>,
    ctx: &ReturnWrapContext,
) -> Result<Expr> {
    let span_desc = span
        .map(|(lo, hi)| format!("byte {lo}..{hi}"))
        .unwrap_or_else(|| "unknown location".to_string());

    // 0. Already wrapped: the leaf is a `Ctx::Enum::Variant(inner)` call for the
    // same enum (e.g., convert_lit pre-wrapped a primitive literal via the
    // T6-3 `wrap_in_synthetic_union_variant` path for Option<Union> call-arg
    // coercion, then the same function returns that Union from another site).
    // Skipping avoids `Enum::V(Enum::V(inner))` double-wrap (I-144 T6-3 guard).
    if let Expr::FnCall {
        target:
            crate::ir::CallTarget::UserEnumVariantCtor {
                enum_ty: already_enum,
                ..
            },
        ..
    } = &ir_expr
    {
        if already_enum.as_str() == ctx.enum_name {
            return Ok(ir_expr);
        }
    }

    // 1. Polymorphic None: `return null/undefined/None`
    if is_none_expr(&ir_expr) {
        return match ctx.unique_option_variant() {
            Some(variant) => Ok(Expr::FnCall {
                target: crate::ir::CallTarget::UserEnumVariantCtor {
                    enum_ty: crate::ir::UserTypeRef::new(&ctx.enum_name),
                    variant: variant.to_string(),
                },
                args: vec![Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)],
            }),
            None => Err(anyhow!(
                "ambiguous polymorphic None at {span_desc}: multiple Option<_> variants in {}",
                ctx.enum_name,
            )),
        };
    }

    // 2. Literal inference (string/number/bool/Some)
    if let Some(variant) = infer_variant_from_expr(&ir_expr, ctx) {
        return Ok(wrap_in_variant(&ir_expr, ctx, variant));
    }

    // 3. TypeResolver resolved type
    if let Some(ty) = expr_type {
        if let Some(variant) = ctx.variant_for(ty) {
            return Ok(wrap_in_variant(&ir_expr, ctx, variant));
        }
    }

    // 4. Fallback: if only one non-Option variant, use it
    let non_option_variants: Vec<&str> = ctx
        .variant_by_type
        .iter()
        .filter(|(ty, _)| !matches!(ty, RustType::Option(_)))
        .map(|(_, name)| name.as_str())
        .collect();

    if non_option_variants.len() == 1 {
        return Ok(wrap_in_variant(&ir_expr, ctx, non_option_variants[0]));
    }

    // 5. Cannot determine variant — hard error (INV-3)
    Err(anyhow!(
        "cannot determine return variant at {span_desc} for union {} (expr: {ir_expr:?}, type: {expr_type:?})",
        ctx.enum_name,
    ))
}

/// Wraps an expression in a union variant constructor.
///
/// When the variant expects `String` but the expression is a string literal (`&str`),
/// automatically applies `.to_string()` conversion via [`coerce_string_literal`].
fn wrap_in_variant(expr: &Expr, ctx: &ReturnWrapContext, variant: &str) -> Expr {
    let variant_type = ctx
        .variant_by_type
        .iter()
        .find(|(_, name)| name == variant)
        .map(|(ty, _)| ty);
    let arg = coerce_string_literal(expr, variant_type);
    Expr::FnCall {
        target: crate::ir::CallTarget::UserEnumVariantCtor {
            enum_ty: crate::ir::UserTypeRef::new(&ctx.enum_name),
            variant: variant.to_string(),
        },
        args: vec![arg],
    }
}

/// Converts a string literal to `String` when the expected type is `RustType::String`.
///
/// In Rust, string literals are `&str` but `enum Variant(String)` requires `String`.
fn coerce_string_literal(expr: &Expr, expected_type: Option<&RustType>) -> Expr {
    if matches!(expected_type, Some(RustType::String)) {
        if let Expr::StringLit(_) = expr {
            return Expr::MethodCall {
                object: Box::new(expr.clone()),
                method: "to_string".to_string(),
                args: vec![],
            };
        }
    }
    expr.clone()
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
            target: crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            ..
        } => ctx.unique_option_variant(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::BuiltinVariant;

    // --- is_none_expr (branch coverage) ---

    #[test]
    fn is_none_expr_returns_true_for_builtin_none() {
        // True branch: BuiltinVariantValue(None) is the canonical TS `null`/`undefined`
        // representation in IR — `wrap_leaf` priority 1 dispatches polymorphic None
        // wrapping based on this predicate.
        assert!(is_none_expr(&Expr::BuiltinVariantValue(
            BuiltinVariant::None
        )));
    }

    #[test]
    fn is_none_expr_returns_false_for_other_builtin_variants() {
        // False branch (Some / Ok / Err): non-None builtin variants must NOT trigger
        // the polymorphic None path (they are infer-by-shape inputs for
        // `infer_variant_from_expr`).
        assert!(!is_none_expr(&Expr::BuiltinVariantValue(
            BuiltinVariant::Some
        )));
        assert!(!is_none_expr(&Expr::BuiltinVariantValue(
            BuiltinVariant::Ok
        )));
        assert!(!is_none_expr(&Expr::BuiltinVariantValue(
            BuiltinVariant::Err
        )));
    }

    #[test]
    fn is_none_expr_returns_false_for_non_builtin_expressions() {
        // False branch (literals / idents / calls): only the structural None marker
        // qualifies. String "null" or NumberLit 0 must NOT be treated as None.
        assert!(!is_none_expr(&Expr::StringLit("null".to_string())));
        assert!(!is_none_expr(&Expr::NumberLit(0.0)));
        assert!(!is_none_expr(&Expr::Ident("x".to_string())));
    }

    // --- coerce_string_literal (branch coverage) ---

    #[test]
    fn coerce_string_literal_wraps_when_string_expected_and_str_lit() {
        // True branch: expected = String + StringLit → MethodCall .to_string()
        // (TS string literal is `&str` in Rust IR; enum Variant(String) requires owned).
        let result =
            coerce_string_literal(&Expr::StringLit("hi".to_string()), Some(&RustType::String));
        match result {
            Expr::MethodCall { method, .. } => {
                assert_eq!(method, "to_string");
            }
            _ => panic!("expected MethodCall (.to_string()), got {result:?}"),
        }
    }

    #[test]
    fn coerce_string_literal_passes_through_when_expected_not_string() {
        // False branch (expected != String): even StringLit passes through unchanged
        // — the variant doesn't need owned String.
        let result =
            coerce_string_literal(&Expr::StringLit("hi".to_string()), Some(&RustType::F64));
        assert_eq!(result, Expr::StringLit("hi".to_string()));
    }

    #[test]
    fn coerce_string_literal_passes_through_when_expected_none() {
        // False branch (expected = None): no coercion regardless of expr shape.
        let result = coerce_string_literal(&Expr::StringLit("hi".to_string()), None);
        assert_eq!(result, Expr::StringLit("hi".to_string()));
    }

    #[test]
    fn coerce_string_literal_passes_through_when_expr_not_str_lit() {
        // False branch (expr != StringLit): non-literal expressions stay unchanged
        // even when expected = String (Ident-typed values are already owned String).
        let result = coerce_string_literal(&Expr::Ident("s".to_string()), Some(&RustType::String));
        assert_eq!(result, Expr::Ident("s".to_string()));
    }

    // --- infer_variant_from_expr (branch coverage) ---

    #[test]
    fn infer_variant_string_literal() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (RustType::F64, "F64".to_string()),
            ],
        };
        assert_eq!(
            infer_variant_from_expr(&Expr::StringLit("hello".to_string()), &ctx),
            Some("String")
        );
    }

    #[test]
    fn infer_variant_number_literal() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (RustType::F64, "F64".to_string()),
            ],
        };
        assert_eq!(
            infer_variant_from_expr(&Expr::NumberLit(42.0), &ctx),
            Some("F64")
        );
    }

    #[test]
    fn infer_variant_bool_literal() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::Bool, "Bool".to_string()),
                (RustType::String, "String".to_string()),
            ],
        };
        assert_eq!(
            infer_variant_from_expr(&Expr::BoolLit(true), &ctx),
            Some("Bool")
        );
    }

    #[test]
    fn infer_variant_some_call() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (
                    RustType::Option(Box::new(RustType::F64)),
                    "OptionF64".to_string(),
                ),
            ],
        };
        let some_expr = Expr::FnCall {
            target: crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            args: vec![Expr::NumberLit(1.0)],
        };
        assert_eq!(infer_variant_from_expr(&some_expr, &ctx), Some("OptionF64"));
    }

    #[test]
    fn infer_variant_unknown_returns_none() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (RustType::F64, "F64".to_string()),
            ],
        };
        assert_eq!(
            infer_variant_from_expr(&Expr::Ident("x".to_string()), &ctx),
            None
        );
    }

    // --- wrap_leaf ---

    #[test]
    fn wrap_leaf_wraps_literal_by_inference() {
        let ctx = ReturnWrapContext {
            enum_name: "F64OrString".to_string(),
            variant_by_type: vec![
                (RustType::F64, "F64".to_string()),
                (RustType::String, "String".to_string()),
            ],
        };
        // Priority 2: literal inference (no TypeResolver type needed)
        let result = wrap_leaf(Expr::NumberLit(42.0), None, None, &ctx).unwrap();
        match result {
            Expr::FnCall { target, args } => {
                assert!(
                    matches!(&target, crate::ir::CallTarget::UserEnumVariantCtor { variant, .. } if variant == "F64")
                );
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected FnCall, got {result:?}"),
        }
    }

    #[test]
    fn wrap_leaf_uses_type_resolver_type() {
        let ctx = ReturnWrapContext {
            enum_name: "F64OrString".to_string(),
            variant_by_type: vec![
                (RustType::F64, "F64".to_string()),
                (RustType::String, "String".to_string()),
            ],
        };
        // Priority 3: TypeResolver resolved type (for non-literal expressions)
        let result = wrap_leaf(
            Expr::Ident("c".to_string()),
            Some(&RustType::String),
            Some((10, 11)),
            &ctx,
        )
        .unwrap();
        match result {
            Expr::FnCall { target, args } => {
                assert!(
                    matches!(&target, crate::ir::CallTarget::UserEnumVariantCtor { variant, .. } if variant == "String")
                );
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], Expr::Ident("c".to_string()));
            }
            _ => panic!("expected FnCall, got {result:?}"),
        }
    }

    #[test]
    fn wrap_leaf_priority0_already_wrapped_same_enum_returns_as_is() {
        // Priority 0 guard (I-144 T6-3): if the leaf is already a ctor call for
        // the target enum, do NOT double-wrap. This happens when
        // `convert_lit` pre-wraps a primitive via the Option<Union> call-arg
        // coercion path and then the same function returns that Union from a
        // different site; without this guard we would emit
        // `F64OrString::F64(F64OrString::F64(inner))`.
        let ctx = ReturnWrapContext {
            enum_name: "F64OrString".to_string(),
            variant_by_type: vec![
                (RustType::F64, "F64".to_string()),
                (RustType::String, "String".to_string()),
            ],
        };
        let pre_wrapped = Expr::FnCall {
            target: crate::ir::CallTarget::UserEnumVariantCtor {
                enum_ty: crate::ir::UserTypeRef::new("F64OrString".to_string()),
                variant: "F64".to_string(),
            },
            args: vec![Expr::NumberLit(5.0)],
        };
        let result = wrap_leaf(pre_wrapped.clone(), None, None, &ctx).unwrap();
        assert_eq!(
            result, pre_wrapped,
            "already-wrapped expr for same enum must be returned unchanged"
        );
    }

    #[test]
    fn wrap_leaf_priority0_different_enum_falls_through_to_inference() {
        // If the pre-wrapped ctor call is for a DIFFERENT enum than the target,
        // the guard deliberately falls through so inference / type-resolver
        // priorities can still attempt to (correctly or incorrectly) wrap it.
        // This test locks in that fall-through behaviour — a future change
        // that treats any UserEnumVariantCtor as "do not re-wrap" would break
        // this contract.
        let ctx = ReturnWrapContext {
            enum_name: "F64OrString".to_string(),
            variant_by_type: vec![
                (RustType::F64, "F64".to_string()),
                (RustType::String, "String".to_string()),
            ],
        };
        let other_enum_expr = Expr::FnCall {
            target: crate::ir::CallTarget::UserEnumVariantCtor {
                enum_ty: crate::ir::UserTypeRef::new("OtherEnum".to_string()),
                variant: "A".to_string(),
            },
            args: vec![Expr::NumberLit(5.0)],
        };
        // With no type info and no literal-inference match (FnCall is not a
        // Lit), the function falls through to priority 4 "single non-Option
        // fallback". The `F64OrString` context has two variants → fallback is
        // not single → priority 5 error. Verify the error path (this means
        // the priority-0 guard did NOT absorb the expr).
        let result = wrap_leaf(other_enum_expr, None, None, &ctx);
        assert!(
            result.is_err(),
            "different-enum wrapped expr must NOT be absorbed by the priority-0 guard"
        );
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
        let result = wrap_leaf(
            Expr::BuiltinVariantValue(BuiltinVariant::None),
            None,
            None,
            &ctx,
        )
        .unwrap();
        match result {
            Expr::FnCall { target, .. } => {
                assert!(
                    matches!(&target, crate::ir::CallTarget::UserEnumVariantCtor { variant, .. } if variant == "OptionString")
                );
            }
            _ => panic!("expected FnCall, got {result:?}"),
        }
    }
}
