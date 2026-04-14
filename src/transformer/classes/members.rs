//! フィールド・メソッド・プロパティ変換 (impl Transformer)。

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use super::helpers::{body_has_self_assignment, resolve_member_visibility};
use crate::ir::{
    sanitize_field_name, AssocConst, Expr, Method, Param, RustType, Stmt, StructField, Visibility,
};
use crate::pipeline::type_converter::convert_ts_type;
use crate::transformer::extract_prop_name;
use crate::transformer::functions::convert_last_return_to_tail;
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Converts a static class property to an associated constant.
    ///
    /// Returns `None` if the property has no initializer (cannot become a const without a value).
    pub(super) fn convert_static_prop(
        &mut self,
        prop: &ast::ClassProp,
        vis: &Visibility,
    ) -> Result<Option<AssocConst>> {
        let name = extract_prop_name(&prop.key)
            .map_err(|_| anyhow!("unsupported static property key (only identifiers)"))?;

        let type_ann = prop
            .type_ann
            .as_ref()
            .ok_or_else(|| anyhow!("static property '{}' has no type annotation", name))?;
        let ty = convert_ts_type(&type_ann.type_ann, self.synthetic, self.reg())?;

        let value = match &prop.value {
            Some(init) => self.spawn_nested_scope().convert_expr(init)?,
            None => return Ok(None), // No initializer — skip
        };

        Ok(Some(AssocConst {
            vis: *vis,
            name,
            ty,
            value,
        }))
    }

    /// Converts a class property to a struct field.
    pub(super) fn convert_class_prop(
        &mut self,
        prop: &ast::ClassProp,
        class_vis: &Visibility,
    ) -> Result<StructField> {
        let field_name = extract_prop_name(&prop.key)
            .map_err(|_| anyhow!("unsupported class property key (only identifiers)"))?;

        let ty = match prop.type_ann.as_ref() {
            Some(ann) => convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?,
            None => RustType::Any, // Fallback to Any for unannotated class properties
        };
        let member_vis = resolve_member_visibility(prop.accessibility, class_vis);

        Ok(StructField {
            vis: Some(member_vis),
            name: sanitize_field_name(&field_name),
            ty,
        })
    }

    /// Converts a constructor to a `new()` associated function.
    ///
    /// Returns the method and any additional struct fields extracted from
    /// `TsParamProp` (parameter properties like `constructor(public x: number)`).
    pub(super) fn convert_constructor(
        &mut self,
        ctor: &ast::Constructor,
        vis: &Visibility,
    ) -> Result<(Method, Vec<StructField>)> {
        let mut params = Vec::new();
        let mut param_prop_fields = Vec::new();
        // Names of parameter properties — used to inject `this.field = param`
        // assignments into the constructor body.
        let mut param_prop_names = Vec::new();

        let mut default_expansion_stmts = Vec::new();
        for param in &ctor.params {
            match param {
                ast::ParamOrTsParamProp::Param(p) => {
                    let (param, expansion) = self.convert_param_pat(&p.pat)?;
                    default_expansion_stmts.extend(expansion);
                    params.push(param);
                }
                ast::ParamOrTsParamProp::TsParamProp(prop) => {
                    let (ir_param, field) = self.convert_ts_param_prop(prop, vis)?;
                    param_prop_names.push(ir_param.name.clone());
                    params.push(ir_param);
                    param_prop_fields.push(field);
                }
            }
        }

        let body = match &ctor.body {
            Some(block) => {
                // Prepend synthetic `this.field = field` for each param prop
                // before the original body statements, then run through the
                // existing constructor body conversion which recognises
                // `this.field = value` patterns and folds them into `Self { ... }`.
                let synthetic_stmts = build_param_prop_assignments(&param_prop_names);
                let all_stmts: Vec<ast::Stmt> = synthetic_stmts
                    .into_iter()
                    .chain(block.stmts.iter().cloned())
                    .collect();
                let mut body = self.convert_constructor_body(&all_stmts)?;
                // Insert default parameter expansion stmts at the beginning
                for (i, stmt) in default_expansion_stmts.into_iter().enumerate() {
                    body.insert(i, stmt);
                }
                Some(body)
            }
            None if !param_prop_names.is_empty() => {
                let synthetic_stmts = build_param_prop_assignments(&param_prop_names);
                let mut body = self.convert_constructor_body(&synthetic_stmts)?;
                for (i, stmt) in default_expansion_stmts.into_iter().enumerate() {
                    body.insert(i, stmt);
                }
                Some(body)
            }
            None if !default_expansion_stmts.is_empty() => {
                // No body but has default params — need to create body with expansion stmts
                Some(default_expansion_stmts)
            }
            None => None,
        };

        let method = Method {
            vis: *vis,
            name: "new".to_string(),
            is_async: false,
            has_self: false,
            has_mut_self: false,
            params,
            return_type: Some(RustType::Named {
                name: "Self".to_string(),
                type_args: vec![],
            }),
            body,
        };
        Ok((method, param_prop_fields))
    }

    /// Converts a `TsParamProp` into an IR parameter and a struct field.
    fn convert_ts_param_prop(
        &mut self,
        prop: &ast::TsParamProp,
        class_vis: &Visibility,
    ) -> Result<(Param, StructField)> {
        let (name, ty) = match &prop.param {
            ast::TsParamPropParam::Ident(ident) => {
                let ir_param = self.convert_ident_to_param(ident)?;
                (ir_param.name, ir_param.ty)
            }
            ast::TsParamPropParam::Assign(assign) => {
                // `public x: number = 42` — extract name and type from the left side
                match assign.left.as_ref() {
                    ast::Pat::Ident(ident) => {
                        let ir_param = self.convert_ident_to_param(ident)?;
                        (ir_param.name, ir_param.ty)
                    }
                    _ => return Err(anyhow!("unsupported parameter property pattern")),
                }
            }
        };

        let field_vis = resolve_member_visibility(prop.accessibility, class_vis);

        let field = StructField {
            vis: Some(field_vis),
            name: sanitize_field_name(&name),
            ty: ty.clone().unwrap_or(RustType::Any),
        };

        let param = Param { name, ty };

        Ok((param, field))
    }

    /// Converts constructor body statements.
    ///
    /// Recognizes the pattern of `this.field = value` assignments and converts them
    /// into a `Self { field: value, ... }` tail expression. Statements that don't
    /// match this pattern are converted as normal statements.
    fn convert_constructor_body(&mut self, stmts: &[ast::Stmt]) -> Result<Vec<Stmt>> {
        let mut fields = Vec::new();
        let mut other_stmts = Vec::new();

        // Sub-Transformer for constructor body.
        // TypeResolver handles parameter types via scope_stack.
        let mut sub_t = self.spawn_nested_scope();
        for stmt in stmts {
            if let Some((field_name, value_expr)) = try_extract_this_assignment(stmt) {
                let value = sub_t.convert_expr(value_expr)?;
                fields.push((field_name, value));
            } else {
                other_stmts.extend(sub_t.convert_stmt(stmt, None)?);
            }
        }

        if !fields.is_empty() {
            other_stmts.push(Stmt::Return(Some(Expr::StructInit {
                name: "Self".to_string(),
                fields,
                base: None,
            })));
        }

        convert_last_return_to_tail(&mut other_stmts);
        Ok(other_stmts)
    }

    /// Converts a class method (including getters/setters) to an impl method.
    ///
    /// - `MethodKind::Getter` → `fn name(&self) -> T { ... }`
    /// - `MethodKind::Setter` → `fn set_name(&mut self, v: T) { ... }`
    /// - `MethodKind::Method` → `fn name(&self, ...) -> T { ... }`
    pub(super) fn convert_class_method(
        &mut self,
        method: &ast::ClassMethod,
        vis: &Visibility,
    ) -> Result<Method> {
        let raw_name = extract_prop_name(&method.key)
            .map_err(|_| anyhow!("unsupported method key (only identifiers)"))?;
        let member_vis = resolve_member_visibility(method.accessibility, vis);
        self.build_method(
            raw_name,
            member_vis,
            method.kind,
            &method.function,
            method.is_static,
        )
    }

    /// Converts a private method (`#method()`) to a non-pub impl method.
    ///
    /// ECMAScript private methods (`#name`) are converted to Rust module-private
    /// methods (no `pub` modifier). The `#` prefix is stripped from the name.
    pub(super) fn convert_private_method(&mut self, pm: &ast::PrivateMethod) -> Result<Method> {
        let name = pm.key.name.to_string();
        self.build_method(
            name,
            Visibility::Private,
            pm.kind,
            &pm.function,
            pm.is_static,
        )
    }

    /// Shared implementation for converting a TS method (public or private) to an IR [`Method`].
    fn build_method(
        &mut self,
        raw_name: String,
        vis: Visibility,
        kind: ast::MethodKind,
        function: &ast::Function,
        is_static: bool,
    ) -> Result<Method> {
        // I-383 T8: メソッドの generic 型パラメータを scope に append する。
        // append-merge 意味論なので、外部の class type_params (`extract_class_info` で push 済)
        // と method 自身の type_params が両方アクティブになる。class 内のメソッド呼び出し終端で
        // 内部 method type_params だけが restore される。
        let method_tp_names: Vec<String> = function
            .type_params
            .as_ref()
            .map(|tpd| tpd.params.iter().map(|p| p.name.sym.to_string()).collect())
            .unwrap_or_default();
        let prev_method_scope = self.synthetic.push_type_param_scope(method_tp_names);
        let result = self.build_method_inner(raw_name, vis, kind, function, is_static);
        self.synthetic.restore_type_param_scope(prev_method_scope);
        result
    }

    fn build_method_inner(
        &mut self,
        raw_name: String,
        vis: Visibility,
        kind: ast::MethodKind,
        function: &ast::Function,
        is_static: bool,
    ) -> Result<Method> {
        let is_setter = kind == ast::MethodKind::Setter;
        let name = if is_setter {
            format!("set_{raw_name}")
        } else {
            raw_name
        };

        let mut params = Vec::new();
        let mut default_expansion_stmts = Vec::new();
        for param in &function.params {
            let (p, expansion) = self.convert_param_pat(&param.pat)?;
            default_expansion_stmts.extend(expansion);
            params.push(p);
        }

        let return_type = function
            .return_type
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()))
            .transpose()?
            .map(|ty| {
                // Unwrap Promise<T> → T for async methods
                if function.is_async {
                    ty.unwrap_promise()
                } else {
                    ty
                }
            })
            .and_then(|ty| {
                if matches!(ty, RustType::Unit) {
                    None
                } else {
                    Some(ty)
                }
            });

        let body = match &function.body {
            Some(block) => {
                let mut sub_t = self.spawn_nested_scope();
                let mut stmts = default_expansion_stmts;
                for stmt in &block.stmts {
                    stmts.extend(sub_t.convert_stmt(stmt, return_type.as_ref())?);
                }
                convert_last_return_to_tail(&mut stmts);
                Some(stmts)
            }
            None => None,
        };

        let body_stmts = body.as_deref().unwrap_or(&[]);
        let needs_mut = is_setter || body_has_self_assignment(body_stmts);

        Ok(Method {
            vis,
            name,
            is_async: function.is_async,
            has_self: !is_static,
            has_mut_self: !is_static && needs_mut,
            params,
            return_type,
            body,
        })
    }

    /// Converts a private property (`#field`) to a non-pub struct field.
    ///
    /// The `#` prefix is stripped. Visibility is set to `Private`.
    pub(super) fn convert_private_prop(&mut self, pp: &ast::PrivateProp) -> Result<StructField> {
        let field_name = pp.key.name.to_string();
        let ty = match pp.type_ann.as_ref() {
            Some(ann) => convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?,
            None => RustType::Any,
        };
        Ok(StructField {
            vis: Some(Visibility::Private),
            name: sanitize_field_name(&field_name),
            ty,
        })
    }

    /// Converts a static block (`static { ... }`) to a `_init_static()` method.
    ///
    /// The block's statements become the method body. The method is static
    /// (`has_self: false`) and non-pub.
    pub(super) fn convert_static_block(&mut self, sb: &ast::StaticBlock) -> Result<Method> {
        let mut stmts = Vec::new();
        for stmt in &sb.body.stmts {
            stmts.extend(self.convert_stmt(stmt, None)?);
        }
        Ok(Method {
            vis: Visibility::Private,
            name: "_init_static".to_string(),
            is_async: false,
            has_self: false,
            has_mut_self: false,
            params: vec![],
            return_type: None,
            body: Some(stmts),
        })
    }

    /// Converts a parameter pattern into an IR [`Param`] and optional expansion statements.
    ///
    /// Supports:
    /// - `x: number` → simple parameter
    /// - `x: number = 0` → `Option<f64>` + `let x = x.unwrap_or(0.0);`
    /// - `options: Options = {}` → `Option<Options>` + `let options = options.unwrap_or_default();`
    fn convert_param_pat(&mut self, pat: &ast::Pat) -> Result<(Param, Vec<Stmt>)> {
        match pat {
            ast::Pat::Ident(ident) => {
                let param = self.convert_ident_to_param(ident)?;
                Ok((param, vec![]))
            }
            ast::Pat::Assign(assign) => {
                // Default value parameter: x: T = value
                match assign.left.as_ref() {
                    ast::Pat::Ident(ident) => {
                        let inner_param = self.convert_ident_to_param(ident)?;
                        let param_name = inner_param.name.clone();
                        let inner_type = inner_param.ty.ok_or_else(|| {
                            anyhow!("default parameter requires a type annotation")
                        })?;
                        // `wrap_optional` guarantees idempotency so that `x?: T = value`
                        // (rare but valid TS) doesn't become `Option<Option<T>>` —
                        // `convert_ident_to_param` already applied the optional wrap.
                        let option_type = inner_type.wrap_optional();

                        let (default_expr, use_unwrap_or_default) =
                            self.convert_default_value(&assign.right)?;

                        let unwrap_call = if use_unwrap_or_default {
                            Expr::MethodCall {
                                object: Box::new(Expr::Ident(param_name.clone())),
                                method: "unwrap_or_default".to_string(),
                                args: vec![],
                            }
                        } else {
                            crate::transformer::build_option_unwrap_with_default(
                                Expr::Ident(param_name.clone()),
                                default_expr.unwrap(),
                            )
                        };

                        let expansion_stmt = Stmt::Let {
                            mutable: false,
                            name: param_name.clone(),
                            ty: None,
                            init: Some(unwrap_call),
                        };

                        Ok((
                            Param {
                                name: param_name,
                                ty: Some(option_type),
                            },
                            vec![expansion_stmt],
                        ))
                    }
                    _ => Err(anyhow!("unsupported parameter pattern")),
                }
            }
            _ => Err(anyhow!("unsupported parameter pattern")),
        }
    }

    /// Converts an identifier parameter pattern into an IR [`Param`].
    ///
    /// Extracts name and type annotation from a `BindingIdent`, converts the type,
    /// and returns a `Param`. Used by both function and class method parameter conversion.
    /// `?:` optional parameters are wrapped in `Option<T>` via [`RustType::wrap_if_optional`].
    fn convert_ident_to_param(&mut self, ident: &ast::BindingIdent) -> Result<Param> {
        let name = ident.id.sym.to_string();
        let ty = ident
            .type_ann
            .as_ref()
            .ok_or_else(|| anyhow!("parameter '{}' has no type annotation", name))?;
        let rust_type = crate::pipeline::type_converter::convert_type_for_position(
            &ty.type_ann,
            crate::transformer::TypePosition::Param,
            self.synthetic,
            self.reg(),
        )?
        .wrap_if_optional(ident.id.optional);
        Ok(Param {
            name,
            ty: Some(rust_type),
        })
    }
}

