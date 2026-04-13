//! Function/method call resolution for TypeResolver.
//!
//! Resolves return types and propagates expected argument types for:
//! - Direct function calls (`foo(args)`)
//! - Method calls (`obj.method(args)`)
//! - Overloaded method resolution

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_resolution::Span;
use crate::registry::select_overload;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_call_expr(&mut self, call: &ast::CallExpr) -> ResolvedType {
        let callee = match &call.callee {
            ast::Callee::Expr(expr) => expr.as_ref(),
            _ => return ResolvedType::Unknown,
        };

        // Convert explicit type arguments: `fn<string>()` → [String]
        let explicit_type_args: Vec<RustType> = call
            .type_args
            .as_ref()
            .map(|ta| convert_explicit_type_args(ta, self.synthetic, self.registry))
            .unwrap_or_default();

        // Temporarily bind explicit type args to callee's type parameters
        let prev_constraints = self.push_call_type_arg_bindings(callee, &explicit_type_args);

        // Set expected types for arguments based on callee's parameter types
        self.set_call_arg_expected_types(callee, &call.args);

        // Resolve the callee to determine return type
        let result = match callee {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();
                // Check scope for Fn type or Named function type alias
                match self.lookup_var(&fn_name) {
                    ResolvedType::Known(RustType::Fn { return_type, .. }) => {
                        let resolved = self.resolve_type_params_in_type(&return_type);
                        ResolvedType::Known(resolved)
                    }
                    ResolvedType::Known(
                        ref var_ty @ RustType::Named {
                            ref name,
                            ref type_args,
                        },
                    ) => {
                        // Callable interface: select the matching overload for
                        // accurate return type (widest would return a synthetic union).
                        if let Some(sig) = select_callable_overload(
                            self.registry,
                            name,
                            type_args,
                            call.args.len(),
                        ) {
                            sig.return_type
                                .map(|ty| {
                                    ResolvedType::Known(self.resolve_type_params_in_type(&ty))
                                })
                                .unwrap_or(ResolvedType::Unknown)
                        } else {
                            // Fallback for non-callable Named types (function type alias, etc.)
                            let (ret, _) =
                                resolve_fn_type_info(var_ty, self.registry, self.synthetic);
                            ret.map(|ty| ResolvedType::Known(self.resolve_type_params_in_type(&ty)))
                                .unwrap_or(ResolvedType::Unknown)
                        }
                    }
                    _ => {
                        // Fall back to TypeRegistry
                        if let Some(TypeDef::Function { return_type, .. }) =
                            self.registry.get(&fn_name)
                        {
                            let ty = return_type.clone().unwrap_or(RustType::Unit);
                            ResolvedType::Known(self.resolve_type_params_in_type(&ty))
                        } else {
                            ResolvedType::Unknown
                        }
                    }
                }
            }
            ast::Expr::Member(member) => {
                let obj_type = self.resolve_expr(&member.obj);
                let obj_rust_type = match &obj_type {
                    ResolvedType::Known(ty) => ty,
                    ResolvedType::Unknown => {
                        for arg in &call.args {
                            self.resolve_expr(&arg.expr);
                        }
                        return ResolvedType::Unknown;
                    }
                };
                let method_name = match &member.prop {
                    ast::MemberProp::Ident(ident) => ident.sym.to_string(),
                    _ => {
                        for arg in &call.args {
                            self.resolve_expr(&arg.expr);
                        }
                        return ResolvedType::Unknown;
                    }
                };
                // Resolve arguments BEFORE collecting their types for overload resolution.
                // set_call_arg_expected_types (called above) has already set expected
                // types on args, so resolve_expr will use them. Then collect_resolved_arg_types
                // can provide actual types for select_overload Stage 3.
                for arg in &call.args {
                    self.resolve_expr(&arg.expr);
                }
                let arg_types = self.collect_resolved_arg_types(&call.args);
                self.resolve_method_return_type(
                    obj_rust_type,
                    &method_name,
                    call.args.len(),
                    &arg_types,
                )
            }
            _ => {
                // Non-Ident/non-Member callees (e.g., IIFE: ((...) => expr)())
                // Resolve the callee expression to walk arrow/fn bodies and determine return type.
                let callee_type = self.resolve_expr(callee);
                if let ResolvedType::Known(RustType::Fn { return_type, .. }) = callee_type {
                    ResolvedType::Known(return_type.as_ref().clone())
                } else {
                    ResolvedType::Unknown
                }
            }
        };

        // Resolve argument expressions (Member callee already resolves above;
        // resolve_expr is idempotent for non-arrow/fn exprs, but we skip to
        // avoid re-visiting arrow/fn bodies).
        if !matches!(callee, ast::Expr::Member(_)) {
            for arg in &call.args {
                self.resolve_expr(&arg.expr);
            }
        }

        // Infer type arguments from resolved argument types and feed back
        // inferred bindings as expected types to arguments (2nd pass).
        let result = if explicit_type_args.is_empty() {
            self.infer_type_args_and_feedback(callee, &call.args, result)
        } else {
            result
        };

        // Restore type_param_constraints
        if let Some(prev) = prev_constraints {
            self.type_param_constraints = prev;
        }

        result
    }

    /// Sets expected types for function call arguments based on the callee's parameter types.
    pub(super) fn set_call_arg_expected_types(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::ExprOrSpread],
    ) {
        // Resolve param types and has_rest flag
        let param_info: Option<(Vec<RustType>, bool)> = match callee {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();
                // Check TypeRegistry for function parameter types
                if let Some(TypeDef::Function {
                    params, has_rest, ..
                }) = self.registry.get(&fn_name)
                {
                    Some((params.iter().map(|p| p.ty.clone()).collect(), *has_rest))
                } else if let ResolvedType::Known(ref var_ty) = self.lookup_var(&fn_name) {
                    match var_ty {
                        RustType::Fn { params, .. } => {
                            // Scope lookup for Fn type variables (no rest info available)
                            Some((params.clone(), false))
                        }
                        RustType::Named {
                            name, type_args, ..
                        } => {
                            // Callable interface: select the matching overload for
                            // accurate expected types (widest signature has Option-wrapped
                            // optional params which would cause incorrect Some() wrapping).
                            if let Some(sig) =
                                select_callable_overload(self.registry, name, type_args, args.len())
                            {
                                Some((
                                    sig.params.iter().map(|p| p.ty.clone()).collect(),
                                    sig.has_rest,
                                ))
                            } else {
                                // Fallback for non-callable Named types (function type alias, etc.)
                                let (ret, params) =
                                    resolve_fn_type_info(var_ty, self.registry, self.synthetic);
                                let _ = ret;
                                params.map(|p| (p, false))
                            }
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            ast::Expr::Member(member) => {
                // Method call: look up method signature from object type
                let obj_type = self.resolve_expr(&member.obj);
                if let ResolvedType::Known(ref rust_ty) = obj_type {
                    // Unwrap Option<T> → T for method lookup (optional chaining)
                    let inner_ty = match rust_ty {
                        RustType::Option(inner) => inner.as_ref(),
                        other => other,
                    };
                    let method_name = match &member.prop {
                        ast::MemberProp::Ident(ident) => Some(ident.sym.to_string()),
                        _ => None,
                    };
                    method_name.and_then(|name| {
                        self.lookup_method_params(inner_ty, &name, args.len(), &[])
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some((param_types, has_rest)) = param_info {
            self.propagate_call_arg_expected_types(args, &param_types, has_rest);
        }
    }

    /// Propagates expected types to call arguments, handling rest parameters.
    ///
    /// For regular parameters, zips args with param types.
    /// For rest parameters (`has_rest = true`), the last param must be `Vec<T>`;
    /// its element type `T` is propagated to all remaining arguments.
    pub(super) fn propagate_call_arg_expected_types(
        &mut self,
        args: &[ast::ExprOrSpread],
        param_types: &[RustType],
        has_rest: bool,
    ) {
        let rest_element_type = if has_rest {
            match param_types.last() {
                Some(RustType::Vec(inner)) => Some(inner.as_ref().clone()),
                _ => None,
            }
        } else {
            None
        };
        let regular_params = if rest_element_type.is_some() {
            &param_types[..param_types.len() - 1]
        } else {
            param_types
        };

        self.propagate_arg_expected_types(args, regular_params);

        if let Some(ref elem_ty) = rest_element_type {
            if args.len() > regular_params.len() {
                let rest_types: Vec<RustType> =
                    std::iter::repeat_n(elem_ty.clone(), args.len() - regular_params.len())
                        .collect();
                self.propagate_arg_expected_types(&args[regular_params.len()..], &rest_types);
            }
        }
    }

    /// Collects resolved argument types from already-resolved expressions.
    ///
    /// Returns `Some(ty)` for arguments whose type is known, `None` otherwise.
    /// Used by overload resolution to select the best matching signature.
    pub(super) fn collect_resolved_arg_types(
        &self,
        args: &[ast::ExprOrSpread],
    ) -> Vec<Option<RustType>> {
        args.iter()
            .map(|arg| {
                let span = Span::from_swc(arg.expr.span());
                self.result.expr_types.get(&span).and_then(|rt| {
                    if let ResolvedType::Known(ty) = rt {
                        Some(ty.clone())
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Looks up method signatures — delegates to `TypeRegistry::lookup_method_sigs`.
    fn lookup_method_sigs(
        &self,
        obj_type: &RustType,
        method_name: &str,
    ) -> Option<Vec<crate::registry::MethodSignature>> {
        self.registry.lookup_method_sigs(obj_type, method_name)
    }

    /// Looks up method parameter types and rest flag from the object type's definition.
    ///
    /// When multiple overloads exist, selects the best match using `select_overload`.
    /// `arg_types` should be `&[]` when resolved argument types are not yet available.
    ///
    /// Returns `(param_types, has_rest)`.
    pub(super) fn lookup_method_params(
        &self,
        obj_type: &RustType,
        method_name: &str,
        arg_count: usize,
        arg_types: &[Option<RustType>],
    ) -> Option<(Vec<RustType>, bool)> {
        let sigs = self.lookup_method_sigs(obj_type, method_name)?;
        let (_, sig) = select_overload(&sigs, arg_count, arg_types);
        Some((
            sig.params.iter().map(|p| p.ty.clone()).collect(),
            sig.has_rest,
        ))
    }

    /// Resolves the return type of a method call, selecting the best overload
    /// based on argument count and types.
    pub(super) fn resolve_method_return_type(
        &self,
        obj_type: &RustType,
        method_name: &str,
        arg_count: usize,
        arg_types: &[Option<RustType>],
    ) -> ResolvedType {
        match self.lookup_method_sigs(obj_type, method_name) {
            Some(sigs) => {
                let (_, sig) = select_overload(&sigs, arg_count, arg_types);
                sig.return_type
                    .clone()
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown)
            }
            None => ResolvedType::Unknown,
        }
    }

    /// Temporarily pushes explicit type argument bindings into `type_param_constraints`.
    ///
    /// Given a callee and explicit type args (e.g., `foo<string>(...)` → `[String]`),
    /// looks up the callee's type parameter names from TypeRegistry and creates
    /// a name→type mapping. Returns the previous constraints for restoration.
    ///
    /// Returns `None` if no bindings were added (no explicit type args or callee
    /// has no type params).
    pub(super) fn push_call_type_arg_bindings(
        &mut self,
        callee: &ast::Expr,
        explicit_type_args: &[RustType],
    ) -> Option<HashMap<String, RustType>> {
        if explicit_type_args.is_empty() {
            return None;
        }

        let type_params = match callee {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();
                self.registry.get(&fn_name).and_then(|td| match td {
                    TypeDef::Function { type_params, .. } => Some(type_params.clone()),
                    TypeDef::Struct { type_params, .. } => Some(type_params.clone()),
                    _ => None,
                })
            }
            _ => None,
        };

        let type_params = type_params?;
        if type_params.is_empty() {
            return None;
        }

        let bindings = build_type_arg_bindings(&type_params, explicit_type_args);
        if bindings.is_empty() {
            return None;
        }

        let mut merged = self.type_param_constraints.clone();
        merged.extend(bindings);
        Some(std::mem::replace(&mut self.type_param_constraints, merged))
    }

    /// Infers type arguments from resolved argument types, re-propagates
    /// expected types to arguments (2nd pass), and re-resolves the return type.
    ///
    /// Called after argument expressions have been resolved, when no explicit
    /// type arguments were provided. If the callee has type parameters and
    /// arguments provide enough information:
    /// 1. Inferred bindings are merged into `type_param_constraints`
    /// 2. Expected types are re-propagated to arguments (TypeVars now resolve
    ///    to concrete types, enabling struct name resolution for object literals)
    /// 3. The return type is re-resolved with inferred bindings
    ///
    /// The re-propagation (step 2) is independent of whether a return type
    /// exists — even `fn<T>(x: T, y: T): void` benefits from feedback.
    fn infer_type_args_and_feedback(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::ExprOrSpread],
        current_result: ResolvedType,
    ) -> ResolvedType {
        let ast::Expr::Ident(ident) = callee else {
            return current_result;
        };
        let fn_name = ident.sym.to_string();

        // Get function type params, param types, return type, and has_rest from registry
        let (type_params, param_types, return_type, has_rest) = match self.registry.get(&fn_name) {
            Some(TypeDef::Function {
                type_params,
                params,
                return_type,
                has_rest,
                ..
            }) if !type_params.is_empty() => (
                type_params.clone(),
                params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
                return_type.clone(),
                *has_rest,
            ),
            _ => return current_result,
        };

        // Collect resolved argument types
        let arg_types = self.collect_resolved_arg_types(args);

        // Infer type parameter bindings from param types and arg types
        let bindings = infer_type_args(&type_params, &param_types, &arg_types);
        if bindings.is_empty() {
            return current_result;
        }

        // Merge inferred bindings into constraints
        let mut merged = self.type_param_constraints.clone();
        merged.extend(bindings);
        let prev = std::mem::replace(&mut self.type_param_constraints, merged);

        // 2nd pass: re-propagate expected types with resolved type params.
        // TypeVars in param_types now resolve to concrete types via the merged
        // bindings, so object literals and other args get correct expected types.
        self.propagate_call_arg_expected_types(args, &param_types, has_rest);

        // Re-resolve return type if available
        let result = match &return_type {
            Some(ret_ty) => ResolvedType::Known(self.resolve_type_params_in_type(ret_ty)),
            None => current_result,
        };

        // Restore constraints
        self.type_param_constraints = prev;
        result
    }
}

/// Selects the best-matching callable interface overload for a Named type.
///
/// Performs classify → type substitution → select_overload in one step.
/// Returns `None` if the type is not a callable interface.
fn select_callable_overload(
    registry: &crate::registry::TypeRegistry,
    name: &str,
    type_args: &[RustType],
    arg_count: usize,
) -> Option<crate::registry::MethodSignature> {
    let def = registry.get(name)?;
    use crate::registry::collection::{classify_callable_interface, CallableInterfaceKind};
    let call_sigs = match classify_callable_interface(def) {
        CallableInterfaceKind::SingleOverload(sig) => vec![sig],
        CallableInterfaceKind::MultiOverload(sigs) => sigs,
        CallableInterfaceKind::NonCallable => return None,
    };
    let type_params = match def {
        TypeDef::Struct { type_params, .. } => type_params.as_slice(),
        _ => return None,
    };
    let apply_sub = crate::pipeline::type_converter::overloaded_callable::apply_type_substitution;
    let substituted: Vec<_> = call_sigs
        .iter()
        .map(|sig| apply_sub(sig, type_params, type_args))
        .collect();
    let (_, selected) = select_overload(&substituted, arg_count, &[]);
    Some(selected.clone())
}

/// Infers type parameter bindings by unifying parameter types with argument types.
///
/// For each parameter type that is a bare type parameter (e.g., `T`), if the
/// corresponding argument has a known type, binds that type parameter to the
/// argument type. Also handles `Option<T>`, `Vec<T>`, and nested Named types.
pub(super) fn infer_type_args(
    type_params: &[crate::ir::TypeParam],
    param_types: &[RustType],
    arg_types: &[Option<RustType>],
) -> HashMap<String, RustType> {
    let param_names: std::collections::HashSet<&str> =
        type_params.iter().map(|p| p.name.as_str()).collect();

    let mut bindings = HashMap::new();

    for (param_ty, arg_ty) in param_types.iter().zip(arg_types.iter()) {
        if let Some(arg_ty) = arg_ty {
            unify_type(&param_names, param_ty, arg_ty, &mut bindings, 0);
        }
    }

    bindings
}

/// Recursively unifies a parameter type with an argument type, extracting
/// type parameter bindings.
///
/// For example:
/// - `T` unified with `String` → binds `T = String`
/// - `Option<T>` unified with `Option<String>` → binds `T = String`
/// - `Vec<T>` unified with `Vec<f64>` → binds `T = f64`
///
/// Depth-limited to prevent infinite recursion on cyclic type structures.
fn unify_type(
    param_names: &std::collections::HashSet<&str>,
    param_ty: &RustType,
    arg_ty: &RustType,
    bindings: &mut HashMap<String, RustType>,
    depth: usize,
) {
    if depth > 10 {
        return;
    }
    match param_ty {
        // I-387: TypeVar (第一級の型パラメータ参照)
        RustType::TypeVar { name } if param_names.contains(name.as_str()) => {
            bindings
                .entry(name.clone())
                .or_insert_with(|| arg_ty.clone());
        }
        // Bare type parameter: T → bind directly (legacy Named form)
        RustType::Named { name, type_args }
            if type_args.is_empty() && param_names.contains(name.as_str()) =>
        {
            bindings
                .entry(name.clone())
                .or_insert_with(|| arg_ty.clone());
        }
        // Named type with type args: Foo<T, U> unified with Foo<String, F64>
        RustType::Named { name, type_args } => {
            if let RustType::Named {
                name: arg_name,
                type_args: arg_type_args,
            } = arg_ty
            {
                if name == arg_name && type_args.len() == arg_type_args.len() {
                    for (pt, at) in type_args.iter().zip(arg_type_args.iter()) {
                        unify_type(param_names, pt, at, bindings, depth + 1);
                    }
                }
            }
        }
        // Option<T> unified with Option<X>
        RustType::Option(inner) => {
            if let RustType::Option(arg_inner) = arg_ty {
                unify_type(param_names, inner, arg_inner, bindings, depth + 1);
            }
        }
        // Vec<T> unified with Vec<X>
        RustType::Vec(inner) => {
            if let RustType::Vec(arg_inner) = arg_ty {
                unify_type(param_names, inner, arg_inner, bindings, depth + 1);
            }
        }
        // Fn types
        RustType::Fn {
            params,
            return_type,
        } => {
            if let RustType::Fn {
                params: arg_params,
                return_type: arg_ret,
            } = arg_ty
            {
                for (pt, at) in params.iter().zip(arg_params.iter()) {
                    unify_type(param_names, pt, at, bindings, depth + 1);
                }
                unify_type(param_names, return_type, arg_ret, bindings, depth + 1);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::TypeParam;

    fn named(n: &str) -> RustType {
        RustType::Named {
            name: n.to_string(),
            type_args: vec![],
        }
    }

    fn named_with(n: &str, args: Vec<RustType>) -> RustType {
        RustType::Named {
            name: n.to_string(),
            type_args: args,
        }
    }

    fn tp(name: &str) -> TypeParam {
        TypeParam {
            name: name.to_string(),
            constraint: None,
            default: None,
        }
    }

    // --- unify_type: bare type parameter ---

    #[test]
    fn test_unify_bare_type_param_binds() {
        let params = [tp("T")];
        let result = infer_type_args(&params, &[named("T")], &[Some(RustType::String)]);
        assert_eq!(result.get("T"), Some(&RustType::String));
    }

    #[test]
    fn test_unify_bare_type_param_first_wins() {
        // If T appears in multiple params, the first binding wins
        let params = [tp("T")];
        let result = infer_type_args(
            &params,
            &[named("T"), named("T")],
            &[Some(RustType::String), Some(RustType::F64)],
        );
        assert_eq!(result.get("T"), Some(&RustType::String));
    }

    #[test]
    fn test_unify_non_param_named_skipped() {
        // "String" is not a type parameter name, so no binding
        let params = [tp("T")];
        let result = infer_type_args(&params, &[RustType::String], &[Some(RustType::String)]);
        assert!(result.is_empty());
    }

    // --- unify_type: Named with type args ---

    #[test]
    fn test_unify_named_with_type_args() {
        let params = [tp("T"), tp("U")];
        let param_ty = named_with("Foo", vec![named("T"), named("U")]);
        let arg_ty = named_with("Foo", vec![RustType::String, RustType::F64]);
        let result = infer_type_args(&params, &[param_ty], &[Some(arg_ty)]);
        assert_eq!(result.get("T"), Some(&RustType::String));
        assert_eq!(result.get("U"), Some(&RustType::F64));
    }

    #[test]
    fn test_unify_named_different_names_skipped() {
        let params = [tp("T")];
        let param_ty = named_with("Foo", vec![named("T")]);
        let arg_ty = named_with("Bar", vec![RustType::String]);
        let result = infer_type_args(&params, &[param_ty], &[Some(arg_ty)]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_unify_named_different_arity_skipped() {
        let params = [tp("T")];
        let param_ty = named_with("Foo", vec![named("T")]);
        let arg_ty = named_with("Foo", vec![RustType::String, RustType::F64]);
        let result = infer_type_args(&params, &[param_ty], &[Some(arg_ty)]);
        assert!(result.is_empty());
    }

    // --- unify_type: Option ---

    #[test]
    fn test_unify_option() {
        let params = [tp("T")];
        let param_ty = RustType::Option(Box::new(named("T")));
        let arg_ty = RustType::Option(Box::new(RustType::String));
        let result = infer_type_args(&params, &[param_ty], &[Some(arg_ty)]);
        assert_eq!(result.get("T"), Some(&RustType::String));
    }

    #[test]
    fn test_unify_option_vs_non_option_skipped() {
        let params = [tp("T")];
        let param_ty = RustType::Option(Box::new(named("T")));
        let result = infer_type_args(&params, &[param_ty], &[Some(RustType::String)]);
        assert!(result.is_empty());
    }

    // --- unify_type: Vec ---

    #[test]
    fn test_unify_vec() {
        let params = [tp("T")];
        let param_ty = RustType::Vec(Box::new(named("T")));
        let arg_ty = RustType::Vec(Box::new(RustType::F64));
        let result = infer_type_args(&params, &[param_ty], &[Some(arg_ty)]);
        assert_eq!(result.get("T"), Some(&RustType::F64));
    }

    // --- unify_type: Fn ---

    #[test]
    fn test_unify_fn() {
        let params = [tp("T"), tp("U")];
        let param_ty = RustType::Fn {
            params: vec![named("T")],
            return_type: Box::new(named("U")),
        };
        let arg_ty = RustType::Fn {
            params: vec![RustType::String],
            return_type: Box::new(RustType::F64),
        };
        let result = infer_type_args(&params, &[param_ty], &[Some(arg_ty)]);
        assert_eq!(result.get("T"), Some(&RustType::String));
        assert_eq!(result.get("U"), Some(&RustType::F64));
    }

    // --- unify_type: partial inference ---

    #[test]
    fn test_unify_partial_inference() {
        // fn<T, U>(x: T) called with ("hello") → T=String, U unresolved
        let params = [tp("T"), tp("U")];
        let result = infer_type_args(&params, &[named("T")], &[Some(RustType::String)]);
        assert_eq!(result.get("T"), Some(&RustType::String));
        assert_eq!(result.get("U"), None);
    }

    #[test]
    fn test_unify_unknown_arg_skipped() {
        let params = [tp("T")];
        let result = infer_type_args(&params, &[named("T")], &[None]);
        assert!(result.is_empty());
    }

    // --- unify_type: depth limit ---

    #[test]
    fn test_unify_deeply_nested_terminates() {
        // Build Vec<Vec<Vec<...Vec<T>...>>> with depth > 10
        let params = [tp("T")];
        let mut param_ty = named("T");
        let mut arg_ty: RustType = RustType::String;
        for _ in 0..15 {
            param_ty = RustType::Vec(Box::new(param_ty));
            arg_ty = RustType::Vec(Box::new(arg_ty));
        }
        // Should terminate without panic; depth limit truncates
        let result = infer_type_args(&params, &[param_ty], &[Some(arg_ty)]);
        // Due to depth limit, T might not be bound — that's acceptable
        // The key assertion is that this doesn't hang or panic
        let _ = result;
    }
}
