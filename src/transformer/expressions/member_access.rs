//! Member access expression conversion (property access, optional chaining, discriminated unions).

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{ClosureBody, Expr, MatchArm, MatchPattern, Param, RustType, Stmt};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::TypeEnv;

use super::convert_expr;
use super::methods::map_method_call;
use super::type_resolution::{get_expr_type, resolve_field_type};
use crate::transformer::context::TransformContext;

/// Resolves a member access expression, applying special conversions for known fields.
///
/// - `.length` → `.len() as f64`
/// - enum member access → `EnumName::Variant`
/// - otherwise → `object.field`
pub(super) fn resolve_member_access(
    object: &Expr,
    field: &str,
    ts_obj: &ast::Expr,
    reg: &TypeRegistry,
) -> Result<Expr> {
    // Check if the TS object is an identifier referring to an enum
    if let ast::Expr::Ident(ident) = ts_obj {
        let name = ident.sym.as_ref();
        if let Some(TypeDef::Enum { .. }) = reg.get(name) {
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
pub(super) fn convert_opt_chain_expr(
    opt_chain: &ast::OptChainExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    match opt_chain.base.as_ref() {
        ast::OptChainBase::Member(member) => {
            let obj_type = get_expr_type(tctx, &member.obj);
            let is_option = obj_type.is_some_and(|ty| matches!(ty, RustType::Option(_)));

            // Non-Option type with known type: plain member access
            if !is_option && obj_type.is_some() {
                return convert_member_expr(member, tctx, reg, type_env, synthetic);
            }

            // Cat A: receiver object for optional chaining
            let object = convert_expr(&member.obj, tctx, reg, type_env, synthetic)?;
            let body_expr = match &member.prop {
                ast::MemberProp::Ident(ident) => {
                    let field = ident.sym.to_string();
                    resolve_member_access(&Expr::Ident("_v".to_string()), &field, &member.obj, reg)?
                }
                ast::MemberProp::Computed(computed) => {
                    // Cat A: computed index
                    let index = convert_expr(&computed.expr, tctx, reg, type_env, synthetic)?;
                    Expr::Index {
                        object: Box::new(Expr::Ident("_v".to_string())),
                        index: Box::new(index),
                    }
                }
                _ => return Err(anyhow!("unsupported optional chaining property")),
            };

            // If the field type is Option, use and_then to avoid Option<Option<T>>
            let field_type =
                resolve_field_type(obj_type.unwrap_or(&RustType::Any), &member.prop, tctx, reg);
            let method_name = if field_type.is_some_and(|ty| matches!(ty, RustType::Option(_))) {
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
                ast::Expr::Member(m) => get_expr_type(tctx, &m.obj),
                ast::Expr::OptChain(oc) => match oc.base.as_ref() {
                    ast::OptChainBase::Member(m) => get_expr_type(tctx, &m.obj),
                    _ => None,
                },
                _ => None,
            };
            let is_option = callee_obj_type.is_some_and(|ty| matches!(ty, RustType::Option(_)));

            let (object, method) =
                extract_method_from_callee(&opt_call.callee, tctx, reg, type_env, synthetic)?;

            let args: Vec<Expr> = opt_call
                .args
                .iter()
                .map(|arg| convert_expr(&arg.expr, tctx, reg, type_env, synthetic))
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

/// Extracts the object and method name from an optional call's callee.
///
/// Handles both `x.method` (`Member`) and `x?.method` (`OptChain(Member)`) patterns.
pub(super) fn extract_method_from_callee(
    callee: &ast::Expr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<(Expr, String)> {
    let member = match callee {
        ast::Expr::Member(member) => member,
        ast::Expr::OptChain(opt) => match opt.base.as_ref() {
            ast::OptChainBase::Member(member) => member,
            _ => return Err(anyhow!("unsupported optional call callee")),
        },
        _ => return Err(anyhow!("unsupported optional call callee: {:?}", callee)),
    };
    // Cat A: receiver object
    let object = convert_expr(&member.obj, tctx, reg, type_env, synthetic)?;
    let method = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported optional call property")),
    };
    Ok((object, method))
}

/// Converts a member expression (`obj.field`) to `Expr::FieldAccess`.
///
/// `this.x` becomes `self.x`.
pub(super) fn convert_member_expr(
    member: &ast::MemberExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    // Computed property: arr[0], arr[i] → Expr::Index or tuple.N → Expr::FieldAccess
    if let ast::MemberProp::Computed(computed) = &member.prop {
        // Cat A: receiver object
        let object = convert_expr(&member.obj, tctx, reg, type_env, synthetic)?;

        // Tuple index access: pair[0] → pair.0 (Rust uses dot notation for tuples)
        if let Some(RustType::Tuple(_)) = get_expr_type(tctx, &member.obj) {
            if let ast::Expr::Lit(ast::Lit::Num(num)) = &*computed.expr {
                let idx = num.value as usize;
                return Ok(Expr::FieldAccess {
                    object: Box::new(object),
                    field: idx.to_string(),
                });
            }
        }

        // Cat A: computed index
        let index = convert_expr(&computed.expr, tctx, reg, type_env, synthetic)?;
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
    if let Some(RustType::Named { name, .. }) = get_expr_type(tctx, &member.obj) {
        if let Some(TypeDef::Enum {
            tag_field: Some(tag),
            variant_fields,
            ..
        }) = reg.get(name)
        {
            if field == *tag {
                // Tag field → method call (e.g., s.kind() )
                // Cat A: receiver object
                let object = convert_expr(&member.obj, tctx, reg, type_env, synthetic)?;
                return Ok(Expr::MethodCall {
                    object: Box::new(object),
                    method: tag.clone(),
                    args: vec![],
                });
            }
            // Non-tag field: if bound in TypeEnv (match arm destructuring),
            // clone the reference (match on &obj binds fields by reference)
            if type_env.get(&field).is_some() {
                return Ok(Expr::MethodCall {
                    object: Box::new(Expr::Ident(field)),
                    method: "clone".to_string(),
                    args: vec![],
                });
            }
            // Standalone field access → inline match expression
            return convert_du_standalone_field_access(
                &member.obj,
                name,
                &field,
                variant_fields,
                tctx,
                reg,
                type_env,
                synthetic,
            );
        }
    }

    // Cat A: receiver object
    let object = convert_expr(&member.obj, tctx, reg, type_env, synthetic)?;
    resolve_member_access(&object, &field, &member.obj, reg)
}

/// Discriminated union の standalone フィールドアクセスを inline match 式に変換する。
///
/// `s.radius` → `match &s { Shape::Circle { radius, .. } => radius.clone(), _ => panic!("...") }`
pub(super) fn convert_du_standalone_field_access(
    obj_expr: &ast::Expr,
    enum_name: &str,
    field: &str,
    variant_fields: &std::collections::HashMap<String, Vec<(String, RustType)>>,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    // Cat A: receiver object
    let object = convert_expr(obj_expr, tctx, reg, type_env, synthetic)?;
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
