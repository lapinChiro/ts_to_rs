//! Optional chaining type resolution (`OptChain` arm of `resolve_expr_inner`).
//!
//! `x?.y` and `x?.method(args)` always produce `Option<T>` where `T` is the resolved
//! member / call return type. Internal Option<T> obj types are unwrapped before
//! field/method lookup so `Option<Foo>?.bar` resolves `Foo`'s `bar` field; the
//! result is then re-wrapped via `wrap_optional` (idempotent for already-Option
//! inner types).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::super::*;
use crate::pipeline::type_resolution::Span;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_opt_chain_expr(&mut self, opt: &ast::OptChainExpr) -> ResolvedType {
        // Optional chaining: x?.y or x?.method(args)
        // Result is always Option<T> where T is the resolved member/call type.
        let inner_result = match &*opt.base {
            ast::OptChainBase::Member(member) => {
                let obj_type = self.resolve_expr(&member.obj);
                // Unwrap Option<T> → T for field lookup
                let unwrapped = match &obj_type {
                    ResolvedType::Known(RustType::Option(inner)) => {
                        ResolvedType::Known(inner.as_ref().clone())
                    }
                    other => other.clone(),
                };
                // Resolve field type using the same logic as resolve_member_expr
                match &unwrapped {
                    ResolvedType::Known(ty) => self.resolve_member_type(ty, &member.prop),
                    _ => ResolvedType::Unknown,
                }
            }
            ast::OptChainBase::Call(opt_call) => {
                // x?.method(args) — the callee may itself be an OptChain
                // wrapping a Member expression. Unwrap to extract the
                // actual Member for method param lookup.
                let effective_member = match opt_call.callee.as_ref() {
                    ast::Expr::OptChain(inner_opt) => {
                        if let ast::OptChainBase::Member(m) = &*inner_opt.base {
                            Some(m)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                let call_return_type = if let Some(member) = effective_member {
                    let obj_type = self.resolve_expr(&member.obj);
                    if let ResolvedType::Known(ref rust_ty) = obj_type {
                        let inner_ty = match rust_ty {
                            RustType::Option(inner) => inner.as_ref(),
                            other => other,
                        };
                        let method_name = match &member.prop {
                            ast::MemberProp::Ident(ident) => Some(ident.sym.to_string()),
                            _ => None,
                        };
                        let n_args = opt_call.args.len();
                        // Set expected types for args
                        if let Some((param_types, has_rest)) = method_name
                            .as_deref()
                            .and_then(|name| self.lookup_method_params(inner_ty, name, n_args, &[]))
                        {
                            self.propagate_call_arg_expected_types(
                                &opt_call.args,
                                &param_types,
                                has_rest,
                            );
                        }
                        // Collect resolved arg types for overload resolution
                        let arg_types = self.collect_resolved_arg_types(&opt_call.args);
                        // Resolve return type
                        method_name
                            .as_deref()
                            .map(|name| {
                                self.resolve_method_return_type(inner_ty, name, n_args, &arg_types)
                            })
                            .unwrap_or(ResolvedType::Unknown)
                    } else {
                        ResolvedType::Unknown
                    }
                } else {
                    self.set_call_arg_expected_types(&opt_call.callee, &opt_call.args);
                    ResolvedType::Unknown
                };
                // Walk callee for side effects
                let callee_span = Span::from_swc(opt_call.callee.span());
                let callee_ty = self.resolve_expr(&opt_call.callee);
                self.result.expr_types.insert(callee_span, callee_ty);
                // Resolve all argument expressions
                for arg in &opt_call.args {
                    self.resolve_expr(&arg.expr);
                }
                call_return_type
            }
        };
        // Wrap in Option<T>. OptChain always produces an optional result.
        // `wrap_optional` is idempotent: an already-Option inner stays single-wrapped.
        match inner_result {
            ResolvedType::Known(ty) => ResolvedType::Known(ty.wrap_optional()),
            ResolvedType::Unknown => ResolvedType::Known(RustType::Any.wrap_optional()),
        }
    }
}
