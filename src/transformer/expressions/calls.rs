//! Function and method call conversions.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType, Stmt};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::TypeEnv;

use super::convert_arrow_expr;
use super::convert_expr;
use super::convert_fn_expr;
use super::literals::needs_debug_format;
use super::methods::map_method_call;
use super::type_resolution::resolve_expr_type;
use crate::transformer::context::TransformContext;

/// Converts a function/method call expression.
///
/// - `foo(x, y)` → `Expr::FnCall { name: "foo", args }`
/// - `obj.method(x)` → `Expr::MethodCall { object, method, args }`
pub(super) fn convert_call_expr(
    call: &ast::CallExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    match call.callee {
        ast::Callee::Expr(ref callee) => match callee.as_ref() {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();

                // parseInt(s) → s.parse::<f64>().unwrap()
                // parseFloat(s) → s.parse::<f64>().unwrap()
                // isNaN(x) → x.is_nan()
                if let Some(result) =
                    convert_global_builtin(&fn_name, &call.args, tctx, reg, type_env, synthetic)?
                {
                    return Ok(result);
                }

                // Look up function parameter types from the registry or TypeEnv
                let typeenv_params: Vec<(String, RustType)>;
                let mut has_rest = false;
                let param_types: Option<&[(String, RustType)]> = if let Some(TypeDef::Function {
                    params,
                    has_rest: rest,
                    ..
                }) = reg.get(&fn_name)
                {
                    has_rest = *rest;
                    Some(params.as_slice())
                } else if let Some(RustType::Fn { params, .. }) = type_env.get(&fn_name) {
                    typeenv_params = params
                        .iter()
                        .enumerate()
                        .map(|(i, ty)| (format!("_p{i}"), ty.clone()))
                        .collect();
                    Some(typeenv_params.as_slice())
                } else {
                    None
                };
                let args = convert_call_args_with_types(
                    &call.args,
                    tctx,
                    reg,
                    param_types,
                    has_rest,
                    type_env,
                    synthetic,
                )?;
                Ok(Expr::FnCall {
                    name: fn_name,
                    args,
                })
            }
            ast::Expr::Member(member) => {
                let method = match &member.prop {
                    ast::MemberProp::Ident(ident) => ident.sym.to_string(),
                    ast::MemberProp::PrivateName(private) => format!("_{}", private.name),
                    _ => return Err(anyhow!("unsupported call target member property")),
                };

                if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                    // console.log/error/warn → println!/eprintln!
                    if obj_ident.sym.as_ref() == "console" {
                        let macro_name = match method.as_str() {
                            "log" => "println",
                            "error" | "warn" => "eprintln",
                            _ => return Err(anyhow!("unsupported console method: {}", method)),
                        };
                        let args = convert_call_args(&call.args, tctx, reg, type_env, synthetic)?;
                        let use_debug = call
                            .args
                            .iter()
                            .map(|arg| {
                                let ty = resolve_expr_type(&arg.expr, type_env, tctx, reg);
                                needs_debug_format(ty.as_ref())
                            })
                            .collect();
                        return Ok(Expr::MacroCall {
                            name: macro_name.to_string(),
                            args,
                            use_debug,
                        });
                    }

                    // Math.method(args) → first_arg.method(rest_args)
                    if obj_ident.sym.as_ref() == "Math" {
                        return convert_math_call(
                            &method, &call.args, tctx, reg, type_env, synthetic,
                        );
                    }

                    // Number.isNaN(x) → x.is_nan(), Number.isFinite(x) → x.is_finite()
                    if obj_ident.sym.as_ref() == "Number" {
                        return convert_number_static_call(
                            &method, &call.args, tctx, reg, type_env, synthetic,
                        );
                    }

                    // fs.readFileSync/writeFileSync/existsSync → std::fs equivalents
                    if obj_ident.sym.as_ref() == "fs" {
                        return convert_fs_call(
                            &method, &call.args, tctx, reg, type_env, synthetic,
                        );
                    }
                }

                // Cat A: method receiver — converted before method resolution
                let object = convert_expr(
                    &member.obj,
                    tctx,
                    reg,
                    type_env,
                    synthetic,
                )?;
                // Look up method parameter types from the object's type
                let method_sig =
                    resolve_expr_type(&member.obj, type_env, tctx, reg).and_then(|ty| {
                        if let RustType::Named { name, .. } = &ty {
                            if let Some(TypeDef::Struct { methods, .. }) = reg.get(name) {
                                return methods.get(&method).cloned();
                            }
                        }
                        None
                    });
                let method_params = method_sig.as_ref().map(|sig| sig.params.as_slice());
                let args = convert_call_args_with_types(
                    &call.args,
                    tctx,
                    reg,
                    method_params,
                    false,
                    type_env,
                    synthetic,
                )?;
                let method_call = map_method_call(object, &method, args);
                Ok(method_call)
            }
            // Unwrap parenthesized expression and retry: (foo)(args) → foo(args)
            ast::Expr::Paren(paren) => {
                let unwrapped_call = ast::CallExpr {
                    callee: ast::Callee::Expr(paren.expr.clone()),
                    args: call.args.clone(),
                    span: call.span,
                    ctxt: call.ctxt,
                    type_args: call.type_args.clone(),
                };
                convert_call_expr(&unwrapped_call, tctx, reg, type_env, synthetic)
            }
            // Chained call: f(x)(y) → { let _f = f(x); _f(y) }
            ast::Expr::Call(inner_call) => {
                let inner_result = convert_call_expr(inner_call, tctx, reg, type_env, synthetic)?;
                let args = convert_call_args(&call.args, tctx, reg, type_env, synthetic)?;
                Ok(Expr::Block(vec![
                    Stmt::Let {
                        name: "_f".to_string(),
                        mutable: false,
                        ty: None,
                        init: Some(inner_result),
                    },
                    Stmt::TailExpr(Expr::FnCall {
                        name: "_f".to_string(),
                        args,
                    }),
                ]))
            }
            // IIFE: (() => expr)() or (function() { ... })()
            // Arrow/Fn expressions as callee → convert to closure and call immediately
            ast::Expr::Arrow(arrow) => {
                let mut warnings = Vec::new();
                let closure = convert_arrow_expr(
                    arrow,
                    tctx,
                    reg,
                    false,
                    &mut warnings,
                    type_env,
                    synthetic,
                )?;
                let args = convert_call_args(&call.args, tctx, reg, type_env, synthetic)?;
                Ok(Expr::Block(vec![
                    Stmt::Let {
                        name: "__iife".to_string(),
                        mutable: false,
                        ty: None,
                        init: Some(closure),
                    },
                    Stmt::TailExpr(Expr::FnCall {
                        name: "__iife".to_string(),
                        args,
                    }),
                ]))
            }
            ast::Expr::Fn(fn_expr) => {
                let closure = convert_fn_expr(fn_expr, tctx, reg, type_env, synthetic)?;
                let args = convert_call_args(&call.args, tctx, reg, type_env, synthetic)?;
                Ok(Expr::Block(vec![
                    Stmt::Let {
                        name: "__iife".to_string(),
                        mutable: false,
                        ty: None,
                        init: Some(closure),
                    },
                    Stmt::TailExpr(Expr::FnCall {
                        name: "__iife".to_string(),
                        args,
                    }),
                ]))
            }
            _ => Err(anyhow!("unsupported call target expression")),
        },
        ast::Callee::Super(_) => {
            let args = convert_call_args(&call.args, tctx, reg, type_env, synthetic)?;
            Ok(Expr::FnCall {
                name: "super".to_string(),
                args,
            })
        }
        _ => Err(anyhow!("unsupported callee type")),
    }
}

