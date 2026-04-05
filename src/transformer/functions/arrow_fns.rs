//! Arrow function conversion from `const` variable declarations to `Item::Fn`.
//!
//! Converts `const double = (x: number): number => x * 2;` into
//! `fn double(x: f64) -> f64 { x * 2.0 }`.

use super::*;

impl<'a> Transformer<'a> {
    /// Converts `const` variable declarations with arrow function initializers into `Item::Fn`.
    ///
    /// `const double = (x: number): number => x * 2;`
    /// becomes `fn double(x: f64) -> f64 { x * 2.0 }`
    ///
    /// Non-arrow-function variable declarations are skipped.
    pub(crate) fn convert_var_decl_arrow_fns(
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
            // Only handle arrow function initializers
            let arrow = match init.as_ref() {
                ast::Expr::Arrow(arrow) => arrow,
                _ => continue,
            };
            let (name, var_return_type, var_param_types) = match &decl.name {
                ast::Pat::Ident(ident) => {
                    let n = ident.id.sym.to_string();
                    // Extract variable's type annotation and resolve to return type + param types
                    let var_rust_type = ident.type_ann.as_ref().and_then(|ann| {
                        convert_ts_type(&ann.type_ann, self.synthetic, self.reg()).ok()
                    });
                    let ret = var_rust_type
                        .as_ref()
                        .and_then(|ty| self.extract_fn_return_type(ty));
                    let param_types = var_rust_type
                        .as_ref()
                        .and_then(|ty| self.extract_fn_param_types(ty));
                    (n, ret, param_types)
                }
                _ => continue,
            };

            // Convert the arrow to a closure IR, then extract parts for Item::Fn
            // Pass var_return_type so it propagates into the arrow body
            let mut fallback_warnings = Vec::new();

            // Use arrow.span (includes params) for override lookup
            let arrow_scope_start = arrow.span.lo.0;

            let closure = crate::transformer::Transformer {
                tctx: self.tctx,
                synthetic: self.synthetic,
                mut_method_names: self.mut_method_names.clone(),
            }
            .convert_arrow_expr_with_return_type(
                arrow,
                resilient,
                &mut fallback_warnings,
                var_return_type.as_ref(),
                var_param_types.as_deref(),
            )?;
            match closure {
                Expr::Closure {
                    mut params,
                    return_type,
                    body,
                } => {
                    // return_type already includes the override from variable annotation
                    // (applied inside convert_arrow_expr_with_return_type)
                    let ret = return_type;
                    let mut fn_body = match body {
                        crate::ir::ClosureBody::Expr(expr) => {
                            vec![Stmt::Return(Some(*expr))]
                        }
                        crate::ir::ClosureBody::Block(stmts) => stmts,
                    };
                    convert_last_return_to_tail(&mut fn_body);
                    // Untyped parameters → fallback to Any, then override with enum if available
                    for p in &mut params {
                        if p.ty.is_none() {
                            p.ty = Some(RustType::Any);
                        }
                        // Override Any params with generated enum type from FileTypeResolution
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

                    let (type_params, mono_subs) = extract_type_params(
                        arrow.type_params.as_deref(),
                        self.synthetic,
                        self.reg(),
                    );
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
                    // Item::substitute で params, return_type, body 全体に一括適用
                    items.push(if mono_subs.is_empty() {
                        item
                    } else {
                        item.substitute(&mono_subs)
                    });
                    all_warnings.extend(fallback_warnings);
                }
                _ => continue,
            }
        }
        Ok((items, all_warnings))
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
