//! Return value wrapping for divergent callable interface return types.
//!
//! When a callable interface has overloads with different return types,
//! the inner function returns a synthetic union enum. Each return expression
//! in the arrow body must be wrapped in the appropriate enum variant.
//!
//! Phase 7 (P7.0) で inner fn body の return wrap に使用。

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType};
use crate::pipeline::synthetic_registry::variant_name_for_type;
use crate::pipeline::type_resolution::FileTypeResolution;
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

/// Builds a `ReturnWrapContext` from a synthetic union enum's variants.
///
/// Used for general functions (not callable interfaces) whose return type
/// is a synthetic union enum. The variant mapping is derived from the
/// enum's `EnumVariant` definitions.
pub(crate) fn build_return_wrap_context_from_enum(
    enum_name: &str,
    variants: &[crate::ir::EnumVariant],
) -> ReturnWrapContext {
    let variant_by_type: Vec<(RustType, String)> = variants
        .iter()
        .filter_map(|v| v.data.as_ref().map(|ty| (ty.clone(), v.name.clone())))
        .collect();
    ReturnWrapContext {
        enum_name: enum_name.to_string(),
        variant_by_type,
    }
}

impl ReturnWrapContext {
    /// Finds the variant name for the given return type.
    ///
    /// Tries exact match first, then Option<T> narrowing (T matches Option<T> variant).
    pub(crate) fn variant_for(&self, ty: &RustType) -> Option<&str> {
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

/// Pre-collected type and span for a return leaf expression.
///
/// Collected from SWC AST before IR conversion. Consumed positionally
/// by `wrap_body_returns` / `wrap_expr_tail` during IR post-processing.
#[derive(Debug, Clone)]
pub(crate) struct ReturnLeafType {
    /// Resolved type from TypeResolver (`None` if unknown).
    pub ty: Option<RustType>,
    /// Source byte span `(lo, hi)` for error reporting.
    pub span: (u32, u32),
}

/// Collects return leaf expression types from a SWC arrow body.
///
/// Walks the arrow body in depth-first order, finding all return/tail leaf
/// expressions and resolving their types from `FileTypeResolution::expr_types`.
/// Ternary branches (`CondExpr`) are recursively expanded to leaf level.
///
/// The resulting `Vec` is consumed positionally by `wrap_body_returns`.
/// The positional invariant (SWC and IR return leaves in same depth-first order)
/// holds because the Transformer preserves statement structure and return ordering.
pub(crate) fn collect_return_leaf_types(
    arrow: &ast::ArrowExpr,
    type_resolution: &FileTypeResolution,
) -> Vec<ReturnLeafType> {
    let mut out = Vec::new();
    match &*arrow.body {
        ast::BlockStmtOrExpr::Expr(expr) => {
            collect_expr_leaf_types(expr, type_resolution, &mut out);
        }
        ast::BlockStmtOrExpr::BlockStmt(block) => {
            collect_stmts_return_leaf_types(&block.stmts, type_resolution, &mut out);
        }
    }
    out
}

/// Collects return leaf types from a sequence of SWC statements.
pub(crate) fn collect_stmts_return_leaf_types(
    stmts: &[ast::Stmt],
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    for stmt in stmts {
        collect_stmt_return_leaf_types(stmt, type_resolution, out);
    }
}

/// Collects return leaf types from a single SWC statement.
///
/// Recurses into all block-containing statement structures (if/else, for, while,
/// try/catch, switch, labeled blocks) to find all nested return statements.
/// Must mirror the IR-side walk in `wrap_body_returns` to maintain the
/// positional invariant.
fn collect_stmt_return_leaf_types(
    stmt: &ast::Stmt,
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    match stmt {
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                collect_expr_leaf_types(arg, type_resolution, out);
            }
        }
        ast::Stmt::If(if_stmt) => {
            collect_stmt_return_leaf_types(&if_stmt.cons, type_resolution, out);
            if let Some(alt) = &if_stmt.alt {
                collect_stmt_return_leaf_types(alt, type_resolution, out);
            }
        }
        ast::Stmt::Block(block) => {
            collect_stmts_return_leaf_types(&block.stmts, type_resolution, out);
        }
        ast::Stmt::Switch(switch) => {
            for case in &switch.cases {
                collect_stmts_return_leaf_types(&case.cons, type_resolution, out);
            }
        }
        ast::Stmt::For(for_stmt) => {
            collect_stmt_return_leaf_types(&for_stmt.body, type_resolution, out);
        }
        ast::Stmt::ForIn(for_in) => {
            collect_stmt_return_leaf_types(&for_in.body, type_resolution, out);
        }
        ast::Stmt::ForOf(for_of) => {
            collect_stmt_return_leaf_types(&for_of.body, type_resolution, out);
        }
        ast::Stmt::While(while_stmt) => {
            collect_stmt_return_leaf_types(&while_stmt.body, type_resolution, out);
        }
        ast::Stmt::DoWhile(do_while) => {
            collect_stmt_return_leaf_types(&do_while.body, type_resolution, out);
        }
        ast::Stmt::Try(try_stmt) => {
            collect_stmts_return_leaf_types(&try_stmt.block.stmts, type_resolution, out);
            if let Some(catch) = &try_stmt.handler {
                collect_stmts_return_leaf_types(&catch.body.stmts, type_resolution, out);
            }
            // finally は collect しない。IR 側では finally body が
            // scopeguard::guard クロージャ内に封入されるため、
            // wrap_body_returns が walk せず位置不一致になる。
            // finally 内の return は JS でも非推奨パターン。
        }
        ast::Stmt::Labeled(labeled) => {
            collect_stmt_return_leaf_types(&labeled.body, type_resolution, out);
        }
        _ => {}
    }
}

