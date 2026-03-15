//! Statement conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC statement nodes into the IR [`Stmt`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType, Stmt, UnOp};
use crate::registry::TypeRegistry;
use crate::transformer::expressions::convert_expr;
use crate::transformer::types::convert_ts_type;
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
) -> Result<Stmt> {
    match stmt {
        ast::Stmt::Return(ret) => {
            let expr = ret
                .arg
                .as_ref()
                .map(|e| convert_expr(e, reg, return_type))
                .transpose()?;
            Ok(Stmt::Return(expr))
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => convert_var_decl(var_decl, reg),
        ast::Stmt::If(if_stmt) => convert_if_stmt(if_stmt, reg, return_type),
        ast::Stmt::Expr(expr_stmt) => {
            let expr = convert_expr(&expr_stmt.expr, reg, None)?;
            Ok(Stmt::Expr(expr))
        }
        ast::Stmt::Throw(throw_stmt) => convert_throw_stmt(throw_stmt, reg),
        ast::Stmt::While(while_stmt) => convert_while_stmt(while_stmt, reg, return_type),
        ast::Stmt::ForOf(for_of) => convert_for_of_stmt(for_of, reg, return_type),
        ast::Stmt::For(for_stmt) => convert_for_stmt(for_stmt, reg, return_type),
        ast::Stmt::Break(break_stmt) => {
            let label = break_stmt.label.as_ref().map(|l| l.sym.to_string());
            Ok(Stmt::Break { label })
        }
        ast::Stmt::Continue(cont_stmt) => {
            let label = cont_stmt.label.as_ref().map(|l| l.sym.to_string());
            Ok(Stmt::Continue { label })
        }
        ast::Stmt::Labeled(labeled_stmt) => convert_labeled_stmt(labeled_stmt, reg, return_type),
        ast::Stmt::DoWhile(do_while) => convert_do_while_stmt(do_while, reg, return_type),
        ast::Stmt::Try(try_stmt) => convert_try_stmt(try_stmt, reg, return_type),
        ast::Stmt::Decl(ast::Decl::Fn(fn_decl)) => convert_nested_fn_decl(fn_decl, reg),
        _ => Err(anyhow!("unsupported statement: {:?}", stmt)),
    }
}

/// Converts a variable declaration to an IR `Stmt::Let`.
///
/// - `const` → immutable (`let`)
/// - `let` / `var` → mutable (`let mut`)
fn convert_var_decl(var_decl: &ast::VarDecl, reg: &TypeRegistry) -> Result<Stmt> {
    // We only handle single-declarator variable declarations
    let declarator = single_declarator(var_decl)?;

    let name = extract_pat_ident_name(&declarator.name)?;

    let mutable = !matches!(var_decl.kind, ast::VarDeclKind::Const);

    let ty = match &declarator.name {
        ast::Pat::Ident(ident) => ident
            .type_ann
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann))
            .transpose()?,
        _ => None,
    };

    let init = declarator
        .init
        .as_ref()
        .map(|e| convert_expr(e, reg, ty.as_ref()))
        .transpose()?;

    Ok(Stmt::Let {
        mutable,
        name,
        ty,
        init,
    })
}

