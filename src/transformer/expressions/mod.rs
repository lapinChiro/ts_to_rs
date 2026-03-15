//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{self, BinOp, ClosureBody, Expr, Param, RustType, UnOp};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::functions::convert_ts_type_with_fallback;
use crate::transformer::statements::convert_stmt;

/// Converts an SWC [`ast::Expr`] into an IR [`Expr`], with an optional expected type.
///
/// The `expected` type is used for:
/// - Object literals: determines the struct name from `RustType::Named`
/// - String literals: adds `.to_string()` when `RustType::String` is expected
/// - Array literals: propagates element type from `RustType::Vec`
///
/// # Errors
///
/// Returns an error for unsupported expression types.
pub fn convert_expr(
    expr: &ast::Expr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    match expr {
        ast::Expr::Ident(ident) => Ok(Expr::Ident(ident.sym.to_string())),
        ast::Expr::Lit(lit) => convert_lit(lit, expected),
        ast::Expr::Bin(bin) => convert_bin_expr(bin, reg),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl, reg),
        ast::Expr::Paren(paren) => convert_expr(&paren.expr, reg, expected),
        ast::Expr::Member(member) => convert_member_expr(member, reg),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign, reg),
        ast::Expr::Arrow(arrow) => convert_arrow_expr(arrow, reg, false, &mut Vec::new()),
        ast::Expr::Call(call) => convert_call_expr(call, reg),
        ast::Expr::New(new_expr) => convert_new_expr(new_expr, reg),
        ast::Expr::Array(array_lit) => convert_array_lit(array_lit, reg, expected),
        ast::Expr::Object(obj_lit) => convert_object_lit(obj_lit, reg, expected),
        ast::Expr::Cond(cond) => convert_cond_expr(cond, reg, expected),
        ast::Expr::Unary(unary) => convert_unary_expr(unary, reg),
        ast::Expr::TsAs(ts_as) => convert_expr(&ts_as.expr, reg, expected),
        ast::Expr::OptChain(opt_chain) => convert_opt_chain_expr(opt_chain, reg),
        ast::Expr::Await(await_expr) => {
            let inner = convert_expr(&await_expr.arg, reg, None)?;
            Ok(Expr::Await(Box::new(inner)))
        }
        _ => Err(anyhow!("unsupported expression: {:?}", expr)),
    }
}

/// Converts an SWC literal to an IR expression.
///
/// When `expected` is `RustType::String`, string literals are wrapped with `.to_string()`
/// to produce an owned `String` instead of `&str`.
fn convert_lit(lit: &ast::Lit, expected: Option<&RustType>) -> Result<Expr> {
    match lit {
        ast::Lit::Num(n) => Ok(Expr::NumberLit(n.value)),
        ast::Lit::Str(s) => {
            let expr = Expr::StringLit(s.value.to_string_lossy().into_owned());
            if matches!(expected, Some(RustType::String)) {
                Ok(Expr::MethodCall {
                    object: Box::new(expr),
                    method: "to_string".to_string(),
                    args: vec![],
                })
            } else {
                Ok(expr)
            }
        }
        ast::Lit::Bool(b) => Ok(Expr::BoolLit(b.value)),
        _ => Err(anyhow!("unsupported literal: {:?}", lit)),
    }
}

/// Converts an SWC binary expression to an IR `BinaryOp`.
fn convert_bin_expr(bin: &ast::BinExpr, reg: &TypeRegistry) -> Result<Expr> {
    // `x ?? y` → `x.unwrap_or_else(|| y)`
    if bin.op == ast::BinaryOp::NullishCoalescing {
        let left = convert_expr(&bin.left, reg, None)?;
        let right = convert_expr(&bin.right, reg, None)?;
        return Ok(Expr::MethodCall {
            object: Box::new(left),
            method: "unwrap_or_else".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(right)),
            }],
        });
    }

    let left = convert_expr(&bin.left, reg, None)?;
    let right = convert_expr(&bin.right, reg, None)?;
    let op = convert_binary_op(bin.op)?;
    Ok(Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
    })
}

