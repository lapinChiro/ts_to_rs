//! Function parameter conversion from SWC TypeScript AST to IR.
//!
//! Handles simple identifier parameters, inline type literals, default parameters,
//! and rest parameters.

use super::*;

impl<'a> Transformer<'a> {
    /// Converts a function parameter pattern into an IR [`Param`] and optional expansion
    /// statements.
    ///
    /// For simple identifier parameters, returns the param with no expansion.
    /// For object destructuring parameters (`{ x, y }: Point`), returns a synthetic
    /// parameter (named from the type annotation) and `let` statements to expand the fields.
    ///
    /// When `resilient` is true, unsupported type annotations fall back to [`RustType::Any`].
    pub(super) fn convert_param(
        &mut self,
        pat: &ast::Pat,
        fn_name: &str,
        vis: Visibility,
        resilient: bool,
        fallback_warnings: &mut Vec<String>,
    ) -> Result<(Param, Vec<Stmt>, Vec<Item>)> {
        match pat {
            ast::Pat::Ident(ident) => {
                let param_name = ident.id.sym.to_string();
                let optional = ident.id.optional;
                let ty = match ident.type_ann.as_ref() {
                    Some(ann) => ann,
                    None => {
                        // No type annotation — fallback to Any
                        let rust_type = RustType::Any.wrap_if_optional(optional);
                        return Ok((
                            Param {
                                name: param_name,
                                ty: Some(rust_type),
                            },
                            vec![],
                            vec![],
                        ));
                    }
                };

                // Check if the type annotation is an inline type literal
                if let Ok(crate::ts_type_info::TsTypeInfo::TypeLiteral(lit)) =
                    crate::ts_type_info::convert_to_ts_type_info(&ty.type_ann)
                {
                    let struct_name = to_pascal_case(&format!("{fn_name}_{param_name}"));
                    let fields =
                        crate::ts_type_info::resolve::intersection::resolve_type_literal_fields(
                            &lit,
                            self.reg(),
                            self.synthetic,
                        )?;
                    let struct_item = Item::Struct {
                        vis,
                        name: struct_name.clone(),
                        type_params: vec![],
                        fields,
                        is_unit_struct: false,
                    };
                    let rust_type = RustType::Named {
                        name: struct_name,
                        type_args: vec![],
                    }
                    .wrap_if_optional(optional);
                    return Ok((
                        Param {
                            name: param_name,
                            ty: Some(rust_type),
                        },
                        vec![],
                        vec![struct_item],
                    ));
                }

                let rust_type =
                    self.convert_ts_type_with_fallback(&ty.type_ann, resilient, fallback_warnings)?;
                // Trait types in parameter position → &dyn Trait
                let rust_type = wrap_trait_for_position(rust_type, TypePosition::Param, self.reg())
                    .wrap_if_optional(optional);
                Ok((
                    Param {
                        name: param_name,
                        ty: Some(rust_type),
                    },
                    vec![],
                    vec![],
                ))
            }
            ast::Pat::Object(obj_pat) => {
                let (param, stmts) = self.convert_object_destructuring_param(obj_pat)?;
                Ok((param, stmts, vec![]))
            }
            ast::Pat::Assign(assign) => {
                self.convert_default_param(assign, fn_name, vis, resilient, fallback_warnings)
            }
            ast::Pat::Rest(rest) => {
                if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                    let name = ident.id.sym.to_string();
                    let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                    let rust_type = type_ann
                        .map(|ann| {
                            self.convert_ts_type_with_fallback(
                                &ann.type_ann,
                                resilient,
                                fallback_warnings,
                            )
                        })
                        .transpose()?;
                    Ok((
                        Param {
                            name,
                            ty: rust_type,
                        },
                        vec![],
                        vec![],
                    ))
                } else {
                    Err(anyhow!("unsupported rest parameter pattern"))
                }
            }
            _ => Err(anyhow!("unsupported parameter pattern")),
        }
    }

    /// Converts a parameter with a default value into an `Option<T>` parameter
    /// with an `unwrap_or` / `unwrap_or_default` expansion statement.
    ///
    /// Example: `(x: number = 0)` → param `x: Option<f64>` + `let x = x.unwrap_or(0.0);`
    fn convert_default_param(
        &mut self,
        assign: &ast::AssignPat,
        fn_name: &str,
        vis: Visibility,
        resilient: bool,
        fallback_warnings: &mut Vec<String>,
    ) -> Result<(Param, Vec<Stmt>, Vec<Item>)> {
        // Recursively convert the inner parameter (left side)
        let (inner_param, inner_stmts, extra) =
            self.convert_param(&assign.left, fn_name, vis, resilient, fallback_warnings)?;

        let (param, stmts) =
            self.wrap_param_with_default(inner_param, inner_stmts, &assign.right)?;
        Ok((param, stmts, extra))
    }

    /// Wraps a converted inner parameter with `Option<T>` and generates
    /// the `unwrap_or` / `unwrap_or_default` expansion statement.
    ///
    /// Shared logic for both regular function and arrow/fn_expr default parameters.
    /// If the inner type is `None` or `Any`, infers the type from the default value literal.
    pub(crate) fn wrap_param_with_default(
        &mut self,
        inner_param: Param,
        mut inner_stmts: Vec<Stmt>,
        default_expr: &ast::Expr,
    ) -> Result<(Param, Vec<Stmt>)> {
        let param_name = inner_param.name.clone();

        // Wrap the type in Option<T>
        // If no type annotation (or Any fallback), infer from default value literal
        let inner_type = match inner_param.ty {
            Some(RustType::Any) | None => {
                infer_type_from_default(default_expr).unwrap_or(RustType::Any)
            }
            Some(ty) => ty,
        };
        // `wrap_optional` is idempotent, so `x?: T = value` (rare but valid TS)
        // stays `Option<T>` instead of becoming `Option<Option<T>>` — the inner
        // `convert_param` call already applied the optional wrap via
        // `wrap_if_optional(ident.id.optional)`.
        let option_type = inner_type.wrap_optional();

        // Convert default value to IR expression
        let (default_ir, use_unwrap_or_default) = self.convert_default_value(default_expr)?;

        // Generate expansion statement:
        //   `let x = x.unwrap_or_default();`       — for empty objects/arrays/new
        //   `let x = x.unwrap_or_else(|| expr);`   — for allocating expressions (strings, method calls)
        //   `let x = x.unwrap_or(value);`           — for cheap values (numbers, bools, idents)
        let unwrap_call = if use_unwrap_or_default {
            Expr::MethodCall {
                object: Box::new(Expr::Ident(param_name.clone())),
                method: "unwrap_or_default".to_string(),
                args: vec![],
            }
        } else {
            let default_ir = default_ir.unwrap();
            crate::transformer::build_option_unwrap_with_default(
                Expr::Ident(param_name.clone()),
                default_ir,
            )
        };

        inner_stmts.insert(
            0,
            Stmt::Let {
                mutable: false,
                name: param_name.clone(),
                ty: None,
                init: Some(unwrap_call),
            },
        );

        Ok((
            Param {
                name: param_name,
                ty: Some(option_type),
            },
            inner_stmts,
        ))
    }

    /// Converts a default value expression to an IR [`Expr`].
    ///
    /// Returns `(Some(expr), false)` for literal values (use `unwrap_or`),
    /// or `(None, true)` for empty objects (use `unwrap_or_default`).
    pub(crate) fn convert_default_value(
        &mut self,
        expr: &ast::Expr,
    ) -> Result<(Option<Expr>, bool)> {
        match expr {
            ast::Expr::Lit(lit) => match lit {
                ast::Lit::Num(n) => Ok((Some(Expr::NumberLit(n.value)), false)),
                ast::Lit::Str(s) => Ok((
                    Some(Expr::MethodCall {
                        object: Box::new(Expr::StringLit(s.value.to_string_lossy().into_owned())),
                        method: "to_string".to_string(),
                        args: vec![],
                    }),
                    false,
                )),
                ast::Lit::Bool(b) => Ok((Some(Expr::BoolLit(b.value)), false)),
                _ => Err(anyhow!("unsupported default parameter value")),
            },
            ast::Expr::Object(obj) if obj.props.is_empty() => {
                // `= {}` → unwrap_or_default()
                Ok((None, true))
            }
            ast::Expr::Ident(ident) => {
                // `= someVariable` → unwrap_or(someVariable)
                Ok((Some(Expr::Ident(ident.sym.to_string())), false))
            }
            ast::Expr::Array(arr) if arr.elems.is_empty() => {
                // `= []` → unwrap_or_default()
                Ok((None, true))
            }
            ast::Expr::New(_) => {
                // `= new Map()` → unwrap_or_default()
                Ok((None, true))
            }
            ast::Expr::Unary(unary)
                if unary.op == ast::UnaryOp::Minus
                    && matches!(unary.arg.as_ref(), ast::Expr::Lit(ast::Lit::Num(_))) =>
            {
                // `= -1` → unwrap_or(-1.0)
                if let ast::Expr::Lit(ast::Lit::Num(n)) = unary.arg.as_ref() {
                    Ok((Some(Expr::NumberLit(-n.value)), false))
                } else {
                    unreachable!()
                }
            }
            // General expression: use unwrap_or_else(|| expr) for any expression
            // that can be converted (e.g., console.log, function calls, member access)
            other => {
                let ir_expr = self.convert_expr(other)?;
                Ok((Some(ir_expr), false))
            }
        }
    }
}

/// Infers the type of a default parameter from its literal value.
///
/// - Number literal → `f64`
/// - String literal → `String`
/// - Boolean literal → `bool`
/// - Other expressions → `None`
fn infer_type_from_default(expr: &ast::Expr) -> Option<RustType> {
    match expr {
        ast::Expr::Lit(lit) => match lit {
            ast::Lit::Num(_) => Some(RustType::F64),
            ast::Lit::Str(_) => Some(RustType::String),
            ast::Lit::Bool(_) => Some(RustType::Bool),
            _ => None,
        },
        ast::Expr::Unary(unary)
            if unary.op == ast::UnaryOp::Minus
                && matches!(unary.arg.as_ref(), ast::Expr::Lit(ast::Lit::Num(_))) =>
        {
            Some(RustType::F64)
        }
        _ => None,
    }
}
