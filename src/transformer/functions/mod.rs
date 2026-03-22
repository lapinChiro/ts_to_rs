//! Function declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC function declarations into the IR [`Item::Fn`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, MatchArm, Param, RustType, Stmt, Visibility};
use crate::pipeline::type_converter::{
    convert_property_signature, convert_ts_type, extract_type_params,
};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::context::TransformContext;
use crate::transformer::statements::convert_stmt_list;
use crate::transformer::{
    extract_pat_ident_name, extract_prop_name, wrap_trait_for_position, TypeEnv, TypePosition,
};

/// Converts an SWC [`ast::FnDecl`] into an IR [`Item::Fn`].
///
/// Extracts the function name, parameters (with type annotations),
/// return type, and body statements.
///
/// # Errors
///
/// Returns an error if parameter patterns are unsupported, type annotations
/// are missing, or body statements fail to convert.
pub fn convert_fn_decl(
    fn_decl: &ast::FnDecl,
    vis: Visibility,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    resilient: bool,
    synthetic_out: &mut SyntheticTypeRegistry,
) -> Result<(Vec<Item>, Vec<String>)> {
    let name = fn_decl.ident.sym.to_string();
    let mut fallback_warnings = Vec::new();
    let mut items = Vec::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut params = Vec::new();
    let mut destructuring_stmts = Vec::new();
    for param in &fn_decl.function.params {
        let (p, stmts, extra) = convert_param(
            &param.pat,
            &name,
            vis.clone(),
            resilient,
            &mut fallback_warnings,
            tctx,
            reg,
            &mut synthetic,
        )?;
        params.push(p);
        destructuring_stmts.extend(stmts);
        items.extend(extra);
    }

    let is_async = fn_decl.function.is_async;

    let return_type = fn_decl
        .function
        .return_type
        .as_ref()
        .map(|ann| {
            convert_ts_type_with_fallback(
                &ann.type_ann,
                resilient,
                &mut fallback_warnings,
                &mut synthetic,
                tctx,
                reg,
            )
        })
        .transpose()?;

    // void → None (Rust omits `-> ()`)
    let return_type = return_type.and_then(|ty| {
        if matches!(ty, RustType::Unit) {
            None
        } else {
            Some(ty)
        }
    });

    // Unwrap Promise<T> → T for async functions (before body conversion
    // so that return type context propagates correctly)
    let return_type = if is_async {
        return_type.and_then(unwrap_promise_type)
    } else {
        return_type
    };

    // Trait types in return position → Box<dyn Trait>
    let return_type = return_type.map(|ty| wrap_trait_for_position(ty, TypePosition::Value, reg));

    // Lazy type materialization for `any`-typed parameters:
    // Scan body for typeof/instanceof usage and generate enum types
    if let Some(body) = &fn_decl.function.body {
        let any_param_names: Vec<String> = params
            .iter()
            .filter(|p| matches!(&p.ty, Some(RustType::Any)))
            .map(|p| p.name.clone())
            .collect();
        if !any_param_names.is_empty() {
            let constraints =
                crate::transformer::any_narrowing::collect_any_constraints(body, &any_param_names);
            for (param_name, constraint) in &constraints {
                if constraint.is_empty() {
                    continue;
                }
                let variants =
                    crate::transformer::any_narrowing::build_any_enum_variants(constraint);
                let enum_name = synthetic_out.register_any_enum(&name, param_name, variants);
                let enum_type = RustType::Named {
                    name: enum_name,
                    type_args: vec![],
                };
                if let Some(p) = params.iter_mut().find(|p| &p.name == param_name) {
                    p.ty = Some(enum_type);
                }
            }
        }
    }

    // Lazy type materialization for `any`-typed LOCAL variables:
    // Scan body for local variable declarations with `any` type and typeof checks
    let mut local_any_overrides: Vec<(String, RustType)> = Vec::new();
    if let Some(body) = &fn_decl.function.body {
        let local_any_names = crate::transformer::any_narrowing::collect_any_local_var_names(body);
        if !local_any_names.is_empty() {
            let constraints =
                crate::transformer::any_narrowing::collect_any_constraints(body, &local_any_names);
            for (var_name, constraint) in &constraints {
                if constraint.is_empty() {
                    continue;
                }
                let variants =
                    crate::transformer::any_narrowing::build_any_enum_variants(constraint);
                let enum_name = synthetic_out.register_any_enum(&name, var_name, variants);
                let enum_type = RustType::Named {
                    name: enum_name,
                    type_args: vec![],
                };
                local_any_overrides.push((var_name.clone(), enum_type));
            }
        }
    }

    let mut fn_type_env = TypeEnv::new();
    for p in &params {
        if let Some(ty) = &p.ty {
            fn_type_env.insert(p.name.clone(), ty.clone());
        }
    }
    // Pre-populate TypeEnv with local any-narrowing enum types so that
    // convert_var_decl uses the enum type instead of Any, and
    // convert_if_stmt generates if-let patterns.
    for (var_name, enum_type) in &local_any_overrides {
        fn_type_env.insert(var_name.clone(), enum_type.clone());
    }

    let body_stmts = match &fn_decl.function.body {
        Some(block) => convert_stmt_list(
            &block.stmts,
            tctx,
            reg,
            return_type.as_ref(),
            &mut fn_type_env,
            &mut synthetic,
        )?,
        None => Vec::new(),
    };
    // Prepend destructuring expansion statements
    let body = if destructuring_stmts.is_empty() {
        body_stmts
    } else {
        let mut combined = destructuring_stmts;
        combined.extend(body_stmts);
        combined
    };

    let type_params =
        extract_type_params(fn_decl.function.type_params.as_deref(), &mut synthetic, reg);

    // If the function body contains `throw`, wrap return type in Result and returns in Ok()
    let has_throw = fn_decl
        .function
        .body
        .as_ref()
        .is_some_and(|block| contains_throw(&block.stmts));

    let (return_type, mut body) = if has_throw {
        let ok_type = return_type.unwrap_or_else(|| RustType::Named {
            name: "()".to_string(),
            type_args: vec![],
        });
        let result_type = RustType::Result {
            ok: Box::new(ok_type),
            err: Box::new(RustType::String),
        };
        let wrapped_body = wrap_returns_in_ok(body);
        (Some(result_type), wrapped_body)
    } else {
        (return_type, body)
    };

    convert_last_return_to_tail(&mut body);
    let mut_rebindings = mark_mut_params_from_body(&body, &params);
    if !mut_rebindings.is_empty() {
        let mut new_body = mut_rebindings;
        new_body.extend(body);
        body = new_body;
    }

    let attributes = if is_async && name == "main" {
        vec!["tokio::main".to_string()]
    } else {
        vec![]
    };

    // Merge local synthetic types into the external registry
    synthetic_out.merge(synthetic);

    // any-type enum items は SyntheticTypeRegistry に登録済み（per-file synthetic 経由で出力される）

    items.push(Item::Fn {
        vis,
        attributes,
        is_async,
        name,
        type_params,
        params,
        return_type,
        body,
    });

    Ok((items, fallback_warnings))
}