/// Converts an SWC binary operator to an IR [`BinOp`].
fn convert_binary_op(op: ast::BinaryOp) -> Result<BinOp> {
    match op {
        ast::BinaryOp::Add => Ok(BinOp::Add),
        ast::BinaryOp::Sub => Ok(BinOp::Sub),
        ast::BinaryOp::Mul => Ok(BinOp::Mul),
        ast::BinaryOp::Div => Ok(BinOp::Div),
        ast::BinaryOp::Mod => Ok(BinOp::Mod),
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq => Ok(BinOp::Eq),
        ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq => Ok(BinOp::NotEq),
        ast::BinaryOp::Lt => Ok(BinOp::Lt),
        ast::BinaryOp::LtEq => Ok(BinOp::LtEq),
        ast::BinaryOp::Gt => Ok(BinOp::Gt),
        ast::BinaryOp::GtEq => Ok(BinOp::GtEq),
        ast::BinaryOp::LogicalAnd => Ok(BinOp::LogicalAnd),
        ast::BinaryOp::LogicalOr => Ok(BinOp::LogicalOr),
        _ => Err(anyhow!("unsupported binary operator: {:?}", op)),
    }
}

/// Resolves a member access expression, applying special conversions for known fields.
///
/// - `.length` → `.len() as f64`
/// - enum member access → `EnumName::Variant`
/// - otherwise → `object.field`
fn resolve_member_access(
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
fn convert_opt_chain_expr(opt_chain: &ast::OptChainExpr, reg: &TypeRegistry) -> Result<Expr> {
    match opt_chain.base.as_ref() {
        ast::OptChainBase::Member(member) => {
            let object = convert_expr(&member.obj, reg, None)?;
            let body_expr = match &member.prop {
                ast::MemberProp::Ident(ident) => {
                    let field = ident.sym.to_string();
                    resolve_member_access(&Expr::Ident("_v".to_string()), &field, &member.obj, reg)?
                }
                ast::MemberProp::Computed(computed) => {
                    let index = convert_expr(&computed.expr, reg, None)?;
                    Expr::Index {
                        object: Box::new(Expr::Ident("_v".to_string())),
                        index: Box::new(index),
                    }
                }
                _ => return Err(anyhow!("unsupported optional chaining property")),
            };
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
        ast::OptChainBase::Call(opt_call) => {
            let args: Vec<Expr> = opt_call
                .args
                .iter()
                .map(|arg| convert_expr(&arg.expr, reg, None))
                .collect::<Result<_>>()?;
            let (object, method) = extract_method_from_callee(&opt_call.callee, reg)?;
            let body_expr = Expr::MethodCall {
                object: Box::new(Expr::Ident("_v".to_string())),
                method,
                args,
            };
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
fn extract_method_from_callee(callee: &ast::Expr, reg: &TypeRegistry) -> Result<(Expr, String)> {
    let member = match callee {
        ast::Expr::Member(member) => member,
        ast::Expr::OptChain(opt) => match opt.base.as_ref() {
            ast::OptChainBase::Member(member) => member,
            _ => return Err(anyhow!("unsupported optional call callee")),
        },
        _ => return Err(anyhow!("unsupported optional call callee: {:?}", callee)),
    };
    let object = convert_expr(&member.obj, reg, None)?;
    let method = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported optional call property")),
    };
    Ok((object, method))
}

/// Converts a member expression (`obj.field`) to `Expr::FieldAccess`.
///
/// `this.x` becomes `self.x`.
fn convert_member_expr(member: &ast::MemberExpr, reg: &TypeRegistry) -> Result<Expr> {
    let field = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported member property (only identifiers)")),
    };

    let object = convert_expr(&member.obj, reg, None)?;
    resolve_member_access(&object, &field, &member.obj, reg)
}

/// Converts an assignment expression (`target = value`) to `Expr::Assign`.
fn convert_assign_expr(assign: &ast::AssignExpr, reg: &TypeRegistry) -> Result<Expr> {
    let target = match &assign.left {
        ast::AssignTarget::Simple(simple) => match simple {
            ast::SimpleAssignTarget::Member(member) => convert_member_expr(member, reg)?,
            ast::SimpleAssignTarget::Ident(ident) => Expr::Ident(ident.id.sym.to_string()),
            _ => return Err(anyhow!("unsupported assignment target")),
        },
        _ => return Err(anyhow!("unsupported assignment target pattern")),
    };
    let right = convert_expr(&assign.right, reg, None)?;

    // For compound assignment (+=, -=, *=, /=), desugar to target = target op value
    let value = match assign.op {
        ast::AssignOp::Assign => right,
        ast::AssignOp::AddAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Add,
            right: Box::new(right),
        },
        ast::AssignOp::SubAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Sub,
            right: Box::new(right),
        },
        ast::AssignOp::MulAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Mul,
            right: Box::new(right),
        },
        ast::AssignOp::DivAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Div,
            right: Box::new(right),
        },
        _ => return Err(anyhow!("unsupported compound assignment operator")),
    };
    Ok(Expr::Assign {
        target: Box::new(target),
        value: Box::new(value),
    })
}

