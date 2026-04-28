//! Constructor call type resolution (`New` arm of `resolve_expr_inner`).
//!
//! Resolves `new Foo(...)` / `new Foo<T>(...)` to the constructed type, including
//! type-argument inference from the constructor signature when no explicit type
//! arguments are given. Uses [`select_overload`] for constructor overload selection
//! and [`super::super::call_resolution::infer_type_args`] for parameter-driven
//! inference.

use swc_ecma_ast as ast;

use super::super::*;
use crate::registry::select_overload;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_new_expr(&mut self, new_expr: &ast::NewExpr) -> ResolvedType {
        let class_name = match new_expr.callee.as_ref() {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return ResolvedType::Unknown,
        };

        // Convert explicit type arguments: `new Foo<string, number>(...)` → [String, F64]
        let explicit_type_args: Vec<RustType> = new_expr
            .type_args
            .as_ref()
            .map(|ta| convert_explicit_type_args(ta, self.synthetic, self.registry))
            .unwrap_or_default();

        if let Some(type_def) = self.registry.get(&class_name) {
            // Build type param bindings from explicit type args
            let type_param_bindings = match &type_def {
                TypeDef::Struct { type_params, .. } if !explicit_type_args.is_empty() => {
                    build_type_arg_bindings(type_params, &explicit_type_args)
                }
                _ => HashMap::new(),
            };

            // Temporarily add type arg bindings to constraints for param resolution
            let prev_constraints = if !type_param_bindings.is_empty() {
                let mut merged = self.type_param_constraints.clone();
                merged.extend(type_param_bindings);
                Some(std::mem::replace(&mut self.type_param_constraints, merged))
            } else {
                None
            };

            if let Some(args) = &new_expr.args {
                // Resolve parameter types: constructor signature first, then field fallback
                let param_types: Option<(Vec<RustType>, bool)> = match &type_def {
                    TypeDef::Struct {
                        constructor: Some(sigs),
                        ..
                    } if !sigs.is_empty() => {
                        let (_, sig) = select_overload(sigs, args.len(), &[]);
                        Some((
                            sig.params.iter().map(|p| p.ty.clone()).collect(),
                            sig.has_rest,
                        ))
                    }
                    TypeDef::Struct { fields, .. } => {
                        // Fallback: no constructor defined, use field types
                        Some((fields.iter().map(|f| f.ty.clone()).collect(), false))
                    }
                    _ => None,
                };
                if let Some((param_types, has_rest)) = param_types {
                    self.propagate_call_arg_expected_types(args, &param_types, has_rest);
                }
            }

            // Restore constraints
            if let Some(prev) = prev_constraints {
                self.type_param_constraints = prev;
            }

            // Resolve argument expressions. Dual role:
            // (1) populate `expr_types[arg.span]` for downstream lookups (DU field
            //     binding check, truthy narrowing on ident args, call arg
            //     resolution chains), and
            // (2) provide resolved types for `infer_new_expr_type_args` below when
            //     no explicit type args are given.
            // Symmetric with the unregistered `else` branch below (I-150).
            if let Some(args) = &new_expr.args {
                for arg in args {
                    self.resolve_expr(&arg.expr);
                }
            }

            // Infer type args from resolved argument types when no explicit type args
            let inferred_type_args = if explicit_type_args.is_empty() {
                self.infer_new_expr_type_args(type_def, new_expr)
            } else {
                explicit_type_args
            };

            ResolvedType::Known(RustType::Named {
                name: class_name,
                type_args: inferred_type_args,
            })
        } else {
            // I-150: Symmetric with the registered branch above and
            // `resolve_call_expr` (call_resolution.rs). Visit argument expressions
            // so that their `expr_types` entries are populated, enabling downstream
            // lookups (e.g., DU field binding check in member_access.rs, truthy
            // narrowing on ident args). Without this, `new UnknownClass(expr)`
            // leaves `expr_types[expr.span]` unpopulated; the transformer then
            // falls back to non-structured emission, producing compile error
            // E0609 for DU field access inside `new Error("..." + s.field)` in
            // no-builtin mode.
            if let Some(args) = &new_expr.args {
                for arg in args {
                    self.resolve_expr(&arg.expr);
                }
            }
            ResolvedType::Unknown
        }
    }

    /// Infers type arguments for a `new` expression from resolved argument types.
    fn infer_new_expr_type_args(
        &self,
        type_def: &TypeDef,
        new_expr: &ast::NewExpr,
    ) -> Vec<RustType> {
        let TypeDef::Struct {
            type_params,
            constructor,
            ..
        } = type_def
        else {
            return vec![];
        };
        if type_params.is_empty() {
            return vec![];
        }

        let Some(args) = &new_expr.args else {
            return vec![];
        };

        // Get constructor param types
        let param_types: Vec<RustType> = match constructor {
            Some(sigs) if !sigs.is_empty() => {
                let (_, sig) = select_overload(sigs, args.len(), &[]);
                sig.params.iter().map(|p| p.ty.clone()).collect()
            }
            _ => return vec![],
        };

        let arg_types = self.collect_resolved_arg_types(args);
        let bindings =
            super::super::call_resolution::infer_type_args(type_params, &param_types, &arg_types);

        if bindings.is_empty() {
            return vec![];
        }

        // Build type args in the order of type_params.
        // Only return type args if ALL type params were inferred.
        // Partial inference (some params unresolved) returns empty to avoid
        // introducing Any as a type argument (type-fallback-safety concern).
        let inferred: Vec<RustType> = type_params
            .iter()
            .filter_map(|tp| bindings.get(&tp.name).cloned())
            .collect();

        if inferred.len() == type_params.len() {
            inferred
        } else {
            vec![]
        }
    }
}
