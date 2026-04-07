//! Spread array detection and expansion at SWC AST level.
//!
//! Detects `[...arr, 1]` patterns in variable declarations, return statements,
//! and expression statements, expanding them into `Vec::new()` + `push`/`extend` sequences.

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{Expr, Stmt};
use crate::pipeline::type_converter::convert_ts_type;
use crate::transformer::{extract_pat_ident_name, Transformer};

/// Returns true if an SWC ArrayLit contains spread elements.
fn has_spread_elements(array_lit: &ast::ArrayLit) -> bool {
    array_lit
        .elems
        .iter()
        .filter_map(|e| e.as_ref())
        .any(|e| e.spread.is_some())
}

/// Extracts the initializer array literal from a VarDecl if it is a spread array.
fn extract_spread_array_init(var_decl: &ast::VarDecl) -> Option<(&ast::Pat, &ast::ArrayLit)> {
    let declarator = var_decl.decls.first()?;
    let init = declarator.init.as_ref()?;
    let array_lit = match init.as_ref() {
        ast::Expr::Array(a) => a,
        _ => return None,
    };
    if has_spread_elements(array_lit) {
        Some((&declarator.name, array_lit))
    } else {
        None
    }
}

/// Generates push/extend statements from spread segments for a given variable name.
fn emit_spread_ops(var_name: &str, segments: &[(bool, Expr)], result: &mut Vec<Stmt>) {
    for (is_spread, expr) in segments {
        if *is_spread {
            result.push(Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident(var_name.to_string())),
                method: "extend".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(expr.clone()),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }],
            }));
        } else {
            result.push(Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident(var_name.to_string())),
                method: "push".to_string(),
                args: vec![expr.clone()],
            }));
        }
    }
}

impl<'a> Transformer<'a> {
    /// Converts spread array elements to IR expressions.
    fn convert_spread_segments(&mut self, array_lit: &ast::ArrayLit) -> Result<Vec<(bool, Expr)>> {
        array_lit
            .elems
            .iter()
            .filter_map(|e| e.as_ref())
            .map(|elem| {
                let expr = self.convert_expr(&elem.expr)?;
                Ok((elem.spread.is_some(), expr))
            })
            .collect()
    }

    /// Detects `let x = [...arr, 1]` and expands to IR statements.
    pub(super) fn try_expand_spread_var_decl(
        &mut self,
        var_decl: &ast::VarDecl,
    ) -> Result<Option<Vec<Stmt>>> {
        let (pat, array_lit) = match extract_spread_array_init(var_decl) {
            Some(v) => v,
            None => return Ok(None),
        };
        let name = extract_pat_ident_name(pat)?;
        let ty = match pat {
            ast::Pat::Ident(ident) => {
                if let Some(ann) = ident.type_ann.as_ref() {
                    Some(convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?)
                } else {
                    None
                }
            }
            _ => None,
        };

        let segments = self.convert_spread_segments(array_lit)?;

        if segments.len() == 1 && segments[0].0 {
            return Ok(Some(vec![Stmt::Let {
                mutable: false,
                name,
                ty,
                init: Some(Expr::MethodCall {
                    object: Box::new(segments[0].1.clone()),
                    method: "clone".to_string(),
                    args: vec![],
                }),
            }]));
        }

        let mut result = Vec::new();
        result.push(Stmt::Let {
            mutable: true,
            name: name.clone(),
            ty,
            init: Some(Expr::FnCall {
                // `Vec::new()` is a std call, not a user type reference.
                target: crate::ir::CallTarget::ExternalPath(vec![
                    "Vec".to_string(),
                    "new".to_string(),
                ]),
                args: vec![],
            }),
        });
        emit_spread_ops(&name, &segments, &mut result);
        Ok(Some(result))
    }

    /// Detects `return [...arr, 1]` and expands to IR statements.
    pub(super) fn try_expand_spread_return(
        &mut self,
        ret: &ast::ReturnStmt,
    ) -> Result<Option<Vec<Stmt>>> {
        let arg = match &ret.arg {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let array_lit = match arg.as_ref() {
            ast::Expr::Array(a) if has_spread_elements(a) => a,
            _ => return Ok(None),
        };

        let segments = self.convert_spread_segments(array_lit)?;

        if segments.len() == 1 && segments[0].0 {
            return Ok(Some(vec![Stmt::Return(Some(Expr::MethodCall {
                object: Box::new(segments[0].1.clone()),
                method: "clone".to_string(),
                args: vec![],
            }))]));
        }

        let var_name = "__spread_vec".to_string();
        let mut result = Vec::new();
        result.push(Stmt::Let {
            mutable: true,
            name: var_name.clone(),
            ty: None,
            init: Some(Expr::FnCall {
                // `Vec::new()` is a std call, not a user type reference.
                target: crate::ir::CallTarget::ExternalPath(vec![
                    "Vec".to_string(),
                    "new".to_string(),
                ]),
                args: vec![],
            }),
        });
        emit_spread_ops(&var_name, &segments, &mut result);
        result.push(Stmt::Return(Some(Expr::Ident(var_name))));
        Ok(Some(result))
    }

    /// Detects `[...arr, 1]` as a bare expression statement and expands.
    pub(super) fn try_expand_spread_expr_stmt(
        &mut self,
        expr_stmt: &ast::ExprStmt,
    ) -> Result<Option<Vec<Stmt>>> {
        let array_lit = match expr_stmt.expr.as_ref() {
            ast::Expr::Array(a) if has_spread_elements(a) => a,
            _ => return Ok(None),
        };

        let segments = self.convert_spread_segments(array_lit)?;

        if segments.len() == 1 && segments[0].0 {
            return Ok(Some(vec![Stmt::Expr(Expr::MethodCall {
                object: Box::new(segments[0].1.clone()),
                method: "clone".to_string(),
                args: vec![],
            })]));
        }

        let var_name = "__spread_vec".to_string();
        let mut result = Vec::new();
        result.push(Stmt::Let {
            mutable: true,
            name: var_name.clone(),
            ty: None,
            init: Some(Expr::FnCall {
                // `Vec::new()` is a std call, not a user type reference.
                target: crate::ir::CallTarget::ExternalPath(vec![
                    "Vec".to_string(),
                    "new".to_string(),
                ]),
                args: vec![],
            }),
        });
        emit_spread_ops(&var_name, &segments, &mut result);
        Ok(Some(result))
    }
}