/// Converts an arrow expression into an IR [`Expr::Closure`].
///
/// - Expression body: `(x: number) => x + 1` → `|x: f64| x + 1`
/// - Block body: `(x: number) => { return x + 1; }` → `|x: f64| { x + 1 }`
///
/// When `resilient` is true, unsupported types fall back to [`RustType::Any`] and
/// the error message is appended to `fallback_warnings`.
pub fn convert_arrow_expr(
    arrow: &ast::ArrowExpr,
    reg: &TypeRegistry,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
) -> Result<Expr> {
    let mut params = Vec::new();
    for param in &arrow.params {
        match param {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let rust_type = ident
                    .type_ann
                    .as_ref()
                    .map(|ann| {
                        convert_ts_type_with_fallback(&ann.type_ann, resilient, fallback_warnings)
                    })
                    .transpose()?;
                params.push(Param {
                    name,
                    ty: rust_type,
                });
            }
            _ => return Err(anyhow!("unsupported arrow parameter pattern")),
        }
    }

    let return_type = arrow
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type_with_fallback(&ann.type_ann, resilient, fallback_warnings))
        .transpose()?;

    let body = match arrow.body.as_ref() {
        ast::BlockStmtOrExpr::Expr(expr) => {
            let ir_expr = convert_expr(expr, reg, return_type.as_ref())?;
            ClosureBody::Expr(Box::new(ir_expr))
        }
        ast::BlockStmtOrExpr::BlockStmt(block) => {
            let mut stmts = Vec::new();
            for stmt in &block.stmts {
                stmts.extend(convert_stmt(stmt, reg, return_type.as_ref())?);
            }
            ClosureBody::Block(stmts)
        }
    };

    Ok(Expr::Closure {
        params,
        return_type,
        body,
    })
}

