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
        let value = self.convert_expr(&ast::Expr::Lit(lit.clone()))?;
        Ok(Some(Item::Const {
            vis,
            name,
            ty,
            value,
        }))
    }

    /// Converts an arrow function `const` declaration to `Item::Fn`.
    fn convert_arrow_var_decl(
        &mut self,
        decl: &ast::VarDeclarator,
        arrow: &ast::ArrowExpr,
        vis: Visibility,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        let (name, var_return_type, var_param_types) = match &decl.name {
            ast::Pat::Ident(ident) => {
                let n = ident.id.sym.to_string();
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
            _ => return Ok((vec![], vec![])),
        };

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
