//! Function declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC function declarations into the IR [`Item::Fn`] representation.

mod arrow_fns;
mod destructuring;
mod helpers;
mod params;

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, MatchArm, Param, RustType, Stmt, Visibility};
use crate::pipeline::type_converter::{
    convert_property_signature, convert_ts_type, extract_type_params,
};
use crate::pipeline::SyntheticTypeRegistry;
use crate::transformer::{
    extract_pat_ident_name, extract_prop_name, wrap_trait_for_position, Transformer, TypePosition,
    UnsupportedSyntaxError,
};

pub(crate) use helpers::convert_last_return_to_tail;

/// Converts a snake_case name to PascalCase.
///
/// Example: `"foo_opts"` → `"FooOpts"`, `"bar_config"` → `"BarConfig"`
use crate::pipeline::any_narrowing::to_pascal_case;

use helpers::{
    contains_throw, mark_mut_params_from_body, pascal_to_snake, unwrap_promise_type,
    wrap_returns_in_ok,
};

impl<'a> Transformer<'a> {
    /// Converts an SWC [`ast::FnDecl`] into an IR [`Item::Fn`].
    ///
    /// Extracts the function name, parameters (with type annotations),
    /// return type, and body statements.
    ///
    /// # Errors
    ///
    /// Returns an error if parameter patterns are unsupported, type annotations
    /// are missing, or body statements fail to convert.
    pub(crate) fn convert_fn_decl(
        &mut self,
        fn_decl: &ast::FnDecl,
        vis: Visibility,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        let name = fn_decl.ident.sym.to_string();
        let mut fallback_warnings = Vec::new();
        let mut items = Vec::new();
        // Per-function synthetic registry: isolates types from failed conversions.
        // Only merged into self.synthetic on success (end of function).
        let mut local_synthetic = SyntheticTypeRegistry::new();

        let mut params = Vec::new();
        let mut destructuring_stmts = Vec::new();
        let return_type = {
            let mut sub = Transformer {
                tctx: self.tctx,
                synthetic: &mut local_synthetic,
                mut_method_names: self.mut_method_names.clone(),
            };

            for param in &fn_decl.function.params {
                let (p, stmts, extra) = sub.convert_param(
                    &param.pat,
                    &name,
                    vis.clone(),
                    resilient,
                    &mut fallback_warnings,
                )?;
                params.push(p);
                destructuring_stmts.extend(stmts);
                items.extend(extra);
            }

            fn_decl
                .function
                .return_type
                .as_ref()
                .map(|ann| {
                    sub.convert_ts_type_with_fallback(
                        &ann.type_ann,
                        resilient,
                        &mut fallback_warnings,
                    )
                })
                .transpose()?
        };

        let is_async = fn_decl.function.is_async;

        // void → None (Rust omits `-> ()`)
        let return_type = return_type.and_then(|ty| {
            if matches!(ty, RustType::Unit) {
                None
            } else {
                Some(ty)
            }
        });

        // Unwrap Promise<T> → T for async functions (before body conversion
        // so that return type context propagates correctly)
        let return_type = if is_async {
            return_type.and_then(unwrap_promise_type)
        } else {
            return_type
        };

        // Trait types in return position → Box<dyn Trait>
        let return_type =
            return_type.map(|ty| wrap_trait_for_position(ty, TypePosition::Value, self.reg()));

        // Override any-typed parameters with enum types from FileTypeResolution
        // (computed by pipeline's any_enum_analyzer before transformation)
        {
            let fn_start = fn_decl.function.span.lo.0;
            for p in &mut params {
                if matches!(&p.ty, Some(RustType::Any)) {
                    if let Some(enum_type) = self
                        .tctx
                        .type_resolution
                        .any_enum_override(&p.name, fn_start)
                    {
                        p.ty = Some(enum_type.clone());
                    }
                }
            }
        }

        // Sub-Transformer for function body: uses local SyntheticTypeRegistry.
        // TypeResolver + FileTypeResolution handle all type tracking.
        let body_stmts = match &fn_decl.function.body {
            Some(block) => Transformer {
                tctx: self.tctx,
                synthetic: &mut local_synthetic,
                mut_method_names: self.mut_method_names.clone(),
            }
            .convert_stmt_list(&block.stmts, return_type.as_ref())?,
            None => Vec::new(),
        };
        // Prepend destructuring expansion statements
        let body = if destructuring_stmts.is_empty() {
            body_stmts
        } else {
            let mut combined = destructuring_stmts;
            combined.extend(body_stmts);
            combined
        };

        let type_params = extract_type_params(
            fn_decl.function.type_params.as_deref(),
            &mut local_synthetic,
            self.reg(),
        );

        // If the function body contains `throw`, wrap return type in Result and returns in Ok()
        let has_throw = fn_decl
            .function
            .body
            .as_ref()
            .is_some_and(|block| contains_throw(&block.stmts));

        let (return_type, mut body) = if has_throw {
            let ok_type = return_type.unwrap_or_else(|| RustType::Named {
                name: "()".to_string(),
                type_args: vec![],
            });
            let result_type = RustType::Result {
                ok: Box::new(ok_type),
                err: Box::new(RustType::String),
            };
            let wrapped_body = wrap_returns_in_ok(body);
            (Some(result_type), wrapped_body)
        } else {
            (return_type, body)
        };

        convert_last_return_to_tail(&mut body);
        let mut_rebindings = mark_mut_params_from_body(&body, &params, &self.mut_method_names);
        if !mut_rebindings.is_empty() {
            let mut new_body = mut_rebindings;
            new_body.extend(body);
            body = new_body;
        }

        let attributes = if is_async && name == "main" {
            vec!["tokio::main".to_string()]
        } else {
            vec![]
        };

        // Merge local synthetic types into the outer registry (only on success)
        self.synthetic.merge(local_synthetic);

        items.push(Item::Fn {
            vis,
            attributes,
            is_async,
            name,
            type_params,
            params,
            return_type,
            body,
        });

        Ok((items, fallback_warnings))
    }

    /// Converts a TypeScript type to an IR type, falling back to [`RustType::Any`] when
    /// `resilient` is true and the type is unsupported.
    ///
    /// When falling back, appends the error message to `fallback_warnings` for reporting.
    pub(crate) fn convert_ts_type_with_fallback(
        &mut self,
        ts_type: &swc_ecma_ast::TsType,
        resilient: bool,
        fallback_warnings: &mut Vec<String>,
    ) -> Result<RustType> {
        match convert_ts_type(ts_type, self.synthetic, self.reg()) {
            Ok(ty) => Ok(ty),
            Err(e) => {
                if resilient {
                    fallback_warnings.push(e.to_string());
                    Ok(RustType::Any)
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
