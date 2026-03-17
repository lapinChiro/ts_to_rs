//! Statement conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC statement nodes into the IR [`Stmt`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, ClosureBody, Expr, MatchArm, MatchPattern, Param, RustType, Stmt, UnOp};
use crate::registry::TypeRegistry;
use crate::transformer::expressions::convert_expr;
use crate::transformer::types::convert_ts_type;
use crate::transformer::TypeEnv;
use crate::transformer::{extract_pat_ident_name, extract_prop_name, single_declarator};

/// Converts an SWC [`ast::Stmt`] into an IR [`Stmt`].
///
/// The `return_type` parameter is the enclosing function's return type, propagated to
/// return statements so that expected-type-based coercions (e.g., `StringLit` → `.to_string()`)
/// are applied automatically.
///
/// # Supported conversions
///
/// - Variable declarations (`const` → `let`, `let` → `let mut`)
/// - Return statements
/// - If/else statements
/// - Expression statements
///
/// # Errors
///
/// Returns an error for unsupported statement types.
pub fn convert_stmt(
    stmt: &ast::Stmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    match stmt {
        ast::Stmt::Return(ret) => {
            // Spread array detection at SWC AST level
            if let Some(stmts) = try_expand_spread_return(ret, reg, return_type, type_env)? {
                return Ok(stmts);
            }
            let expr = ret
                .arg
                .as_ref()
                .map(|e| convert_expr(e, reg, return_type, type_env))
                .transpose()?;
            Ok(vec![Stmt::Return(expr)])
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
            // Spread array detection at SWC AST level
            if let Some(stmts) = try_expand_spread_var_decl(var_decl, reg, type_env)? {
                return Ok(stmts);
            }
            if let Some(expanded) = try_convert_object_destructuring(var_decl, reg, type_env)? {
                Ok(expanded)
            } else if let Some(expanded) = try_convert_array_destructuring(var_decl, reg, type_env)?
            {
                Ok(expanded)
            } else {
                Ok(vec![convert_var_decl(var_decl, reg, type_env)?])
            }
        }
        ast::Stmt::If(if_stmt) => Ok(vec![convert_if_stmt(if_stmt, reg, return_type, type_env)?]),
        ast::Stmt::Expr(expr_stmt) => {
            // Spread array detection at SWC AST level
            if let Some(stmts) = try_expand_spread_expr_stmt(expr_stmt, reg, type_env)? {
                return Ok(stmts);
            }
            let expr = convert_expr(&expr_stmt.expr, reg, None, type_env)?;
            Ok(vec![Stmt::Expr(expr)])
        }
        ast::Stmt::Throw(throw_stmt) => Ok(vec![convert_throw_stmt(throw_stmt, reg, type_env)?]),
        ast::Stmt::While(while_stmt) => Ok(vec![convert_while_stmt(
            while_stmt,
            reg,
            return_type,
            type_env,
        )?]),
        ast::Stmt::ForOf(for_of) => Ok(vec![convert_for_of_stmt(
            for_of,
            reg,
            return_type,
            type_env,
        )?]),
        ast::Stmt::For(for_stmt) => match convert_for_stmt(for_stmt, reg, return_type, type_env) {
            Ok(s) => Ok(vec![s]),
            Err(_) => convert_for_stmt_as_loop(for_stmt, reg, return_type, type_env),
        },
        ast::Stmt::Break(break_stmt) => {
            let label = break_stmt.label.as_ref().map(|l| l.sym.to_string());
            Ok(vec![Stmt::Break { label, value: None }])
        }
        ast::Stmt::Continue(cont_stmt) => {
            let label = cont_stmt.label.as_ref().map(|l| l.sym.to_string());
            Ok(vec![Stmt::Continue { label }])
        }
        ast::Stmt::Labeled(labeled_stmt) => Ok(vec![convert_labeled_stmt(
            labeled_stmt,
            reg,
            return_type,
            type_env,
        )?]),
        ast::Stmt::DoWhile(do_while) => Ok(vec![convert_do_while_stmt(
            do_while,
            reg,
            return_type,
            type_env,
        )?]),
        ast::Stmt::Try(try_stmt) => convert_try_stmt(try_stmt, reg, return_type, type_env),
        ast::Stmt::Decl(ast::Decl::Fn(fn_decl)) => Ok(vec![convert_nested_fn_decl(fn_decl, reg)?]),
        ast::Stmt::Switch(switch_stmt) => {
            convert_switch_stmt(switch_stmt, reg, return_type, type_env)
        }
        // Local type declarations are skipped — they don't produce runtime code
        ast::Stmt::Decl(ast::Decl::TsInterface(_) | ast::Decl::TsTypeAlias(_)) => Ok(vec![]),
        _ => Err(anyhow!("unsupported statement: {:?}", stmt)),
    }
}

