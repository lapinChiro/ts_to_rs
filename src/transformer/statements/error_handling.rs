//! Try/catch/throw statement conversion.
//!
//! Converts TypeScript `try`/`catch`/`finally` and `throw` statements into IR
//! using `scopeguard` for finally, labeled blocks for try, and `Err()` for throw.

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{CallTarget, Expr, Pattern, RustType, Stmt};
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Expands a `try` statement into primitive IR statements.
    pub(super) fn convert_try_stmt(
        &mut self,
        try_stmt: &ast::TryStmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let mut result = Vec::new();

        if let Some(finalizer) = &try_stmt.finalizer {
            let finally_body = self.convert_stmt_list(&finalizer.stmts, return_type)?;
            result.push(Stmt::Let {
                mutable: false,
                name: "_finally_guard".to_string(),
                ty: None,
                init: Some(Expr::FnCall {
                    // `scopeguard::guard` is a module-qualified free function call,
                    // not a type reference.
                    target: CallTarget::ExternalPath(vec![
                        "scopeguard".to_string(),
                        "guard".to_string(),
                    ]),
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

        let try_body = self.convert_stmt_list(&try_stmt.block.stmts, return_type)?;

        if let Some(handler) = &try_stmt.handler {
            let catch_param = handler
                .param
                .as_ref()
                .and_then(|p| match p {
                    swc_ecma_ast::Pat::Ident(ident) => Some(ident.id.sym.to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| "_e".to_string());
            let catch_body = self.convert_stmt_list(&handler.body.stmts, return_type)?;

            result.push(Stmt::Let {
                mutable: true,
                name: "_try_result".to_string(),
                ty: Some(RustType::Result {
                    ok: Box::new(RustType::Unit),
                    err: Box::new(RustType::String),
                }),
                init: Some(Expr::FnCall {
                    // `Ok(())` — `Result` variant constructor; builtin, no type ref.
                    target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
                    args: vec![Expr::Unit],
                }),
            });

            let mut rewrite = TryBodyRewrite::default();
            let expanded_body = rewrite.rewrite(try_body, 0);

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

            let try_ends_with_return = ends_with_return(&expanded_body);
            let catch_ends_with_return = ends_with_return(&catch_body);

            result.push(Stmt::LabeledBlock {
                label: "try_block".to_string(),
                body: expanded_body,
            });

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

            result.push(Stmt::IfLet {
                pattern: Pattern::TupleStruct {
                    ctor: crate::ir::PatternCtor::Builtin(crate::ir::BuiltinVariant::Err),
                    fields: vec![Pattern::binding(catch_param.as_str())],
                },
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
            result.extend(try_body);
        }

        Ok(result)
    }

    /// Converts a `throw` statement into `return Err(...)`.
    pub(super) fn convert_throw_stmt(&mut self, throw_stmt: &ast::ThrowStmt) -> Result<Stmt> {
        let err_arg = self.extract_error_message(&throw_stmt.arg);
        let err_expr = Expr::MethodCall {
            object: Box::new(err_arg),
            method: "to_string".to_string(),
            args: vec![],
        };
        Ok(Stmt::Return(Some(Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
            args: vec![err_expr],
        })))
    }

    /// Extracts the error message expression from a `throw` argument.
    ///
    /// For `throw new Error(msg)` the message arg is lifted out of the
    /// constructor call. The TS signature of `Error` declares
    /// `message?: string`, which after strict-null-checks extraction resolves
    /// to `Option<String>`. TypeResolver therefore propagates `Option<String>`
    /// as the expected type for the arg, causing `convert_expr` to:
    ///
    /// 1. Wrap string-literal args with `.to_string()` (via `convert_lit` for
    ///    expected `String` inside the Option's inner type)
    /// 2. Wrap the whole result in `Some(...)`
    ///
    /// The extracted expression is then passed to an outer `.to_string()` call
    /// in [`Self::convert_throw_stmt`]. Both the `Some(...)` wrap and the
    /// inner `.to_string()` are meaningless in that context — stripping them
    /// yields a single, clean `expr.to_string()` at the call site rather than
    /// `Some(expr).to_string()` (compile error) or
    /// `expr.to_string().to_string()` (ugly but valid).
    fn extract_error_message(&mut self, expr: &ast::Expr) -> Expr {
        let raw = match expr {
            ast::Expr::New(new_expr) => new_expr
                .args
                .as_ref()
                .and_then(|args| args.first())
                .and_then(|first| self.convert_expr(&first.expr).ok()),
            other => self.convert_expr(other).ok(),
        };
        let Some(raw) = raw else {
            return Expr::StringLit("unknown error".to_string());
        };
        // Strip outer `Some(...)` introduced by the `Option<String>` expected
        // type. `convert_expr` only constructs `Some` via `BuiltinVariant::Some`
        // with exactly one arg, so matching that shape is precise.
        let stripped_some = match raw {
            Expr::FnCall {
                target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                mut args,
            } if args.len() == 1 => args.swap_remove(0),
            other => other,
        };
        // Strip a redundant trailing `.to_string()` call — `convert_throw_stmt`
        // will append its own `.to_string()` unconditionally, so keeping an
        // inner one produces `"x".to_string().to_string()`.
        match stripped_some {
            Expr::MethodCall {
                object,
                method,
                args,
            } if method == "to_string" && args.is_empty() => *object,
            other => other,
        }
    }
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
    matches!(
        expr,
        Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
            ..
        }
    )
}
