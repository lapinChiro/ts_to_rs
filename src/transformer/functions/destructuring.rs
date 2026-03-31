//! Object destructuring parameter conversion from SWC TypeScript AST to IR.
//!
//! Handles `{ x, y }: Point` style parameters, nested destructuring,
//! rest patterns (`...rest`), and default values within destructured fields.

use super::*;

impl<'a> Transformer<'a> {
    /// Converts an object destructuring parameter pattern into a synthetic [`Param`]
    /// and expansion statements.
    ///
    /// Example: `{ x, y }: Point` → param `point: Point` + `let x = point.x; let y = point.y;`
    pub(crate) fn convert_object_destructuring_param(
        &mut self,
        obj_pat: &ast::ObjectPat,
    ) -> Result<(Param, Vec<Stmt>)> {
        let rust_type = if let Some(type_ann) = obj_pat.type_ann.as_ref() {
            convert_ts_type(&type_ann.type_ann, self.synthetic, self.reg())?
        } else {
            // No type annotation — fallback to serde_json::Value
            RustType::Named {
                name: "serde_json::Value".to_string(),
                type_args: vec![],
            }
        };

        // Generate parameter name from type name (PascalCase → snake_case)
        let param_name = match &rust_type {
            RustType::Named { name, .. } => pascal_to_snake(name),
            _ => "param".to_string(),
        };

        // Keep a reference to the type for rest pattern expansion before moving into param
        let rust_type_ref = rust_type.clone();
        let param = Param {
            name: param_name.clone(),
            ty: Some(rust_type),
        };
        let rust_type = rust_type_ref;

        let mut stmts = Vec::new();
        for prop in &obj_pat.props {
            match prop {
                ast::ObjectPatProp::Assign(assign) => {
                    // { x } or { x = default } — shorthand with optional default
                    let field_name = assign.key.sym.to_string();
                    let field_access = Expr::FieldAccess {
                        object: Box::new(Expr::Ident(param_name.clone())),
                        field: field_name.clone(),
                    };
                    let init_expr = if let Some(default_expr) = &assign.value {
                        // Cat B: field type could be looked up from struct definition
                        let default_ir = crate::transformer::Transformer {
                            tctx: self.tctx,

                            synthetic: self.synthetic,
                            mut_method_names: self.mut_method_names.clone(),
                        }
                        .convert_expr(default_expr)?;
                        match &default_ir {
                            Expr::MethodCall { method, .. } if method == "to_string" => {
                                Expr::MethodCall {
                                    object: Box::new(field_access),
                                    method: "unwrap_or_else".to_string(),
                                    args: vec![Expr::Closure {
                                        params: vec![],
                                        return_type: None,
                                        body: crate::ir::ClosureBody::Expr(Box::new(default_ir)),
                                    }],
                                }
                            }
                            Expr::StringLit(_) => Expr::MethodCall {
                                object: Box::new(field_access),
                                method: "unwrap_or_else".to_string(),
                                args: vec![Expr::Closure {
                                    params: vec![],
                                    return_type: None,
                                    body: crate::ir::ClosureBody::Expr(Box::new(default_ir)),
                                }],
                            },
                            _ => Expr::MethodCall {
                                object: Box::new(field_access),
                                method: "unwrap_or".to_string(),
                                args: vec![default_ir],
                            },
                        }
                    } else {
                        field_access
                    };
                    stmts.push(Stmt::Let {
                        mutable: false,
                        name: field_name,
                        ty: None,
                        init: Some(init_expr),
                    });
                }
                ast::ObjectPatProp::KeyValue(kv) => {
                    let field_name = extract_prop_name(&kv.key)
                        .map_err(|_| anyhow!("unsupported destructuring key"))?;
                    let nested_source = Expr::FieldAccess {
                        object: Box::new(Expr::Ident(param_name.clone())),
                        field: field_name.clone(),
                    };
                    match kv.value.as_ref() {
                        // { a: { b, c } } — nested destructuring
                        ast::Pat::Object(inner_pat) => {
                            let field_type = self.lookup_field_type(&rust_type, &field_name);
                            self.expand_fn_param_object_props(
                                &inner_pat.props,
                                &nested_source,
                                &mut stmts,
                                field_type.as_ref(),
                            )?;
                        }
                        // { x: newX } — rename
                        _ => {
                            let binding_name = extract_pat_ident_name(kv.value.as_ref())
                                .map_err(|_| anyhow!("unsupported destructuring value pattern"))?;
                            let binding_name = pascal_to_snake(&binding_name);
                            stmts.push(Stmt::Let {
                                mutable: false,
                                name: binding_name,
                                ty: None,
                                init: Some(nested_source),
                            });
                        }
                    }
                }
                ast::ObjectPatProp::Rest(rest) => {
                    let source_expr = Expr::Ident(param_name.clone());
                    self.expand_rest_as_synthetic_struct(
                        rest,
                        &obj_pat.props,
                        &source_expr,
                        Some(&rust_type),
                        &mut stmts,
                    )?;
                }
            }
        }

        Ok((param, stmts))
    }

