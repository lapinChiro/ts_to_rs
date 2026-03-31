//! Member access expression conversion (property access, optional chaining, discriminated unions).

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{ClosureBody, Expr, MatchArm, MatchPattern, Param, RustType, Stmt};
use crate::registry::TypeDef;

use super::methods::map_method_call;
use crate::transformer::Transformer;

/// Converts an index expression to a usize-compatible form for `Vec::get()`.
///
/// Integer-valued `NumberLit` → `IntLit` (renders as `0`, not `0.0`).
/// Other expressions → `Cast { target: usize }`.
fn convert_index_to_usize(index: Expr) -> Expr {
    match &index {
        Expr::NumberLit(n) if n.fract() == 0.0 => Expr::IntLit(*n as i128),
        _ => Expr::Cast {
            expr: Box::new(index),
            target: RustType::Named {
                name: "usize".to_string(),
                type_args: vec![],
            },
        },
    }
}

impl<'a> Transformer<'a> {
    /// Resolves a member access expression, applying special conversions for known fields.
    ///
    /// - `.length` → `.len() as f64`
    /// - enum member access → `EnumName::Variant`
    /// - otherwise → `object.field`
    pub(crate) fn resolve_member_access(
        &self,
        object: &Expr,
        field: &str,
        ts_obj: &ast::Expr,
    ) -> Result<Expr> {
        // Check if the TS object is an identifier referring to an enum
        if let ast::Expr::Ident(ident) = ts_obj {
            let name = ident.sym.as_ref();
            if let Some(TypeDef::Enum { .. }) = self.reg().get(name) {
                return Ok(Expr::Ident(format!("{name}::{field}")));
            }
        }

        // Math.PI, Math.E etc. → std::f64::consts::PI, std::f64::consts::E
        if let ast::Expr::Ident(ident) = ts_obj {
            if ident.sym.as_ref() == "Math" {
                let const_name = match field {
                    "PI" => Some("PI"),
                    "E" => Some("E"),
                    "LN2" => Some("LN_2"),
                    "LN10" => Some("LN_10"),
                    "LOG2E" => Some("LOG2_E"),
                    "LOG10E" => Some("LOG10_E"),
                    "SQRT2" => Some("SQRT_2"),
                    _ => None,
                };
                if let Some(name) = const_name {
                    return Ok(Expr::Ident(format!("std::f64::consts::{name}")));
                }
            }
        }

        // .length → .len() as f64
        if field == "length" {
            let len_call = Expr::MethodCall {
                object: Box::new(object.clone()),
                method: "len".to_string(),
                args: vec![],
            };
            return Ok(Expr::Cast {
                expr: Box::new(len_call),
                target: RustType::F64,
            });
        }

        Ok(Expr::FieldAccess {
            object: Box::new(object.clone()),
            field: field.to_string(),
        })
    }