/// Converts a function/method call expression.
///
/// - `foo(x, y)` → `Expr::FnCall { name: "foo", args }`
/// - `obj.method(x)` → `Expr::MethodCall { object, method, args }`
fn convert_call_expr(call: &ast::CallExpr, reg: &TypeRegistry) -> Result<Expr> {
    match call.callee {
        ast::Callee::Expr(ref callee) => match callee.as_ref() {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();

                // parseInt(s) → s.parse::<f64>().unwrap()
                // parseFloat(s) → s.parse::<f64>().unwrap()
                // isNaN(x) → x.is_nan()
                if let Some(result) = convert_global_builtin(&fn_name, &call.args, reg)? {
                    return Ok(result);
                }

                // Look up function parameter types from the registry
                let param_types = reg.get(&fn_name).and_then(|def| match def {
                    TypeDef::Function { params, .. } => Some(params.as_slice()),
                    _ => None,
                });
                let args = convert_call_args_with_types(&call.args, reg, param_types)?;
                Ok(Expr::FnCall {
                    name: fn_name,
                    args,
                })
            }
            ast::Expr::Member(member) => {
                let method = match &member.prop {
                    ast::MemberProp::Ident(ident) => ident.sym.to_string(),
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
                        let args = convert_call_args(&call.args, reg)?;
                        return Ok(Expr::MacroCall {
                            name: macro_name.to_string(),
                            args,
                        });
                    }

                    // Math.method(args) → first_arg.method(rest_args)
                    if obj_ident.sym.as_ref() == "Math" {
                        return convert_math_call(&method, &call.args, reg);
                    }

                    // Number.isNaN(x) → x.is_nan(), Number.isFinite(x) → x.is_finite()
                    if obj_ident.sym.as_ref() == "Number" {
                        return convert_number_static_call(&method, &call.args, reg);
                    }
                }

                let object = convert_expr(&member.obj, reg, None)?;
                let args = convert_call_args(&call.args, reg)?;
                let method_call = map_method_call(object, &method, args);
                Ok(method_call)
            }
            _ => Err(anyhow!("unsupported call target expression")),
        },
        ast::Callee::Super(_) => {
            let args = convert_call_args(&call.args, reg)?;
            Ok(Expr::FnCall {
                name: "super".to_string(),
                args,
            })
        }
        _ => Err(anyhow!("unsupported callee type")),
    }
}

/// Maps TypeScript method names to Rust equivalents.
///
/// Handles simple renames, methods that need wrapping (e.g., `trim` → `trim().to_string()`),
/// methods that need chaining (e.g., `split` → `split(s).collect::<Vec<&str>>()`),
/// and array methods (e.g., `reduce` → `iter().fold()`, `indexOf` → `iter().position()`,
/// `slice` → `[a..b].to_vec()`, `splice` → `drain().collect()`).
fn map_method_call(object: Expr, method: &str, args: Vec<Expr>) -> Expr {
    match method {
        // Simple name mappings
        "includes" => Expr::MethodCall {
            object: Box::new(object),
            method: "contains".to_string(),
            args,
        },
        "startsWith" => Expr::MethodCall {
            object: Box::new(object),
            method: "starts_with".to_string(),
            args,
        },
        "endsWith" => Expr::MethodCall {
            object: Box::new(object),
            method: "ends_with".to_string(),
            args,
        },
        "toLowerCase" => Expr::MethodCall {
            object: Box::new(object),
            method: "to_lowercase".to_string(),
            args,
        },
        "toUpperCase" => Expr::MethodCall {
            object: Box::new(object),
            method: "to_uppercase".to_string(),
            args,
        },
        // trim() returns &str, wrap with .to_string()
        "trim" => Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(object),
                method: "trim".to_string(),
                args,
            }),
            method: "to_string".to_string(),
            args: vec![],
        },
        // split() returns an iterator, chain .collect::<Vec<&str>>()
        "split" => Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(object),
                method: "split".to_string(),
                args,
            }),
            method: "collect::<Vec<&str>>".to_string(),
            args: vec![],
        },
        // Iterator methods that collect: .map(fn) / .filter(fn) → .iter().method(fn).collect()
        "map" | "filter" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            let method_call = Expr::MethodCall {
                object: Box::new(iter_call),
                method: method.to_string(),
                args,
            };
            Expr::MethodCall {
                object: Box::new(method_call),
                method: "collect::<Vec<_>>".to_string(),
                args: vec![],
            }
        }
        // Iterator methods without collect: .find(fn), .some(fn), .every(fn)
        "find" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "find".to_string(),
                args,
            }
        }
        "some" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "any".to_string(),
                args,
            }
        }
        "every" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "all".to_string(),
                args,
            }
        }
        // slice(start, end) → [start..end].to_vec()
        "slice" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let start = iter.next().unwrap();
            let end = iter.next().unwrap();
            Expr::MethodCall {
                object: Box::new(Expr::Index {
                    object: Box::new(object),
                    index: Box::new(Expr::Range {
                        start: Some(Box::new(start)),
                        end: Some(Box::new(end)),
                    }),
                }),
                method: "to_vec".to_string(),
                args: vec![],
            }
        }
        // splice(start, count) → .drain(start..start+count).collect::<Vec<_>>()
        "splice" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let start = iter.next().unwrap();
            let count = iter.next().unwrap();
            let end = Expr::BinaryOp {
                left: Box::new(start.clone()),
                op: BinOp::Add,
                right: Box::new(count),
            };
            let drain_call = Expr::MethodCall {
                object: Box::new(object),
                method: "drain".to_string(),
                args: vec![Expr::Range {
                    start: Some(Box::new(start)),
                    end: Some(Box::new(end)),
                }],
            };
            Expr::MethodCall {
                object: Box::new(drain_call),
                method: "collect::<Vec<_>>".to_string(),
                args: vec![],
            }
        }
        // sort(fn) → .sort_by(fn) when comparator provided, otherwise passthrough
        "sort" => {
            if args.is_empty() {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: "sort".to_string(),
                    args,
                };
            }
            Expr::MethodCall {
                object: Box::new(object),
                method: "sort_by".to_string(),
                args,
            }
        }
        // indexOf(x) → .iter().position(|item| *item == x)
        "indexOf" => {
            if args.len() != 1 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let search_value = args.into_iter().next().unwrap();
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "position".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "item".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("*item".to_string())),
                        op: BinOp::Eq,
                        right: Box::new(search_value),
                    })),
                }],
            }
        }
        // reduce(fn, init) → .iter().fold(init, fn)
        "reduce" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let callback = iter.next().unwrap();
            let init = iter.next().unwrap();
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "fold".to_string(),
                args: vec![init, callback],
            }
        }
        // forEach → .iter().for_each(fn)
        "forEach" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "for_each".to_string(),
                args,
            }
        }
        // No mapping needed — pass through unchanged
        _ => Expr::MethodCall {
            object: Box::new(object),
            method: method.to_string(),
            args,
        },
    }
}

