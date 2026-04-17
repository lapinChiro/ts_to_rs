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

            let mut rewrite = TryBodyRewrite::default();
            let expanded_body = rewrite.rewrite(try_body, 0);

            let try_ends_with_return = ends_with_return(&expanded_body);
            let has_break_to_try_block =
                rewrite.has_throw || rewrite.needs_break_flag || rewrite.needs_continue_flag;

            // I-023: if the try body always returns and no path breaks to
            // `'try_block`, the labeled block is `!`-typed. In that case the
            // downstream `_try_result` / `if let Err { catch_body }` /
            // `unreachable!()` emission is unreachable Rust code, triggering
            // the `unreachable_code` lint (denied in compile_test). The catch
            // body is likewise unreachable because the only way to reach it is
            // via a break, of which there is none. Emit the rewritten body
            // inline and drop the rest of the machinery. The top-level
            // `_try_result` `Stmt::Let`, which would become an unused `mut`
            // binding, is built below — so we must return *before* pushing it.
            //
            // Note: the `_finally_guard` (if any) was pushed upstream and is
            // unaffected — its drop-order semantics still fire when the
            // function returns from inside `expanded_body`.
            if try_ends_with_return && !has_break_to_try_block {
                result.extend(expanded_body);
                return Ok(result);
            }

            // Result placeholder for the "normal" path — must precede the
            // labeled block so that `_try_result = Err(...)` inside the throw
            // rewrite finds a valid target.
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
///
/// Handles constructs whose arms each terminate the enclosing function:
/// - `Stmt::Return` — direct return.
/// - `Stmt::If` with both branches returning — exhaustive by construction.
/// - `Stmt::Match` with at least one arm and every arm's body returning —
///   exhaustive per Rust's match requirement. Guards don't short-circuit
///   exhaustiveness because every concrete input still hits one of the arms.
/// - `Stmt::IfLet` with both then and else bodies returning — same logic as
///   `Stmt::If`. `Stmt::IfLet` without `else_body` is NOT terminating because
///   a pattern-mismatch falls through to following code.
fn ends_with_return(stmts: &[Stmt]) -> bool {
    match stmts.last() {
        Some(Stmt::Return(_)) => true,
        Some(Stmt::If {
            then_body,
            else_body: Some(else_body),
            ..
        }) => ends_with_return(then_body) && ends_with_return(else_body),
        Some(Stmt::IfLet {
            then_body,
            else_body: Some(else_body),
            ..
        }) => ends_with_return(then_body) && ends_with_return(else_body),
        Some(Stmt::Match { arms, .. }) => {
            !arms.is_empty() && arms.iter().all(|a| ends_with_return(&a.body))
        }
        _ => false,
    }
}

/// Rewrites try body statements: converts throws to assign+break,
/// and converts break/continue (at loop_depth 0) to flag+break.
///
/// All three fields are monotonic (false → true), with symmetric semantics:
/// each records whether at least one corresponding rewrite fired during the
/// walk. Their OR is consumed by `convert_try_stmt` to decide whether any
/// path exits the labeled block via `break 'try_block` (which is what makes
/// the block `()`-typed rather than `!`-typed, and therefore whether the
/// downstream `if let Err { catch_body }` + `unreachable!()` machinery is
/// reachable). The exact number of throws/breaks is never used, so a boolean
/// is the correct type — tracking a count would be boolean blindness in
/// reverse.
#[derive(Default)]
struct TryBodyRewrite {
    needs_break_flag: bool,
    needs_continue_flag: bool,
    has_throw: bool,
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
                    self.has_throw = true;
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
                // `if let` / `while let`: destructuring control flow. `if let`
                // is same-depth (not a loop); `while let` introduces a loop.
                Stmt::IfLet {
                    pattern,
                    expr,
                    then_body,
                    else_body,
                } => {
                    result.push(Stmt::IfLet {
                        pattern,
                        expr,
                        then_body: self.rewrite(then_body, loop_depth),
                        else_body: else_body.map(|e| self.rewrite(e, loop_depth)),
                    });
                }
                Stmt::WhileLet {
                    label,
                    pattern,
                    expr,
                    body,
                } => {
                    result.push(Stmt::WhileLet {
                        label,
                        pattern,
                        expr,
                        body: self.rewrite(body, loop_depth + 1),
                    });
                }
                // `match` arm bodies carry user statements that may throw. SWC
                // `switch` converts to IR `Match`, so a `throw` inside a case
                // arm must be rewritten here or the I-023 short-circuit would
                // see `throw_count == 0` and silently drop the catch body.
                // Arm bodies stay at the same loop depth (match is not a loop).
                Stmt::Match { expr, arms } => {
                    let rewritten_arms = arms
                        .into_iter()
                        .map(|arm| crate::ir::MatchArm {
                            patterns: arm.patterns,
                            guard: arm.guard,
                            body: self.rewrite(arm.body, loop_depth),
                        })
                        .collect();
                    result.push(Stmt::Match {
                        expr,
                        arms: rewritten_arms,
                    });
                }
                // User-labeled blocks (e.g. `foo: { ... break foo; }`): recurse
                // at same depth so nested throws are rewritten. Skip only our
                // own `'try_block` label — that is a nested try/catch which
                // has already rewritten its own throws to target its own
                // `_try_result`; re-rewriting them would wire throws from an
                // inner try to our outer `_try_result`, leaking exceptions
                // past the inner catch.
                Stmt::LabeledBlock { label, body } => {
                    if label == "try_block" {
                        result.push(Stmt::LabeledBlock { label, body });
                    } else {
                        result.push(Stmt::LabeledBlock {
                            label,
                            body: self.rewrite(body, loop_depth),
                        });
                    }
                }
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