    /// Converts an optional chaining expression (`x?.y`) to `x.as_ref().map(|_v| _v.y)`.
    ///
    /// Supports property access, method calls, and computed access.
    /// Chained optional chaining (`x?.y?.z`) is handled recursively.
    pub(crate) fn convert_opt_chain_expr(&mut self, opt_chain: &ast::OptChainExpr) -> Result<Expr> {
        match opt_chain.base.as_ref() {
            ast::OptChainBase::Member(member) => {
                let obj_type = self.get_expr_type(&member.obj);
                let is_option = obj_type.is_some_and(|ty| matches!(ty, RustType::Option(_)));

                // Non-Option type with known type: plain member access
                if !is_option && obj_type.is_some() {
                    return self.convert_member_expr(member);
                }

                // Cat A: receiver object for optional chaining
                let object = self.convert_expr(&member.obj)?;
                let body_expr = match &member.prop {
                    ast::MemberProp::Ident(ident) => {
                        let field = ident.sym.to_string();
                        self.resolve_member_access(
                            &Expr::Ident("_v".to_string()),
                            &field,
                            &member.obj,
                        )?
                    }
                    ast::MemberProp::Computed(computed) => {
                        // Use .get() for safe bounds-checked access (I-316).
                        // Direct indexing (_v[i]) panics on out-of-bounds;
                        // TS returns undefined, which maps to None.
                        let index = self.convert_expr(&computed.expr)?;
                        let safe_index = convert_index_to_usize(index);
                        Expr::MethodCall {
                            object: Box::new(Expr::MethodCall {
                                object: Box::new(Expr::Ident("_v".to_string())),
                                method: "get".to_string(),
                                args: vec![safe_index],
                            }),
                            method: "cloned".to_string(),
                            args: vec![],
                        }
                    }
                    _ => return Err(anyhow!("unsupported optional chaining property")),
                };

                // Use and_then when the body returns Option (to avoid Option<Option<T>>):
                // - Computed index: .get() returns Option<&T>
                // - Option field type: field is already Option<T>
                let is_computed = matches!(&member.prop, ast::MemberProp::Computed(_));
                let field_type =
                    self.resolve_field_type(obj_type.unwrap_or(&RustType::Any), &member.prop);
                let method_name = if is_computed
                    || field_type.is_some_and(|ty| matches!(ty, RustType::Option(_)))
                {
                    "and_then"
                } else {
                    "map"
                };

                Ok(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(object),
                        method: "as_ref".to_string(),
                        args: vec![],
                    }),
                    method: method_name.to_string(),
                    args: vec![Expr::Closure {
                        params: vec![Param {
                            name: "_v".to_string(),
                            ty: None,
                        }],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(body_expr)),
                    }],
                })
            }
            ast::OptChainBase::Call(opt_call) => {
                // Check if the callee object is a non-Option type
                let callee_obj_type = match opt_call.callee.as_ref() {
                    ast::Expr::Member(m) => self.get_expr_type(&m.obj),
                    ast::Expr::OptChain(oc) => match oc.base.as_ref() {
                        ast::OptChainBase::Member(m) => self.get_expr_type(&m.obj),
                        _ => None,
                    },
                    _ => None,
                };
                let is_option = callee_obj_type.is_some_and(|ty| matches!(ty, RustType::Option(_)));

                let (object, method) = self.extract_method_from_callee(&opt_call.callee)?;

                let args: Vec<Expr> = opt_call
                    .args
                    .iter()
                    .map(|arg| self.convert_expr(&arg.expr))
                    .collect::<Result<_>>()?;

                // Non-Option type: plain method call
                if !is_option && callee_obj_type.is_some() {
                    return Ok(Expr::MethodCall {
                        object: Box::new(object),
                        method,
                        args,
                    });
                }

                let body_expr = map_method_call(Expr::Ident("_v".to_string()), &method, args);
                Ok(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(object),
                        method: "as_ref".to_string(),
                        args: vec![],
                    }),
                    method: "map".to_string(),
                    args: vec![Expr::Closure {
                        params: vec![Param {
                            name: "_v".to_string(),
                            ty: None,
                        }],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(body_expr)),
                    }],
                })
            }
        }
    }

    /// Converts a member expression (`obj.field`) to `Expr::FieldAccess`.
    ///
    /// `this.x` becomes `self.x`.
    pub(crate) fn convert_member_expr(&mut self, member: &ast::MemberExpr) -> Result<Expr> {
        // Computed property: arr[0], arr[i] → Expr::Index or tuple.N → Expr::FieldAccess
        if let ast::MemberProp::Computed(computed) = &member.prop {
            // Cat A: receiver object
            let object = self.convert_expr(&member.obj)?;

            // Tuple index access: pair[0] → pair.0 (Rust uses dot notation for tuples)
            if let Some(RustType::Tuple(_)) = self.get_expr_type(&member.obj) {
                if let ast::Expr::Lit(ast::Lit::Num(num)) = &*computed.expr {
                    let idx = num.value as usize;
                    return Ok(Expr::FieldAccess {
                        object: Box::new(object),
                        field: idx.to_string(),
                    });
                }
            }

            // Cat A: computed index
            let index = self.convert_expr(&computed.expr)?;
            return Ok(Expr::Index {
                object: Box::new(object),
                index: Box::new(index),
            });
        }

        let field = match &member.prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            ast::MemberProp::PrivateName(private) => format!("_{}", private.name),
            _ => return Err(anyhow!("unsupported member property (only identifiers)")),
        };

        // process.env.VAR → std::env::var("VAR").unwrap()
        if let ast::Expr::Member(inner) = member.obj.as_ref() {
            if let (ast::Expr::Ident(obj), ast::MemberProp::Ident(prop)) =
                (inner.obj.as_ref(), &inner.prop)
            {
                if obj.sym.as_ref() == "process" && prop.sym.as_ref() == "env" {
                    return Ok(Expr::MethodCall {
                        object: Box::new(Expr::FnCall {
                            name: "std::env::var".to_string(),
                            args: vec![Expr::StringLit(field)],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    });
                }
            }
        }

        // Check if accessing a field of a discriminated union enum
        if let Some(RustType::Named { name, .. }) = self.get_expr_type(&member.obj) {
            if let Some(TypeDef::Enum {
                tag_field: Some(tag),
                variant_fields,
                ..
            }) = self.reg().get(name)
            {
                if field == *tag {
                    // Tag field → method call (e.g., s.kind() )
                    // Cat A: receiver object
                    let object = self.convert_expr(&member.obj)?;
                    return Ok(Expr::MethodCall {
                        object: Box::new(object),
                        method: tag.clone(),
                        args: vec![],
                    });
                }
                // Non-tag field: if bound in match arm destructuring,
                // clone the reference (match on &obj binds fields by reference)
                if self
                    .tctx
                    .type_resolution
                    .is_du_field_binding(&field, member.span.lo.0)
                {
                    return Ok(Expr::MethodCall {
                        object: Box::new(Expr::Ident(field)),
                        method: "clone".to_string(),
                        args: vec![],
                    });
                }
                // Standalone field access → inline match expression
                let variant_fields = variant_fields.clone();
                return self.convert_du_standalone_field_access(
                    &member.obj,
                    name,
                    &field,
                    &variant_fields,
                );
            }
        }

        // Cat A: receiver object
        let object = self.convert_expr(&member.obj)?;
        self.resolve_member_access(&object, &field, &member.obj)
    }

    /// Discriminated union の standalone フィールドアクセスを inline match 式に変換する。
    ///
    /// `s.radius` → `match &s { Shape::Circle { radius, .. } => radius.clone(), _ => panic!("...") }`
    pub(crate) fn convert_du_standalone_field_access(
        &mut self,
        obj_expr: &ast::Expr,
        enum_name: &str,
        field: &str,
        variant_fields: &std::collections::HashMap<String, Vec<(String, RustType)>>,
    ) -> Result<Expr> {
        // Cat A: receiver object
        let object = self.convert_expr(obj_expr)?;
        let match_expr = Expr::Ref(Box::new(object));

        let mut arms: Vec<MatchArm> = Vec::new();

        // Create arms for variants that have this field
        for (variant_name, fields) in variant_fields {
            if fields.iter().any(|(n, _)| n == field) {
                arms.push(MatchArm {
                    patterns: vec![MatchPattern::EnumVariant {
                        path: format!("{enum_name}::{variant_name}"),
                        bindings: vec![field.to_string()],
                    }],
                    guard: None,
                    body: vec![Stmt::TailExpr(Expr::MethodCall {
                        object: Box::new(Expr::Ident(field.to_string())),
                        method: "clone".to_string(),
                        args: vec![],
                    })],
                });
            }
        }

        // Add wildcard arm with panic
        arms.push(MatchArm {
            patterns: vec![MatchPattern::Wildcard],
            guard: None,
            body: vec![Stmt::TailExpr(Expr::MacroCall {
                name: "panic".to_string(),
                args: vec![Expr::StringLit(format!(
                    "variant does not have field '{field}'"
                ))],
                use_debug: vec![false],
            })],
        });

        Ok(Expr::Match {
            expr: Box::new(match_expr),
            arms,
        })
    }
}