/// Converts an if statement to an IR `Stmt::If`.
fn convert_if_stmt(
    if_stmt: &ast::IfStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Result<Stmt> {
    let condition = convert_expr(&if_stmt.test, reg, None)?;

    let then_body = convert_block_or_stmt(&if_stmt.cons, reg, return_type)?;

    let else_body = if_stmt
        .alt
        .as_ref()
        .map(|alt| convert_block_or_stmt(alt, reg, return_type))
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
            let start_expr = convert_expr(init, reg, None)?;
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
                convert_expr(&bin.right, reg, None)?
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

    let body = convert_block_or_stmt(&for_stmt.body, reg, return_type)?;
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
    let iterable = convert_expr(&for_of.right, reg, None)?;
    let body = convert_block_or_stmt(&for_of.body, reg, return_type)?;
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
) -> Result<Stmt> {
    let label_name = labeled.label.sym.to_string();
    match labeled.body.as_ref() {
        ast::Stmt::While(while_stmt) => {
            let condition = convert_expr(&while_stmt.test, reg, None)?;
            let body = convert_block_or_stmt(&while_stmt.body, reg, return_type)?;
            Ok(Stmt::While {
                label: Some(label_name),
                condition,
                body,
            })
        }
        ast::Stmt::ForOf(for_of) => {
            let mut stmt = convert_for_of_stmt(for_of, reg, return_type)?;
            if let Stmt::ForIn { ref mut label, .. } = stmt {
                *label = Some(label_name);
            }
            Ok(stmt)
        }
        ast::Stmt::For(for_stmt) => {
            let mut stmt = convert_for_stmt(for_stmt, reg, return_type)?;
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
) -> Result<Stmt> {
    let condition = convert_expr(&while_stmt.test, reg, None)?;
    let body = convert_block_or_stmt(&while_stmt.body, reg, return_type)?;
    Ok(Stmt::While {
        label: None,
        condition,
        body,
    })
}

/// Converts a `try` statement into `Stmt::TryCatch`.
fn convert_try_stmt(
    try_stmt: &ast::TryStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Result<Stmt> {
    let try_body: Vec<Stmt> = try_stmt
        .block
        .stmts
        .iter()
        .map(|s| convert_stmt(s, reg, return_type))
        .collect::<Result<Vec<_>>>()?;

    let (catch_param, catch_body) = if let Some(handler) = &try_stmt.handler {
        let param_name = handler.param.as_ref().and_then(|p| match p {
            swc_ecma_ast::Pat::Ident(ident) => Some(ident.id.sym.to_string()),
            _ => None,
        });
        let body: Vec<Stmt> = handler
            .body
            .stmts
            .iter()
            .map(|s| convert_stmt(s, reg, return_type))
            .collect::<Result<Vec<_>>>()?;
        (param_name, Some(body))
    } else {
        (None, None)
    };

    let finally_body = if let Some(finalizer) = &try_stmt.finalizer {
        let body: Vec<Stmt> = finalizer
            .stmts
            .iter()
            .map(|s| convert_stmt(s, reg, return_type))
            .collect::<Result<Vec<_>>>()?;
        Some(body)
    } else {
        None
    };

    Ok(Stmt::TryCatch {
        try_body,
        catch_param,
        catch_body,
        finally_body,
    })
}

/// Converts a `throw` statement into `return Err(...)`.
///
/// - `throw new Error("msg")` → `return Err("msg".to_string())`
/// - `throw "msg"` → `return Err("msg".to_string())`
/// - Other throw expressions → `return Err(expr.to_string())`
fn convert_throw_stmt(throw_stmt: &ast::ThrowStmt, reg: &TypeRegistry) -> Result<Stmt> {
    let err_arg = extract_error_message(&throw_stmt.arg, reg);
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
fn extract_error_message(expr: &ast::Expr, reg: &TypeRegistry) -> Expr {
    match expr {
        ast::Expr::New(new_expr) => {
            // `throw new Error("msg")` → extract "msg"
            if let Some(args) = &new_expr.args {
                if let Some(first) = args.first() {
                    if let Ok(e) = convert_expr(&first.expr, reg, None) {
                        return e;
                    }
                }
            }
            Expr::StringLit("unknown error".to_string())
        }
        other => convert_expr(other, reg, None)
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
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();
    for stmt in stmts {
        match stmt {
            ast::Stmt::Try(try_stmt) => {
                result.push(convert_try_stmt(try_stmt, reg, return_type)?);
            }
            ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
                if let Some(expanded) = try_convert_object_destructuring(var_decl, reg)? {
                    result.extend(expanded);
                } else if let Some(expanded) = try_convert_array_destructuring(var_decl, reg)? {
                    result.extend(expanded);
                } else {
                    result.push(convert_stmt(stmt, reg, return_type)?);
                }
            }
            ast::Stmt::For(for_stmt) => {
                // Try simple counter pattern first; fall back to loop
                match convert_for_stmt(for_stmt, reg, return_type) {
                    Ok(s) => result.push(s),
                    Err(_) => {
                        result.extend(convert_for_stmt_as_loop(for_stmt, reg, return_type)?);
                    }
                }
            }
            other => {
                result.push(convert_stmt(other, reg, return_type)?);
            }
        }
    }
    Ok(result)
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
    let source_expr = convert_expr(source, reg, None)?;

    let mutable = !matches!(var_decl.kind, ast::VarDeclKind::Const);
    let mut stmts = Vec::new();

    for prop in &obj_pat.props {
        match prop {
            ast::ObjectPatProp::Assign(assign) => {
                // { x } — shorthand, key and binding name are the same
                let field_name = assign.key.sym.to_string();
                stmts.push(Stmt::Let {
                    mutable,
                    name: field_name.clone(),
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
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
                stmts.push(Stmt::Let {
                    mutable,
                    name: binding_name,
                    ty: None,
                    init: Some(Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
                        field: field_name,
                    }),
                });
            }
            ast::ObjectPatProp::Rest(_) => {
                return Err(anyhow!("rest pattern in destructuring is not supported"));
            }
        }
    }

    Ok(Some(stmts))
}

/// Converts a `do...while` statement to `loop { body; if !(cond) { break; } }`.
fn convert_do_while_stmt(
    do_while: &ast::DoWhileStmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Result<Stmt> {
    let body_stmts = match do_while.body.as_ref() {
        ast::Stmt::Block(block) => convert_stmt_list(&block.stmts, reg, return_type)?,
        single => vec![convert_stmt(single, reg, return_type)?],
    };

    let condition = convert_expr(&do_while.test, reg, None)?;
    let break_check = Stmt::If {
        condition: Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(condition),
        },
        then_body: vec![Stmt::Break { label: None }],
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
    let source_expr = convert_expr(source, reg, None)?;

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
                .map(|e| convert_expr(e, reg, None))
                .transpose()?;
            result.push(Stmt::Let {
                mutable: true,
                name,
                ty: None,
                init: init_expr,
            });
        }
        Some(ast::VarDeclOrExpr::Expr(expr)) => {
            let e = convert_expr(expr, reg, None)?;
            result.push(Stmt::Expr(e));
        }
        None => {}
    }

    // 2. Build loop body
    let mut loop_body = Vec::new();

    // 2a. Condition → if !(condition) { break; }
    if let Some(test) = &for_stmt.test {
        let condition = convert_expr(test, reg, None)?;
        loop_body.push(Stmt::If {
            condition: Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(condition),
            },
            then_body: vec![Stmt::Break { label: None }],
            else_body: None,
        });
    }

    // 2b. Original body
    let body_stmts = convert_block_or_stmt(&for_stmt.body, reg, return_type)?;
    loop_body.extend(body_stmts);

    // 2c. Update expression
    if let Some(update) = &for_stmt.update {
        let update_stmt = convert_update_to_stmt(update, reg)?;
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
fn convert_update_to_stmt(expr: &ast::Expr, reg: &TypeRegistry) -> Result<Stmt> {
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
            let e = convert_expr(&ast::Expr::Assign(assign.clone()), reg, None)?;
            Ok(Stmt::Expr(e))
        }
        other => {
            let e = convert_expr(other, reg, None)?;
            Ok(Stmt::Expr(e))
        }
    }
}

/// Converts a block statement or single statement into a `Vec<Stmt>`.
fn convert_block_or_stmt(
    stmt: &ast::Stmt,
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Result<Vec<Stmt>> {
    match stmt {
        ast::Stmt::Block(block) => {
            let mut stmts = Vec::new();
            for s in &block.stmts {
                stmts.push(convert_stmt(s, reg, return_type)?);
            }
            Ok(stmts)
        }
        other => {
            let s = convert_stmt(other, reg, return_type)?;
            Ok(vec![s])
        }
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
                .map(|ann| convert_ts_type(&ann.type_ann))
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
        .map(|ann| convert_ts_type(&ann.type_ann))
        .transpose()?
        .and_then(|ty| {
            if matches!(ty, RustType::Unit) {
                None
            } else {
                Some(ty)
            }
        });

    let body = match &fn_decl.function.body {
        Some(block) => convert_stmt_list(&block.stmts, reg, return_type.as_ref())?,
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