/// Converts global built-in functions (`parseInt`, `parseFloat`, `isNaN`) to Rust equivalents.
///
/// Returns `Ok(Some(expr))` if the function is a known built-in, `Ok(None)` otherwise.
pub(super) fn convert_global_builtin(
    fn_name: &str,
    args: &[ast::ExprOrSpread],
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Option<Expr>> {
    match fn_name {
        // parseInt(s) → s.parse::<f64>().unwrap_or(f64::NAN)
        "parseInt" => {
            let converted = convert_call_args(args, tctx, reg, type_env, synthetic)?;
            if converted.len() != 1 {
                return Err(anyhow!("parseInt expects 1 argument"));
            }
            let arg = converted.into_iter().next().unwrap();
            Ok(Some(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(arg),
                    method: "parse::<f64>".to_string(),
                    args: vec![],
                }),
                method: "unwrap_or".to_string(),
                args: vec![Expr::Ident("f64::NAN".to_string())],
            }))
        }
        // parseFloat(s) → s.parse::<f64>().unwrap_or(f64::NAN)
        "parseFloat" => {
            let converted = convert_call_args(args, tctx, reg, type_env, synthetic)?;
            if converted.len() != 1 {
                return Err(anyhow!("parseFloat expects 1 argument"));
            }
            let arg = converted.into_iter().next().unwrap();
            Ok(Some(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(arg),
                    method: "parse::<f64>".to_string(),
                    args: vec![],
                }),
                method: "unwrap_or".to_string(),
                args: vec![Expr::Ident("f64::NAN".to_string())],
            }))
        }
        // isNaN(x) → x.is_nan()
        "isNaN" => {
            let converted = convert_call_args(args, tctx, reg, type_env, synthetic)?;
            if converted.len() != 1 {
                return Err(anyhow!("isNaN expects 1 argument"));
            }
            let arg = converted.into_iter().next().unwrap();
            Ok(Some(Expr::MethodCall {
                object: Box::new(arg),
                method: "is_nan".to_string(),
                args: vec![],
            }))
        }
        _ => Ok(None),
    }
}