impl<'a> Transformer<'a> {
    /// Extracts the object and method name from an optional call's callee.
    ///
    /// Handles both `x.method` (`Member`) and `x?.method` (`OptChain(Member)`) patterns.
    fn extract_method_from_callee(&mut self, callee: &ast::Expr) -> Result<(Expr, String)> {
        let member = match callee {
            ast::Expr::Member(member) => member,
            ast::Expr::OptChain(opt) => match opt.base.as_ref() {
                ast::OptChainBase::Member(member) => member,
                _ => return Err(anyhow!("unsupported optional call callee")),
            },
            _ => return Err(anyhow!("unsupported optional call callee: {:?}", callee)),
        };
        let object = self.convert_expr(&member.obj)?;
        let method = match &member.prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            _ => return Err(anyhow!("unsupported optional call property")),
        };
        Ok((object, method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_index_to_usize_integer_number_lit() {
        let result = convert_index_to_usize(Expr::NumberLit(0.0));
        assert_eq!(result, Expr::IntLit(0));
    }

    #[test]
    fn test_convert_index_to_usize_large_integer() {
        let result = convert_index_to_usize(Expr::NumberLit(42.0));
        assert_eq!(result, Expr::IntLit(42));
    }

    #[test]
    fn test_convert_index_to_usize_fractional_gets_cast() {
        let result = convert_index_to_usize(Expr::NumberLit(1.5));
        assert!(matches!(result, Expr::Cast { .. }));
    }

    #[test]
    fn test_convert_index_to_usize_negative_becomes_int_lit() {
        // -1.0 has fract() == 0.0, so it becomes IntLit(-1)
        // When used as usize, this wraps to a large number, but .get() safely returns None
        let result = convert_index_to_usize(Expr::NumberLit(-1.0));
        assert_eq!(result, Expr::IntLit(-1));
    }

    #[test]
    fn test_convert_index_to_usize_variable_gets_cast() {
        let result = convert_index_to_usize(Expr::Ident("i".to_string()));
        match result {
            Expr::Cast { expr, target } => {
                assert_eq!(*expr, Expr::Ident("i".to_string()));
                assert_eq!(
                    target,
                    RustType::Named {
                        name: "usize".to_string(),
                        type_args: vec![]
                    }
                );
            }
            other => panic!("expected Cast, got: {other:?}"),
        }
    }
}
