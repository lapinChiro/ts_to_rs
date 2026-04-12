//! Module-level `const` variable declaration conversion.
//!
//! Handles arrow function initializers (`const double = (x: number) => x * 2`)
//! and literal initializers (`const N: number = 42`).
//! Other init types (call, ident, object, array) are currently skipped.

use super::*;

impl<'a> Transformer<'a> {
    /// Converts module-level `const` variable declarations into IR items.
    ///
    /// - Arrow function init → `Item::Fn`
    /// - Literal init → `Item::Const`
    /// - Other init types → skipped (follow-up PRD)
    pub(crate) fn convert_var_decl_module_level(
        &mut self,
        var_decl: &ast::VarDecl,
        vis: Visibility,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        let mut items = Vec::new();
        let mut all_warnings = Vec::new();
        for decl in &var_decl.decls {
            let init = match &decl.init {
                Some(init) => init,
                None => continue,
            };
            // Dispatch by init expression type
            match init.as_ref() {
                ast::Expr::Arrow(arrow) => {
                    let result = self.convert_arrow_var_decl(decl, arrow, vis, resilient)?;
                    items.extend(result.0);
                    all_warnings.extend(result.1);
                    continue;
                }
                ast::Expr::Lit(lit) => {
                    if let Some(item) = self.convert_lit_var_decl(decl, lit, vis)? {
                        items.push(item);
                    }
                    continue;
                }
                _ => continue, // Call, Ident, Object, Array → follow-up PRD
            }
        }
        Ok((items, all_warnings))
    }

    /// Converts a const-safe literal `const` declaration to `Item::Const`.
    ///
    /// Only handles literal types that produce valid Rust `const` values:
    /// `Num`, `Bool`, `Null`. String and Regex literals are not const-safe
    /// in Rust (`to_string()` and `Regex::new()` are not const fn) and are skipped.
    fn convert_lit_var_decl(
        &mut self,
        decl: &ast::VarDeclarator,
        lit: &ast::Lit,
        vis: Visibility,
    ) -> Result<Option<Item>> {
        // Only const-safe literals: Num, Bool, Null
        // Str → "x".to_string() is not const; Regex → Regex::new() is not const;
        // BigInt → not supported; JSXText → not applicable
        match lit {
            ast::Lit::Num(_) | ast::Lit::Bool(_) | ast::Lit::Null(_) => {}
            _ => return Ok(None),
        }

        let ident = match &decl.name {
            ast::Pat::Ident(ident) => ident,
            _ => return Ok(None),
        };
        let name = ident.id.sym.to_string();
        let ty = match ident.type_ann.as_ref() {
            Some(ann) => convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?,
            None => Self::infer_const_type(lit),
        };
        // Skip if type resolves to Any (serde_json::Value) — not const-constructible from literal
        if matches!(ty, RustType::Any) {
            return Ok(None);
        }
        let value = self.convert_expr(&ast::Expr::Lit(lit.clone()))?;
        Ok(Some(Item::Const {
            vis,
            name,
            ty,
            value,
        }))
    }

    /// Converts an arrow function `const` declaration to `Item::Fn` or callable trait items.
    fn convert_arrow_var_decl(
        &mut self,
        decl: &ast::VarDeclarator,
        arrow: &ast::ArrowExpr,
        vis: Visibility,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        let ident = match &decl.name {
            ast::Pat::Ident(ident) => ident,
            _ => return Ok((vec![], vec![])),
        };
        let name = ident.id.sym.to_string();
        let var_rust_type = ident
            .type_ann
            .as_ref()
            .and_then(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()).ok());

        // Check if type annotation refers to a callable interface → route to trait const
        if let Some((trait_name, trait_type_args)) =
            self.callable_trait_name_and_args(var_rust_type.as_ref())
        {
            let items = self.convert_callable_trait_const(
                &name,
                &trait_name,
                &trait_type_args,
                arrow,
                vis,
                resilient,
            )?;
            return Ok((items, vec![]));
        }

        let var_return_type = var_rust_type
            .as_ref()
            .and_then(|ty| self.extract_fn_return_type(ty));
        let var_param_types = var_rust_type
            .as_ref()
            .and_then(|ty| self.extract_fn_param_types(ty));

        let mut fallback_warnings = Vec::new();
        let arrow_scope_start = arrow.span.lo.0;