/// Converts a TypeScript type to an IR type, falling back to [`RustType::Any`] when
/// `resilient` is true and the type is unsupported.
///
/// When falling back, appends the error message to `fallback_warnings` for reporting.
pub(crate) fn convert_ts_type_with_fallback(
    ts_type: &swc_ecma_ast::TsType,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    synthetic: &mut SyntheticTypeRegistry,
    _tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    match convert_ts_type(ts_type, synthetic, reg) {
        Ok(ty) => Ok(ty),
        Err(e) => {
            if resilient {
                fallback_warnings.push(e.to_string());
                Ok(RustType::Any)
            } else {
                Err(e)
            }
        }
    }
}

/// Converts a function parameter pattern into an IR [`Param`] and optional expansion statements.
///
/// For simple identifier parameters, returns the param with no expansion.
/// For object destructuring parameters (`{ x, y }: Point`), returns a synthetic
/// parameter (named from the type annotation) and `let` statements to expand the fields.
///
/// When `resilient` is true, unsupported type annotations fall back to [`RustType::Any`].
fn convert_param(
    pat: &ast::Pat,
    fn_name: &str,
    vis: Visibility,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<(Param, Vec<Stmt>, Vec<Item>)> {
    match pat {
        ast::Pat::Ident(ident) => {
            let param_name = ident.id.sym.to_string();
            let ty = match ident.type_ann.as_ref() {
                Some(ann) => ann,
                None => {
                    // No type annotation — fallback to Any
                    return Ok((
                        Param {
                            name: param_name,
                            ty: Some(RustType::Any),
                        },
                        vec![],
                        vec![],
                    ));
                }
            };

            // Check if the type annotation is an inline type literal
            if let ast::TsType::TsTypeLit(type_lit) = ty.type_ann.as_ref() {
                let struct_name = to_pascal_case(&format!("{fn_name}_{param_name}"));
                let mut fields = Vec::new();
                for member in &type_lit.members {
                    match member {
                        ast::TsTypeElement::TsPropertySignature(prop) => {
                            fields.push(convert_property_signature(prop, synthetic, reg)?);
                        }
                        _ => {
                            return Err(anyhow!(
                                "unsupported inline type literal member (only property signatures)"
                            ))
                        }
                    }
                }
                let struct_item = Item::Struct {
                    vis,
                    name: struct_name.clone(),
                    type_params: vec![],
                    fields,
                };
                let rust_type = RustType::Named {
                    name: struct_name,
                    type_args: vec![],
                };
                return Ok((
                    Param {
                        name: param_name,
                        ty: Some(rust_type),
                    },
                    vec![],
                    vec![struct_item],
                ));
            }

            let rust_type = convert_ts_type_with_fallback(
                &ty.type_ann,
                resilient,
                fallback_warnings,
                synthetic,
                tctx,
                reg,
            )?;
            // Trait types in parameter position → &dyn Trait
            let rust_type = wrap_trait_for_position(rust_type, TypePosition::Param, reg);
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
            let (param, stmts) = convert_object_destructuring_param(obj_pat, tctx, reg, synthetic)?;
            Ok((param, stmts, vec![]))
        }
        ast::Pat::Assign(assign) => convert_default_param(
            assign,
            fn_name,
            vis,
            resilient,
            fallback_warnings,
            tctx,
            reg,
            synthetic,
        ),
        ast::Pat::Rest(rest) => {
            if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                let name = ident.id.sym.to_string();
                let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                let rust_type = type_ann
                    .map(|ann| {
                        crate::transformer::functions::convert_ts_type_with_fallback(
                            &ann.type_ann,
                            resilient,
                            fallback_warnings,
                            synthetic,
                            tctx,
                            reg,
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
    assign: &ast::AssignPat,
    fn_name: &str,
    vis: Visibility,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<(Param, Vec<Stmt>, Vec<Item>)> {
    // Recursively convert the inner parameter (left side)
    let (inner_param, mut stmts, extra) = convert_param(
        &assign.left,
        fn_name,
        vis,
        resilient,
        fallback_warnings,
        tctx,
        reg,
        synthetic,
    )?;
    let param_name = inner_param.name.clone();

    // Wrap the type in Option<T>
    // If no type annotation (or Any fallback), infer from default value literal
    let inner_type = match inner_param.ty {
        Some(RustType::Any) | None => {
            infer_type_from_default(&assign.right).unwrap_or(RustType::Any)
        }
        Some(ty) => ty,
    };
    let option_type = RustType::Option(Box::new(inner_type));

    // Convert default value to IR expression
    let (default_expr, use_unwrap_or_default) = convert_default_value(&assign.right, synthetic)?;

    // Generate expansion statement: `let x = x.unwrap_or(value);` or `let x = x.unwrap_or_default();`
    let unwrap_call = if use_unwrap_or_default {
        Expr::MethodCall {
            object: Box::new(Expr::Ident(param_name.clone())),
            method: "unwrap_or_default".to_string(),
            args: vec![],
        }
    } else {
        Expr::MethodCall {
            object: Box::new(Expr::Ident(param_name.clone())),
            method: "unwrap_or".to_string(),
            args: vec![default_expr.unwrap()],
        }
    };

    stmts.insert(
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
        stmts,
        extra,
    ))
}

/// Converts a default value expression to an IR [`Expr`].
///
/// Returns `(Some(expr), false)` for literal values (use `unwrap_or`),
/// or `(None, true)` for empty objects (use `unwrap_or_default`).
pub(crate) fn convert_default_value(
    expr: &ast::Expr,
    synthetic: &mut SyntheticTypeRegistry,
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
            // Cat B: parameter type annotation available
            let dummy_mg = crate::pipeline::ModuleGraph::empty();
            let dummy_res = crate::pipeline::type_resolution::FileTypeResolution::empty();
            let dummy_reg = TypeRegistry::new();
            let dummy_tctx =
                TransformContext::new(&dummy_mg, &dummy_reg, &dummy_res, std::path::Path::new(""));
            let expr = crate::transformer::expressions::convert_expr(
                other,
                &dummy_tctx,
                &dummy_reg,
                &crate::transformer::expressions::ExprContext::none(),
                &crate::transformer::TypeEnv::new(),
                synthetic,
            )?;
            Ok((Some(expr), false))
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

/// Converts an object destructuring parameter pattern into a synthetic [`Param`]
/// and expansion statements.
///
/// Example: `{ x, y }: Point` → param `point: Point` + `let x = point.x; let y = point.y;`
pub(crate) fn convert_object_destructuring_param(
    obj_pat: &ast::ObjectPat,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<(Param, Vec<Stmt>)> {
    let rust_type = if let Some(type_ann) = obj_pat.type_ann.as_ref() {
        convert_ts_type(&type_ann.type_ann, synthetic, reg)?
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
                    let default_ir = crate::transformer::expressions::convert_expr(
                        default_expr,
                        tctx,
                        reg,
                        &crate::transformer::expressions::ExprContext::none(),
                        &crate::transformer::TypeEnv::new(),
                        synthetic,
                    )?;
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
                    field: field_name,
                };
                match kv.value.as_ref() {
                    // { a: { b, c } } — nested destructuring
                    ast::Pat::Object(inner_pat) => {
                        expand_fn_param_object_props(
                            &inner_pat.props,
                            &nested_source,
                            &mut stmts,
                            tctx,
                            reg,
                            synthetic,
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
            ast::ObjectPatProp::Rest(_rest) => {
                // Collect explicitly named fields
                let explicit_fields: Vec<String> = obj_pat
                    .props
                    .iter()
                    .filter_map(|p| match p {
                        ast::ObjectPatProp::Assign(a) => Some(a.key.sym.to_string()),
                        ast::ObjectPatProp::KeyValue(kv) => extract_prop_name(&kv.key).ok(),
                        _ => None,
                    })
                    .collect();

                // Expand remaining fields from TypeRegistry
                let type_name = match &rust_type {
                    RustType::Named { name, .. } => Some(name.as_str()),
                    _ => None,
                };
                if let Some(crate::registry::TypeDef::Struct { fields, .. }) =
                    type_name.and_then(|n| reg.get(n))
                {
                    for (field_name, _) in fields {
                        if !explicit_fields.contains(field_name) {
                            stmts.push(Stmt::Let {
                                mutable: false,
                                name: field_name.clone(),
                                ty: None,
                                init: Some(Expr::FieldAccess {
                                    object: Box::new(Expr::Ident(param_name.clone())),
                                    field: field_name.clone(),
                                }),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok((param, stmts))
}

/// Recursively expands nested object destructuring properties for function parameters.
fn expand_fn_param_object_props(
    props: &[ast::ObjectPatProp],
    source_expr: &Expr,
    stmts: &mut Vec<Stmt>,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
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
                    let default_ir = crate::transformer::expressions::convert_expr(
                        default_expr,
                        tctx,
                        reg,
                        &crate::transformer::expressions::ExprContext::none(),
                        &crate::transformer::TypeEnv::new(),
                        synthetic,
                    )?;
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
                    field: field_name,
                };
                match kv.value.as_ref() {
                    ast::Pat::Object(inner_pat) => {
                        expand_fn_param_object_props(
                            &inner_pat.props,
                            &nested_source,
                            stmts,
                            tctx,
                            reg,
                            synthetic,
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
            ast::ObjectPatProp::Rest(_) => {
                // Rest in nested destructuring: silently skip
                // (type info not available at this level)
            }
        }
    }
    Ok(())
}

/// Converts a snake_case name to PascalCase.
///
/// Example: `"foo_opts"` → `"FooOpts"`, `"bar_config"` → `"BarConfig"`
use crate::transformer::any_narrowing::to_pascal_case;

/// Converts a PascalCase name to snake_case.
///
/// Example: `"HonoOptions"` → `"hono_options"`, `"Point"` → `"point"`
fn pascal_to_snake(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap_or(ch));
        } else {
            result.push(ch);
        }
    }
    result
}

/// Unwraps `Promise<T>` to `T` for async function return types.
///
/// If the type is `Named { name: "Promise", type_args: [T] }`, returns `Some(T)`.
/// Otherwise returns the type unchanged.
fn unwrap_promise_type(ty: RustType) -> Option<RustType> {
    match ty {
        RustType::Named {
            ref name,
            ref type_args,
        } if name == "Promise" && type_args.len() == 1 => Some(type_args[0].clone()),
        other => Some(other),
    }
}

/// Converts the last `Stmt::Return(Some(expr))` in a function body to `Stmt::TailExpr(expr)`.
///
/// This enables idiomatic Rust tail expressions (implicit return without `return` keyword).
/// `Stmt::Return(None)` is not converted because `return;` cannot be a tail expression.
pub(crate) fn convert_last_return_to_tail(body: &mut Vec<Stmt>) {
    if let Some(Stmt::Return(Some(_))) = body.last() {
        if let Some(Stmt::Return(Some(expr))) = body.pop() {
            body.push(Stmt::TailExpr(expr));
        }
    }
}

/// Methods that require `&mut self` on the receiver.
const MUTATING_METHODS: &[&str] = &[
    "reverse", "sort", "sort_by", "drain", "push", "pop", "remove", "insert", "clear", "truncate",
    "retain",
];

/// Scans function body for method calls that require `&mut self` and inserts
/// `let mut name = name;` rebinding statements at the start of the body.
fn mark_mut_params_from_body(body: &[Stmt], params: &[Param]) -> Vec<Stmt> {
    let mut needs_mut = std::collections::HashSet::new();
    collect_mut_receivers(body, &mut needs_mut);

    let mut rebindings = Vec::new();
    for param in params {
        if needs_mut.contains(&param.name) {
            rebindings.push(Stmt::Let {
                mutable: true,
                name: param.name.clone(),
                ty: None,
                init: Some(Expr::Ident(param.name.clone())),
            });
        }
    }
    rebindings
}

/// Recursively collects variable names that are receivers of mutating method calls.
fn collect_mut_receivers(stmts: &[Stmt], receivers: &mut std::collections::HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::TailExpr(expr) => {
                collect_mut_receivers_from_expr(expr, receivers);
            }
            Stmt::Let {
                init: Some(expr), ..
            } => {
                collect_mut_receivers_from_expr(expr, receivers);
            }
            Stmt::Return(Some(expr)) => {
                collect_mut_receivers_from_expr(expr, receivers);
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect_mut_receivers(then_body, receivers);
                if let Some(els) = else_body {
                    collect_mut_receivers(els, receivers);
                }
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::Loop { body, .. } => {
                collect_mut_receivers(body, receivers);
            }
            _ => {}
        }
    }
}

/// Checks if an expression contains a mutating method call and collects the receiver name.
fn collect_mut_receivers_from_expr(expr: &Expr, receivers: &mut std::collections::HashSet<String>) {
    if let Expr::MethodCall { object, method, .. } = expr {
        if MUTATING_METHODS.contains(&method.as_str()) {
            if let Expr::Ident(name) = object.as_ref() {
                receivers.insert(name.clone());
            }
        }
        // Also recurse into chained calls (e.g., arr.drain(...).collect())
        collect_mut_receivers_from_expr(object, receivers);
    }
}

/// Checks whether a list of SWC statements contains a `throw` statement.
///
/// Recursively scans all control flow structures. `try` block throw is excluded
/// (caught by `catch`), but `catch` block throw is included (re-throw).
fn contains_throw(stmts: &[ast::Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        ast::Stmt::Throw(_) => true,
        ast::Stmt::If(if_stmt) => {
            stmt_contains_throw(&if_stmt.cons)
                || if_stmt
                    .alt
                    .as_ref()
                    .is_some_and(|alt| stmt_contains_throw(alt))
        }
        ast::Stmt::Block(block) => contains_throw(&block.stmts),
        ast::Stmt::While(w) => stmt_contains_throw(&w.body),
        ast::Stmt::DoWhile(dw) => stmt_contains_throw(&dw.body),
        ast::Stmt::For(f) => stmt_contains_throw(&f.body),
        ast::Stmt::ForOf(fo) => stmt_contains_throw(&fo.body),
        ast::Stmt::ForIn(fi) => stmt_contains_throw(&fi.body),
        ast::Stmt::Labeled(l) => stmt_contains_throw(&l.body),
        ast::Stmt::Switch(s) => s.cases.iter().any(|c| contains_throw(&c.cons)),
        ast::Stmt::Try(t) => {
            // try block throw is excluded (caught by catch)
            // catch block throw is included (re-throw escapes the function)
            let catch_has = t
                .handler
                .as_ref()
                .is_some_and(|h| contains_throw(&h.body.stmts));
            let finally_has = t
                .finalizer
                .as_ref()
                .is_some_and(|f| contains_throw(&f.stmts));
            catch_has || finally_has
        }
        _ => false,
    })
}

/// Checks whether a single statement contains a `throw`.
fn stmt_contains_throw(stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Block(block) => contains_throw(&block.stmts),
        ast::Stmt::Throw(_) => true,
        other => contains_throw(std::slice::from_ref(other)),
    }
}

/// Wraps `return expr` statements in `Ok(expr)` for functions that use `Result`.
///
/// `throw` statements are already converted to `return Err(...)` by `convert_stmt`,
/// so only non-Err returns need wrapping.
fn wrap_returns_in_ok(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts.into_iter().map(wrap_stmt_return).collect()
}

/// Recursively wraps return expressions in `Ok(...)`.
fn wrap_stmt_return(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Return(Some(expr)) => {
            // Don't wrap if already an Err(...) call
            if matches!(&expr, Expr::FnCall { name, .. } if name == "Err") {
                Stmt::Return(Some(expr))
            } else {
                Stmt::Return(Some(Expr::FnCall {
                    name: "Ok".to_string(),
                    args: vec![expr],
                }))
            }
        }
        Stmt::Return(None) => Stmt::Return(Some(Expr::FnCall {
            name: "Ok".to_string(),
            args: vec![Expr::Unit],
        })),
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => Stmt::If {
            condition,
            then_body: wrap_returns_in_ok(then_body),
            else_body: else_body.map(wrap_returns_in_ok),
        },
        Stmt::While {
            label,
            condition,
            body,
        } => Stmt::While {
            label,
            condition,
            body: wrap_returns_in_ok(body),
        },
        Stmt::WhileLet {
            label,
            pattern,
            expr,
            body,
        } => Stmt::WhileLet {
            label,
            pattern,
            expr,
            body: wrap_returns_in_ok(body),
        },
        Stmt::ForIn {
            label,
            var,
            iterable,
            body,
        } => Stmt::ForIn {
            label,
            var,
            iterable,
            body: wrap_returns_in_ok(body),
        },
        Stmt::Loop { label, body } => Stmt::Loop {
            label,
            body: wrap_returns_in_ok(body),
        },
        Stmt::Match { expr, arms } => Stmt::Match {
            expr,
            arms: arms
                .into_iter()
                .map(|arm| MatchArm {
                    body: wrap_returns_in_ok(arm.body),
                    ..arm
                })
                .collect(),
        },
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => Stmt::IfLet {
            pattern,
            expr,
            then_body: wrap_returns_in_ok(then_body),
            else_body: else_body.map(wrap_returns_in_ok),
        },
        Stmt::LabeledBlock { label, body } => Stmt::LabeledBlock {
            label,
            body: wrap_returns_in_ok(body),
        },
        other => other,
    }
}

/// Converts `const` variable declarations with arrow function initializers into `Item::Fn`.
///
/// `const double = (x: number): number => x * 2;`
/// becomes `fn double(x: f64) -> f64 { x * 2.0 }`
///
/// Non-arrow-function variable declarations are skipped.
pub(crate) fn convert_var_decl_arrow_fns(
    var_decl: &ast::VarDecl,
    vis: Visibility,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    resilient: bool,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<(Vec<Item>, Vec<String>)> {
    let mut items = Vec::new();
    let mut all_warnings = Vec::new();
    for decl in &var_decl.decls {
        let init = match &decl.init {
            Some(init) => init,
            None => continue,
        };
        // Only handle arrow function initializers
        let arrow = match init.as_ref() {
            ast::Expr::Arrow(arrow) => arrow,
            _ => continue,
        };
        let (name, var_return_type, var_param_types) = match &decl.name {
            ast::Pat::Ident(ident) => {
                let n = ident.id.sym.to_string();
                // Extract variable's type annotation and resolve to return type + param types
                let var_rust_type = ident
                    .type_ann
                    .as_ref()
                    .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, reg).ok());
                let ret = var_rust_type
                    .as_ref()
                    .and_then(|ty| extract_fn_return_type(ty, tctx, reg));
                let param_types = var_rust_type
                    .as_ref()
                    .and_then(|ty| extract_fn_param_types(ty, tctx, reg));
                (n, ret, param_types)
            }
            _ => continue,
        };

        // Convert the arrow to a closure IR, then extract parts for Item::Fn
        // Pass var_return_type so it propagates into the arrow body
        let mut fallback_warnings = Vec::new();

        // Lazy type materialization for any-typed arrow params:
        // Pre-populate TypeEnv with enum types from registry (generated by build_registry)
        let mut arrow_type_env = TypeEnv::new();
        {
            let any_param_names: Vec<String> = arrow
                .params
                .iter()
                .filter_map(|p| {
                    if let ast::Pat::Ident(ident) = p {
                        let has_any_type = ident.type_ann.as_ref().is_none_or(|ann| {
                            matches!(
                                ann.type_ann.as_ref(),
                                swc_ecma_ast::TsType::TsKeywordType(kw)
                                    if kw.kind == swc_ecma_ast::TsKeywordTypeKind::TsAnyKeyword
                            )
                        });
                        if has_any_type {
                            Some(ident.id.sym.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            // Collect constraints from either block body or expression body
            let constraints = match arrow.body.as_ref() {
                ast::BlockStmtOrExpr::BlockStmt(body) => {
                    crate::transformer::any_narrowing::collect_any_constraints(
                        body,
                        &any_param_names,
                    )
                }
                ast::BlockStmtOrExpr::Expr(expr) => {
                    crate::transformer::any_narrowing::collect_any_constraints_from_expr(
                        expr,
                        &any_param_names,
                    )
                }
            };
            if !any_param_names.is_empty() {
                for (param_name, constraint) in &constraints {
                    if constraint.is_empty() {
                        continue;
                    }
                    let variants =
                        crate::transformer::any_narrowing::build_any_enum_variants(constraint);
                    let enum_name = synthetic.register_any_enum(&name, param_name, variants);
                    let enum_type = RustType::Named {
                        name: enum_name,
                        type_args: vec![],
                    };
                    arrow_type_env.insert(param_name.clone(), enum_type);
                }
            }
        }

        let closure = crate::transformer::expressions::convert_arrow_expr_with_return_type(
            arrow,
            tctx,
            reg,
            resilient,
            &mut fallback_warnings,
            &arrow_type_env,
            var_return_type.as_ref(),
            var_param_types.as_deref(),
            synthetic,
        )?;
        match closure {
            Expr::Closure {
                mut params,
                return_type,
                body,
            } => {
                // return_type already includes the override from variable annotation
                // (applied inside convert_arrow_expr_with_return_type)
                let ret = return_type;
                let mut fn_body = match body {
                    crate::ir::ClosureBody::Expr(expr) => {
                        vec![Stmt::Return(Some(*expr))]
                    }
                    crate::ir::ClosureBody::Block(stmts) => stmts,
                };
                convert_last_return_to_tail(&mut fn_body);
                // Untyped parameters → fallback to Any, then override with enum if available
                for p in &mut params {
                    if p.ty.is_none() {
                        p.ty = Some(RustType::Any);
                    }
                    // Override Any params with generated enum type from any_narrowing
                    if matches!(&p.ty, Some(RustType::Any)) {
                        if let Some(enum_ty) = arrow_type_env.get(&p.name) {
                            p.ty = Some(enum_ty.clone());
                        }
                    }
                }

                let type_params = extract_type_params(arrow.type_params.as_deref(), synthetic, reg);
                items.push(Item::Fn {
                    vis: vis.clone(),
                    attributes: vec![],
                    is_async: arrow.is_async,
                    name,
                    type_params,
                    params,
                    return_type: ret,
                    body: fn_body,
                });
                all_warnings.extend(fallback_warnings);
            }
            _ => continue,
        }
    }
    Ok((items, all_warnings))
}

/// Extracts the return type from a function type.
///
/// Handles two cases:
/// - `RustType::Fn { return_type, .. }` → returns the return_type directly
/// - `RustType::Named { name, .. }` → looks up TypeRegistry for `TypeDef::Function` and extracts return_type
pub(super) fn extract_fn_return_type(
    ty: &RustType,
    _tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<RustType> {
    match ty {
        RustType::Fn { return_type, .. } => {
            let rt = return_type.as_ref();
            if matches!(rt, RustType::Unit) {
                None
            } else {
                Some(rt.clone())
            }
        }
        RustType::Named { name, .. } => {
            if let Some(crate::registry::TypeDef::Function { return_type, .. }) = reg.get(name) {
                return_type.clone()
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extracts parameter types from a function type.
///
/// Handles two cases:
/// - `RustType::Fn { params, .. }` → returns the params directly
/// - `RustType::Named { name, .. }` → looks up TypeRegistry for `TypeDef::Function` and extracts params
pub(super) fn extract_fn_param_types(
    ty: &RustType,
    _tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<Vec<RustType>> {
    match ty {
        RustType::Fn { params, .. } => Some(params.clone()),
        RustType::Named { name, .. } => {
            if let Some(crate::registry::TypeDef::Function { params, .. }) = reg.get(name) {
                Some(params.iter().map(|(_, ty)| ty.clone()).collect())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
