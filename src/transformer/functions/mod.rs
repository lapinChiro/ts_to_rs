//! Function declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC function declarations into the IR [`Item::Fn`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, Param, RustType, Stmt, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::statements::convert_stmt_list;
use crate::transformer::types::{convert_property_signature, convert_ts_type, extract_type_params};
use crate::transformer::{extract_pat_ident_name, extract_prop_name, TypeEnv};

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
    reg: &TypeRegistry,
    resilient: bool,
) -> Result<(Vec<Item>, Vec<String>)> {
    let name = fn_decl.ident.sym.to_string();
    let mut fallback_warnings = Vec::new();
    let mut extra_items = Vec::new();

    let mut params = Vec::new();
    let mut destructuring_stmts = Vec::new();
    for param in &fn_decl.function.params {
        let (p, stmts, extra) = convert_param(
            &param.pat,
            &name,
            vis.clone(),
            resilient,
            &mut fallback_warnings,
        )?;
        params.push(p);
        destructuring_stmts.extend(stmts);
        extra_items.extend(extra);
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
                &mut extra_items,
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

    let mut fn_type_env = TypeEnv::new();
    for p in &params {
        if let Some(ty) = &p.ty {
            fn_type_env.insert(p.name.clone(), ty.clone());
        }
    }

    let body_stmts = match &fn_decl.function.body {
        Some(block) => {
            convert_stmt_list(&block.stmts, reg, return_type.as_ref(), &mut fn_type_env)?
        }
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

    let type_params = extract_type_params(fn_decl.function.type_params.as_deref());

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

    extra_items.push(Item::Fn {
        vis,
        is_async,
        name,
        type_params,
        params,
        return_type,
        body,
    });

    Ok((extra_items, fallback_warnings))
}

/// Converts a TypeScript type to an IR type, falling back to [`RustType::Any`] when
/// `resilient` is true and the type is unsupported.
///
/// When falling back, appends the error message to `fallback_warnings` for reporting.
pub(crate) fn convert_ts_type_with_fallback(
    ts_type: &swc_ecma_ast::TsType,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    extra_items: &mut Vec<Item>,
) -> Result<RustType> {
    match convert_ts_type(ts_type, extra_items) {
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
) -> Result<(Param, Vec<Stmt>, Vec<Item>)> {
    match pat {
        ast::Pat::Ident(ident) => {
            let param_name = ident.id.sym.to_string();
            let ty = ident
                .type_ann
                .as_ref()
                .ok_or_else(|| anyhow!("parameter '{}' has no type annotation", param_name))?;

            // Check if the type annotation is an inline type literal
            if let ast::TsType::TsTypeLit(type_lit) = ty.type_ann.as_ref() {
                let struct_name = to_pascal_case(&format!("{fn_name}_{param_name}"));
                let mut fields = Vec::new();
                for member in &type_lit.members {
                    match member {
                        ast::TsTypeElement::TsPropertySignature(prop) => {
                            fields.push(convert_property_signature(prop, &mut Vec::new())?);
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

            let mut type_extra_items = Vec::new();
            let rust_type = convert_ts_type_with_fallback(
                &ty.type_ann,
                resilient,
                fallback_warnings,
                &mut type_extra_items,
            )?;
            Ok((
                Param {
                    name: param_name,
                    ty: Some(rust_type),
                },
                vec![],
                type_extra_items,
            ))
        }
        ast::Pat::Object(obj_pat) => {
            let (param, stmts) = convert_object_destructuring_param(obj_pat)?;
            Ok((param, stmts, vec![]))
        }
        ast::Pat::Assign(assign) => {
            convert_default_param(assign, fn_name, vis, resilient, fallback_warnings)
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
) -> Result<(Param, Vec<Stmt>, Vec<Item>)> {
    // Recursively convert the inner parameter (left side)
    let (inner_param, mut stmts, extra) =
        convert_param(&assign.left, fn_name, vis, resilient, fallback_warnings)?;
    let param_name = inner_param.name.clone();

    // Wrap the type in Option<T>
    let inner_type = inner_param
        .ty
        .ok_or_else(|| anyhow!("default parameter requires a type annotation"))?;
    let option_type = RustType::Option(Box::new(inner_type));

    // Convert default value to IR expression
    let (default_expr, use_unwrap_or_default) = convert_default_value(&assign.right)?;

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
fn convert_default_value(expr: &ast::Expr) -> Result<(Option<Expr>, bool)> {
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
        _ => Err(anyhow!("unsupported default parameter value")),
    }
}

/// Converts an object destructuring parameter pattern into a synthetic [`Param`]
/// and expansion statements.
///
/// Example: `{ x, y }: Point` → param `point: Point` + `let x = point.x; let y = point.y;`
fn convert_object_destructuring_param(obj_pat: &ast::ObjectPat) -> Result<(Param, Vec<Stmt>)> {
    let type_ann = obj_pat
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("object destructuring parameter requires a type annotation"))?;
    let rust_type = convert_ts_type(&type_ann.type_ann, &mut Vec::new())?;

    // Generate parameter name from type name (PascalCase → snake_case)
    let param_name = match &rust_type {
        RustType::Named { name, .. } => pascal_to_snake(name),
        _ => "param".to_string(),
    };

    let param = Param {
        name: param_name.clone(),
        ty: Some(rust_type),
    };

    let mut stmts = Vec::new();
    for prop in &obj_pat.props {
        match prop {
            ast::ObjectPatProp::Assign(assign) => {
                // { x } — shorthand
                let field_name = assign.key.sym.to_string();
                stmts.push(Stmt::Let {
                    mutable: false,
                    name: field_name.clone(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident(param_name.clone())),
                        field: field_name,
                    }),
                });
            }
            ast::ObjectPatProp::KeyValue(kv) => {
                // { x: newX } — rename
                let field_name = extract_prop_name(&kv.key)
                    .map_err(|_| anyhow!("unsupported destructuring key"))?;
                let binding_name = extract_pat_ident_name(kv.value.as_ref())
                    .map_err(|_| anyhow!("unsupported destructuring value pattern"))?;
                let binding_name = pascal_to_snake(&binding_name);
                stmts.push(Stmt::Let {
                    mutable: false,
                    name: binding_name,
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(Expr::Ident(param_name.clone())),
                        field: field_name,
                    }),
                });
            }
            ast::ObjectPatProp::Rest(_) => {
                return Err(anyhow!(
                    "rest pattern in destructuring parameter is not supported"
                ));
            }
        }
    }

    Ok((param, stmts))
}

/// Converts a snake_case name to PascalCase.
///
/// Example: `"foo_opts"` → `"FooOpts"`, `"bar_config"` → `"BarConfig"`
fn to_pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect()
}

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
            args: vec![Expr::Ident("()".to_string())],
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
        other => other,
    }
}

#[cfg(test)]
mod tests;