        let closure = self
            .spawn_nested_scope()
            .convert_arrow_expr_with_return_type(
                arrow,
                resilient,
                &mut fallback_warnings,
                var_return_type.as_ref(),
                var_param_types.as_deref(),
            )?;

        let mut items = Vec::new();
        if let Expr::Closure {
            mut params,
            return_type,
            body,
        } = closure
        {
            let ret = return_type;
            let mut fn_body = match body {
                crate::ir::ClosureBody::Expr(expr) => {
                    vec![Stmt::Return(Some(*expr))]
                }
                crate::ir::ClosureBody::Block(stmts) => stmts,
            };
            convert_last_return_to_tail(&mut fn_body);
            for p in &mut params {
                if p.ty.is_none() {
                    p.ty = Some(RustType::Any);
                }
                if matches!(&p.ty, Some(RustType::Any)) {
                    if let Some(enum_ty) = self
                        .tctx
                        .type_resolution
                        .any_enum_override(&p.name, arrow_scope_start)
                    {
                        p.ty = Some(enum_ty.clone());
                    }
                }
            }

            let (type_params, mono_subs) =
                extract_type_params(arrow.type_params.as_deref(), self.synthetic, self.reg());
            let item = Item::Fn {
                vis,
                attributes: vec![],
                is_async: arrow.is_async,
                name,
                type_params,
                params,
                return_type: ret,
                body: fn_body,
            };
            items.push(if mono_subs.is_empty() {
                item
            } else {
                item.substitute(&mono_subs)
            });
        }
        Ok((items, fallback_warnings))
    }

    /// Returns (trait_name, type_args) if `var_rust_type` refers to a callable interface.
    ///
    /// Looks up the type name in the registry and classifies it. Returns `None` if
    /// the type is not a callable interface.
    fn callable_trait_name_and_args(
        &self,
        var_rust_type: Option<&RustType>,
    ) -> Option<(String, Vec<RustType>)> {
        let (name, type_args) = match var_rust_type? {
            RustType::Named { name, type_args } => (name.as_str(), type_args.clone()),
            _ => return None,
        };
        let def = self.reg().get(name)?;
        use crate::registry::collection::{classify_callable_interface, CallableInterfaceKind};
        let kind = classify_callable_interface(def);
        match kind {
            CallableInterfaceKind::NonCallable => None,
            _ => Some((name.to_string(), type_args)),
        }
    }

    /// Converts a callable interface const declaration to marker struct + inner fn.
    ///
    /// Phase 5: marker struct (ZST) + inner fn (widest signature)
    /// Phase 6-7: return wrap + delegate impl (TODO)
    /// Phase 8: const instance (TODO)
    fn convert_callable_trait_const(
        &mut self,
        value_name: &str,
        trait_name: &str,
        _trait_type_args: &[RustType],
        arrow: &ast::ArrowExpr,
        _vis: Visibility, // Phase 8 で const instance 生成時に使用
        resilient: bool,
    ) -> Result<Vec<Item>> {
        // --- Compute widest signature from registry ---
        let call_sigs = match self.reg().get(trait_name) {
            Some(crate::registry::TypeDef::Struct {
                call_signatures, ..
            }) => call_signatures.clone(),
            _ => {
                return Err(anyhow::anyhow!(
                    "callable interface '{trait_name}' not found"
                ))
            }
        };
        let widest = crate::pipeline::type_converter::overloaded_callable::compute_widest_signature(
            &call_sigs,
            self.synthetic,
        );

        // --- Marker struct name (INV-1) ---
        let base_marker = Self::marker_struct_name(trait_name, value_name);
        let marker_name = self.allocate_marker_name(&base_marker);

        // --- ZST marker struct (P5.2) ---
        let marker_struct = Item::Struct {
            vis: Visibility::Private,
            name: marker_name.clone(),
            type_params: vec![],
            fields: vec![],
            is_unit_struct: true,
        };

        // --- Inner fn body from arrow (P5.4) ---
        // fallback_warnings は callable interface path では使用しない
        // (型注釈付きのため fallback が少なく、呼び出し元で空 vec を返す)
        let mut fallback_warnings = Vec::new();
        let widest_return = widest.return_type.clone();
        let widest_param_types: Vec<RustType> =
            widest.params.iter().map(|p| p.ty.clone()).collect();

        let closure = self
            .spawn_nested_scope()
            .convert_arrow_expr_with_return_type(
                arrow,
                resilient,
                &mut fallback_warnings,
                widest_return.as_ref(),
                Some(&widest_param_types),
            )?;

        // Extract body and params from closure.
        // Closure params use arrow's names + widest types (applied by convert_arrow_expr_with_return_type).
        // We must use arrow param names for inner fn because the body references them.
        let (mut fn_body, closure_params) = if let Expr::Closure { body, params, .. } = closure {
            let stmts = match body {
                crate::ir::ClosureBody::Expr(expr) => vec![Stmt::Return(Some(*expr))],
                crate::ir::ClosureBody::Block(stmts) => stmts,
            };
            (stmts, params)
        } else {
            (vec![], vec![])
        };
        convert_last_return_to_tail(&mut fn_body);

        // --- Return wrap for divergent returns (P6) ---
        // Inner fn body does NOT apply return wrap here. The body returns raw values.
        // Type coercion between the raw return value and the widest union enum type
        // will be handled by Phase 7 (delegate impl) and Phase 9 (TypeResolver update).
        //
        // The ReturnWrapContext infrastructure (P6.0-P6.1) is prepared for future use
        // when the delegate methods need to wrap/unwrap values.

        // Inner method params: use closure params (arrow names + widest types) for positions
        // within arrow arity, then append widest params for positions beyond arrow arity.
        // This ensures body variable references match param names.
        let mut inner_params = closure_params;
        // Ensure all params have types (fallback to widest type if closure left ty as None)
        for (i, p) in inner_params.iter_mut().enumerate() {
            if p.ty.is_none() {
                if let Some(wp) = widest.params.get(i) {
                    p.ty = Some(wp.ty.clone());
                } else {
                    p.ty = Some(RustType::Any);
                }
            }
        }
        // Append widest params beyond arrow arity
        for i in inner_params.len()..widest.params.len() {
            inner_params.push(crate::ir::Param {
                name: widest.params[i].name.clone(),
                ty: Some(widest.params[i].ty.clone()),
            });
        }

        let inner_method = Method {
            vis: Visibility::Private,
            name: "inner".to_string(),
            is_async: arrow.is_async,
            has_self: true,
            has_mut_self: false,
            params: inner_params,
            return_type: widest.return_type.clone(),
            body: Some(fn_body),
        };

        // Marker impl with inner method
        let marker_impl = Item::Impl {
            struct_name: marker_name.clone(),
            type_params: vec![],
            for_trait: None,
            consts: vec![],
            methods: vec![inner_method],
        };

        // Phase 7: delegate impl (TODO)
        // Phase 8: const instance (TODO)

        Ok(vec![marker_struct, marker_impl])
    }

    /// Generates the marker struct name for a callable interface const.
    ///
    /// `marker_struct_name("GetCookie", "getCookie")` → `"GetCookieGetCookieImpl"`
    /// `marker_struct_name("Handler", "request_handler")` → `"HandlerRequestHandlerImpl"`
    ///
    /// value_name の先頭を大文字化して trait_name と結合する。
    /// snake_case は `string_to_pascal_case` で PascalCase 化し、
    /// camelCase はそのまま先頭大文字化する。
    pub(crate) fn marker_struct_name(trait_name: &str, value_name: &str) -> String {
        let capitalized = if value_name.contains('_') {
            // snake_case → PascalCase
            crate::ir::string_to_pascal_case(value_name)
        } else {
            // camelCase / single word → 先頭大文字化
            let mut chars = value_name.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        };
        format!("{trait_name}{capitalized}Impl")
    }

    /// Infers the Rust type for a const-safe literal without type annotation.
    fn infer_const_type(lit: &ast::Lit) -> RustType {
        match lit {
            ast::Lit::Num(_) => RustType::F64,
            ast::Lit::Bool(_) => RustType::Bool,
            // Null without type annotation → Option<serde_json::Value> (best-effort)
            ast::Lit::Null(_) => RustType::Option(Box::new(RustType::Any)),
            // Unreachable: const-safe filter in convert_lit_var_decl rejects other literals
            _ => RustType::Any,
        }
    }

    /// Extracts the return type from a function type.
    ///
    /// Handles two cases:
    /// - `RustType::Fn { return_type, .. }` → returns the return_type directly
    /// - `RustType::Named { name, .. }` → looks up TypeRegistry for `TypeDef::Function`
    pub(crate) fn extract_fn_return_type(&self, ty: &RustType) -> Option<RustType> {
        match ty {
            RustType::Fn { return_type, .. } => {
                let rt = return_type.as_ref();
                if matches!(rt, RustType::Unit) {
                    None
                } else {
                    Some(rt.clone())
                }
            }
            RustType::Named { name, .. } => {
                if let Some(crate::registry::TypeDef::Function { return_type, .. }) =
                    self.reg().get(name)
                {
                    return_type.clone()
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Extracts parameter types from a function type.
    ///
    /// Handles two cases:
    /// - `RustType::Fn { params, .. }` → returns the params directly
    /// - `RustType::Named { name, .. }` → looks up TypeRegistry for `TypeDef::Function`
    pub(crate) fn extract_fn_param_types(&self, ty: &RustType) -> Option<Vec<RustType>> {
        match ty {
            RustType::Fn { params, .. } => Some(params.clone()),
            RustType::Named { name, .. } => {
                if let Some(crate::registry::TypeDef::Function { params, .. }) =
                    self.reg().get(name)
                {
                    Some(params.iter().map(|p| p.ty.clone()).collect())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Wraps return expressions in the inner fn body with union variant constructors.
///
/// Walks `Stmt::Return(Some(expr))` and `Stmt::TailExpr(expr)` and applies `wrap_leaf`.
/// For `Expr::If` (ternary), recursively wraps then/else branches (P6.3).
/// `Expr::Match` at return position does not occur (P0.1: YAGNI).
///
/// Currently unused: inner fn body returns raw values without variant wrapping.
/// Phase 7 delegate impl unwraps via `match self.inner(...) { Variant(v) => v }`.
/// Will be needed if/when inner fn body return wrap is required (Phase 9.2+ で再評価).
#[allow(dead_code)]
fn wrap_body_returns(
    stmts: &mut [Stmt],
    arrow: &ast::ArrowExpr,
    ctx: &crate::transformer::return_wrap::ReturnWrapContext,
) -> anyhow::Result<()> {
    // Create a dummy AST expression for span reporting
    let dummy_span_expr = ast::Expr::Lit(ast::Lit::Null(ast::Null { span: arrow.span }));

    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Return(Some(ref mut expr)) => {
                *expr = wrap_expr_tail(expr.clone(), &dummy_span_expr, ctx)?;
            }
            Stmt::TailExpr(ref mut expr) => {
                *expr = wrap_expr_tail(expr.clone(), &dummy_span_expr, ctx)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Recursively wraps an expression for return position.
///
/// For `Expr::If` (ternary from cond), wraps then/else branches individually (P6.3).
/// For leaf expressions, delegates to `wrap_leaf`.
#[allow(dead_code)]
fn wrap_expr_tail(
    expr: Expr,
    ast_arg: &ast::Expr,
    ctx: &crate::transformer::return_wrap::ReturnWrapContext,
) -> anyhow::Result<Expr> {
    match expr {
        // P6.3: ternary → wrap each branch
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            let wrapped_then = wrap_expr_tail(*then_expr, ast_arg, ctx)?;
            let wrapped_else = wrap_expr_tail(*else_expr, ast_arg, ctx)?;
            Ok(Expr::If {
                condition,
                then_expr: Box::new(wrapped_then),
                else_expr: Box::new(wrapped_else),
            })
        }
        // P6.4: IfLet at return position (P0.1: occurs in ternary narrowing)
        Expr::IfLet {
            pattern,
            expr: scrutinee,
            then_expr,
            else_expr,
        } => {
            let wrapped_then = wrap_expr_tail(*then_expr, ast_arg, ctx)?;
            let wrapped_else = wrap_expr_tail(*else_expr, ast_arg, ctx)?;
            Ok(Expr::IfLet {
                pattern,
                expr: scrutinee,
                then_expr: Box::new(wrapped_then),
                else_expr: Box::new(wrapped_else),
            })
        }
        // Leaf expression → wrap in variant
        leaf => crate::transformer::return_wrap::wrap_leaf(leaf, ast_arg, ctx),
    }
}