/// Converts global built-in functions (`parseInt`, `parseFloat`, `isNaN`) to Rust equivalents.
///
/// Returns `Ok(Some(expr))` if the function is a known built-in, `Ok(None)` otherwise.
fn convert_global_builtin(
    fn_name: &str,
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
) -> Result<Option<Expr>> {
    match fn_name {
        // parseInt(s) → s.parse::<f64>().unwrap()
        "parseInt" => {
            let converted = convert_call_args(args, reg)?;
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
                method: "unwrap".to_string(),
                args: vec![],
            }))
        }
        // parseFloat(s) → s.parse::<f64>().unwrap()
        "parseFloat" => {
            let converted = convert_call_args(args, reg)?;
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
                method: "unwrap".to_string(),
                args: vec![],
            }))
        }
        // isNaN(x) → x.is_nan()
        "isNaN" => {
            let converted = convert_call_args(args, reg)?;
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
fn convert_number_static_call(
    method: &str,
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
) -> Result<Expr> {
    let converted = convert_call_args(args, reg)?;
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

/// Converts `Math.method(args)` to Rust `f64` method calls.
///
/// - 1-arg methods (same name): `Math.floor(x)` → `x.floor()`, `Math.trunc(x)` → `x.trunc()`
/// - 1-arg methods (renamed): `Math.sign(x)` → `x.signum()`, `Math.log(x)` → `x.ln()`
/// - 2-arg methods: `Math.max(a, b)` → `a.max(b)`
/// - `Math.pow(x, y)` → `x.powf(y)`
fn convert_math_call(method: &str, args: &[ast::ExprOrSpread], reg: &TypeRegistry) -> Result<Expr> {
    let converted_args = convert_call_args(args, reg)?;
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
        // 2-arg methods: first arg is receiver, second is argument
        "max" | "min" => {
            if converted_args.len() != 2 {
                return Err(anyhow!("Math.{method} expects 2 arguments"));
            }
            let mut iter = converted_args.into_iter();
            let receiver = iter.next().unwrap();
            let arg = iter.next().unwrap();
            Ok(Expr::MethodCall {
                object: Box::new(receiver),
                method: method.to_string(),
                args: vec![arg],
            })
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
fn convert_new_expr(new_expr: &ast::NewExpr, reg: &TypeRegistry) -> Result<Expr> {
    let class_name = match new_expr.callee.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported new expression target")),
    };
    let args = match &new_expr.args {
        Some(args) => convert_call_args(args, reg)?,
        None => vec![],
    };
    Ok(Expr::FnCall {
        name: format!("{class_name}::new"),
        args,
    })
}

/// Converts call arguments from SWC `ExprOrSpread` to IR `Expr`.
fn convert_call_args(args: &[ast::ExprOrSpread], reg: &TypeRegistry) -> Result<Vec<Expr>> {
    convert_call_args_with_types(args, reg, None)
}

/// Converts call arguments with optional parameter type information from the registry.
///
/// When `param_types` is provided, each argument gets the corresponding parameter's type
/// as its expected type. This enables object literal arguments to resolve their struct name.
fn convert_call_args_with_types(
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
    param_types: Option<&[(String, RustType)]>,
) -> Result<Vec<Expr>> {
    args.iter()
        .enumerate()
        .map(|(i, arg)| {
            let expected = param_types.and_then(|params| params.get(i).map(|(_, ty)| ty));
            convert_expr(&arg.expr, reg, expected)
        })
        .collect()
}

/// Converts a template literal to `Expr::FormatMacro`.
///
/// `` `Hello ${name}` `` becomes `format!("Hello {}", name)`.
fn convert_template_literal(tpl: &ast::Tpl, reg: &TypeRegistry) -> Result<Expr> {
    let mut template = String::new();
    let mut args = Vec::new();

    for (i, quasi) in tpl.quasis.iter().enumerate() {
        // raw text of the quasi (the string parts between expressions)
        template.push_str(&quasi.raw);
        if i < tpl.exprs.len() {
            template.push_str("{}");
            let arg = convert_expr(&tpl.exprs[i], reg, None)?;
            args.push(arg);
        }
    }

    Ok(Expr::FormatMacro { template, args })
}

/// Converts an SWC conditional (ternary) expression to `Expr::If`.
///
/// `condition ? consequent : alternate` → `if condition { consequent } else { alternate }`
fn convert_cond_expr(
    cond: &ast::CondExpr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    let condition = convert_expr(&cond.test, reg, None)?;
    let then_expr = convert_expr(&cond.cons, reg, expected)?;
    let else_expr = convert_expr(&cond.alt, reg, expected)?;
    Ok(Expr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    })
}

/// Converts an SWC object literal to an IR `Expr::StructInit`.
///
/// Requires an expected type (`RustType::Named`) from the enclosing context (e.g., a variable
/// declaration's type annotation). Without a named type, returns an error because Rust requires
/// a named struct.
///
/// `{ x: 1, y: 2 }` with expected `RustType::Named { name: "Point" }` →
/// `Expr::StructInit { name: "Point", fields: [...] }`
fn convert_object_lit(
    obj_lit: &ast::ObjectLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    let struct_name = match expected {
        Some(RustType::Named { name, .. }) => name.as_str(),
        _ => {
            return Err(anyhow!(
                "object literal requires a type annotation to determine struct name"
            ))
        }
    };

    // Look up field types from the registry to propagate expected types to nested values
    let struct_fields = reg.get(struct_name).and_then(|def| match def {
        TypeDef::Struct { fields } => Some(fields.as_slice()),
        _ => None,
    });

    let mut fields = Vec::new();
    let mut base: Option<Box<Expr>> = None;

    for prop in &obj_lit.props {
        match prop {
            ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                ast::Prop::KeyValue(kv) => {
                    let key = match &kv.key {
                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                        ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => return Err(anyhow!("unsupported object literal key")),
                    };
                    // Resolve the expected type for this field from the registry
                    let field_expected = struct_fields
                        .and_then(|fs| fs.iter().find(|(name, _)| name == &key).map(|(_, ty)| ty));
                    let value = convert_expr(&kv.value, reg, field_expected)?;
                    fields.push((key, value));
                }
                ast::Prop::Shorthand(ident) => {
                    let key = ident.sym.to_string();
                    let field_expected = struct_fields
                        .and_then(|fs| fs.iter().find(|(name, _)| name == &key).map(|(_, ty)| ty));
                    let value =
                        convert_expr(&ast::Expr::Ident(ident.clone()), reg, field_expected)?;
                    fields.push((key, value));
                }
                _ => {
                    return Err(anyhow!(
                        "unsupported object literal property (only key-value pairs and shorthand)"
                    ))
                }
            },
            ast::PropOrSpread::Spread(spread_elem) => {
                if base.is_some() {
                    return Err(anyhow!(
                        "multiple spreads in object literal are not supported"
                    ));
                }
                let spread_expr = convert_expr(&spread_elem.expr, reg, None)?;
                base = Some(Box::new(spread_expr));
            }
        }
    }

    // When spread is present, expand remaining struct fields from the base expression
    if let Some(base_expr) = base {
        let all_fields = struct_fields.ok_or_else(|| {
            anyhow!(
                "spread in object literal requires struct '{}' to be registered in TypeRegistry",
                struct_name
            )
        })?;
        let explicit_keys: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
        for (field_name, _) in all_fields {
            if !explicit_keys.iter().any(|k| k == field_name) {
                fields.push((
                    field_name.clone(),
                    Expr::FieldAccess {
                        object: base_expr.clone(),
                        field: field_name.clone(),
                    },
                ));
            }
        }
    }

    Ok(Expr::StructInit {
        name: struct_name.to_string(),
        fields,
    })
}

/// Converts an SWC array literal to an IR `Expr::Vec` or `Expr::VecSpread`.
///
/// When `expected` is `RustType::Vec(inner)`, the inner type is propagated to each element.
/// If any element uses spread syntax (`...expr`), produces `Expr::VecSpread`.
fn convert_array_lit(
    array_lit: &ast::ArrayLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    let element_type = match expected {
        Some(RustType::Vec(inner)) => Some(inner.as_ref()),
        _ => None,
    };

    let has_spread = array_lit
        .elems
        .iter()
        .filter_map(|elem| elem.as_ref())
        .any(|elem| elem.spread.is_some());

    if !has_spread {
        let elements = array_lit
            .elems
            .iter()
            .filter_map(|elem| elem.as_ref())
            .map(|elem| convert_expr(&elem.expr, reg, element_type))
            .collect::<Result<Vec<_>>>()?;
        return Ok(Expr::Vec { elements });
    }

    let segments = array_lit
        .elems
        .iter()
        .filter_map(|elem| elem.as_ref())
        .map(|elem| {
            let expr = convert_expr(&elem.expr, reg, element_type)?;
            if elem.spread.is_some() {
                Ok(ir::VecSegment::Spread(expr))
            } else {
                Ok(ir::VecSegment::Element(expr))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Expr::VecSpread { segments })
}

/// Converts an SWC unary expression to an IR `UnaryOp`.
///
/// Supported operators: `!` (logical NOT), `-` (negation).
fn convert_unary_expr(unary: &ast::UnaryExpr, reg: &TypeRegistry) -> Result<Expr> {
    let op = match unary.op {
        ast::UnaryOp::Bang => UnOp::Not,
        ast::UnaryOp::Minus => UnOp::Neg,
        _ => return Err(anyhow!("unsupported unary operator: {:?}", unary.op)),
    };
    let operand = convert_expr(&unary.arg, reg, None)?;
    Ok(Expr::UnaryOp {
        op,
        operand: Box::new(operand),
    })
}

#[cfg(test)]
mod tests;