/// Converts `Number.method(x)` static calls to Rust `f64` method calls.
///
/// - `Number.isNaN(x)` → `x.is_nan()`
/// - `Number.isFinite(x)` → `x.is_finite()`
/// - `Number.isInteger(x)` → `x.fract() == 0.0`
pub(super) fn convert_number_static_call(
    method: &str,
    args: &[ast::ExprOrSpread],
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let converted = convert_call_args(args, tctx, reg, type_env, synthetic)?;
    if converted.len() != 1 {
        return Err(anyhow!("Number.{method} expects 1 argument"));
    }
    let arg = converted.into_iter().next().unwrap();
    match method {
        "isNaN" | "isFinite" => {
            let rust_method = match method {
                "isNaN" => "is_nan",
                "isFinite" => "is_finite",
                _ => unreachable!(),
            };
            Ok(Expr::MethodCall {
                object: Box::new(arg),
                method: rust_method.to_string(),
                args: vec![],
            })
        }
        // Number.isInteger(x) → x.fract() == 0.0
        "isInteger" => Ok(Expr::BinaryOp {
            left: Box::new(Expr::MethodCall {
                object: Box::new(arg),
                method: "fract".to_string(),
                args: vec![],
            }),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        _ => Err(anyhow!("unsupported Number method: {method}")),
    }
}

/// Converts Node.js `fs` module method calls to `std::fs` equivalents.
pub(super) fn convert_fs_call(
    method: &str,
    args: &[ast::ExprOrSpread],
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    match method {
        "readFileSync" => {
            if args.is_empty() {
                return Err(anyhow!("fs.readFileSync requires at least 1 argument"));
            }
            // Cat A: built-in fs API path arg
            let path_arg = convert_expr(
                &args[0].expr,
                tctx,
                reg,
                type_env,
                synthetic,
            )?;

            // Special case: fs.readFileSync("/dev/stdin", ...) or fs.readFileSync(0, ...)
            let is_stdin = matches!(&path_arg, Expr::StringLit(s) if s == "/dev/stdin")
                || matches!(&path_arg, Expr::NumberLit(n) if *n == 0.0)
                || matches!(&path_arg, Expr::IntLit(n) if *n == 0);

            if is_stdin {
                // std::io::read_to_string(std::io::stdin()).unwrap()
                return Ok(Expr::MethodCall {
                    object: Box::new(Expr::FnCall {
                        name: "std::io::read_to_string".to_string(),
                        args: vec![Expr::FnCall {
                            name: "std::io::stdin".to_string(),
                            args: vec![],
                        }],
                    }),
                    method: "unwrap".to_string(),
                    args: vec![],
                });
            }

            // fs.readFileSync(path, "utf8") → std::fs::read_to_string(&path).unwrap()
            Ok(Expr::MethodCall {
                object: Box::new(Expr::FnCall {
                    name: "std::fs::read_to_string".to_string(),
                    args: vec![Expr::Ref(Box::new(path_arg))],
                }),
                method: "unwrap".to_string(),
                args: vec![],
            })
        }
        "writeFileSync" => {
            if args.len() < 2 {
                return Err(anyhow!("fs.writeFileSync requires at least 2 arguments"));
            }
            // Cat A: built-in fs API path/data args
            let path_arg = convert_expr(
                &args[0].expr,
                tctx,
                reg,
                type_env,
                synthetic,
            )?;
            let data_arg = convert_expr(
                &args[1].expr,
                tctx,
                reg,
                type_env,
                synthetic,
            )?;
            // fs.writeFileSync(path, data) → std::fs::write(&path, &data).unwrap()
            Ok(Expr::MethodCall {
                object: Box::new(Expr::FnCall {
                    name: "std::fs::write".to_string(),
                    args: vec![Expr::Ref(Box::new(path_arg)), Expr::Ref(Box::new(data_arg))],
                }),
                method: "unwrap".to_string(),
                args: vec![],
            })
        }
        "existsSync" => {
            if args.is_empty() {
                return Err(anyhow!("fs.existsSync requires 1 argument"));
            }
            // Cat A: built-in fs API path arg
            let path_arg = convert_expr(
                &args[0].expr,
                tctx,
                reg,
                type_env,
                synthetic,
            )?;
            // fs.existsSync(path) → std::path::Path::new(&path).exists()
            Ok(Expr::MethodCall {
                object: Box::new(Expr::FnCall {
                    name: "std::path::Path::new".to_string(),
                    args: vec![Expr::Ref(Box::new(path_arg))],
                }),
                method: "exists".to_string(),
                args: vec![],
            })
        }
        _ => Err(anyhow!("unsupported fs method: {method}")),
    }
}

/// Converts `Math.method(args)` to Rust `f64` method calls.
///
/// - 1-arg methods (same name): `Math.floor(x)` → `x.floor()`, `Math.trunc(x)` → `x.trunc()`
/// - 1-arg methods (renamed): `Math.sign(x)` → `x.signum()`, `Math.log(x)` → `x.ln()`
/// - 2-arg methods: `Math.max(a, b)` → `a.max(b)`
/// - `Math.pow(x, y)` → `x.powf(y)`
pub(super) fn convert_math_call(
    method: &str,
    args: &[ast::ExprOrSpread],
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let converted_args = convert_call_args(args, tctx, reg, type_env, synthetic)?;
    match method {
        // 1-arg methods: first arg becomes receiver (same name)
        "floor" | "ceil" | "round" | "abs" | "sqrt" | "trunc" => {
            if converted_args.len() != 1 {
                return Err(anyhow!("Math.{method} expects 1 argument"));
            }
            let receiver = converted_args.into_iter().next().unwrap();
            Ok(Expr::MethodCall {
                object: Box::new(receiver),
                method: method.to_string(),
                args: vec![],
            })
        }
        // 1-arg methods with name mapping: first arg becomes receiver
        "sign" | "log" => {
            if converted_args.len() != 1 {
                return Err(anyhow!("Math.{method} expects 1 argument"));
            }
            let receiver = converted_args.into_iter().next().unwrap();
            let rust_method = match method {
                "sign" => "signum",
                "log" => "ln",
                _ => unreachable!(),
            };
            Ok(Expr::MethodCall {
                object: Box::new(receiver),
                method: rust_method.to_string(),
                args: vec![],
            })
        }
        // variadic methods: chain calls for 2+ args: Math.max(a,b,c) → a.max(b).max(c)
        "max" | "min" => {
            if converted_args.len() < 2 {
                return Err(anyhow!("Math.{method} expects at least 2 arguments"));
            }
            let mut iter = converted_args.into_iter();
            let mut result = iter.next().unwrap();
            for arg in iter {
                result = Expr::MethodCall {
                    object: Box::new(result),
                    method: method.to_string(),
                    args: vec![arg],
                };
            }
            Ok(result)
        }
        // pow → powf
        "pow" => {
            if converted_args.len() != 2 {
                return Err(anyhow!("Math.pow expects 2 arguments"));
            }
            let mut iter = converted_args.into_iter();
            let receiver = iter.next().unwrap();
            let arg = iter.next().unwrap();
            Ok(Expr::MethodCall {
                object: Box::new(receiver),
                method: "powf".to_string(),
                args: vec![arg],
            })
        }
        _ => Err(anyhow!("unsupported Math method: {method}")),
    }
}

/// Converts a `new` expression to a `ClassName::new(args)` call.
///
/// `new Foo(x, y)` → `Expr::FnCall { name: "Foo::new", args }`
pub(super) fn convert_new_expr(
    new_expr: &ast::NewExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    let class_name = match new_expr.callee.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported new expression target")),
    };
    // Look up constructor param types from struct fields in TypeRegistry
    let param_types = reg.get(&class_name).and_then(|def| match def {
        TypeDef::Struct { fields, .. } => Some(fields.as_slice()),
        _ => None,
    });
    let args = match &new_expr.args {
        Some(args) => {
            convert_call_args_with_types(args, tctx, reg, param_types, false, type_env, synthetic)?
        }
        None => vec![],
    };
    Ok(Expr::FnCall {
        name: format!("{class_name}::new"),
        args,
    })
}

/// Converts call arguments from SWC `ExprOrSpread` to IR `Expr`.
pub(super) fn convert_call_args(
    args: &[ast::ExprOrSpread],
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<Expr>> {
    convert_call_args_with_types(args, tctx, reg, None, false, type_env, synthetic)
}

/// Converts call arguments with optional parameter type information from the registry.
///
/// When `param_types` is provided, each argument gets the corresponding parameter's type
/// as its expected type. This enables object literal arguments to resolve their struct name.
///
/// When `has_rest` is true, the last parameter is a rest parameter (`Vec<T>`).
/// Extra arguments beyond the regular parameters are packed into a `vec![...]`.
pub(super) fn convert_call_args_with_types(
    args: &[ast::ExprOrSpread],
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    param_types: Option<&[(String, RustType)]>,
    has_rest: bool,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<Expr>> {
    // Determine how many regular (non-rest) parameters there are
    let regular_param_count = if has_rest {
        param_types.map(|p| p.len().saturating_sub(1)).unwrap_or(0)
    } else {
        usize::MAX // No rest param → treat all as regular
    };

    // Get the element type for the rest parameter (inner type of Vec<T>)
    let rest_element_type = if has_rest {
        param_types.and_then(|p| p.last()).and_then(|(_, ty)| {
            if let RustType::Vec(inner) = ty {
                Some(inner.as_ref())
            } else {
                None
            }
        })
    } else {
        None
    };

    // Convert regular arguments
    let regular_args_count = args.len().min(regular_param_count);
    let mut result: Vec<Expr> = args[..regular_args_count]
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            let param_ty = param_types.and_then(|params| params.get(i).map(|(_, ty)| ty));
            // For Option<T> params, pass the inner type as expected so conversions apply
            let expected = match param_ty {
                Some(RustType::Option(inner)) => Some(inner.as_ref()),
                other => other,
            };
            let mut expr = convert_expr(&arg.expr, tctx, reg, type_env, synthetic)?;
            // Wrap in Some(...) when the parameter type is Option<T>,
            // but skip if the value is already None (from undefined)
            if let Some(RustType::Option(_)) = param_ty {
                if !matches!(&expr, Expr::Ident(name) if name == "None") {
                    expr = Expr::FnCall {
                        name: "Some".to_string(),
                        args: vec![expr],
                    };
                }
            }
            // Wrap in Box::new(...) when the parameter type is Fn (Box<dyn Fn>)
            // and the argument is an identifier (function name), not an inline closure
            if matches!(param_ty, Some(RustType::Fn { .. })) && matches!(&expr, Expr::Ident(_)) {
                expr = Expr::FnCall {
                    name: "Box::new".to_string(),
                    args: vec![expr],
                };
            }
            // Trait type coercion: when param is a trait type, the generated
            // Rust param is `&dyn Trait`. If the argument is `Box<dyn Trait>`, use `&*arg`.
            if let Some(RustType::Named { name, .. }) = param_ty {
                if reg.is_trait_type(name) {
                    let arg_type = resolve_expr_type(&arg.expr, type_env, tctx, reg);
                    if is_box_dyn_trait(&arg_type) {
                        expr = Expr::Ref(Box::new(Expr::Deref(Box::new(expr))));
                    }
                }
            }
            Ok(expr)
        })
        .collect::<Result<Vec<_>>>()?;

    if has_rest {
        // Pack remaining arguments into a vec![]
        let rest_args = &args[regular_args_count..];
        if rest_args.len() == 1 && rest_args[0].spread.is_some() {
            // Cat A: single spread source — type is the array itself
            let expr = convert_expr(
                &rest_args[0].expr,
                tctx,
                reg,
                type_env,
                synthetic,
            )?;
            result.push(expr);
        } else if rest_args.iter().any(|a| a.spread.is_some()) {
            // Mixed literals and spread: foo(1, ...arr) → foo([vec![1.0], arr].concat())
            let mut parts: Vec<Expr> = Vec::new();
            let mut literal_buf: Vec<Expr> = Vec::new();

            for arg in rest_args {
                if arg.spread.is_some() {
                    // Flush literal buffer as vec![...]
                    if !literal_buf.is_empty() {
                        parts.push(Expr::Vec {
                            elements: std::mem::take(&mut literal_buf),
                        });
                    }
                    // Cat A: spread source — type is the array itself
                    let expr = convert_expr(
                        &arg.expr,
                        tctx,
                        reg,
                        type_env,
                        synthetic,
                    )?;
                    parts.push(expr);
                } else {
                    let expr = convert_expr(&arg.expr, tctx, reg, type_env, synthetic)?;
                    literal_buf.push(expr);
                }
            }
            if !literal_buf.is_empty() {
                parts.push(Expr::Vec {
                    elements: literal_buf,
                });
            }

            // [part1, part2, ...].concat()
            let concat_receiver = Expr::Vec { elements: parts };
            result.push(Expr::MethodCall {
                object: Box::new(concat_receiver),
                method: "concat".to_string(),
                args: vec![],
            });
        } else {
            // All literal args: foo(1, 2, 3) → foo(vec![1.0, 2.0, 3.0])
            let rest_exprs: Vec<Expr> = rest_args
                .iter()
                .map(|arg| {
                    convert_expr(&arg.expr, tctx, reg, type_env, synthetic)
                })
                .collect::<Result<Vec<_>>>()?;
            result.push(Expr::Vec {
                elements: rest_exprs,
            });
        }
    } else {
        // Append None for missing Option parameters (default arguments)
        if let Some(params) = param_types {
            for param in params.iter().skip(result.len()) {
                if matches!(param.1, RustType::Option(_)) {
                    result.push(Expr::Ident("None".to_string()));
                }
            }
        }
    }

    Ok(result)
}

/// Returns true if the type is `Box<dyn Trait>` (i.e., `Named { name: "Box", type_args: [DynTrait(_)] }`).
fn is_box_dyn_trait(ty: &Option<RustType>) -> bool {
    matches!(
        ty,
        Some(RustType::Named { name, type_args })
            if name == "Box" && type_args.len() == 1 && matches!(&type_args[0], RustType::DynTrait(_))
    )
}
