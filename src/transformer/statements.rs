//! Statement conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC statement nodes into the IR [`Stmt`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType, Stmt};
use crate::registry::TypeRegistry;
use crate::transformer::expressions::convert_expr;
use crate::transformer::types::convert_ts_type;

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
        _ => Err(anyhow!("unsupported statement: {:?}", stmt)),
    }
}

/// Converts a variable declaration to an IR `Stmt::Let`.
///
/// - `const` → immutable (`let`)
/// - `let` / `var` → mutable (`let mut`)
fn convert_var_decl(var_decl: &ast::VarDecl, reg: &TypeRegistry) -> Result<Stmt> {
    // We only handle single-declarator variable declarations
    if var_decl.decls.len() != 1 {
        return Err(anyhow!(
            "multiple variable declarators in one statement are not supported"
        ));
    }
    let declarator = &var_decl.decls[0];

    let name = match &declarator.name {
        ast::Pat::Ident(ident) => ident.id.sym.to_string(),
        _ => return Err(anyhow!("unsupported variable binding pattern")),
    };

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
            if var_decl.decls.len() != 1 {
                return Err(anyhow!("unsupported for loop: multiple declarators"));
            }
            let decl = &var_decl.decls[0];
            let name = match &decl.name {
                ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                _ => return Err(anyhow!("unsupported for loop: non-ident binding")),
            };
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
            start: Box::new(start),
            end: Box::new(end),
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
            if var_decl.decls.len() != 1 {
                return Err(anyhow!(
                    "for...of with multiple declarators is not supported"
                ));
            }
            match &var_decl.decls[0].name {
                ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                _ => return Err(anyhow!("unsupported for...of binding pattern")),
            }
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

/// Converts a list of SWC statements into IR statements, expanding `try/catch` blocks inline.
///
/// `try { stmts... } catch (e) { ... }` is expanded to just the try body statements.
/// The catch block is dropped (throw statements in the try body are already converted to `return Err(...)`).
pub fn convert_stmt_list(
    stmts: &[ast::Stmt],
    reg: &TypeRegistry,
    return_type: Option<&RustType>,
) -> Result<Vec<Stmt>> {
    let mut result = Vec::new();
    for stmt in stmts {
        match stmt {
            ast::Stmt::Try(try_stmt) => {
                // Expand try body inline
                for s in &try_stmt.block.stmts {
                    result.push(convert_stmt(s, reg, return_type)?);
                }
                // catch block is dropped — throw is already Err(), and ? propagation
                // requires function call support which is not yet available
            }
            ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
                if let Some(expanded) = try_convert_object_destructuring(var_decl, reg)? {
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
    if var_decl.decls.len() != 1 {
        return Ok(None);
    }
    let declarator = &var_decl.decls[0];

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
                let field_name = match &kv.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported destructuring key")),
                };
                let binding_name = match kv.value.as_ref() {
                    ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                    _ => return Err(anyhow!("unsupported destructuring value pattern")),
                };
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
            if var_decl.decls.len() != 1 {
                return Err(anyhow!("unsupported for loop: multiple declarators"));
            }
            let decl = &var_decl.decls[0];
            let name = match &decl.name {
                ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                _ => return Err(anyhow!("unsupported for loop: non-ident binding")),
            };
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
                op: "!".to_string(),
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
                ast::UpdateOp::PlusPlus => "+",
                ast::UpdateOp::MinusMinus => "-",
            };
            Ok(Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident(name.clone())),
                value: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident(name)),
                    op: op.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, RustType, Stmt};
    use crate::parser::parse_typescript;
    use crate::registry::TypeRegistry;
    use swc_ecma_ast::{Decl, ModuleItem};

    /// Helper: parse TS source containing a function and return its body statements.
    fn parse_fn_body(source: &str) -> Vec<ast::Stmt> {
        let module = parse_typescript(source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(ast::Stmt::Decl(Decl::Fn(fn_decl))) => fn_decl
                .function
                .body
                .as_ref()
                .expect("no function body")
                .stmts
                .clone(),
            _ => panic!("expected function declaration"),
        }
    }

    #[test]
    fn test_convert_stmt_return_expr() {
        let stmts = parse_fn_body("function f() { return 42; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
    }

    #[test]
    fn test_convert_stmt_return_no_value() {
        let stmts = parse_fn_body("function f() { return; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Stmt::Return(None));
    }

    #[test]
    fn test_convert_stmt_const_decl() {
        let stmts = parse_fn_body("function f() { const x = 1; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_let_decl_mutable() {
        let stmts = parse_fn_body("function f() { let x = 1; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: true,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_const_with_type_annotation() {
        let stmts = parse_fn_body("function f() { const x: number = 1; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: Some(RustType::F64),
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_if_no_else() {
        let stmts = parse_fn_body("function f() { if (true) { return 1; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::If {
                condition: Expr::BoolLit(true),
                then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
                else_body: None,
            }
        );
    }

    #[test]
    fn test_convert_stmt_if_else() {
        let stmts = parse_fn_body("function f() { if (true) { return 1; } else { return 2; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::If {
                condition: Expr::BoolLit(true),
                then_body: vec![Stmt::Return(Some(Expr::NumberLit(1.0)))],
                else_body: Some(vec![Stmt::Return(Some(Expr::NumberLit(2.0)))]),
            }
        );
    }

    #[test]
    fn test_convert_stmt_for_counter_zero_to_n() {
        let stmts = parse_fn_body("function f(n: number) { for (let i = 0; i < n; i++) { i; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::ForIn {
                label: None,
                var: "i".to_string(),
                iterable: Expr::Range {
                    start: Box::new(Expr::NumberLit(0.0)),
                    end: Box::new(Expr::Ident("n".to_string())),
                },
                body: vec![Stmt::Expr(Expr::Ident("i".to_string()))],
            }
        );
    }

    #[test]
    fn test_convert_stmt_for_counter_start_to_literal() {
        let stmts = parse_fn_body("function f() { for (let i = 1; i < 10; i++) { i; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::ForIn {
                label: None,
                var: "i".to_string(),
                iterable: Expr::Range {
                    start: Box::new(Expr::NumberLit(1.0)),
                    end: Box::new(Expr::NumberLit(10.0)),
                },
                body: vec![Stmt::Expr(Expr::Ident("i".to_string()))],
            }
        );
    }

    #[test]
    fn test_convert_stmt_for_of() {
        let stmts = parse_fn_body("function f() { for (const item of items) { item; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::ForIn {
                label: None,
                var: "item".to_string(),
                iterable: Expr::Ident("items".to_string()),
                body: vec![Stmt::Expr(Expr::Ident("item".to_string()))],
            }
        );
    }

    #[test]
    fn test_convert_stmt_while() {
        let stmts = parse_fn_body("function f() { while (x > 0) { x = x - 1; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::While {
                label: None,
                condition: Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: ">".to_string(),
                    right: Box::new(Expr::NumberLit(0.0)),
                },
                body: vec![Stmt::Expr(Expr::Assign {
                    target: Box::new(Expr::Ident("x".to_string())),
                    value: Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("x".to_string())),
                        op: "-".to_string(),
                        right: Box::new(Expr::NumberLit(1.0)),
                    }),
                })],
            }
        );
    }

    #[test]
    fn test_convert_stmt_list_try_catch_expands_try_body() {
        let stmts = parse_fn_body(
            "function f() { try { const x = 1; return x; } catch (e) { return 0; } }",
        );
        // try/catch is expanded: try body is inlined, catch is dropped
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
        assert_eq!(result[1], Stmt::Return(Some(Expr::Ident("x".to_string()))));
    }

    #[test]
    fn test_convert_stmt_list_try_catch_empty_catch() {
        let stmts = parse_fn_body("function f() { try { const x = 1; } catch (e) { } }");
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(1.0)),
            }
        );
    }

    #[test]
    fn test_convert_stmt_throw_new_error_string() {
        let stmts = parse_fn_body("function f() { throw new Error(\"something went wrong\"); }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        // throw new Error("msg") → return Err("msg".to_string())
        assert_eq!(
            result,
            Stmt::Return(Some(Expr::FnCall {
                name: "Err".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::StringLit("something went wrong".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }],
            }))
        );
    }

    #[test]
    fn test_convert_stmt_throw_string_literal() {
        let stmts = parse_fn_body("function f() { throw \"error msg\"; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        // throw "msg" → return Err("msg".to_string())
        assert_eq!(
            result,
            Stmt::Return(Some(Expr::FnCall {
                name: "Err".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::StringLit("error msg".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }],
            }))
        );
    }

    // -- Object literal in variable declaration tests --

    #[test]
    fn test_convert_stmt_var_decl_object_literal_with_type_annotation() {
        let stmts = parse_fn_body("function f() { const p: Point = { x: 1, y: 2 }; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: false,
                name: "p".to_string(),
                ty: Some(RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                }),
                init: Some(Expr::StructInit {
                    name: "Point".to_string(),
                    fields: vec![
                        ("x".to_string(), Expr::NumberLit(1.0)),
                        ("y".to_string(), Expr::NumberLit(2.0)),
                    ],
                }),
            }
        );
    }

    #[test]
    fn test_convert_stmt_expression_statement() {
        let stmts = parse_fn_body("function f() { foo; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Stmt::Expr(Expr::Ident("foo".to_string())));
    }

    // -- Expected type propagation tests --

    #[test]
    fn test_convert_stmt_var_decl_string_type_annotation_adds_to_string() {
        let stmts = parse_fn_body(r#"function f() { const s: string = "hello"; }"#);
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: false,
                name: "s".to_string(),
                ty: Some(RustType::String),
                init: Some(Expr::MethodCall {
                    object: Box::new(Expr::StringLit("hello".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }),
            }
        );
    }

    #[test]
    fn test_convert_stmt_var_decl_string_array_type_annotation() {
        let stmts = parse_fn_body(r#"function f() { const a: string[] = ["a", "b"]; }"#);
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Stmt::Let {
                mutable: false,
                name: "a".to_string(),
                ty: Some(RustType::Vec(Box::new(RustType::String))),
                init: Some(Expr::Vec {
                    elements: vec![
                        Expr::MethodCall {
                            object: Box::new(Expr::StringLit("a".to_string())),
                            method: "to_string".to_string(),
                            args: vec![],
                        },
                        Expr::MethodCall {
                            object: Box::new(Expr::StringLit("b".to_string())),
                            method: "to_string".to_string(),
                            args: vec![],
                        },
                    ],
                }),
            }
        );
    }

    #[test]
    fn test_convert_stmt_return_string_with_string_return_type() {
        let stmts = parse_fn_body(r#"function f(): string { return "ok"; }"#);
        let result =
            convert_stmt(&stmts[0], &TypeRegistry::new(), Some(&RustType::String)).unwrap();
        assert_eq!(
            result,
            Stmt::Return(Some(Expr::MethodCall {
                object: Box::new(Expr::StringLit("ok".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            }))
        );
    }

    #[test]
    fn test_convert_stmt_return_number_with_f64_return_type_unchanged() {
        let stmts = parse_fn_body("function f(): number { return 42; }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), Some(&RustType::F64)).unwrap();
        assert_eq!(result, Stmt::Return(Some(Expr::NumberLit(42.0))));
    }

    // -- break / continue tests --

    #[test]
    fn test_convert_stmt_break_no_label() {
        let stmts = parse_fn_body("function f() { while (true) { break; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        match result {
            Stmt::While { body, .. } => {
                assert_eq!(body[0], Stmt::Break { label: None });
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn test_convert_stmt_continue_no_label() {
        let stmts = parse_fn_body("function f() { while (true) { continue; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        match result {
            Stmt::While { body, .. } => {
                assert_eq!(body[0], Stmt::Continue { label: None });
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn test_convert_stmt_break_with_label() {
        let stmts = parse_fn_body("function f() { outer: while (true) { break outer; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        match result {
            Stmt::While { label, body, .. } => {
                assert_eq!(label, Some("outer".to_string()));
                assert_eq!(
                    body[0],
                    Stmt::Break {
                        label: Some("outer".to_string())
                    }
                );
            }
            _ => panic!("expected labeled While"),
        }
    }

    #[test]
    fn test_convert_stmt_continue_with_label() {
        let stmts =
            parse_fn_body("function f() { outer: for (const x of items) { continue outer; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        match result {
            Stmt::ForIn { label, body, .. } => {
                assert_eq!(label, Some("outer".to_string()));
                assert_eq!(
                    body[0],
                    Stmt::Continue {
                        label: Some("outer".to_string())
                    }
                );
            }
            _ => panic!("expected labeled ForIn"),
        }
    }

    // -- General for loop (loop fallback) tests --

    #[test]
    fn test_convert_stmt_list_for_decrement_becomes_loop() {
        let stmts = parse_fn_body(
            "function f(n: number) { for (let i = n; i >= 0; i--) { console.log(i); } }",
        );
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        // Should produce: let mut i = n; loop { if !(i >= 0) { break; } body; i--; }
        assert_eq!(result.len(), 2); // init + loop
        assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
        assert!(matches!(&result[1], Stmt::Loop { .. }));
    }

    #[test]
    fn test_convert_stmt_list_for_step_by_two_becomes_loop() {
        let stmts = parse_fn_body(
            "function f(n: number) { for (let i = 0; i < n; i += 2) { console.log(i); } }",
        );
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "i"));
        assert!(matches!(&result[1], Stmt::Loop { .. }));
    }

    #[test]
    fn test_convert_stmt_for_simple_counter_unchanged() {
        // Existing simple counter pattern should still produce ForIn
        let stmts = parse_fn_body(
            "function f(n: number) { for (let i = 0; i < n; i++) { console.log(i); } }",
        );
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], Stmt::ForIn { .. }));
    }

    // -- Object destructuring tests --

    #[test]
    fn test_convert_stmt_list_object_destructuring_basic() {
        let stmts = parse_fn_body("function f() { const { x, y } = obj; }");
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("obj".to_string())),
                    field: "x".to_string(),
                }),
            }
        );
        assert_eq!(
            result[1],
            Stmt::Let {
                mutable: false,
                name: "y".to_string(),
                ty: None,
                init: Some(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("obj".to_string())),
                    field: "y".to_string(),
                }),
            }
        );
    }

    #[test]
    fn test_convert_stmt_list_object_destructuring_let_mutable() {
        let stmts = parse_fn_body("function f() { let { x, y } = obj; }");
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0], Stmt::Let { mutable: true, name, .. } if name == "x"));
        assert!(matches!(&result[1], Stmt::Let { mutable: true, name, .. } if name == "y"));
    }

    #[test]
    fn test_convert_stmt_list_object_destructuring_rename() {
        let stmts = parse_fn_body("function f() { const { x: newX } = obj; }");
        let result = convert_stmt_list(&stmts, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            Stmt::Let {
                mutable: false,
                name: "newX".to_string(),
                ty: None,
                init: Some(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("obj".to_string())),
                    field: "x".to_string(),
                }),
            }
        );
    }

    #[test]
    fn test_convert_stmt_labeled_for_range() {
        let stmts =
            parse_fn_body("function f() { outer: for (let i = 0; i < 10; i++) { break outer; } }");
        let result = convert_stmt(&stmts[0], &TypeRegistry::new(), None).unwrap();
        match result {
            Stmt::ForIn { label, .. } => {
                assert_eq!(label, Some("outer".to_string()));
            }
            _ => panic!("expected labeled ForIn"),
        }
    }
}