    /// Recursively expands nested object destructuring properties for function parameters.
    ///
    /// `parent_type` is the RustType of the object being destructured at this nesting level.
    /// It is used to look up remaining field types when expanding rest patterns (`...rest`).
    fn expand_fn_param_object_props(
        &mut self,
        props: &[ast::ObjectPatProp],
        source_expr: &Expr,
        stmts: &mut Vec<Stmt>,
        parent_type: Option<&RustType>,
    ) -> Result<()> {
        for prop in props {
            match prop {
                ast::ObjectPatProp::Assign(assign) => {
                    let field_name = assign.key.sym.to_string();
                    let field_access = Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
                        field: field_name.clone(),
                    };
                    let init_expr = if let Some(default_expr) = &assign.value {
                        // Cat B: field type could be looked up from struct definition
                        let default_ir = crate::transformer::Transformer {
                            tctx: self.tctx,

                            synthetic: self.synthetic,
                            mut_method_names: self.mut_method_names.clone(),
                        }
                        .convert_expr(default_expr)?;
                        match &default_ir {
                            Expr::MethodCall { method, .. } if method == "to_string" => {
                                Expr::MethodCall {
                                    object: Box::new(field_access),
                                    method: "unwrap_or_else".to_string(),
                                    args: vec![Expr::Closure {
                                        params: vec![],
                                        return_type: None,
                                        body: crate::ir::ClosureBody::Expr(Box::new(default_ir)),
                                    }],
                                }
                            }
                            Expr::StringLit(_) => Expr::MethodCall {
                                object: Box::new(field_access),
                                method: "unwrap_or_else".to_string(),
                                args: vec![Expr::Closure {
                                    params: vec![],
                                    return_type: None,
                                    body: crate::ir::ClosureBody::Expr(Box::new(default_ir)),
                                }],
                            },
                            _ => Expr::MethodCall {
                                object: Box::new(field_access),
                                method: "unwrap_or".to_string(),
                                args: vec![default_ir],
                            },
                        }
                    } else {
                        field_access
                    };
                    stmts.push(Stmt::Let {
                        mutable: false,
                        name: field_name,
                        ty: None,
                        init: Some(init_expr),
                    });
                }
                ast::ObjectPatProp::KeyValue(kv) => {
                    let field_name = extract_prop_name(&kv.key)
                        .map_err(|_| anyhow!("unsupported destructuring key"))?;
                    let nested_source = Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
                        field: field_name.clone(),
                    };
                    match kv.value.as_ref() {
                        ast::Pat::Object(inner_pat) => {
                            let nested_type =
                                parent_type.and_then(|pt| self.lookup_field_type(pt, &field_name));
                            self.expand_fn_param_object_props(
                                &inner_pat.props,
                                &nested_source,
                                stmts,
                                nested_type.as_ref(),
                            )?;
                        }
                        _ => {
                            let binding_name = extract_pat_ident_name(kv.value.as_ref())
                                .map_err(|_| anyhow!("unsupported destructuring value pattern"))?;
                            let binding_name = pascal_to_snake(&binding_name);
                            stmts.push(Stmt::Let {
                                mutable: false,
                                name: binding_name,
                                ty: None,
                                init: Some(nested_source),
                            });
                        }
                    }
                }
                ast::ObjectPatProp::Rest(rest) => {
                    self.expand_rest_as_synthetic_struct(
                        rest,
                        props,
                        source_expr,
                        parent_type,
                        stmts,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Looks up a field's RustType from a parent struct type via TypeRegistry.
    ///
    /// Handles `Option<Named>` unwrapping and generic type instantiation,
    /// consistent with `resolve_field_type` in `type_resolution.rs`.
    fn lookup_field_type(&self, parent_type: &RustType, field_name: &str) -> Option<RustType> {
        let (type_name, type_args) = match parent_type {
            RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
            RustType::Option(inner) => match inner.as_ref() {
                RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
                _ => return None,
            },
            _ => return None,
        };
        let type_def = if type_args.is_empty() {
            self.reg().get(type_name)?.clone()
        } else {
            self.reg().instantiate(type_name, type_args)?
        };
        match &type_def {
            crate::registry::TypeDef::Struct { fields, .. } => fields
                .iter()
                .find(|(name, _)| name == field_name)
                .map(|(_, ty)| ty.clone()),
            _ => None,
        }
    }

    /// Expands a rest pattern (`...rest`) into a synthetic struct initialization.
    ///
    /// Creates a synthetic struct containing the remaining fields (those not explicitly
    /// destructured by sibling patterns), and generates a `let rest = RestStruct { ... }`.
    fn expand_rest_as_synthetic_struct(
        &mut self,
        rest: &ast::RestPat,
        sibling_props: &[ast::ObjectPatProp],
        source_expr: &Expr,
        parent_type: Option<&RustType>,
        stmts: &mut Vec<Stmt>,
    ) -> Result<()> {
        use swc_common::Spanned;

        let rest_name = extract_pat_ident_name(&rest.arg)
            .map_err(|_| anyhow!("unsupported rest pattern binding"))?;
        let rest_name = pascal_to_snake(&rest_name);

        // Determine parent struct type (with generic instantiation support)
        let (type_name, type_args) = parent_type
            .and_then(|t| match t {
                RustType::Named { name, type_args } => Some((name.as_str(), type_args.as_slice())),
                RustType::Option(inner) => match inner.as_ref() {
                    RustType::Named { name, type_args } => {
                        Some((name.as_str(), type_args.as_slice()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .ok_or_else(|| {
                UnsupportedSyntaxError::new(
                    "rest in nested destructuring requires known struct type",
                    rest.span(),
                )
            })?;

        let type_def = if type_args.is_empty() {
            self.reg().get(type_name).cloned()
        } else {
            self.reg().instantiate(type_name, type_args)
        };
        let struct_fields = match type_def {
            Some(crate::registry::TypeDef::Struct { fields, .. }) => fields,
            _ => {
                return Err(UnsupportedSyntaxError::new(
                    format!(
                        "rest in nested destructuring: type '{type_name}' not found in registry"
                    ),
                    rest.span(),
                )
                .into());
            }
        };

        // Collect explicitly destructured field names from sibling props
        let explicit_fields: Vec<String> = sibling_props
            .iter()
            .filter_map(|p| match p {
                ast::ObjectPatProp::Assign(a) => Some(a.key.sym.to_string()),
                ast::ObjectPatProp::KeyValue(kv) => extract_prop_name(&kv.key).ok(),
                _ => None,
            })
            .collect();

        // Remaining fields = struct fields - explicit fields
        let remaining_fields: Vec<(String, RustType)> = struct_fields
            .into_iter()
            .filter(|(name, _)| !explicit_fields.contains(name))
            .collect();

        // Register synthetic rest struct (content-based deduplication)
        let rest_struct_name = self.synthetic.register_inline_struct(&remaining_fields);

        // Create StructInit expression: RestStruct { field: source.field, ... }
        let init_fields: Vec<(String, Expr)> = remaining_fields
            .iter()
            .map(|(name, _)| {
                (
                    name.clone(),
                    Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
                        field: name.clone(),
                    },
                )
            })
            .collect();

        stmts.push(Stmt::Let {
            mutable: false,
            name: rest_name,
            ty: None,
            init: Some(Expr::StructInit {
                name: rest_struct_name,
                fields: init_fields,
                base: None,
            }),
        });

        Ok(())
    }
}
