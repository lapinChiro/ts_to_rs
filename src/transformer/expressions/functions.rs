//! Function and arrow expression conversion to IR closures.
//!
//! Converts function expressions (`function(x) { ... }`) and arrow expressions
//! (`(x) => ...`) into [`Expr::Closure`] IR nodes.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{ClosureBody, Expr, Param, RustType, Stmt};
use crate::pipeline::type_converter::convert_ts_type;
use crate::transformer::functions::convert_last_return_to_tail;
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Converts a single parameter pattern into IR [`Param`] and expansion statements.
    ///
    /// Shared logic for both function expressions and arrow expressions.
    fn convert_function_param_pat(
        &mut self,
        pat: &ast::Pat,
        params: &mut Vec<Param>,
        expansion_stmts: &mut Vec<Stmt>,
        context: &str,
    ) -> Result<()> {
        match pat {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let rust_type = ident
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()))
                    .transpose()?;
                // Optional parameter → wrap in Option<T> (avoid double-wrapping)
                let rust_type = if ident.id.optional {
                    rust_type.map(|ty| ty.wrap_optional())
                } else {
                    rust_type
                };
                params.push(Param {
                    name,
                    ty: rust_type,
                });
            }
            ast::Pat::Object(obj_pat) => {
                let (param, stmts) = self.convert_object_destructuring_param(obj_pat)?;
                params.push(param);
                expansion_stmts.extend(stmts);
            }
            ast::Pat::Assign(assign) => {
                // Recursively convert the inner pattern (left side of assignment)
                let mut inner_params = Vec::new();
                let mut inner_stmts = Vec::new();
                self.convert_function_param_pat(
                    &assign.left,
                    &mut inner_params,
                    &mut inner_stmts,
                    context,
                )?;
                let inner_param = inner_params
                    .pop()
                    .ok_or_else(|| anyhow!("default parameter produced no inner param"))?;

                // Wrap with Option<T> + unwrap_or expansion via shared helper
                let (wrapped_param, wrapped_stmts) =
                    self.wrap_param_with_default(inner_param, inner_stmts, &assign.right)?;
                params.push(wrapped_param);
                expansion_stmts.extend(wrapped_stmts);
            }
            ast::Pat::Rest(rest) => {
                if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                    let name = ident.id.sym.to_string();
                    let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                    let rust_type = type_ann
                        .map(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()))
                        .transpose()?;
                    params.push(Param {
                        name,
                        ty: rust_type,
                    });
                } else {
                    return Err(anyhow!("unsupported {context} rest parameter"));
                }
            }
            _ => return Err(anyhow!("unsupported {context} parameter pattern")),
        }
        Ok(())
    }

    /// Converts a function expression to `Expr::Closure`.
    ///
    /// Function expressions (`function(x) { ... }` or `function name(x) { ... }`)
    /// are treated identically to arrow functions — the optional name is ignored.
    pub(crate) fn convert_fn_expr(&mut self, fn_expr: &ast::FnExpr) -> Result<Expr> {
        let func = &fn_expr.function;

        // Convert parameters — reuse the same logic as arrow functions
        let mut params = Vec::new();
        let mut expansion_stmts = Vec::new();
        for param in &func.params {
            self.convert_function_param_pat(
                &param.pat,
                &mut params,
                &mut expansion_stmts,
                "function expression",
            )?;
        }

        let return_type = func
            .return_type
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()))
            .transpose()?;

        // void → None
        let return_type = return_type.and_then(|ty| {
            if matches!(ty, RustType::Unit) {
                None
            } else {
                Some(ty)
            }
        });

        let body = match &func.body {
            Some(block) => {
                let mut sub_t = self.spawn_nested_scope();
                let mut stmts = expansion_stmts;
                for stmt in &block.stmts {
                    stmts.extend(sub_t.convert_stmt(stmt, return_type.as_ref())?);
                }
                convert_last_return_to_tail(&mut stmts);
                ClosureBody::Block(stmts)
            }
            None => ClosureBody::Block(expansion_stmts),
        };

        Ok(Expr::Closure {
            params,
            return_type,
            body,
        })
    }

    /// Converts an arrow expression into an IR [`Expr::Closure`].
    ///
    /// - Expression body: `(x: number) => x + 1` → `|x: f64| x + 1`
    /// - Block body: `(x: number) => { return x + 1; }` → `|x: f64| { x + 1 }`
    ///
    /// When `resilient` is true, unsupported types fall back to [`RustType::Any`] and
    /// the error message is appended to `fallback_warnings`.
    pub(crate) fn convert_arrow_expr(
        &mut self,
        arrow: &ast::ArrowExpr,
        resilient: bool,
        fallback_warnings: &mut Vec<String>,
    ) -> Result<Expr> {
        self.convert_arrow_expr_with_return_type(arrow, resilient, fallback_warnings, None, None)
    }

    /// Inner implementation of arrow expression conversion with optional type overrides.
    ///
    /// `override_return_type` allows callers to inject a return type from an external source
    /// (e.g., variable type annotation `const f: FnType = () => ...`).
    /// `override_param_types` allows callers to inject parameter types from an external source
    /// (e.g., variable type annotation `const f: (x: number) => void = (x) => ...`).
    pub(crate) fn convert_arrow_expr_with_return_type(
        &mut self,
        arrow: &ast::ArrowExpr,
        resilient: bool,
        fallback_warnings: &mut Vec<String>,
        override_return_type: Option<&RustType>,
        override_param_types: Option<&[RustType]>,
    ) -> Result<Expr> {
        // I-383 T7: arrow 関数の generic 型パラメータを scope に push する。
        // arrow_expr は self.synthetic を直接使用するため (fn_decl のような per-function
        // local_synthetic を持たない)、明示的な restore が必須。`?` 経路があっても restore
        // 漏れが起きないよう、本体処理を inner closure で囲み、戻り値取得後に必ず restore する。
        let arrow_tp_names: Vec<String> = arrow
            .type_params
            .as_ref()
            .map(|tpd| tpd.params.iter().map(|p| p.name.sym.to_string()).collect())
            .unwrap_or_default();
        let prev_scope = self.synthetic.push_type_param_scope(arrow_tp_names);
        let result = self.convert_arrow_expr_inner(
            arrow,
            resilient,
            fallback_warnings,
            override_return_type,
            override_param_types,
        );
        self.synthetic.restore_type_param_scope(prev_scope);
        result
    }

    /// Inner implementation. Caller (`convert_arrow_expr_with_return_type`) is responsible
    /// for `push_type_param_scope` / `restore_type_param_scope` lifecycle.
    fn convert_arrow_expr_inner(
        &mut self,
        arrow: &ast::ArrowExpr,
        resilient: bool,
        fallback_warnings: &mut Vec<String>,
        override_return_type: Option<&RustType>,
        override_param_types: Option<&[RustType]>,
    ) -> Result<Expr> {
        let mut params = Vec::new();
        let mut expansion_stmts = Vec::new();
        {
            for (i, param) in arrow.params.iter().enumerate() {
                match param {
                    // Ident needs arrow-specific override_param_types fallback
                    ast::Pat::Ident(ident) => {
                        let name = ident.id.sym.to_string();
                        let rust_type = ident
                            .type_ann
                            .as_ref()
                            .map(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()))
                            .transpose()?;
                        // If no direct annotation, try override from variable type annotation
                        let rust_type = rust_type.or_else(|| {
                            override_param_types.and_then(|types| types.get(i).cloned())
                        });
                        // Optional parameter → wrap in Option<T> (avoid double-wrapping)
                        let rust_type = if ident.id.optional {
                            rust_type.map(|ty| ty.wrap_optional())
                        } else {
                            rust_type
                        };
                        params.push(Param {
                            name,
                            ty: rust_type,
                        });
                    }
                    // Array destructuring is arrow-only
                    ast::Pat::Array(arr_pat) => {
                        let names: Vec<String> = arr_pat
                            .elems
                            .iter()
                            .map(|elem| match elem {
                                Some(ast::Pat::Ident(ident)) => Ok(ident.id.sym.to_string()),
                                Some(_) => {
                                    Err(anyhow!("unsupported arrow array destructuring element"))
                                }
                                None => Ok("_".to_string()),
                            })
                            .collect::<Result<_>>()?;
                        let tuple_name = format!("({})", names.join(", "));
                        let rust_type = arr_pat
                            .type_ann
                            .as_ref()
                            .map(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()))
                            .transpose()?;
                        params.push(Param {
                            name: tuple_name,
                            ty: rust_type,
                        });
                    }
                    // Object, Assign, Rest, and catch-all are shared with fn_expr
                    other => self.convert_function_param_pat(
                        other,
                        &mut params,
                        &mut expansion_stmts,
                        "arrow",
                    )?,
                }
            }
        }

        // Arrow's explicit return type annotation takes priority;
        // fall back to override_return_type from variable type annotation
        let return_type = arrow
            .return_type
            .as_ref()
            .map(|ann| {
                self.convert_ts_type_with_fallback(&ann.type_ann, resilient, fallback_warnings)
            })
            .transpose()?
            .or_else(|| override_return_type.cloned());

        let body = if expansion_stmts.is_empty() {
            match arrow.body.as_ref() {
                ast::BlockStmtOrExpr::Expr(expr) => {
                    let ir_expr = self.convert_expr(expr)?;
                    ClosureBody::Expr(Box::new(ir_expr))
                }
                ast::BlockStmtOrExpr::BlockStmt(block) => {
                    let mut sub_t = self.spawn_nested_scope();
                    let mut stmts = Vec::new();
                    for stmt in &block.stmts {
                        stmts.extend(sub_t.convert_stmt(stmt, return_type.as_ref())?);
                    }
                    convert_last_return_to_tail(&mut stmts);
                    ClosureBody::Block(stmts)
                }
            }
        } else {
            // When we have expansion stmts, the body must be a Block
            let mut body_stmts = expansion_stmts;
            match arrow.body.as_ref() {
                ast::BlockStmtOrExpr::Expr(expr) => {
                    let ir_expr = self.convert_expr(expr)?;
                    body_stmts.push(Stmt::Return(Some(ir_expr)));
                }
                ast::BlockStmtOrExpr::BlockStmt(block) => {
                    let mut sub_t = self.spawn_nested_scope();
                    for stmt in &block.stmts {
                        body_stmts.extend(sub_t.convert_stmt(stmt, return_type.as_ref())?);
                    }
                    convert_last_return_to_tail(&mut body_stmts);
                }
            }
            ClosureBody::Block(body_stmts)
        };

        Ok(Expr::Closure {
            params,
            return_type,
            body,
        })
    }
}