/// Converts a variable declaration to an IR `Stmt::Let`.
///
/// - `const` with primitive type → immutable (`let`)
/// - `const` with object/struct type → mutable (`let mut`), because TS `const` allows
///   field mutation while Rust `let` does not
/// - `let` / `var` → mutable (`let mut`)
fn convert_var_decl(
    var_decl: &ast::VarDecl,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Stmt> {
    // We only handle single-declarator variable declarations
    let declarator = single_declarator(var_decl)?;

    let name = extract_pat_ident_name(&declarator.name)?;

    let ty = match &declarator.name {
        ast::Pat::Ident(ident) => ident
            .type_ann
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
            .transpose()?,
        _ => None,
    };

    let mutable = if matches!(var_decl.kind, ast::VarDeclKind::Const) {
        // const + object type → let mut (TS const allows field mutation)
        ty.as_ref().is_some_and(is_object_type)
    } else {
        true
    };

    let init = declarator
        .init
        .as_ref()
        .map(|e| convert_expr(e, reg, ty.as_ref(), type_env))
        .transpose()?;

    Ok(Stmt::Let {
        mutable,
        name,
        ty,
        init,
    })
}

/// Infers a `RustType::Fn` from a closure expression for TypeEnv registration.
///
/// When `const greet = (name: string): string => ...` is converted, the variable's type
/// annotation is absent. This function extracts param/return types from the `Expr::Closure`
/// so the `Fn` type can be registered in TypeEnv, enabling `.to_string()` at call sites.
fn infer_fn_type_from_closure(init: &Option<Expr>) -> Option<RustType> {
    if let Some(Expr::Closure {
        params,
        return_type,
        ..
    }) = init
    {
        let param_types: Vec<RustType> = params.iter().filter_map(|p| p.ty.clone()).collect();
        // Only infer if at least one parameter has a type annotation
        if param_types.is_empty() && return_type.is_none() {
            return None;
        }
        let ret = return_type.clone().unwrap_or(RustType::Unit);
        Some(RustType::Fn {
            params: param_types,
            return_type: Box::new(ret),
        })
    } else {
        None
    }
}

/// Returns true if the type is an object/struct type that may need mutability
/// for field assignment in Rust (TS `const` allows field mutation).
fn is_object_type(ty: &RustType) -> bool {
    matches!(ty, RustType::Named { .. } | RustType::Vec(_))
}

/// Converts an if statement to an IR `Stmt::If`.
fn convert_if_stmt(
    if_stmt: &ast::IfStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Stmt> {
    let condition = convert_expr(&if_stmt.test, reg, None, type_env)?;

    let then_body = convert_block_or_stmt(&if_stmt.cons, reg, return_type, type_env)?;

    let else_body = if_stmt
        .alt
        .as_ref()
        .map(|alt| convert_block_or_stmt(alt, reg, return_type, type_env))
        .transpose()?;

    Ok(Stmt::If {
        condition,
        then_body,
        else_body,
    })
}

/// Converts a C-style `for` statement to `Stmt::ForIn` if it matches the simple counter pattern.
///
/// Pattern: `for (let i = start; i < end; i++)` → `for i in start..end`
///
/// Only `i++` and `i += 1` are recognized as increment expressions.
fn convert_for_stmt(
    for_stmt: &ast::ForStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Stmt> {
    // Extract: let <var> = <start>
    let (var, start) = match &for_stmt.init {
        Some(ast::VarDeclOrExpr::VarDecl(var_decl)) => {
            let decl = single_declarator(var_decl)
                .map_err(|_| anyhow!("unsupported for loop: multiple declarators"))?;
            let name = extract_pat_ident_name(&decl.name)
                .map_err(|_| anyhow!("unsupported for loop: non-ident binding"))?;
            let init = decl
                .init
                .as_ref()
                .ok_or_else(|| anyhow!("unsupported for loop: no initializer"))?;
            let start_expr = convert_expr(init, reg, None, type_env)?;
            (name, start_expr)
        }
        _ => {
            return Err(anyhow!(
                "unsupported for loop: no variable declaration init"
            ))
        }
    };

    // Extract: <var> < <end>
    let end = match &for_stmt.test {
        Some(test) => match test.as_ref() {
            ast::Expr::Bin(bin) if bin.op == ast::BinaryOp::Lt => {
                let left_name = match bin.left.as_ref() {
                    ast::Expr::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported for loop: non-ident in condition")),
                };
                if left_name != var {
                    return Err(anyhow!("unsupported for loop: condition var mismatch"));
                }
                convert_expr(&bin.right, reg, None, type_env)?
            }
            _ => return Err(anyhow!("unsupported for loop: non-simple condition")),
        },
        None => return Err(anyhow!("unsupported for loop: no test expression")),
    };

    // Verify: <var>++ or <var> += 1
    match &for_stmt.update {
        Some(update) => {
            let valid = match update.as_ref() {
                ast::Expr::Update(up) => {
                    up.op == ast::UpdateOp::PlusPlus
                        && matches!(up.arg.as_ref(), ast::Expr::Ident(ident) if ident.sym.as_ref() == var)
                }
                ast::Expr::Assign(assign) => {
                    matches!(&assign.left, ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) if ident.id.sym.as_ref() == var)
                        && matches!(assign.right.as_ref(), ast::Expr::Lit(ast::Lit::Num(n)) if n.value == 1.0)
                }
                _ => false,
            };
            if !valid {
                return Err(anyhow!("unsupported for loop: non-simple increment"));
            }
        }
        None => return Err(anyhow!("unsupported for loop: no update expression")),
    }

    let mut body = convert_block_or_stmt(&for_stmt.body, reg, return_type, type_env)?;

    // Range iterates over integers; shadow the loop variable as f64
    // to match TS's `number` type: `let i = i as f64;`
    body.insert(
        0,
        Stmt::Let {
            mutable: false,
            name: var.clone(),
            ty: None,
            init: Some(Expr::Cast {
                expr: Box::new(Expr::Ident(var.clone())),
                target: RustType::F64,
            }),
        },
    );

    Ok(Stmt::ForIn {
        label: None,
        var,
        iterable: Expr::Range {
            start: Some(Box::new(start)),
            end: Some(Box::new(end)),
        },
        body,
    })
}

/// Converts a `for...of` statement to `Stmt::ForIn`.
///
/// `for (const item of items) { ... }` → `for item in items { ... }`
fn convert_for_of_stmt(
    for_of: &ast::ForOfStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Stmt> {
    let var = match &for_of.left {
        ast::ForHead::VarDecl(var_decl) => {
            let decl = single_declarator(var_decl)
                .map_err(|_| anyhow!("for...of with multiple declarators is not supported"))?;
            extract_pat_ident_name(&decl.name)
                .map_err(|_| anyhow!("unsupported for...of binding pattern"))?
        }
        _ => return Err(anyhow!("unsupported for...of left-hand side")),
    };
    let iterable = convert_expr(&for_of.right, reg, None, type_env)?;
    let body = convert_block_or_stmt(&for_of.body, reg, return_type, type_env)?;
    Ok(Stmt::ForIn {
        label: None,
        var,
        iterable,
        body,
    })
}

/// Converts a labeled statement by attaching the label to the inner loop.
///
/// `label: for ...` → `'label: for ...`
/// `label: while ...` → `'label: while ...`
fn convert_labeled_stmt(
    labeled: &ast::LabeledStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Stmt> {
    let label_name = labeled.label.sym.to_string();
    match labeled.body.as_ref() {
        ast::Stmt::While(while_stmt) => {
            let condition = convert_expr(&while_stmt.test, reg, None, type_env)?;
            let body = convert_block_or_stmt(&while_stmt.body, reg, return_type, type_env)?;
            Ok(Stmt::While {
                label: Some(label_name),
                condition,
                body,
            })
        }
        ast::Stmt::ForOf(for_of) => {
            let mut stmt = convert_for_of_stmt(for_of, reg, return_type, type_env)?;
            if let Stmt::ForIn { ref mut label, .. } = stmt {
                *label = Some(label_name);
            }
            Ok(stmt)
        }
        ast::Stmt::For(for_stmt) => {
            let mut stmt = convert_for_stmt(for_stmt, reg, return_type, type_env)?;
            if let Stmt::ForIn { ref mut label, .. } = stmt {
                *label = Some(label_name);
            }
            Ok(stmt)
        }
        _ => Err(anyhow!(
            "unsupported labeled statement: label on non-loop statement"
        )),
    }
}

/// Converts a `while` statement to `Stmt::While`.
fn convert_while_stmt(
    while_stmt: &ast::WhileStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Stmt> {
    let condition = convert_expr(&while_stmt.test, reg, None, type_env)?;
    let body = convert_block_or_stmt(&while_stmt.body, reg, return_type, type_env)?;
    Ok(Stmt::While {
        label: None,
        condition,
        body,
    })
}

/// Expands a `try` statement into primitive IR statements.
///
/// - try/catch → `let mut _try_result = Ok(()); 'try_block: { body } if let Err(e) = ... { catch }`
/// - try/finally → `let _finally_guard = scopeguard::guard(...); body`
/// - try/catch/finally → combines both patterns
fn convert_try_stmt(
    try_stmt: &ast::TryStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();

    // 1. finally → scopeguard
    if let Some(finalizer) = &try_stmt.finalizer {
        let finally_body = convert_stmt_list(&finalizer.stmts, reg, return_type, type_env)?;
        result.push(Stmt::Let {
            mutable: false,
            name: "_finally_guard".to_string(),
            ty: None,
            init: Some(Expr::FnCall {
                name: "scopeguard::guard".to_string(),
                args: vec![
                    Expr::Unit,
                    Expr::Closure {
                        params: vec![crate::ir::Param {
                            name: "_".to_string(),
                            ty: None,
                        }],
                        return_type: None,
                        body: crate::ir::ClosureBody::Block(finally_body),
                    },
                ],
            }),
        });
    }

    // 2. Convert try body
    let try_body = convert_stmt_list(&try_stmt.block.stmts, reg, return_type, type_env)?;

    // 3. catch → labeled block + if-let-err
    if let Some(handler) = &try_stmt.handler {
        let catch_param = handler
            .param
            .as_ref()
            .and_then(|p| match p {
                swc_ecma_ast::Pat::Ident(ident) => Some(ident.id.sym.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| "_e".to_string());
        let catch_body = convert_stmt_list(&handler.body.stmts, reg, return_type, type_env)?;

        // let mut _try_result: Result<(), String> = Ok(());
        result.push(Stmt::Let {
            mutable: true,
            name: "_try_result".to_string(),
            ty: Some(RustType::Result {
                ok: Box::new(RustType::Unit),
                err: Box::new(RustType::String),
            }),
            init: Some(Expr::FnCall {
                name: "Ok".to_string(),
                args: vec![Expr::Unit],
            }),
        });

        // Rewrite throws and detect break/continue in try body
        let mut rewrite = TryBodyRewrite::default();
        let expanded_body = rewrite.rewrite(try_body, 0);

        // Add flag declarations if needed
        if rewrite.needs_break_flag {
            result.push(Stmt::Let {
                mutable: true,
                name: "_try_break".to_string(),
                ty: None,
                init: Some(Expr::BoolLit(false)),
            });
        }
        if rewrite.needs_continue_flag {
            result.push(Stmt::Let {
                mutable: true,
                name: "_try_continue".to_string(),
                ty: None,
                init: Some(Expr::BoolLit(false)),
            });
        }

        // Check if both try and catch always return — if so, add unreachable!()
        // after the if-let-Err to satisfy Rust's exhaustive return requirement.
        let try_ends_with_return = ends_with_return(&expanded_body);
        let catch_ends_with_return = ends_with_return(&catch_body);

        // 'try_block: { ...body with throw→assign+break, break/continue→flag+break... }
        result.push(Stmt::LabeledBlock {
            label: "try_block".to_string(),
            body: expanded_body,
        });

        // Post-block flag checks (before the if-let-err)
        if rewrite.needs_break_flag {
            result.push(Stmt::If {
                condition: Expr::Ident("_try_break".to_string()),
                then_body: vec![Stmt::Break {
                    label: None,
                    value: None,
                }],
                else_body: None,
            });
        }
        if rewrite.needs_continue_flag {
            result.push(Stmt::If {
                condition: Expr::Ident("_try_continue".to_string()),
                then_body: vec![Stmt::Continue { label: None }],
                else_body: None,
            });
        }

        // if let Err(param) = _try_result { ...catch... }
        result.push(Stmt::IfLet {
            pattern: format!("Err({catch_param})"),
            expr: Expr::Ident("_try_result".to_string()),
            then_body: catch_body,
            else_body: None,
        });

        if return_type.is_some() && try_ends_with_return && catch_ends_with_return {
            result.push(Stmt::Expr(Expr::MacroCall {
                name: "unreachable".to_string(),
                args: vec![],
                use_debug: vec![],
            }));
        }
    } else {
        // No catch → inline try body
        result.extend(try_body);
    }

    Ok(result)
}

/// Checks whether a statement list ends with a return on all exit paths.
fn ends_with_return(stmts: &[Stmt]) -> bool {
    match stmts.last() {
        Some(Stmt::Return(_)) => true,
        Some(Stmt::If {
            then_body,
            else_body: Some(else_body),
            ..
        }) => ends_with_return(then_body) && ends_with_return(else_body),
        _ => false,
    }
}

/// Rewrites try body statements: converts throws to assign+break,
/// and converts break/continue (at loop_depth 0) to flag+break.
#[derive(Default)]
struct TryBodyRewrite {
    needs_break_flag: bool,
    needs_continue_flag: bool,
}

impl TryBodyRewrite {
    /// Rewrites statements in a try body.
    ///
    /// `loop_depth`: 0 = directly in try body, >0 = inside an inner loop.
    /// At depth 0, bare break/continue target the try_block's enclosing loop,
    /// so they must be converted to flag + break 'try_block.
    fn rewrite(&mut self, stmts: Vec<Stmt>, loop_depth: usize) -> Vec<Stmt> {
        let mut result = Vec::new();
        for stmt in stmts {
            match stmt {
                // throw → assign + break 'try_block
                Stmt::Return(Some(ref expr)) if is_err_call(expr) => {
                    result.push(Stmt::Expr(Expr::Assign {
                        target: Box::new(Expr::Ident("_try_result".to_string())),
                        value: Box::new(expr.clone()),
                    }));
                    result.push(Stmt::Break {
                        label: Some("try_block".to_string()),
                        value: None,
                    });
                }
                // break (no label) at try body level → flag + break 'try_block
                Stmt::Break {
                    label: None,
                    value: None,
                } if loop_depth == 0 => {
                    self.needs_break_flag = true;
                    result.push(Stmt::Expr(Expr::Assign {
                        target: Box::new(Expr::Ident("_try_break".to_string())),
                        value: Box::new(Expr::BoolLit(true)),
                    }));
                    result.push(Stmt::Break {
                        label: Some("try_block".to_string()),
                        value: None,
                    });
                }
                // continue (no label) at try body level → flag + break 'try_block
                Stmt::Continue { label: None } if loop_depth == 0 => {
                    self.needs_continue_flag = true;
                    result.push(Stmt::Expr(Expr::Assign {
                        target: Box::new(Expr::Ident("_try_continue".to_string())),
                        value: Box::new(Expr::BoolLit(true)),
                    }));
                    result.push(Stmt::Break {
                        label: Some("try_block".to_string()),
                        value: None,
                    });
                }
                // Recurse into if/else (same loop depth)
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    result.push(Stmt::If {
                        condition,
                        then_body: self.rewrite(then_body, loop_depth),
                        else_body: else_body.map(|e| self.rewrite(e, loop_depth)),
                    });
                }
                // Recurse into loops (increment depth)
                Stmt::ForIn {
                    label,
                    var,
                    iterable,
                    body,
                } => {
                    result.push(Stmt::ForIn {
                        label,
                        var,
                        iterable,
                        body: self.rewrite(body, loop_depth + 1),
                    });
                }
                Stmt::While {
                    label,
                    condition,
                    body,
                } => {
                    result.push(Stmt::While {
                        label,
                        condition,
                        body: self.rewrite(body, loop_depth + 1),
                    });
                }
                Stmt::Loop { label, body } => {
                    result.push(Stmt::Loop {
                        label,
                        body: self.rewrite(body, loop_depth + 1),
                    });
                }
                // Don't recurse into nested LabeledBlock (nested try/catch)
                other => result.push(other),
            }
        }
        result
    }
}

/// Checks if an expression is an `Err(...)` call.
fn is_err_call(expr: &Expr) -> bool {
    matches!(expr, Expr::FnCall { name, .. } if name == "Err")
}

/// Converts a `throw` statement into `return Err(...)`.
///
/// - `throw new Error("msg")` → `return Err("msg".to_string())`
/// - `throw "msg"` → `return Err("msg".to_string())`
/// - Other throw expressions → `return Err(expr.to_string())`
fn convert_throw_stmt(
    throw_stmt: &ast::ThrowStmt,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Stmt> {
    let err_arg = extract_error_message(&throw_stmt.arg, reg, type_env);
    let err_expr = Expr::MethodCall {
        object: Box::new(err_arg),
        method: "to_string".to_string(),
        args: vec![],
    };
    Ok(Stmt::Return(Some(Expr::FnCall {
        name: "Err".to_string(),
        args: vec![err_expr],
    })))
}

/// Extracts the error message expression from a `throw` argument.
///
/// - `new Error("msg")` → `StringLit("msg")`
/// - `"msg"` → `StringLit("msg")`
/// - Other → converts as generic expression
fn extract_error_message(expr: &ast::Expr, reg: &TypeRegistry, type_env: &TypeEnv) -> Expr {
    match expr {
        ast::Expr::New(new_expr) => {
            // `throw new Error("msg")` → extract "msg"
            if let Some(args) = &new_expr.args {
                if let Some(first) = args.first() {
                    if let Ok(e) = convert_expr(&first.expr, reg, None, type_env) {
                        return e;
                    }
                }
            }
            Expr::StringLit("unknown error".to_string())
        }
        other => convert_expr(other, reg, None, type_env)
            .unwrap_or_else(|_| Expr::StringLit("unknown error".to_string())),
    }
}

/// Converts a list of SWC statements into IR statements.
///
/// Handles special cases like `try/catch` blocks, variable destructuring,
/// and labeled statements that need expansion at the list level.
pub fn convert_stmt_list(
    stmts: &[ast::Stmt],
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();
    for stmt in stmts {
        let converted = convert_stmt(stmt, reg, return_type, type_env)?;
        for s in &converted {
            match s {
                Stmt::Let {
                    name, ty: Some(ty), ..
                } => {
                    type_env.insert(name.clone(), ty.clone());
                }
                // Infer Fn type from closure init for TypeEnv (enables .to_string() at call sites)
                Stmt::Let {
                    name,
                    ty: None,
                    init: Some(init),
                    ..
                } => {
                    if let Some(fn_type) = infer_fn_type_from_closure(&Some(init.clone())) {
                        type_env.insert(name.clone(), fn_type);
                    }
                }
                _ => {}
            }
        }
        result.extend(converted);
    }
    mark_mutated_vars(&mut result);
    Ok(result)
}

/// Mutating methods that require `&mut self` on the receiver.
const MUTATING_METHODS: &[&str] = &[
    "reverse", "sort", "sort_by", "drain", "push", "pop", "remove", "insert", "clear", "truncate",
    "retain",
];

/// Post-processes a statement list to mark immutable variables as `let mut`
/// when subsequent statements mutate them (field assignment or mutating method call).
/// Also marks closure bindings as `let mut` when the closure captures mutably (FnMut).
fn mark_mutated_vars(stmts: &mut [Stmt]) {
    let mut needs_mut = std::collections::HashSet::new();
    collect_mutated_vars(stmts, &mut needs_mut);

    // Detect closures that capture outer variables mutably → closure binding needs `let mut`
    let mut closure_needs_mut = std::collections::HashSet::new();
    for stmt in stmts.iter() {
        if let Stmt::Let {
            name,
            init: Some(Expr::Closure { body, .. }),
            ..
        } = stmt
        {
            let mut closure_mutations = std::collections::HashSet::new();
            match body {
                ClosureBody::Block(body_stmts) => {
                    collect_closure_assigns(body_stmts, &mut closure_mutations);
                }
                ClosureBody::Expr(expr) => {
                    collect_assigns_from_expr(expr, &mut closure_mutations);
                }
            }
            if !closure_mutations.is_empty() {
                closure_needs_mut.insert(name.clone());
            }
        }
    }
    needs_mut.extend(closure_needs_mut);

    for stmt in stmts.iter_mut() {
        if let Stmt::Let { mutable, name, .. } = stmt {
            if !*mutable && needs_mut.contains(name.as_str()) {
                *mutable = true;
            }
        }
    }
}

/// Collects variable names that are assigned to inside closure bodies (direct assignment).
fn collect_closure_assigns(stmts: &[Stmt], names: &mut std::collections::HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::TailExpr(expr) => {
                collect_assigns_from_expr(expr, names);
            }
            _ => {}
        }
    }
}

/// Collects variable names from direct assignment expressions (`x = ...`, `x += ...`).
fn collect_assigns_from_expr(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    if let Expr::Assign { target, .. } = expr {
        if let Expr::Ident(name) = target.as_ref() {
            names.insert(name.clone());
        }
    }
}

/// Recursively collects variable names that are targets of field assignments or mutating methods.
fn collect_mutated_vars(stmts: &[Stmt], names: &mut std::collections::HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::TailExpr(expr) => {
                collect_mutated_vars_from_expr(expr, names);
            }
            Stmt::Let {
                init: Some(expr), ..
            } => {
                collect_mutated_vars_from_expr(expr, names);
            }
            Stmt::Return(Some(expr)) => {
                collect_mutated_vars_from_expr(expr, names);
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect_mutated_vars(then_body, names);
                if let Some(els) = else_body {
                    collect_mutated_vars(els, names);
                }
            }
            Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::Loop { body, .. } => {
                collect_mutated_vars(body, names);
            }
            _ => {}
        }
    }
}

/// Checks if an expression mutates a variable via field assignment or mutating method call.
fn collect_mutated_vars_from_expr(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    match expr {
        // Field assignment: obj.field = value
        Expr::Assign { target, value, .. } => {
            if let Expr::FieldAccess { object, .. } = target.as_ref() {
                if let Expr::Ident(name) = object.as_ref() {
                    names.insert(name.clone());
                }
            }
            collect_mutated_vars_from_expr(value, names);
        }
        // Mutating method call: arr.push(...)
        Expr::MethodCall { object, method, .. } => {
            if MUTATING_METHODS.contains(&method.as_str()) {
                if let Expr::Ident(name) = object.as_ref() {
                    names.insert(name.clone());
                }
            }
            collect_mutated_vars_from_expr(object, names);
        }
        _ => {}
    }
}

// --- Spread array detection and expansion at SWC AST level ---

/// Returns true if an SWC ArrayLit contains spread elements.
fn has_spread_elements(array_lit: &ast::ArrayLit) -> bool {
    array_lit
        .elems
        .iter()
        .filter_map(|e| e.as_ref())
        .any(|e| e.spread.is_some())
}

/// Extracts the initializer array literal from a VarDecl if it is a spread array.
fn extract_spread_array_init(var_decl: &ast::VarDecl) -> Option<(&ast::Pat, &ast::ArrayLit)> {
    let declarator = var_decl.decls.first()?;
    let init = declarator.init.as_ref()?;
    let array_lit = match init.as_ref() {
        ast::Expr::Array(a) => a,
        _ => return None,
    };
    if has_spread_elements(array_lit) {
        Some((&declarator.name, array_lit))
    } else {
        None
    }
}

/// Converts spread array elements to IR expressions and marks whether each is a spread.
fn convert_spread_segments(
    array_lit: &ast::ArrayLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Vec<(bool, Expr)>> {
    let element_type = match expected {
        Some(RustType::Vec(inner)) => Some(inner.as_ref()),
        _ => None,
    };
    array_lit
        .elems
        .iter()
        .filter_map(|e| e.as_ref())
        .map(|elem| {
            let expr = convert_expr(&elem.expr, reg, element_type, type_env)?;
            Ok((elem.spread.is_some(), expr))
        })
        .collect()
}

/// Generates push/extend statements from spread segments for a given variable name.
fn emit_spread_ops(var_name: &str, segments: &[(bool, Expr)], result: &mut Vec<Stmt>) {
    for (is_spread, expr) in segments {
        if *is_spread {
            result.push(Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident(var_name.to_string())),
                method: "extend".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(expr.clone()),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }],
            }));
        } else {
            result.push(Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident(var_name.to_string())),
                method: "push".to_string(),
                args: vec![expr.clone()],
            }));
        }
    }
}

