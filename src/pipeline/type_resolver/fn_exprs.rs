//! Function/arrow expression and array literal type resolution for TypeResolver.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::visitors::extract_type_ann_from_pat;
use super::*;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
use crate::transformer::type_position::{wrap_trait_for_position, TypePosition};

/// 関数式解決で使うスコープ状態。enter 時に生成し、cleanup 時に消費する。
struct FnScopeState {
    prev_type_param_state: Option<(HashMap<String, RustType>, Vec<String>)>,
    prev_return_type: Option<RustType>,
    expected_free_var_scope: Option<Vec<String>>,
}

impl<'a> TypeResolver<'a> {
    // ── scope lifecycle helpers ──

    /// Type param constraints を親スコープの constraints にマージして登録する。
    /// 復元用の状態 (旧 constraints, 旧 type_param_scope) を返す。
    ///
    /// `visit_fn_decl`, `resolve_arrow_expr`, `resolve_fn_expr`,
    /// class method 等で共通使用。
    pub(super) fn push_type_param_constraints(
        &mut self,
        type_params: Option<&ast::TsTypeParamDecl>,
    ) -> Option<(HashMap<String, RustType>, Vec<String>)> {
        type_params.map(|tp| {
            let (inner_constraints, prev_scope) =
                enter_type_param_scope(tp, self.synthetic, self.registry);
            let mut merged = self.type_param_constraints.clone();
            merged.extend(inner_constraints);
            let prev_constraints = std::mem::replace(&mut self.type_param_constraints, merged);
            (prev_constraints, prev_scope)
        })
    }

    /// `push_type_param_constraints` で保存した状態を復元する。
    pub(super) fn restore_type_param_constraints(
        &mut self,
        prev_state: Option<(HashMap<String, RustType>, Vec<String>)>,
    ) {
        if let Some((prev_constraints, prev_scope)) = prev_state {
            self.type_param_constraints = prev_constraints;
            self.synthetic.restore_type_param_scope(prev_scope);
        }
    }

    /// Return type annotation を解決し current_fn_return_type に設定する。
    /// 以前の return type を返す (復元用)。
    pub(super) fn setup_fn_return_type(
        &mut self,
        return_ann: Option<&ast::TsTypeAnn>,
    ) -> Option<RustType> {
        let prev = self.current_fn_return_type.take();
        if let Some(ann) = return_ann {
            if let Ok(ty) = convert_ts_type(&ann.type_ann, self.synthetic, self.registry) {
                let unwrapped = ty.unwrap_promise();
                self.current_fn_return_type = if matches!(unwrapped, RustType::Unit) {
                    None
                } else {
                    Some(wrap_trait_for_position(
                        unwrapped,
                        TypePosition::Value,
                        self.registry,
                    ))
                };
            }
        }
        prev
    }

    /// Expected type から free type var scope を push し、return type / param types を解決する。
    ///
    /// Return type annotation が既に設定済みの場合は何もしない。
    /// Arrow では `expected_param_types` を使い、FnExpr では捨てる。
    fn resolve_expected_fn_info(
        &mut self,
        fn_span: Span,
    ) -> (Option<Vec<String>>, Option<Vec<RustType>>) {
        if self.current_fn_return_type.is_some() {
            return (None, None);
        }
        let Some(expected) = self.result.expected_types.get(&fn_span).cloned() else {
            return (None, None);
        };
        // I-387: TypeVar walker で free type var を構造的に収集。
        let mut free_vars = Vec::new();
        collect_type_vars(&expected, &mut free_vars);
        // 既に type_param_constraints に存在する type var は除外
        free_vars.retain(|name| !self.type_param_constraints.contains_key(name));
        let free_var_scope = if !free_vars.is_empty() {
            Some(self.synthetic.push_type_param_scope(free_vars))
        } else {
            None
        };

        let (ret, params) = resolve_fn_type_info(&expected, self.registry, self.synthetic);
        if let Some(ret_ty) = ret {
            let unwrapped = ret_ty.unwrap_promise();
            self.current_fn_return_type = if matches!(unwrapped, RustType::Unit) {
                None
            } else {
                Some(unwrapped)
            };
        }
        (free_var_scope, params)
    }

    /// 関数式 scope のクリーンアップ。解決された return type を返す。
    fn cleanup_fn_scope(&mut self, state: FnScopeState) -> RustType {
        let return_type = self.current_fn_return_type.take().unwrap_or(RustType::Unit);
        self.current_fn_return_type = state.prev_return_type;
        if let Some(prev) = state.expected_free_var_scope {
            self.synthetic.restore_type_param_scope(prev);
        }
        self.restore_type_param_constraints(state.prev_type_param_state);
        self.leave_scope();
        return_type
    }

    // ── expression resolvers ──