/// Collects leaf types from a SWC expression in return position.
///
/// Recursively collects from ternary branches and parenthesized expressions.
/// For leaf expressions, resolves the type via the canonical
/// [`FileTypeResolution::resolve_expr_type`] primitive (I-177-B), so the
/// `narrowed_type` 優先 → `expr_type` fallback precedence shared with
/// [`Transformer::get_expr_type`](crate::transformer::Transformer::get_expr_type)
/// is preserved (single source of truth, canonical primitive 経由による DRY 保証)。
///
/// Note: SeqExpr (comma operator) は IR にサポートされておらず、
/// Transformer で変換エラーになるため collect しない。
fn collect_expr_leaf_types(
    expr: &ast::Expr,
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    match expr {
        // Ternary: recurse into both branches
        ast::Expr::Cond(cond) => {
            collect_expr_leaf_types(&cond.cons, type_resolution, out);
            collect_expr_leaf_types(&cond.alt, type_resolution, out);
        }
        // Parenthesized: unwrap
        ast::Expr::Paren(paren) => {
            collect_expr_leaf_types(&paren.expr, type_resolution, out);
        }
        // Leaf expression: resolve type via canonical primitive
        leaf => {
            let swc_span = leaf.span();
            let ty = type_resolution.resolve_expr_type(leaf).cloned();
            out.push(ReturnLeafType {
                ty,
                span: (swc_span.lo.0, swc_span.hi.0),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::BuiltinVariant;
    use crate::pipeline::{parse_files, SyntheticTypeRegistry};
    use crate::registry::{build_registry, ParamDef};

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

    // --- collect_return_leaf_types ---

    /// Helper: parse source, build registry with callable interface, resolve types,
    /// extract the arrow from the const declaration, and collect return leaf types.
    fn collect_leaves_for_callable(interface_def: &str, const_decl: &str) -> Vec<ReturnLeafType> {
        let (leaves, _events) = collect_leaves_with_events_for_callable(interface_def, const_decl);
        leaves
    }

    /// Helper: same as `collect_leaves_for_callable`, but also returns the narrow
    /// events for debugging / cross-axis verification.
    fn collect_leaves_with_events_for_callable(
        interface_def: &str,
        const_decl: &str,
    ) -> (
        Vec<ReturnLeafType>,
        Vec<crate::pipeline::narrowing_analyzer::NarrowEvent>,
    ) {
        let source = format!("{interface_def}\n{const_decl}");
        let files = parse_files(vec![(std::path::PathBuf::from("test.ts"), source)]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();

        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // Extract the arrow from the second module item (first is interface)
        let var_decl = match &file.module.body[1] {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(
                vd,
            ))) => vd,
            _ => panic!("expected var decl"),
        };
        let arrow = match var_decl.decls[0].init.as_deref() {
            Some(ast::Expr::Arrow(a)) => a,
            _ => panic!("expected arrow expr"),
        };

        let leaves = collect_return_leaf_types(arrow, &resolution);
        (leaves, resolution.narrow_events.clone())
    }

    #[test]
    fn collect_leaves_expression_body_single_ident() {
        let leaves = collect_leaves_for_callable(
            "interface F { (c: string): string; }",
            "const f: F = (c: string): string => c;",
        );
        assert_eq!(leaves.len(), 1, "single expression body → 1 leaf");
        assert_eq!(leaves[0].ty, Some(RustType::String));
    }

    #[test]
    fn collect_leaves_block_body_multiple_returns() {
        let leaves = collect_leaves_for_callable(
            "interface G { (c: string, key: string): number; }",
            r#"const g: G = (c: string, key: string): number => {
                if (key) { return 42; }
                return 0;
            };"#,
        );
        assert_eq!(leaves.len(), 2, "two return statements → 2 leaves");
        assert_eq!(leaves[0].ty, Some(RustType::F64));
        assert_eq!(leaves[1].ty, Some(RustType::F64));
    }

    #[test]
    fn collect_leaves_ternary_expression_body() {
        let leaves = collect_leaves_for_callable(
            "interface H { (c: string): string; }",
            r#"const h: H = (c: string): string => c ? c : "fallback";"#,
        );
        assert_eq!(leaves.len(), 2, "ternary → 2 leaves (then + else)");
        assert_eq!(leaves[0].ty, Some(RustType::String));
        assert_eq!(leaves[1].ty, Some(RustType::String));
    }

    #[test]
    fn collect_leaves_for_loop_nested_return() {
        let leaves = collect_leaves_for_callable(
            "interface I { (c: string): string; }",
            r#"const i: I = (c: string): string => {
                for (let x = 0; x < 10; x++) {
                    if (x > 5) { return c; }
                }
                return "default";
            };"#,
        );
        assert_eq!(
            leaves.len(),
            2,
            "for with nested return + final return → 2 leaves"
        );
        // c is a param (String), "default" is a string literal (String)
        assert_eq!(leaves[0].ty, Some(RustType::String));
        assert_eq!(leaves[1].ty, Some(RustType::String));
    }

    #[test]
    fn collect_leaves_try_catch_returns() {
        let leaves = collect_leaves_for_callable(
            "interface J { (c: string): string; }",
            r#"const j: J = (c: string): string => {
                try {
                    return c;
                } catch (e) {
                    return "error";
                }
            };"#,
        );
        assert_eq!(leaves.len(), 2, "try + catch each with return → 2 leaves");
        assert_eq!(leaves[0].ty, Some(RustType::String));
        assert_eq!(leaves[1].ty, Some(RustType::String));
    }

    /// Helper: parse source containing a top-level `function h(...) {...}` declaration
    /// and return narrow events + leaves for its body.
    fn collect_leaves_with_events_for_fn_decl(
        src: &str,
    ) -> (
        Vec<ReturnLeafType>,
        Vec<crate::pipeline::narrowing_analyzer::NarrowEvent>,
    ) {
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // Walk fn decl body via collect_stmts_return_leaf_types
        let fn_decl = match &file.module.body[0] {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(d))) => {
                d
            }
            _ => panic!("expected fn decl"),
        };
        let body = fn_decl.function.body.as_ref().expect("fn body");
        let mut leaves = Vec::new();
        collect_stmts_return_leaf_types(&body.stmts, &resolution, &mut leaves);
        (leaves, resolution.narrow_events.clone())
    }

    #[test]
    fn test_collect_leaves_typeof_narrow_post_if_return_fn_decl() {
        // Empirical lock-in for declaration form (visit_fn_decl path which already
        // sets current_block_end correctly — pre-existing GREEN baseline).
        let (leaves, _events) = collect_leaves_with_events_for_fn_decl(
            r#"function h(x: string | number): string | number {
    if (typeof x === "string") return 0;
    else { console.log("ne"); }
    return x;
}
console.log(h(42));
console.log(h("a"));"#,
        );
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[0].ty, Some(RustType::F64), "leaf 0 = `0` literal");
        assert_eq!(
            leaves[1].ty,
            Some(RustType::F64),
            "leaf 1 = post-if `x` narrowed to F64"
        );
    }

    /// Helper: parse source containing a top-level `const h = function (...) {...}`
    /// fn-expression assignment and return narrow events + leaves for its body.
    fn collect_leaves_with_events_for_fn_expr(
        src: &str,
    ) -> (
        Vec<ReturnLeafType>,
        Vec<crate::pipeline::narrowing_analyzer::NarrowEvent>,
    ) {
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // Extract the FnExpr from `const h = function (...) {...};`
        let var_decl = match &file.module.body[0] {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(
                vd,
            ))) => vd,
            _ => panic!("expected var decl"),
        };
        let fn_expr = match var_decl.decls[0].init.as_deref() {
            Some(ast::Expr::Fn(fe)) => fe,
            _ => panic!("expected fn expr init"),
        };
        let body = fn_expr.function.body.as_ref().expect("fn body");
        let mut leaves = Vec::new();
        collect_stmts_return_leaf_types(&body.stmts, &resolution, &mut leaves);
        (leaves, resolution.narrow_events.clone())
    }

    #[test]
    fn test_typeof_narrow_post_if_pushes_early_return_complement_in_class_method() {
        // I-177-F (extended): class method body は `visit_method_function` 経由で walk
        // される。pre-fix では `for stmt in &body.stmts` で直接 iterate して
        // `current_block_end` を set しないため、method body 内の if-stmt with
        // typeof guard + then-exit + else-non-exit が EarlyReturnComplement narrow
        // event を post-if scope に push しない (silent type widening risk)。
        // post-fix: `visit_block_stmt(body)` 経由で current_block_end を set し、
        // detect_early_return_narrowing が正しく fire する。
        use crate::pipeline::narrowing_analyzer::NarrowEvent;
        let src = r#"class Processor {
    process(x: string | number): number {
        if (typeof x === "string") return 0;
        else { console.log("ne"); }
        return x.valueOf();
    }
}"#;
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // 期待: 3 narrow events (Primary then-branch + Primary else-branch +
        // EarlyReturnComplement post-if)
        let early_return_complement_count = resolution
            .narrow_events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    NarrowEvent::Narrow {
                        var_name,
                        trigger: crate::pipeline::narrowing_analyzer::NarrowTrigger::EarlyReturnComplement(_),
                        ..
                    } if var_name == "x"
                )
            })
            .count();
        assert_eq!(
            early_return_complement_count, 1,
            "class method body should push EarlyReturnComplement narrow event \
             at post-if scope (visit_block_stmt 経由で current_block_end が set される)"
        );
    }

    #[test]
    fn test_typeof_narrow_post_if_pushes_early_return_complement_in_class_constructor() {
        // I-177-F (extended): constructor body symmetric — visit_class_decl 内の
        // ast::ClassMember::Constructor arm で body を walk するが、pre-fix では
        // `visit_block_stmt` を skip して直接 iterate。post-fix で symmetric に修正。
        use crate::pipeline::narrowing_analyzer::NarrowEvent;
        let src = r#"class Container {
    field: number;
    constructor(x: string | number) {
        if (typeof x === "string") {
            this.field = 0;
            return;
        }
        else { console.log("ne"); }
        this.field = x.valueOf();
    }
}"#;
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        let early_return_complement_count = resolution
            .narrow_events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    NarrowEvent::Narrow {
                        var_name,
                        trigger: crate::pipeline::narrowing_analyzer::NarrowTrigger::EarlyReturnComplement(_),
                        ..
                    } if var_name == "x"
                )
            })
            .count();
        assert_eq!(
            early_return_complement_count, 1,
            "constructor body should push EarlyReturnComplement narrow event \
             at post-if scope"
        );
    }

    #[test]
    fn test_collect_leaves_typeof_narrow_post_if_return_fn_expr() {
        // I-177-F symmetric: function expression form (resolve_fn_expr path).
        // Same defect class as arrow form — pre-fix `current_block_end` not set,
        // post-fix visit_block_stmt 経由で正しく set される.
        let (leaves, _events) = collect_leaves_with_events_for_fn_expr(
            r#"const h = function (x: string | number): string | number {
    if (typeof x === "string") return 0;
    else { console.log("ne"); }
    return x;
};"#,
        );
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[0].ty, Some(RustType::F64));
        assert_eq!(
            leaves[1].ty,
            Some(RustType::F64),
            "leaf 1 = post-if `x` narrowed to F64 in fn-expression body"
        );
    }

    #[test]
    fn test_collect_leaves_typeof_narrow_post_if_return_arrow_form() {
        // I-177-B Matrix cell #9 + I-177-F (callable-interface arrow form):
        // typeof narrow の then-branch が exit して post-if で `return x` が実行される
        // 場合、`x` は narrowed type (`F64`) を持つ。
        //
        // Architectural dependency chain (2026-04-26):
        //  1. I-177-E: synthetic fork inherits types — enables compute_complement_type
        //     to find variants for builtin-pre-registered union types.
        //  2. I-177-B: collect_expr_leaf_types canonical helper — correct query order
        //     for Ident leaf types.
        //  3. I-177-F: `resolve_arrow_expr` body walks via `visit_block_stmt` so that
        //     `current_block_end` is set, allowing `detect_early_return_narrowing` to
        //     push EarlyReturnComplement narrow events in arrow body post-if scope.
        let leaves = collect_leaves_for_callable(
            "interface H { (x: string | number): string | number; }",
            r#"const h: H = (x: string | number): string | number => {
                if (typeof x === "string") return 0;
                else { console.log("ne"); }
                return x;
            };"#,
        );
        // 2 leaves: `0` (return 0, F64 from NumLit) + `x` (return x, F64 from narrow).
        assert_eq!(leaves.len(), 2, "if/else + post-if return → 2 leaves");
        assert_eq!(
            leaves[0].ty,
            Some(RustType::F64),
            "leaf 0 = `0` literal in then branch, NumLit type = F64"
        );
        assert_eq!(
            leaves[1].ty,
            Some(RustType::F64),
            "leaf 1 = `x` Ident in post-if return, narrowed to F64 \
             (else branch was string-excluded by typeof narrow)"
        );
    }
}
