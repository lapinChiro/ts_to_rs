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

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_call_expr(&mut self, call: &ast::CallExpr) -> ResolvedType {
        let callee = match &call.callee {
            ast::Callee::Expr(expr) => expr.as_ref(),
            _ => return ResolvedType::Unknown,
        };

        // Set expected types for arguments based on callee's parameter types
        self.set_call_arg_expected_types(callee, &call.args);

        // Resolve the callee to determine return type
        let result = match callee {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();
                // Check scope for Fn type or Named function type alias
                match self.lookup_var(&fn_name) {
                    ResolvedType::Known(RustType::Fn { return_type, .. }) => {
                        ResolvedType::Known(return_type.as_ref().clone())
                    }
                    ResolvedType::Known(ref var_ty @ RustType::Named { .. }) => {
                        let (ret, _) = resolve_fn_type_info(var_ty, self.registry);
                        ret.map(ResolvedType::Known)
                            .unwrap_or(ResolvedType::Unknown)
                    }
                    _ => {
                        // Fall back to TypeRegistry
                        if let Some(TypeDef::Function { return_type, .. }) =
                            self.registry.get(&fn_name)
                        {
                            ResolvedType::Known(return_type.clone().unwrap_or(RustType::Unit))
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
                        // Still resolve arguments even if callee type is unknown
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
                // Collect resolved arg types for overload resolution
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

        // Resolve all argument expressions to register their types in expr_types.
        for arg in &call.args {
            self.resolve_expr(&arg.expr);
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
                    Some((params.iter().map(|(_, ty)| ty.clone()).collect(), *has_rest))
                } else if let ResolvedType::Known(ref var_ty) = self.lookup_var(&fn_name) {
                    match var_ty {
                        RustType::Fn { params, .. } => {
                            // Scope lookup for Fn type variables (no rest info available)
                            Some((params.clone(), false))
                        }
                        RustType::Named { .. } => {
                            // Named type variable (e.g., `encode: Encoder` where Encoder
                            // is a function type alias) — resolve via registry
                            let (ret, params) = resolve_fn_type_info(var_ty, self.registry);
                            let _ = ret; // return type not needed here
                            params.map(|p| (p, false))
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

    /// Looks up method signatures from the object type's definition.
    ///
    /// For `Vec<T>`, maps to the `Array<T>` definition in TypeRegistry so that
    /// TypeScript's Array methods (push, map, filter, etc.) are available.
    fn lookup_method_sigs(
        &self,
        obj_type: &RustType,
        method_name: &str,
    ) -> Option<Vec<crate::registry::MethodSignature>> {
        // Vec<T> → Array<T>: TypeScript Array methods apply to Rust Vec
        if let RustType::Vec(inner) = obj_type {
            let type_def = self
                .registry
                .instantiate("Array", &[inner.as_ref().clone()]);
            return match &type_def {
                Some(TypeDef::Struct { methods, .. }) => methods.get(method_name).cloned(),
                _ => None,
            };
        }

        let (type_name, type_args) = extract_type_name_for_registry(obj_type)?;

        let type_def = if type_args.is_empty() {
            self.registry.get(type_name).cloned()
        } else {
            self.registry.instantiate(type_name, type_args)
        };

        match &type_def {
            Some(TypeDef::Struct { methods, .. }) => methods.get(method_name).cloned(),
            _ => None,
        }
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
        let sig = select_overload(&sigs, arg_count, arg_types);
        Some((
            sig.params.iter().map(|(_, ty)| ty.clone()).collect(),
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
                let sig = select_overload(&sigs, arg_count, arg_types);
                sig.return_type
                    .clone()
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown)
            }
            None => ResolvedType::Unknown,
        }
    }
}