/// Builds synthetic `this.<name> = <name>` assignment statements (SWC AST nodes).
///
/// These are prepended to the constructor body before conversion, so the
/// existing `try_extract_this_assignment` logic handles them uniformly.
fn build_param_prop_assignments(names: &[String]) -> Vec<ast::Stmt> {
    use swc_common::DUMMY_SP;

    names
        .iter()
        .map(|name| {
            ast::Stmt::Expr(ast::ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(ast::Expr::Assign(ast::AssignExpr {
                    span: DUMMY_SP,
                    op: ast::AssignOp::Assign,
                    left: ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(
                        ast::MemberExpr {
                            span: DUMMY_SP,
                            obj: Box::new(ast::Expr::This(ast::ThisExpr { span: DUMMY_SP })),
                            prop: ast::MemberProp::Ident(ast::IdentName {
                                span: DUMMY_SP,
                                sym: name.clone().into(),
                            }),
                        },
                    )),
                    right: Box::new(ast::Expr::Ident(ast::Ident {
                        span: DUMMY_SP,
                        ctxt: swc_common::SyntaxContext::empty(),
                        sym: name.clone().into(),
                        optional: false,
                    })),
                })),
            })
        })
        .collect()
}

/// Tries to extract a `this.field = value` pattern from a statement.
///
/// Returns `Some((field_name, value_expr))` if the statement matches,
/// `None` otherwise.
fn try_extract_this_assignment(stmt: &ast::Stmt) -> Option<(String, &ast::Expr)> {
    let expr_stmt = match stmt {
        ast::Stmt::Expr(e) => e,
        _ => return None,
    };
    let assign = match expr_stmt.expr.as_ref() {
        ast::Expr::Assign(a) => a,
        _ => return None,
    };
    let member = match &assign.left {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(m)) => m,
        _ => return None,
    };
    // Check that the object is `this`
    if !matches!(member.obj.as_ref(), ast::Expr::This(_)) {
        return None;
    }
    let field_name = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    Some((field_name, assign.right.as_ref()))
}