/// Detects `let x = [...arr, 1]` at SWC AST level and expands to IR statements.
///
/// Returns `None` if the VarDecl does not contain a spread array initializer.
fn try_expand_spread_var_decl(
    var_decl: &ast::VarDecl,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Option<Vec<Stmt>>> {
    let (pat, array_lit) = match extract_spread_array_init(var_decl) {
        Some(v) => v,
        None => return Ok(None),
    };
    let name = extract_pat_ident_name(pat)?;
    let ty = match pat {
        ast::Pat::Ident(ident) => ident
            .type_ann
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
            .transpose()?,
        _ => None,
    };

    let segments = convert_spread_segments(array_lit, reg, ty.as_ref(), type_env)?;

    // Optimization: [...arr] → let name = arr.clone();
    if segments.len() == 1 && segments[0].0 {
        return Ok(Some(vec![Stmt::Let {
            mutable: false,
            name,
            ty,
            init: Some(Expr::MethodCall {
                object: Box::new(segments[0].1.clone()),
                method: "clone".to_string(),
                args: vec![],
            }),
        }]));
    }

    let mut result = Vec::new();
    result.push(Stmt::Let {
        mutable: true,
        name: name.clone(),
        ty,
        init: Some(Expr::FnCall {
            name: "Vec::new".to_string(),
            args: vec![],
        }),
    });
    emit_spread_ops(&name, &segments, &mut result);
    Ok(Some(result))
}

/// Detects `return [...arr, 1]` at SWC AST level and expands to IR statements.
///
/// Returns `None` if the return statement does not contain a spread array.
fn try_expand_spread_return(
    ret: &ast::ReturnStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Option<Vec<Stmt>>> {
    let arg = match &ret.arg {
        Some(arg) => arg,
        None => return Ok(None),
    };
    let array_lit = match arg.as_ref() {
        ast::Expr::Array(a) if has_spread_elements(a) => a,
        _ => return Ok(None),
    };

    let segments = convert_spread_segments(array_lit, reg, return_type, type_env)?;

    // Optimization: return [...arr] → return arr.clone();
    if segments.len() == 1 && segments[0].0 {
        return Ok(Some(vec![Stmt::Return(Some(Expr::MethodCall {
            object: Box::new(segments[0].1.clone()),
            method: "clone".to_string(),
            args: vec![],
        }))]));
    }

    let var_name = "__spread_vec".to_string();
    let mut result = Vec::new();
    result.push(Stmt::Let {
        mutable: true,
        name: var_name.clone(),
        ty: None,
        init: Some(Expr::FnCall {
            name: "Vec::new".to_string(),
            args: vec![],
        }),
    });
    emit_spread_ops(&var_name, &segments, &mut result);
    result.push(Stmt::Return(Some(Expr::Ident(var_name))));
    Ok(Some(result))
}

/// Detects `[...arr, 1]` as a bare expression statement and expands to IR statements.
///
/// Returns `None` if the expression is not a spread array.
fn try_expand_spread_expr_stmt(
    expr_stmt: &ast::ExprStmt,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Option<Vec<Stmt>>> {
    let array_lit = match expr_stmt.expr.as_ref() {
        ast::Expr::Array(a) if has_spread_elements(a) => a,
        _ => return Ok(None),
    };

    let segments = convert_spread_segments(array_lit, reg, None, type_env)?;

    // Optimization: [...arr] → arr.clone();
    if segments.len() == 1 && segments[0].0 {
        return Ok(Some(vec![Stmt::Expr(Expr::MethodCall {
            object: Box::new(segments[0].1.clone()),
            method: "clone".to_string(),
            args: vec![],
        })]));
    }

    let var_name = "__spread_vec".to_string();
    let mut result = Vec::new();
    result.push(Stmt::Let {
        mutable: true,
        name: var_name.clone(),
        ty: None,
        init: Some(Expr::FnCall {
            name: "Vec::new".to_string(),
            args: vec![],
        }),
    });
    emit_spread_ops(&var_name, &segments, &mut result);
    Ok(Some(result))
}

/// Tries to convert a variable declaration with object destructuring pattern.
///
/// `const { x, y } = obj` → `[let x = obj.x, let y = obj.y]`
///
/// Returns `None` if the declaration is not an object destructuring pattern,
/// allowing the caller to fall back to normal processing.
fn try_convert_object_destructuring(
    var_decl: &ast::VarDecl,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Option<Vec<Stmt>>> {
    let declarator = match single_declarator(var_decl) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    let obj_pat = match &declarator.name {
        ast::Pat::Object(obj_pat) => obj_pat,
        _ => return Ok(None),
    };

    let source = declarator
        .init
        .as_ref()
        .ok_or_else(|| anyhow!("object destructuring requires an initializer"))?;
    let source_expr = convert_expr(source, reg, None, type_env)?;

    let mutable = !matches!(var_decl.kind, ast::VarDeclKind::Const);
    let source_type = crate::transformer::expressions::resolve_expr_type(source, type_env, reg);
    let mut stmts = Vec::new();

    expand_object_pat_props(
        &obj_pat.props,
        &source_expr,
        mutable,
        &mut stmts,
        reg,
        type_env,
        source_type.as_ref(),
    )?;

    Ok(Some(stmts))
}

/// Recursively expands object destructuring pattern properties into `let` statements.
fn expand_object_pat_props(
    props: &[ast::ObjectPatProp],
    source_expr: &Expr,
    mutable: bool,
    stmts: &mut Vec<Stmt>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    source_type: Option<&RustType>,
) -> Result<()> {
    for prop in props {
        match prop {
            ast::ObjectPatProp::Assign(assign) => {
                // { x } or { x = default } — shorthand with optional default
                let field_name = assign.key.sym.to_string();
                let field_access = Expr::FieldAccess {
                    object: Box::new(source_expr.clone()),
                    field: field_name.clone(),
                };
                let init_expr = if let Some(default_expr) = &assign.value {
                    // { x = value } → obj.x.unwrap_or(value) or unwrap_or_else(|| value)
                    let default_ir = convert_expr(default_expr, reg, None, type_env)?;
                    match &default_ir {
                        // String values need unwrap_or_else to avoid eager evaluation
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
                    mutable,
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
                    // { a: { b, c } } — nested destructuring
                    ast::Pat::Object(inner_pat) => {
                        expand_object_pat_props(
                            &inner_pat.props,
                            &nested_source,
                            mutable,
                            stmts,
                            reg,
                            type_env,
                            None,
                        )?;
                    }
                    // { x: newX } — rename
                    _ => {
                        let binding_name = extract_pat_ident_name(kv.value.as_ref())
                            .map_err(|_| anyhow!("unsupported destructuring value pattern"))?;
                        stmts.push(Stmt::Let {
                            mutable,
                            name: binding_name,
                            ty: None,
                            init: Some(nested_source),
                        });
                    }
                }
            }
            ast::ObjectPatProp::Rest(_rest) => {
                // Collect explicitly named fields in this destructuring
                let explicit_fields: Vec<String> = props
                    .iter()
                    .filter_map(|p| match p {
                        ast::ObjectPatProp::Assign(a) => Some(a.key.sym.to_string()),
                        ast::ObjectPatProp::KeyValue(kv) => extract_prop_name(&kv.key).ok(),
                        _ => None,
                    })
                    .collect();

                // Try to get remaining fields from TypeRegistry
                let type_name = source_type.and_then(|ty| match ty {
                    RustType::Named { name, .. } => Some(name.as_str()),
                    _ => None,
                });
                if let Some(crate::registry::TypeDef::Struct { fields, .. }) =
                    type_name.and_then(|n| reg.get(n))
                {
                    for (field_name, _) in fields {
                        if !explicit_fields.contains(field_name) {
                            stmts.push(Stmt::Let {
                                mutable,
                                name: field_name.clone(),
                                ty: None,
                                init: Some(Expr::FieldAccess {
                                    object: Box::new(source_expr.clone()),
                                    field: field_name.clone(),
                                }),
                            });
                        }
                    }
                }
                // If type info unavailable, rest is silently skipped
                // (the explicit fields are still expanded)
            }
        }
    }

    Ok(())
}

/// Checks whether a case body is terminated (break, return, throw, or continue).
fn is_case_terminated(stmts: &[ast::Stmt]) -> bool {
    stmts.last().is_some_and(|s| {
        matches!(
            s,
            ast::Stmt::Break(_)
                | ast::Stmt::Return(_)
                | ast::Stmt::Throw(_)
                | ast::Stmt::Continue(_)
        )
    })
}

/// Converts a `switch` statement to a `match` expression or fall-through pattern.
///
/// - If all cases end with `break` (or are empty fall-throughs), generates a clean `Stmt::Match`.
/// - If any case has a non-empty body without `break` (fall-through with code), generates
///   a `LabeledBlock` + flag pattern.
fn convert_switch_stmt(
    switch: &ast::SwitchStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    // Check if this is a switch on a discriminated union's tag field
    if let Some(result) =
        try_convert_discriminated_union_switch(switch, reg, return_type, type_env)?
    {
        return Ok(result);
    }

    let discriminant = convert_expr(&switch.discriminant, reg, None, type_env)?;

    // Analyze cases: detect if any has a non-trivial fall-through
    let case_count = switch.cases.len();
    let has_code_fallthrough = switch.cases.iter().enumerate().any(|(i, case)| {
        let is_last = i == case_count - 1;
        let has_body = !case.cons.is_empty();
        let is_terminated = is_case_terminated(&case.cons);
        // A case with code but not terminated is a fall-through (unless it's the last case)
        has_body && !is_terminated && !is_last
    });

    if has_code_fallthrough {
        convert_switch_fallthrough(switch, &discriminant, reg, return_type, type_env)
    } else {
        convert_switch_clean_match(switch, discriminant, reg, return_type, type_env)
    }
}

/// discriminated union の tag フィールドに対する switch を enum match に変換する。
///
/// `switch (s.kind) { case "circle": ... }` → `match &s { Shape::Circle { .. } => ... }`
fn try_convert_discriminated_union_switch(
    switch: &ast::SwitchStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Option<Vec<Stmt>>> {
    use crate::registry::TypeDef;
    use crate::transformer::expressions::resolve_expr_type;

    // Check if discriminant is a member expression (e.g., s.kind)
    let member = match switch.discriminant.as_ref() {
        ast::Expr::Member(m) => m,
        _ => return Ok(None),
    };

    let field_name = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return Ok(None),
    };

    // Resolve the object's type
    let obj_type = resolve_expr_type(&member.obj, type_env, reg);
    let enum_name = match &obj_type {
        Some(RustType::Named { name, .. }) => name.clone(),
        _ => return Ok(None),
    };

    // Check if this is a discriminated union and the field is the tag
    let (string_values, variant_fields) = match reg.get(&enum_name) {
        Some(TypeDef::Enum {
            tag_field: Some(tag),
            string_values,
            variant_fields,
            ..
        }) if *tag == field_name => (string_values, variant_fields),
        _ => return Ok(None),
    };

    // Extract the object variable name for field access rewriting (e.g., "s" from "s.kind")
    let obj_var_name = match member.obj.as_ref() {
        ast::Expr::Ident(ident) => Some(ident.sym.to_string()),
        _ => None,
    };

    // Convert the match: match on &object (not object.tag)
    let object = convert_expr(&member.obj, reg, None, type_env)?;
    let match_expr = Expr::Ref(Box::new(object));

    let mut arms: Vec<MatchArm> = Vec::new();
    let mut pending_patterns: Vec<MatchPattern> = Vec::new();
    let mut pending_variant_names: Vec<String> = Vec::new();

    for case in &switch.cases {
        if let Some(test) = &case.test {
            // Extract string literal from case
            let str_value = match test.as_ref() {
                ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                _ => return Ok(None), // Non-string case → fallback to normal switch
            };

            if let Some(variant_name) = string_values.get(&str_value) {
                pending_patterns.push(MatchPattern::EnumVariant {
                    path: format!("{enum_name}::{variant_name}"),
                    bindings: vec![],
                });
                pending_variant_names.push(variant_name.clone());
            } else {
                return Ok(None); // Unknown variant → fallback
            }
        }

        // Empty body = fall-through, accumulate patterns
        if case.cons.is_empty() {
            continue;
        }

        // Scan body for field accesses on the DU variable (e.g., s.radius)
        // and collect field names to bind in the match pattern
        let needed_fields = if let Some(ref var_name) = obj_var_name {
            collect_du_field_accesses(&case.cons, var_name, &field_name)
        } else {
            Vec::new()
        };

        // Update bindings on pending patterns and register fields in TypeEnv
        if !needed_fields.is_empty() {
            for pattern in &mut pending_patterns {
                if let MatchPattern::EnumVariant { bindings, path, .. } = pattern {
                    // Extract variant name from path (e.g., "Shape::Circle" → "Circle")
                    let vname = path.rsplit("::").next().unwrap_or("");
                    if let Some(fields) = variant_fields.get(vname) {
                        *bindings = needed_fields
                            .iter()
                            .filter(|f| fields.iter().any(|(n, _)| n == *f))
                            .cloned()
                            .collect();
                    }
                }
            }
        }

        // Collect field types for TypeEnv registration
        let mut field_types: Vec<(String, RustType)> = Vec::new();
        for vname in &pending_variant_names {
            if let Some(fields) = variant_fields.get(vname) {
                for (fname, ftype) in fields {
                    if needed_fields.contains(fname) && !field_types.iter().any(|(n, _)| n == fname)
                    {
                        field_types.push((fname.clone(), ftype.clone()));
                    }
                }
            }
        }

        // Push scope with bound fields, convert body, pop scope
        type_env.push_scope();
        for (fname, ftype) in &field_types {
            type_env.insert(fname.clone(), ftype.clone());
        }

        let body = case
            .cons
            .iter()
            .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
            .map(|s| convert_stmt(s, reg, return_type, type_env))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        type_env.pop_scope();

        if case.test.is_none() {
            pending_patterns.push(MatchPattern::Wildcard);
        }

        arms.push(MatchArm {
            patterns: std::mem::take(&mut pending_patterns),
            guard: None,
            body,
        });
        pending_variant_names.clear();
    }

    Ok(Some(vec![Stmt::Match {
        expr: match_expr,
        arms,
    }]))
}

/// switch arm body 内で `obj_var.field` 形式のフィールドアクセスを収集する。
///
/// `tag_field`（discriminant フィールド）はスキップする。
fn collect_du_field_accesses(stmts: &[ast::Stmt], obj_var: &str, tag_field: &str) -> Vec<String> {
    let mut fields = Vec::new();
    for stmt in stmts {
        collect_du_field_accesses_from_stmt(stmt, obj_var, tag_field, &mut fields);
    }
    fields.sort();
    fields.dedup();
    fields
}

fn collect_du_field_accesses_from_stmt(
    stmt: &ast::Stmt,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    use swc_ecma_ast as ast;
    match stmt {
        ast::Stmt::Expr(expr_stmt) => {
            collect_du_field_accesses_from_expr(&expr_stmt.expr, obj_var, tag_field, fields);
        }
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                collect_du_field_accesses_from_expr(arg, obj_var, tag_field, fields);
            }
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
            for decl in &var_decl.decls {
                if let Some(init) = &decl.init {
                    collect_du_field_accesses_from_expr(init, obj_var, tag_field, fields);
                }
            }
        }
        ast::Stmt::If(if_stmt) => {
            collect_du_field_accesses_from_expr(&if_stmt.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_stmt(&if_stmt.cons, obj_var, tag_field, fields);
            if let Some(alt) = &if_stmt.alt {
                collect_du_field_accesses_from_stmt(alt, obj_var, tag_field, fields);
            }
        }
        ast::Stmt::Block(block) => {
            for s in &block.stmts {
                collect_du_field_accesses_from_stmt(s, obj_var, tag_field, fields);
            }
        }
        _ => {}
    }
}

fn collect_du_field_accesses_from_expr(
    expr: &ast::Expr,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    use swc_ecma_ast as ast;
    match expr {
        ast::Expr::Member(member) => {
            // Check if this is obj_var.field
            if let ast::Expr::Ident(ident) = member.obj.as_ref() {
                if ident.sym.as_ref() == obj_var {
                    if let ast::MemberProp::Ident(prop) = &member.prop {
                        let field_name = prop.sym.to_string();
                        if field_name != tag_field {
                            fields.push(field_name);
                        }
                    }
                }
            }
            // Also recurse into obj in case of nested access
            collect_du_field_accesses_from_expr(&member.obj, obj_var, tag_field, fields);
        }
        ast::Expr::Bin(bin) => {
            collect_du_field_accesses_from_expr(&bin.left, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr(&bin.right, obj_var, tag_field, fields);
        }
        ast::Expr::Unary(unary) => {
            collect_du_field_accesses_from_expr(&unary.arg, obj_var, tag_field, fields);
        }
        ast::Expr::Call(call) => {
            if let ast::Callee::Expr(callee) = &call.callee {
                collect_du_field_accesses_from_expr(callee, obj_var, tag_field, fields);
            }
            for arg in &call.args {
                collect_du_field_accesses_from_expr(&arg.expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Paren(paren) => {
            collect_du_field_accesses_from_expr(&paren.expr, obj_var, tag_field, fields);
        }
        ast::Expr::Tpl(tpl) => {
            for expr in &tpl.exprs {
                collect_du_field_accesses_from_expr(expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Cond(cond) => {
            collect_du_field_accesses_from_expr(&cond.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr(&cond.cons, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr(&cond.alt, obj_var, tag_field, fields);
        }
        _ => {}
    }
}

/// Returns true if the expression is a literal that can safely be used as a Rust match pattern.
///
/// Non-literal expressions (identifiers, function calls, etc.) would become variable bindings
/// in a Rust match, silently changing semantics. These must use match guards instead.
fn is_literal_match_pattern(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit(_) | Expr::NumberLit(_) | Expr::StringLit(_) | Expr::BoolLit(_)
    )
}

/// Builds a combined guard expression from multiple non-literal patterns.
///
/// For a single pattern: `discriminant == pattern`
/// For multiple patterns: `discriminant == p1 || discriminant == p2 || ...`
fn build_combined_guard(discriminant: &Expr, patterns: Vec<Expr>) -> Expr {
    let mut parts = patterns.into_iter().map(|p| Expr::BinaryOp {
        left: Box::new(discriminant.clone()),
        op: BinOp::Eq,
        right: Box::new(p),
    });
    let first = parts.next().expect("at least one pattern");
    parts.fold(first, |acc, part| Expr::BinaryOp {
        left: Box::new(acc),
        op: BinOp::LogicalOr,
        right: Box::new(part),
    })
}

/// Converts a switch with no code fall-through into a clean `Stmt::Match`.
fn convert_switch_clean_match(
    switch: &ast::SwitchStmt,
    discriminant: Expr,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    let mut arms: Vec<MatchArm> = Vec::new();
    let mut pending_patterns: Vec<MatchPattern> = Vec::new();
    let mut pending_exprs: Vec<Expr> = Vec::new();

    for case in &switch.cases {
        if let Some(test) = &case.test {
            let pattern = convert_expr(test, reg, None, type_env)?;
            pending_exprs.push(pattern.clone());
            pending_patterns.push(MatchPattern::Literal(pattern));
        }

        // Empty body = fall-through to next case, accumulate patterns
        if case.cons.is_empty() {
            continue;
        }

        // Non-empty body: create an arm with all accumulated patterns
        let body = case
            .cons
            .iter()
            .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
            .map(|s| convert_stmt(s, reg, return_type, type_env))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        if case.test.is_none() {
            pending_patterns.push(MatchPattern::Wildcard);
        }

        // Check if any pending pattern is non-literal
        let has_non_literal = pending_exprs.iter().any(|e| !is_literal_match_pattern(e));

        let (patterns, guard) = if has_non_literal {
            // Convert to wildcard + guard to avoid variable binding in match
            let guard = build_combined_guard(&discriminant, std::mem::take(&mut pending_exprs));
            std::mem::take(&mut pending_patterns);
            (vec![MatchPattern::Wildcard], Some(guard))
        } else {
            pending_exprs.clear();
            (std::mem::take(&mut pending_patterns), None)
        };

        arms.push(MatchArm {
            patterns,
            guard,
            body,
        });
    }

    Ok(vec![Stmt::Match {
        expr: discriminant,
        arms,
    }])
}

/// Converts a switch with code fall-through into a labeled block + flag pattern.
///
/// ```text
/// 'switch: {
///     let mut _fall = false;
///     if discriminant == val1 || _fall { body1; _fall = true; }
///     if discriminant == val2 || _fall { body2; break 'switch; }
///     // default:
///     default_body;
/// }
/// ```
fn convert_switch_fallthrough(
    switch: &ast::SwitchStmt,
    discriminant: &Expr,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    let mut block_body = Vec::new();

    // let mut _fall = false;
    block_body.push(Stmt::Let {
        mutable: true,
        name: "_fall".to_string(),
        ty: None,
        init: Some(Expr::BoolLit(false)),
    });

    for case in &switch.cases {
        let ends_with_break = case
            .cons
            .last()
            .is_some_and(|s| matches!(s, ast::Stmt::Break(_)));

        let body: Vec<Stmt> = case
            .cons
            .iter()
            .filter(|s| !matches!(s, ast::Stmt::Break(_)))
            .map(|s| convert_stmt(s, reg, return_type, type_env))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        if let Some(test) = &case.test {
            // case val: ...
            let test_expr = convert_expr(test, reg, None, type_env)?;
            let condition = Expr::BinaryOp {
                left: Box::new(Expr::BinaryOp {
                    left: Box::new(discriminant.clone()),
                    op: BinOp::Eq,
                    right: Box::new(test_expr),
                }),
                op: BinOp::LogicalOr,
                right: Box::new(Expr::Ident("_fall".to_string())),
            };

            let mut then_body = body;
            if ends_with_break {
                then_body.push(Stmt::Break {
                    label: Some("switch".to_string()),
                    value: None,
                });
            } else {
                // No break → set fall-through flag
                then_body.push(Stmt::Expr(Expr::Assign {
                    target: Box::new(Expr::Ident("_fall".to_string())),
                    value: Box::new(Expr::BoolLit(true)),
                }));
            }

            block_body.push(Stmt::If {
                condition,
                then_body,
                else_body: None,
            });
        } else {
            // default: ... (always executes if reached)
            block_body.extend(body);
        }
    }

    Ok(vec![Stmt::LabeledBlock {
        label: "switch".to_string(),
        body: block_body,
    }])
}

/// Converts a `do...while` statement to `loop { body; if !(cond) { break; } }`.
fn convert_do_while_stmt(
    do_while: &ast::DoWhileStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Stmt> {
    let body_stmts = match do_while.body.as_ref() {
        ast::Stmt::Block(block) => convert_stmt_list(&block.stmts, reg, return_type, type_env)?,
        single => convert_stmt(single, reg, return_type, type_env)?,
    };

    let condition = convert_expr(&do_while.test, reg, None, type_env)?;
    let break_check = Stmt::If {
        condition: Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(condition),
        },
        then_body: vec![Stmt::Break {
            label: None,
            value: None,
        }],
        else_body: None,
    };

    let mut loop_body = body_stmts;
    loop_body.push(break_check);

    Ok(Stmt::Loop {
        label: None,
        body: loop_body,
    })
}

/// Expands array destructuring into individual indexed `let` bindings.
///
/// `const [a, b] = arr` → `[let a = arr[0], let b = arr[1]]`
///
/// Returns `None` if the declaration is not an array destructuring pattern.
fn try_convert_array_destructuring(
    var_decl: &ast::VarDecl,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Option<Vec<Stmt>>> {
    let declarator = match single_declarator(var_decl) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    let arr_pat = match &declarator.name {
        ast::Pat::Array(arr_pat) => arr_pat,
        _ => return Ok(None),
    };

    let source = declarator
        .init
        .as_ref()
        .ok_or_else(|| anyhow!("array destructuring requires an initializer"))?;
    let source_expr = convert_expr(source, reg, None, type_env)?;

    let mutable = !matches!(var_decl.kind, ast::VarDeclKind::Const);
    let mut stmts = Vec::new();

    for (i, elem) in arr_pat.elems.iter().enumerate() {
        let pat = match elem {
            Some(pat) => pat,
            None => continue, // skip hole: `[a, , b]`
        };

        // Rest element: `[first, ...rest]`
        if let ast::Pat::Rest(rest_pat) = pat {
            let name = extract_pat_ident_name(&rest_pat.arg)?;
            stmts.push(Stmt::Let {
                mutable,
                name,
                ty: None,
                init: Some(Expr::MethodCall {
                    object: Box::new(Expr::Index {
                        object: Box::new(source_expr.clone()),
                        index: Box::new(Expr::Range {
                            start: Some(Box::new(Expr::NumberLit(i as f64))),
                            end: None,
                        }),
                    }),
                    method: "to_vec".to_string(),
                    args: vec![],
                }),
            });
            break; // rest must be last
        }

        let name = extract_pat_ident_name(pat)?;
        stmts.push(Stmt::Let {
            mutable,
            name,
            ty: None,
            init: Some(Expr::Index {
                object: Box::new(source_expr.clone()),
                index: Box::new(Expr::NumberLit(i as f64)),
            }),
        });
    }

    Ok(Some(stmts))
}

/// Converts a general C-style `for` statement to `[Stmt::Let, Stmt::Loop]` pattern.
///
/// Handles any `for` loop that doesn't match the simple counter pattern:
/// - `for (let i = n; i >= 0; i--)` → `let mut i = n; loop { if !(i >= 0) { break; } body; i -= 1; }`
/// - `for (let i = 0; i < n; i += 2)` → `let mut i = 0; loop { if !(i < n) { break; } body; i += 2; }`
fn convert_for_stmt_as_loop(
    for_stmt: &ast::ForStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();

    // 1. Extract init → Stmt::Let { mutable: true, ... }
    match &for_stmt.init {
        Some(ast::VarDeclOrExpr::VarDecl(var_decl)) => {
            let decl = single_declarator(var_decl)
                .map_err(|_| anyhow!("unsupported for loop: multiple declarators"))?;
            let name = extract_pat_ident_name(&decl.name)
                .map_err(|_| anyhow!("unsupported for loop: non-ident binding"))?;
            let init_expr = decl
                .init
                .as_ref()
                .map(|e| convert_expr(e, reg, None, type_env))
                .transpose()?;
            result.push(Stmt::Let {
                mutable: true,
                name,
                ty: None,
                init: init_expr,
            });
        }
        Some(ast::VarDeclOrExpr::Expr(expr)) => {
            let e = convert_expr(expr, reg, None, type_env)?;
            result.push(Stmt::Expr(e));
        }
        None => {}
    }

    // 2. Build loop body
    let mut loop_body = Vec::new();

    // 2a. Condition → if !(condition) { break; }
    if let Some(test) = &for_stmt.test {
        let condition = convert_expr(test, reg, None, type_env)?;
        loop_body.push(Stmt::If {
            condition: Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(condition),
            },
            then_body: vec![Stmt::Break {
                label: None,
                value: None,
            }],
            else_body: None,
        });
    }

    // 2b. Original body
    let body_stmts = convert_block_or_stmt(&for_stmt.body, reg, return_type, type_env)?;
    loop_body.extend(body_stmts);

    // 2c. Update expression
    if let Some(update) = &for_stmt.update {
        let update_stmt = convert_update_to_stmt(update, reg, type_env)?;
        loop_body.push(update_stmt);
    }

    result.push(Stmt::Loop {
        label: None,
        body: loop_body,
    });

    Ok(result)
}

/// Converts a for-loop update expression to an IR statement.
///
/// - `i++` → `i = i + 1.0`
/// - `i--` → `i = i - 1.0`
/// - `i += n` → `i = i + n`
/// - Other expressions → `Stmt::Expr`
fn convert_update_to_stmt(
    expr: &ast::Expr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Stmt> {
    match expr {
        ast::Expr::Update(up) => {
            let name = match up.arg.as_ref() {
                ast::Expr::Ident(ident) => ident.sym.to_string(),
                _ => return Err(anyhow!("unsupported update expression")),
            };
            let op = match up.op {
                ast::UpdateOp::PlusPlus => BinOp::Add,
                ast::UpdateOp::MinusMinus => BinOp::Sub,
            };
            Ok(Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident(name.clone())),
                value: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident(name)),
                    op,
                    right: Box::new(Expr::NumberLit(1.0)),
                }),
            }))
        }
        ast::Expr::Assign(assign) => {
            let e = convert_expr(&ast::Expr::Assign(assign.clone()), reg, None, type_env)?;
            Ok(Stmt::Expr(e))
        }
        other => {
            let e = convert_expr(other, reg, None, type_env)?;
            Ok(Stmt::Expr(e))
        }
    }
}

/// Converts a block statement or single statement into a `Vec<Stmt>`.
fn convert_block_or_stmt(
    stmt: &ast::Stmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
    type_env: &mut TypeEnv,
) -> Result<Vec<Stmt>> {
    match stmt {
        ast::Stmt::Block(block) => convert_stmt_list(&block.stmts, reg, return_type, type_env),
        other => convert_stmt(other, reg, return_type, type_env),
    }
}

/// Converts a nested function declaration into a closure-bound `let` statement.
///
/// `function inner(x: number): number { return x; }`
/// becomes `let inner = |x: f64| -> f64 { x };`
fn convert_nested_fn_decl(fn_decl: &ast::FnDecl, reg: &TypeRegistry) -> Result<Stmt> {
    let name = fn_decl.ident.sym.to_string();

    let mut params = Vec::new();
    for p in &fn_decl.function.params {
        let param_name = extract_pat_ident_name(&p.pat)?;
        let ty = match &p.pat {
            ast::Pat::Ident(ident) => ident
                .type_ann
                .as_ref()
                .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
                .transpose()?,
            _ => None,
        };
        params.push(Param {
            name: param_name,
            ty,
        });
    }

    let return_type = fn_decl
        .function
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
        .transpose()?
        .and_then(|ty| {
            if matches!(ty, RustType::Unit) {
                None
            } else {
                Some(ty)
            }
        });

    let mut fn_type_env = TypeEnv::new();
    for param in &params {
        if let Some(ty) = &param.ty {
            fn_type_env.insert(param.name.clone(), ty.clone());
        }
    }

    let body = match &fn_decl.function.body {
        Some(block) => {
            convert_stmt_list(&block.stmts, reg, return_type.as_ref(), &mut fn_type_env)?
        }
        None => Vec::new(),
    };

    Ok(Stmt::Let {
        name,
        mutable: false,
        ty: None,
        init: Some(Expr::Closure {
            params,
            return_type,
            body: ClosureBody::Block(body),
        }),
    })
}

#[cfg(test)]
mod tests;
