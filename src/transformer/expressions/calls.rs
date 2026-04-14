//! Function and method call conversions.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, CallTarget, ClosureBody, Expr, Param, RustType, Stmt, UserTypeRef};
use crate::registry::collection::{classify_callable_interface, CallableInterfaceKind};
use crate::registry::{ParamDef, TypeDef};

use super::literals::needs_debug_format;
use super::methods::map_method_call;
use crate::transformer::Transformer;

/// Wraps an `Option<T>` expression for TS-compatible display in console.log.
///
/// Generates: `expr.as_ref().map_or("undefined".to_string(), |v| v.to_string())`
/// Uses `as_ref()` to borrow instead of consuming the Option (TS console.log doesn't
/// consume values). For Debug-only inner types (Vec, Tuple): `format!("{:?}", v)`.
fn wrap_option_for_display(expr: Expr, inner_ty: &RustType) -> Expr {
    let closure_body = if needs_debug_format(Some(inner_ty)) {
        Expr::FormatMacro {
            template: "{:?}".to_string(),
            args: vec![Expr::Ident("v".to_string())],
        }
    } else {
        Expr::MethodCall {
            object: Box::new(Expr::Ident("v".to_string())),
            method: "to_string".to_string(),
            args: vec![],
        }
    };

    Expr::MethodCall {
        object: Box::new(Expr::MethodCall {
            object: Box::new(expr),
            method: "as_ref".to_string(),
            args: vec![],
        }),
        method: "map_or".to_string(),
        args: vec![
            Expr::MethodCall {
                object: Box::new(Expr::StringLit("undefined".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            },
            Expr::Closure {
                params: vec![Param {
                    name: "v".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(closure_body)),
            },
        ],
    }
}

impl<'a> Transformer<'a> {
    /// Converts a function/method call expression.
    ///
    /// - `foo(x, y)` → `Expr::FnCall { target: CallTarget::Free("foo".to_string()), args }`
    /// - `obj.method(x)` → `Expr::MethodCall { object, method, args }`
    /// - `super(x)` → `Expr::FnCall { target: CallTarget::Super, args }`
    pub(crate) fn convert_call_expr(&mut self, call: &ast::CallExpr) -> Result<Expr> {
        match call.callee {
            ast::Callee::Expr(ref callee) => match callee.as_ref() {
                ast::Expr::Ident(ident) => {
                    let fn_name = ident.sym.to_string();

                    // parseInt(s) → s.parse::<f64>().unwrap()
                    // parseFloat(s) → s.parse::<f64>().unwrap()
                    // isNaN(x) → x.is_nan()
                    if let Some(result) = self.convert_global_builtin(&fn_name, &call.args)? {
                        return Ok(result);
                    }

                    // Callable interface call dispatch: getCookie(ctx, "k") → getCookie.call_1(ctx, "k".to_string())
                    if let Some(result) =
                        self.try_convert_callable_trait_call(&fn_name, callee, call)
                    {
                        return result;
                    }

                    // Look up function parameter types from the registry or FileTypeResolution.
                    //
                    // Resolution priority:
                    // 1. Global function in the registry: `foo(...)` where `foo` is declared.
                    // 2. Local variable whose type is `RustType::Fn` (inline fn-typed param).
                    // 3. Local variable whose type is `RustType::Named { name }` that aliases
                    //    a `TypeDef::Function` (fn type alias, e.g. `type F = (...) => T`).
                    //
                    // Case 3 is required so that callers of fn-type-aliased parameters still
                    // get Option<T> param types propagated into `convert_call_args_inner` for
                    // trailing-None fill (I-040). Without it, `f: Callback` where
                    // `Callback = (x, y?) => R` would lose the `Option<T>` knowledge.
                    let resolved_params: Vec<ParamDef>;
                    let mut has_rest = false;
                    let param_types: Option<&[ParamDef]> = if let Some(TypeDef::Function {
                        params,
                        has_rest: rest,
                        ..
                    }) = self.reg().get(&fn_name)
                    {
                        has_rest = *rest;
                        Some(params.as_slice())
                    } else if let Some(RustType::Fn { params, .. }) = self.get_expr_type(callee) {
                        resolved_params = params
                            .iter()
                            .enumerate()
                            .map(|(i, ty)| ParamDef {
                                name: format!("_p{i}"),
                                ty: ty.clone(),
                                optional: matches!(ty, RustType::Option(_)),
                                has_default: false,
                            })
                            .collect();
                        Some(resolved_params.as_slice())
                    } else if let Some(RustType::Named { name, .. }) = self.get_expr_type(callee) {
                        if let Some(TypeDef::Function {
                            params,
                            has_rest: rest,
                            ..
                        }) = self.reg().get(name)
                        {
                            has_rest = *rest;
                            Some(params.as_slice())
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let args =
                        self.convert_call_args_with_types(&call.args, param_types, has_rest)?;
                    // Ident callee: classify by `TypeRegistry` lookup.
                    // NOTE: Callable interfaces are intercepted above by
                    // `try_convert_callable_trait_call` and never reach here.
                    // - Struct / Enum → `UserTupleCtor` で walker 登録を維持
                    // - Plain functions / unknown / imported → `Free`
                    let target = match self.reg().get(&fn_name) {
                        Some(TypeDef::Struct { .. } | TypeDef::Enum { .. }) => {
                            CallTarget::UserTupleCtor(UserTypeRef::new(fn_name))
                        }
                        _ => CallTarget::Free(fn_name),
                    };
                    Ok(Expr::FnCall { target, args })
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
                            let raw_args = self.convert_call_args(&call.args)?;
                            let mut args = Vec::with_capacity(raw_args.len());
                            let mut use_debug = Vec::with_capacity(raw_args.len());
                            for (raw_arg, call_arg) in raw_args.into_iter().zip(call.args.iter()) {
                                let ty = self.get_expr_type(&call_arg.expr);
                                if let Some(RustType::Option(inner)) = ty {
                                    // Option<T> → unwrap for TS-compatible display
                                    args.push(wrap_option_for_display(raw_arg, inner));
                                    use_debug.push(false);
                                } else {
                                    use_debug.push(needs_debug_format(ty));
                                    args.push(raw_arg);
                                }
                            }
                            return Ok(Expr::MacroCall {
                                name: macro_name.to_string(),
                                args,
                                use_debug,
                            });
                        }

                        // Math.method(args) → first_arg.method(rest_args)
                        if obj_ident.sym.as_ref() == "Math" {
                            return self.convert_math_call(&method, &call.args);
                        }

                        // Number.isNaN(x) → x.is_nan(), Number.isFinite(x) → x.is_finite()
                        if obj_ident.sym.as_ref() == "Number" {
                            return self.convert_number_static_call(&method, &call.args);
                        }

                        // fs.readFileSync/writeFileSync/existsSync → std::fs equivalents
                        if obj_ident.sym.as_ref() == "fs" {
                            return self.convert_fs_call(&method, &call.args);
                        }

                        // I-378 T9: Static method call on a user-registered type:
                        //   `MyClass.foo(args)` → `Expr::FnCall { CallTarget::UserAssocFn { .. } }`
                        //
                        // Without this, `convert_expr(&member.obj)` would produce
                        // `Expr::Ident("MyClass")` and the fallthrough below would
                        // build `Expr::MethodCall { object: Ident("MyClass"), .. }`,
                        // which the generator previously coerced via the `is_type_ident`
                        // uppercase heuristic. I-378 removes that heuristic, so the
                        // structural classification must happen here.
                        //
                        // Important: this MUST run AFTER the `console`/`Math`/`Number`/`fs`
                        // special handlers above. Otherwise builtin types registered as
                        // synthetic Struct (e.g. `Math`, `Number`) would be intercepted
                        // and emit `Math::sign(x)` instead of `x.signum()`.
                        let obj_name = obj_ident.sym.as_ref();
                        if matches!(
                            self.reg().get(obj_name),
                            Some(TypeDef::Struct { .. } | TypeDef::Enum { .. })
                        ) {
                            let args = self.convert_call_args(&call.args)?;
                            let rust_name = crate::ir::sanitize_rust_type_name(obj_name);
                            return Ok(Expr::FnCall {
                                target: CallTarget::UserAssocFn {
                                    ty: UserTypeRef::new(rust_name),
                                    method,
                                },
                                args,
                            });
                        }
                    }

                    // Cat A: method receiver — converted before method resolution
                    let object = self.convert_expr(&member.obj)?;
                    // Look up method signature via unified TypeRegistry method
                    // (handles Vec→Array, String, Named, DynTrait + select_overload)
                    // For methods remapped by map_method_call (e.g. startsWith, find),
                    // skip the TS-signature lookup entirely: the remap replaces the
                    // callee with a Rust API of different arity, so propagating TS
                    // param types would produce spurious Some() wraps or None fills.
                    let is_remapped = super::methods::is_remapped_method(method.as_str());
                    let method_sig = if is_remapped {
                        None
                    } else {
                        self.get_expr_type(&member.obj).and_then(|ty| {
                            let sigs = self.reg().lookup_method_sigs(ty, &method)?;
                            let (_, sig) =
                                crate::registry::select_overload(&sigs, call.args.len(), &[]);
                            Some(sig.clone())
                        })
                    };
                    let method_params = method_sig.as_ref().map(|sig| sig.params.as_slice());
                    // For methods that map_method_call transforms to Rust APIs expecting
                    // &str / impl Pattern, suppress .to_string() on string literal args.
                    let suppress = super::methods::PATTERN_ARG_METHODS.contains(&method.as_str())
                        || (super::methods::REGEX_PATTERN_ARG_METHODS.contains(&method.as_str())
                            && matches!(&object, Expr::Regex { .. }));
                    let args = if suppress {
                        self.convert_call_args_suppress_string(&call.args, method_params, false)?
                    } else {
                        self.convert_call_args_with_types(&call.args, method_params, false)?
                    };
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
                    self.convert_call_expr(&unwrapped_call)
                }
                // Chained call: f(x)(y) → { let _f = f(x); _f(y) }
                ast::Expr::Call(inner_call) => {
                    let inner_result = self.convert_call_expr(inner_call)?;
                    let args = self.convert_call_args(&call.args)?;
                    Ok(Expr::Block(vec![
                        Stmt::Let {
                            name: "_f".to_string(),
                            mutable: false,
                            ty: None,
                            init: Some(inner_result),
                        },
                        Stmt::TailExpr(Expr::FnCall {
                            target: CallTarget::Free("_f".to_string()),
                            args,
                        }),
                    ]))
                }
                // IIFE: (() => expr)() or (function() { ... })()
                // Arrow/Fn expressions as callee → convert to closure and call immediately
                ast::Expr::Arrow(arrow) => {
                    let mut warnings = Vec::new();
                    let closure = self.convert_arrow_expr(arrow, false, &mut warnings)?;
                    let args = self.convert_call_args(&call.args)?;
                    Ok(Expr::Block(vec![
                        Stmt::Let {
                            name: "__iife".to_string(),
                            mutable: false,
                            ty: None,
                            init: Some(closure),
                        },
                        Stmt::TailExpr(Expr::FnCall {
                            target: CallTarget::Free("__iife".to_string()),
                            args,
                        }),
                    ]))
                }
                ast::Expr::Fn(fn_expr) => {
                    let closure = self.convert_fn_expr(fn_expr)?;
                    let args = self.convert_call_args(&call.args)?;
                    Ok(Expr::Block(vec![
                        Stmt::Let {
                            name: "__iife".to_string(),
                            mutable: false,
                            ty: None,
                            init: Some(closure),
                        },
                        Stmt::TailExpr(Expr::FnCall {
                            target: CallTarget::Free("__iife".to_string()),
                            args,
                        }),
                    ]))
                }
                _ => Err(anyhow!("unsupported call target expression")),
            },
            ast::Callee::Super(_) => {
                let args = self.convert_call_args(&call.args)?;
                Ok(Expr::FnCall {
                    target: CallTarget::Super,
                    args,
                })
            }
            _ => Err(anyhow!("unsupported callee type")),
        }
    }

    /// Converts a `new` expression to a `ClassName::new(args)` call.
    ///
    /// `new Foo(x, y)` → `Expr::FnCall { target: CallTarget::UserAssocFn { ty: crate::ir::UserTypeRef::new("Foo"), method: "new".to_string() }, args }`
    /// (the `type_ref` is set so the reference walker registers `Foo` in the graph)
    pub(crate) fn convert_new_expr(&mut self, new_expr: &ast::NewExpr) -> Result<Expr> {
        let class_name = match new_expr.callee.as_ref() {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return Err(anyhow!("unsupported new expression target")),
        };
        // Look up constructor param types from struct fields in TypeRegistry
        let param_types: Option<Vec<ParamDef>> =
            self.reg().get(&class_name).and_then(|def| match def {
                TypeDef::Struct { fields, .. } => Some(
                    fields
                        .iter()
                        .map(|f| ParamDef {
                            name: f.name.clone(),
                            ty: f.ty.clone(),
                            optional: f.optional,
                            has_default: false,
                        })
                        .collect(),
                ),
                _ => None,
            });
        let param_slice = param_types.as_deref();
        let args = match &new_expr.args {
            Some(args) => self.convert_call_args_with_types(args, param_slice, false)?,
            None => vec![],
        };
        let rust_name = crate::ir::sanitize_rust_type_name(&class_name);
        Ok(Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new(rust_name),
                method: "new".to_string(),
            },
            args,
        })
    }
}

impl<'a> Transformer<'a> {
    /// Attempts to convert a callable interface call to a trait method call.
    ///
    /// Performs a 2-stage registry lookup:
    /// 1. `reg.get(fn_name)` → `ConstValue { type_ref_name }` (const → interface name)
    /// 2. `reg.get(interface_name)` → `Struct { call_signatures }` (interface definition)
    ///
    /// If the target is a callable interface, selects the best-matching overload via
    /// `select_overload` and generates `Expr::MethodCall { object: Ident(fn_name), method: "call_N", args }`.
    ///
    /// Returns `None` if the target is not a callable interface (fallthrough to normal call path).
    fn try_convert_callable_trait_call(
        &mut self,
        fn_name: &str,
        callee: &ast::Expr,
        call: &ast::CallExpr,
    ) -> Option<Result<Expr>> {
        // Stage 1: ConstValue → type_ref_name (interface name)
        let type_ref_name = match self.reg().get(fn_name)? {
            TypeDef::ConstValue { type_ref_name, .. } => type_ref_name.clone()?,
            _ => return None,
        };

        // Stage 2: interface definition → call_signatures + type_params
        let (call_sigs, type_params) = {
            let def = self.reg().get(&type_ref_name)?;
            let sigs = match classify_callable_interface(def) {
                CallableInterfaceKind::NonCallable => return None,
                CallableInterfaceKind::SingleOverload(sig) => vec![sig],
                CallableInterfaceKind::MultiOverload(sigs) => sigs,
            };
            let tp = match def {
                TypeDef::Struct { type_params, .. } => type_params.clone(),
                _ => vec![],
            };
            (sigs, tp)
        };

        // Generic support: extract concrete type args from callee expression type
        let type_args = match self.get_expr_type(callee) {
            Some(RustType::Named { type_args, .. }) => type_args.clone(),
            _ => vec![],
        };

        // Apply type substitution (no-op for non-generic interfaces)
        let apply_sub =
            crate::pipeline::type_converter::overloaded_callable::apply_type_substitution;
        let substituted_sigs: Vec<_> = call_sigs
            .iter()
            .map(|sig| apply_sub(sig, &type_params, &type_args))
            .collect();

        // Select overload and determine call_N index
        let (index, selected) =
            crate::registry::select_overload(&substituted_sigs, call.args.len(), &[]);

        // Convert args with selected overload's parameter types
        let param_defs: Vec<ParamDef> = selected.params.clone();
        let has_rest = selected.has_rest;

        let args = match self.convert_call_args_with_types(&call.args, Some(&param_defs), has_rest)
        {
            Ok(a) => a,
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(Expr::MethodCall {
            object: Box::new(Expr::Ident(fn_name.to_string())),
            method: format!("call_{index}"),
            args,
        }))
    }

    /// Converts global built-in functions (`parseInt`, `parseFloat`, `isNaN`) to Rust equivalents.
    ///
    /// Returns `Ok(Some(expr))` if the function is a known built-in, `Ok(None)` otherwise.
    fn convert_global_builtin(
        &mut self,
        fn_name: &str,
        args: &[ast::ExprOrSpread],
    ) -> Result<Option<Expr>> {
        match fn_name {
            "parseInt" | "parseFloat" => {
                let converted = self.convert_call_args(args)?;
                if converted.len() != 1 {
                    return Err(anyhow!("{fn_name} expects 1 argument"));
                }
                let arg = converted.into_iter().next().unwrap();
                Ok(Some(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(arg),
                        method: "parse::<f64>".to_string(),
                        args: vec![],
                    }),
                    method: "unwrap_or".to_string(),
                    args: vec![Expr::PrimitiveAssocConst {
                        ty: crate::ir::PrimitiveType::F64,
                        name: "NAN".to_string(),
                    }],
                }))
            }
            "isNaN" => {
                let converted = self.convert_call_args(args)?;
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
    fn convert_number_static_call(
        &mut self,
        method: &str,
        args: &[ast::ExprOrSpread],
    ) -> Result<Expr> {
        let converted = self.convert_call_args(args)?;
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
    fn convert_fs_call(&mut self, method: &str, args: &[ast::ExprOrSpread]) -> Result<Expr> {
        match method {
            "readFileSync" => {
                if args.is_empty() {
                    return Err(anyhow!("fs.readFileSync requires at least 1 argument"));
                }
                let path_arg = self.convert_expr(&args[0].expr)?;
                let is_stdin = matches!(&path_arg, Expr::StringLit(s) if s == "/dev/stdin")
                    || matches!(&path_arg, Expr::NumberLit(n) if *n == 0.0)
                    || matches!(&path_arg, Expr::IntLit(n) if *n == 0);
                if is_stdin {
                    return Ok(Expr::MethodCall {
                        object: Box::new(Expr::FnCall {
                            target: CallTarget::ExternalPath(vec![
                                "std".to_string(),
                                "io".to_string(),
                                "read_to_string".to_string(),
                            ]),
                            args: vec![Expr::FnCall {
                                target: CallTarget::ExternalPath(vec![
                                    "std".to_string(),
                                    "io".to_string(),
                                    "stdin".to_string(),
                                ]),
                                args: vec![],
                            }],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    });
                }
                Ok(Expr::MethodCall {
                    object: Box::new(Expr::FnCall {
                        target: CallTarget::ExternalPath(vec![
                            "std".to_string(),
                            "fs".to_string(),
                            "read_to_string".to_string(),
                        ]),
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
                let path_arg = self.convert_expr(&args[0].expr)?;
                let data_arg = self.convert_expr(&args[1].expr)?;
                Ok(Expr::MethodCall {
                    object: Box::new(Expr::FnCall {
                        target: CallTarget::ExternalPath(vec![
                            "std".to_string(),
                            "fs".to_string(),
                            "write".to_string(),
                        ]),
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
                let path_arg = self.convert_expr(&args[0].expr)?;
                Ok(Expr::MethodCall {
                    object: Box::new(Expr::FnCall {
                        // `std::path::Path::new(path)` — `Path` is a std type,
                        // not user-defined, so `type_ref` stays `None`.
                        target: CallTarget::ExternalPath(vec![
                            "std".to_string(),
                            "path".to_string(),
                            "Path".to_string(),
                            "new".to_string(),
                        ]),
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
    fn convert_math_call(&mut self, method: &str, args: &[ast::ExprOrSpread]) -> Result<Expr> {
        let converted_args = self.convert_call_args(args)?;
        match method {
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

    /// Converts call arguments from SWC `ExprOrSpread` to IR `Expr`.
    pub(crate) fn convert_call_args(&mut self, args: &[ast::ExprOrSpread]) -> Result<Vec<Expr>> {
        self.convert_call_args_with_types(args, None, false)
    }

    /// Converts call arguments with optional parameter type information from the registry.
    ///
    /// When `param_types` is provided, each argument gets the corresponding parameter's type
    /// as its expected type. This enables object literal arguments to resolve their struct name.
    ///
    /// When `has_rest` is true, the last parameter is a rest parameter (`Vec<T>`).
    /// Extra arguments beyond the regular parameters are packed into a `vec![...]`.
    ///
    /// When `suppress_string_coercion` is true, string literal arguments are NOT wrapped
    /// with `.to_string()` even if the expected type is `String`. This is needed for methods
    /// that `map_method_call` transforms to Rust APIs expecting `&str` / `impl Pattern`.
    pub(crate) fn convert_call_args_with_types(
        &mut self,
        args: &[ast::ExprOrSpread],
        param_types: Option<&[ParamDef]>,
        has_rest: bool,
    ) -> Result<Vec<Expr>> {
        self.convert_call_args_inner(args, param_types, has_rest, false)
    }

    /// Like `convert_call_args_with_types` but suppresses `.to_string()` on string literals.
    pub(crate) fn convert_call_args_suppress_string(
        &mut self,
        args: &[ast::ExprOrSpread],
        param_types: Option<&[ParamDef]>,
        has_rest: bool,
    ) -> Result<Vec<Expr>> {
        self.convert_call_args_inner(args, param_types, has_rest, true)
    }

    fn convert_call_args_inner(
        &mut self,
        args: &[ast::ExprOrSpread],
        param_types: Option<&[ParamDef]>,
        has_rest: bool,
        suppress_string_coercion: bool,
    ) -> Result<Vec<Expr>> {
        let regular_param_count = if has_rest {
            param_types.map(|p| p.len().saturating_sub(1)).unwrap_or(0)
        } else {
            usize::MAX
        };

        let regular_args_count = args.len().min(regular_param_count);
        let mut result: Vec<Expr> = Vec::with_capacity(args.len());

        for (i, arg) in args[..regular_args_count].iter().enumerate() {
            let param_ty = param_types.and_then(|params| params.get(i).map(|p| &p.ty));
            let mut expr = self.convert_expr(&arg.expr)?;
            // Strip .to_string() from string literals when suppressed
            // (TypeResolver sets expected=String from TS signatures, but Rust API needs &str)
            if suppress_string_coercion {
                expr = match expr {
                    Expr::MethodCall {
                        object,
                        ref method,
                        ref args,
                    } if method == "to_string"
                        && args.is_empty()
                        && matches!(object.as_ref(), Expr::StringLit(_)) =>
                    {
                        *object
                    }
                    other => other,
                };
            }
            if matches!(param_ty, Some(RustType::Fn { .. })) && matches!(&expr, Expr::Ident(_)) {
                // Wrap function values in `Box::new(...)` when the parameter expects
                // a boxed closure. `Box` is a Rust std type, not a user-defined one,
                // so `type_ref` is `None` — the reference walker must not try to
                // generate a stub for it.
                expr = Expr::FnCall {
                    target: CallTarget::ExternalPath(vec!["Box".to_string(), "new".to_string()]),
                    args: vec![expr],
                };
            }
            if let Some(RustType::Named { name, .. }) = param_ty {
                if self.reg().is_trait_type(name) {
                    let arg_type = self.get_expr_type(&arg.expr);
                    if is_box_dyn_trait(arg_type) {
                        expr = Expr::Ref(Box::new(Expr::Deref(Box::new(expr))));
                    }
                }
            }
            result.push(expr);
        }

        if has_rest {
            let rest_args = &args[regular_args_count..];
            if rest_args.len() == 1 && rest_args[0].spread.is_some() {
                let expr = self.convert_expr(&rest_args[0].expr)?;
                result.push(expr);
            } else if rest_args.iter().any(|a| a.spread.is_some()) {
                let mut parts: Vec<Expr> = Vec::new();
                let mut literal_buf: Vec<Expr> = Vec::new();

                for arg in rest_args {
                    if arg.spread.is_some() {
                        if !literal_buf.is_empty() {
                            parts.push(Expr::Vec {
                                elements: std::mem::take(&mut literal_buf),
                            });
                        }
                        let expr = self.convert_expr(&arg.expr)?;
                        parts.push(expr);
                    } else {
                        let expr = self.convert_expr(&arg.expr)?;
                        literal_buf.push(expr);
                    }
                }
                if !literal_buf.is_empty() {
                    parts.push(Expr::Vec {
                        elements: literal_buf,
                    });
                }

                let concat_receiver = Expr::Vec { elements: parts };
                result.push(Expr::MethodCall {
                    object: Box::new(concat_receiver),
                    method: "concat".to_string(),
                    args: vec![],
                });
            } else {
                let rest_exprs: Vec<Expr> = rest_args
                    .iter()
                    .map(|arg| self.convert_expr(&arg.expr))
                    .collect::<Result<Vec<_>>>()?;
                result.push(Expr::Vec {
                    elements: rest_exprs,
                });
            }
        } else {
            if let Some(params) = param_types {
                for param in params.iter().skip(result.len()) {
                    if matches!(param.ty, RustType::Option(_)) {
                        result.push(Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None));
                    }
                }
            }
        }

        Ok(result)
    }
}

/// Returns true if the type is `Box<dyn Trait>`.
fn is_box_dyn_trait(ty: Option<&RustType>) -> bool {
    matches!(
        ty,
        Some(RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::Box,
            args,
        }) if args.len() == 1 && matches!(&args[0], RustType::DynTrait(_))
    )
}
