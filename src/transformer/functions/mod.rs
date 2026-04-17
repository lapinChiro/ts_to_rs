//! Function declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC function declarations into the IR [`Item::Fn`] representation.

mod arrow_fns;
mod destructuring;
mod helpers;
mod params;

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, MatchArm, Method, Param, RustType, Stmt, Visibility};
use crate::pipeline::type_converter::{convert_ts_type, extract_type_params};
use crate::pipeline::SyntheticTypeRegistry;
use crate::transformer::{
    extract_pat_ident_name, extract_prop_name, wrap_trait_for_position, Transformer, TypePosition,
    UnsupportedSyntaxError,
};

pub(crate) use helpers::convert_last_return_to_tail;
use helpers::{append_implicit_none_if_needed, wrap_closures_in_box};

/// Converts a snake_case name to PascalCase.
///
/// Example: `"foo_opts"` → `"FooOpts"`, `"bar_config"` → `"BarConfig"`
use crate::pipeline::any_narrowing::to_pascal_case;

use helpers::{contains_throw, mark_mut_params_from_body, pascal_to_snake, wrap_returns_in_ok};

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

        // I-383 T7: 関数の generic 型パラメータを SyntheticTypeRegistry の scope に push する。
        // これにより convert_param / return_type / body の型解決中に anonymous union/struct/
        // intersection enum が type_param を保持した generic 形で生成される。
        //
        // local_synthetic は per-function ローカル変数で、関数終了時に drop されるため、
        // restore 呼び出しは不要 (scope の lifetime は関数 body と一致)。outer scope への
        // 漏れは構造的に発生しない (outer の self.synthetic は local とは別 instance)。
        // merge() は scope を引き継がない (`other` は drop される) ことも確認済み。
        let fn_tp_names: Vec<String> = fn_decl
            .function
            .type_params
            .as_ref()
            .map(|tpd| tpd.params.iter().map(|p| p.name.sym.to_string()).collect())
            .unwrap_or_default();
        let _prev_scope = local_synthetic.push_type_param_scope(fn_tp_names);

        let mut params = Vec::new();
        let mut destructuring_stmts = Vec::new();
        let return_type = {
            let mut sub = self.spawn_nested_scope_with_local_synthetic(&mut local_synthetic);

            for param in &fn_decl.function.params {
                let (p, stmts, extra) =
                    sub.convert_param(&param.pat, &name, vis, resilient, &mut fallback_warnings)?;
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
            return_type.map(|ty| ty.unwrap_promise())
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
            Some(block) => self
                .spawn_nested_scope_with_local_synthetic(&mut local_synthetic)
                .convert_stmt_list(&block.stmts, return_type.as_ref())?,
            None => Vec::new(),
        };
        // Prepend destructuring expansion statements
        let mut body = if destructuring_stmts.is_empty() {
            body_stmts
        } else {
            let mut combined = destructuring_stmts;
            combined.extend(body_stmts);
            combined
        };

        let (type_params, mono_subs) = extract_type_params(
            fn_decl.function.type_params.as_deref(),
            &mut local_synthetic,
            self.reg(),
        );
        // mono_subs は最終的に Item::substitute で一括適用する（params, return_type, body 全体）

        // Union return wrapping: must happen BEFORE has_throw wrapping.
        // has_throw changes return_type to Result<T, String>, hiding the union type T.
        // wrap_returns_in_ok only handles Stmt::Return (not TailExpr), so union wrap
        // must also happen before convert_last_return_to_tail.
        if let Some(RustType::Named { ref name, .. }) = return_type {
            if let Some(wrap_ctx) = try_build_union_return_wrap_context(name, &local_synthetic) {
                if let Some(block) = &fn_decl.function.body {
                    let mut leaf_types = Vec::new();
                    crate::transformer::return_wrap::collect_stmts_return_leaf_types(
                        &block.stmts,
                        self.tctx.type_resolution,
                        &mut leaf_types,
                    );
                    arrow_fns::wrap_body_returns(
                        &mut body,
                        &mut leaf_types.into_iter(),
                        &wrap_ctx,
                    )?;
                }
            }
        }

        // If the function body contains `throw`, wrap return type in Result and returns in Ok()
        let has_throw = fn_decl
            .function
            .body
            .as_ref()
            .is_some_and(|block| contains_throw(&block.stmts));

        let (return_type, mut body) = if has_throw {
            // I-387: `()` は `RustType::Unit`
            let ok_type = return_type.unwrap_or(RustType::Unit);
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

        // I-025: append implicit `None` when return type is Option<T> and body
        // ends with a control-flow statement that may fall through without returning.
        append_implicit_none_if_needed(&mut body, return_type.as_ref());

        // I-020: wrap closures in Box::new(...) when return type is Fn / Box<dyn Fn(...)>.
        // Walks entire body recursively (return + tail + nested blocks).
        body = wrap_closures_in_box(body, return_type.as_ref());

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

        let item = Item::Fn {
            vis,
            attributes,
            is_async,
            name,
            type_params,
            params,
            return_type,
            body,
        };
        items.push(if mono_subs.is_empty() {
            item
        } else {
            item.substitute(&mono_subs)
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

/// Tries to build a `ReturnWrapContext` for a synthetic union enum.
///
/// Returns `Some(ctx)` if `name` is a union enum registered in `synthetic`,
/// `None` otherwise.
fn try_build_union_return_wrap_context(
    name: &str,
    synthetic: &SyntheticTypeRegistry,
) -> Option<crate::transformer::return_wrap::ReturnWrapContext> {
    use crate::pipeline::synthetic_registry::SyntheticTypeKind;
    let def = synthetic.get(name)?;
    if def.kind != SyntheticTypeKind::UnionEnum {
        return None;
    }
    if let Item::Enum { variants, .. } = &def.item {
        Some(crate::transformer::return_wrap::build_return_wrap_context_from_enum(name, variants))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