    /// Resolves an arrow function expression, walking its body.
    pub(super) fn resolve_arrow_expr(&mut self, arrow: &ast::ArrowExpr) -> ResolvedType {
        self.enter_scope();

        // I-383 T2.A-ii: enter_type_param_scope also pushes the param names into
        // `synthetic.type_param_scope` so that synthetic union/struct types registered
        // while walking the arrow body inherit the correct scope.
        let prev_type_param_state = self.push_type_param_constraints(arrow.type_params.as_deref());
        let prev_return_type = self.setup_fn_return_type(arrow.return_type.as_deref());

        // If no explicit return annotation, check expected type from parent context
        // (e.g., variable type annotation: `const f: FnType = () => ...`)
        // I-383 T2.A-iv: also extract free type variables from the expected type
        // and push them into `synthetic.type_param_scope` for the body resolution.
        // When the expected type is a `RustType::Fn` flattened from a generic
        // interface call signature (e.g., `SSGParamsMiddleware: <E extends Env>(...)`),
        // the `<E>` binding has been lost and `E` appears as a free `Named` ref in
        // the Fn. Without this push, synthetic union/struct registrations during
        // body resolution would leak `E` as a dangling external ref.
        let (expected_free_var_scope, expected_param_types) =
            self.resolve_expected_fn_info(Span::from_swc(arrow.span));

        // Register parameters
        let mut param_types = Vec::new();
        for (i, param) in arrow.params.iter().enumerate() {
            match param {
                ast::Pat::Ident(ident) => {
                    let name = ident.id.sym.to_string();
                    let span = Span::from_swc(ident.id.span);
                    // Arrow's own annotation takes priority; fall back to expected param type
                    let ty = ident
                        .type_ann
                        .as_ref()
                        .and_then(|ann| {
                            convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                        })
                        .or_else(|| {
                            expected_param_types
                                .as_ref()
                                .and_then(|types| types.get(i).cloned())
                        })
                        .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry));
                    let resolved = ty
                        .clone()
                        .map(ResolvedType::Known)
                        .unwrap_or(ResolvedType::Unknown);
                    self.declare_var(&name, resolved, span, false);
                    param_types.push(ty.unwrap_or(RustType::Any));
                }
                // Destructuring / default value patterns: extract type from annotation
                _ => {
                    let ty: Option<RustType> = extract_type_ann_from_pat(param).and_then(|ann| {
                        convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                    });
                    let ty = ty
                        .or_else(|| {
                            expected_param_types
                                .as_ref()
                                .and_then(|types| types.get(i).cloned())
                        })
                        .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry));
                    // Register sub-pattern variables and resolve default values
                    self.visit_param_pat(param);
                    param_types.push(ty.unwrap_or(RustType::Any));
                }
            }
        }

        // Walk body
        match &*arrow.body {
            ast::BlockStmtOrExpr::BlockStmt(block) => {
                self.collect_emission_hints(block);
                for stmt in &block.stmts {
                    self.visit_stmt(stmt);
                }
            }
            ast::BlockStmtOrExpr::Expr(expr) => {
                // Propagate return type as expected type to expression body
                if let Some(return_ty) = self.current_fn_return_type.clone() {
                    let span = Span::from_swc(expr.span());
                    self.result.expected_types.insert(span, return_ty.clone());
                    self.propagate_expected(expr, &return_ty);
                }
                let span = Span::from_swc(expr.span());
                let ty = self.resolve_expr(expr);
                self.result.expr_types.insert(span, ty);
            }
        }

        let return_type = self.cleanup_fn_scope(FnScopeState {
            prev_type_param_state,
            prev_return_type,
            expected_free_var_scope,
        });
        ResolvedType::Known(RustType::Fn {
            params: param_types,
            return_type: Box::new(return_type),
        })
    }

    /// Resolves a function expression, walking its body.
    pub(super) fn resolve_fn_expr(&mut self, fn_expr: &ast::FnExpr) -> ResolvedType {
        self.enter_scope();

        // I-383 T2.A-ii: also pushes the param names into `synthetic.type_param_scope`.
        let prev_type_param_state =
            self.push_type_param_constraints(fn_expr.function.type_params.as_deref());
        let prev_return_type = self.setup_fn_return_type(fn_expr.function.return_type.as_deref());

        // I-383 T2.A-iv: extract free type variables from expected type and push them
        // (same rationale as resolve_arrow_expr).
        let (expected_free_var_scope, _) =
            self.resolve_expected_fn_info(Span::from_swc(fn_expr.function.span));

        // Register parameters and collect their types
        let mut param_types = Vec::new();
        for param in &fn_expr.function.params {
            let ty = extract_type_ann_from_pat(&param.pat)
                .and_then(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok())
                .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry));
            param_types.push(ty.unwrap_or(RustType::Any));
            self.visit_param_pat(&param.pat);
        }

        if let Some(body) = &fn_expr.function.body {
            self.collect_emission_hints(body);
            for stmt in &body.stmts {
                self.visit_stmt(stmt);
            }
        }

        let return_type = self.cleanup_fn_scope(FnScopeState {
            prev_type_param_state,
            prev_return_type,
            expected_free_var_scope,
        });
        ResolvedType::Known(RustType::Fn {
            params: param_types,
            return_type: Box::new(return_type),
        })
    }

    /// Resolves an array literal expression.
    pub(super) fn resolve_array_expr(&mut self, arr: &ast::ArrayLit) -> ResolvedType {
        if arr.elems.is_empty() {
            return ResolvedType::Unknown;
        }

        let mut element_type: Option<RustType> = None;
        let mut all_same = true;

        for elem in arr.elems.iter().flatten() {
            let span = Span::from_swc(elem.expr.span());
            let ty = self.resolve_expr(&elem.expr);
            self.result.expr_types.insert(span, ty.clone());

            if let ResolvedType::Known(rust_ty) = &ty {
                match &element_type {
                    None => element_type = Some(rust_ty.clone()),
                    Some(existing) if existing != rust_ty => all_same = false,
                    _ => {}
                }
            }
        }

        if all_same {
            if let Some(elem_ty) = element_type {
                return ResolvedType::Known(RustType::Vec(Box::new(elem_ty)));
            }
        }

        ResolvedType::Unknown
    }
}
