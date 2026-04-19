//! Loop statement conversion.
//!
//! Converts for/for-of/for-in/while/do-while/labeled statements into IR representations.
//! Handles conditional assignments in while loops and do-while body rewriting.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use super::helpers::{
    extract_conditional_assignment, generate_falsy_condition, ConditionalAssignment,
};
use crate::ir::{BinOp, ClosureBody, Expr, Param, Pattern, RustType, Stmt, UnOp};
use crate::pipeline::type_converter::convert_ts_type;
use crate::transformer::{extract_pat_ident_name, single_declarator, Transformer};

impl<'a> Transformer<'a> {
    fn convert_while_with_conditional_assignment(
        &mut self,
        ca: &ConditionalAssignment<'_>,
        body: Vec<Stmt>,
    ) -> Result<Vec<Stmt>> {
        let rhs_type = self.get_expr_type(ca.rhs);
        let rhs_ir = self.convert_expr(ca.rhs)?;

        match rhs_type {
            Some(RustType::Option(_)) => Ok(vec![Stmt::WhileLet {
                label: None,
                pattern: Pattern::some_binding(&ca.var_name),
                expr: rhs_ir,
                body,
            }]),
            Some(ty) => {
                let falsy_condition = generate_falsy_condition(&ca.var_name, ty);
                let mut loop_body = vec![
                    Stmt::Let {
                        mutable: false,
                        name: ca.var_name.clone(),
                        ty: rhs_type.cloned(),
                        init: Some(rhs_ir),
                    },
                    Stmt::If {
                        condition: falsy_condition,
                        then_body: vec![Stmt::Break {
                            label: None,
                            value: None,
                        }],
                        else_body: None,
                    },
                ];
                loop_body.extend(body);
                Ok(vec![Stmt::Loop {
                    label: None,
                    body: loop_body,
                }])
            }
            None => {
                let mut loop_body = vec![
                    Stmt::Let {
                        mutable: false,
                        name: ca.var_name.clone(),
                        ty: None,
                        init: Some(rhs_ir),
                    },
                    Stmt::If {
                        condition: Expr::UnaryOp {
                            op: UnOp::Not,
                            operand: Box::new(Expr::Ident(ca.var_name.clone())),
                        },
                        then_body: vec![Stmt::Break {
                            label: None,
                            value: None,
                        }],
                        else_body: None,
                    },
                ];
                loop_body.extend(body);
                Ok(vec![Stmt::Loop {
                    label: None,
                    body: loop_body,
                }])
            }
        }
    }

    /// Converts a C-style `for` statement to `Stmt::ForIn` if it matches the simple counter pattern.
    pub(super) fn convert_for_stmt(
        &mut self,
        for_stmt: &ast::ForStmt,
        return_type: Option<&RustType>,
    ) -> Result<Stmt> {
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
                let start_expr = self.convert_expr(init)?;
                (name, start_expr)
            }
            _ => {
                return Err(anyhow!(
                    "unsupported for loop: no variable declaration init"
                ))
            }
        };

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
                    self.convert_expr(&bin.right)?
                }
                _ => return Err(anyhow!("unsupported for loop: non-simple condition")),
            },
            None => return Err(anyhow!("unsupported for loop: no test expression")),
        };

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

        let mut body = self.convert_block_or_stmt(&for_stmt.body, return_type)?;

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
    pub(super) fn convert_for_of_stmt(
        &mut self,
        for_of: &ast::ForOfStmt,
        return_type: Option<&RustType>,
    ) -> Result<Stmt> {
        let var = match &for_of.left {
            ast::ForHead::VarDecl(var_decl) => {
                let decl = single_declarator(var_decl)
                    .map_err(|_| anyhow!("for...of with multiple declarators is not supported"))?;
                match &decl.name {
                    ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                    ast::Pat::Array(arr_pat) => {
                        let names: Vec<String> = arr_pat
                            .elems
                            .iter()
                            .map(|elem| match elem {
                                Some(ast::Pat::Ident(ident)) => Ok(ident.id.sym.to_string()),
                                Some(_) => {
                                    Err(anyhow!("unsupported for...of array binding element"))
                                }
                                None => Ok("_".to_string()),
                            })
                            .collect::<Result<_>>()?;
                        format!("({})", names.join(", "))
                    }
                    _ => {
                        return Err(anyhow!("unsupported for...of binding pattern"));
                    }
                }
            }
            _ => return Err(anyhow!("unsupported for...of left-hand side")),
        };
        let iterable = self.convert_expr(&for_of.right)?;
        let body = self.convert_block_or_stmt(&for_of.body, return_type)?;
        Ok(Stmt::ForIn {
            label: None,
            var,
            iterable,
            body,
        })
    }

    /// Converts a `for...in` statement to `for key in obj.keys()`.
    pub(super) fn convert_for_in_stmt(
        &mut self,
        for_in: &ast::ForInStmt,
        return_type: Option<&RustType>,
    ) -> Result<Stmt> {
        let var = match &for_in.left {
            ast::ForHead::VarDecl(var_decl) => {
                let decl = single_declarator(var_decl)
                    .map_err(|_| anyhow!("for...in with multiple declarators is not supported"))?;
                match &decl.name {
                    ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                    _ => return Err(anyhow!("unsupported for...in binding pattern")),
                }
            }
            _ => return Err(anyhow!("unsupported for...in left-hand side")),
        };
        let obj = self.convert_expr(&for_in.right)?;
        let iterable = Expr::MethodCall {
            object: Box::new(obj),
            method: "keys".to_string(),
            args: vec![],
        };
        let body = self.convert_block_or_stmt(&for_in.body, return_type)?;
        Ok(Stmt::ForIn {
            label: None,
            var,
            iterable,
            body,
        })
    }

    /// Converts a labeled statement by attaching the label to the inner loop.
    ///
    /// I-154 Part E.1: defense-in-depth lint — the `__ts_` namespace is reserved
    /// for ts_to_rs internal emission. Caller (`convert_stmt::Labeled` arm) also
    /// runs this check, but keeping it here ensures future callers / refactors
    /// remain safe.
    pub(super) fn convert_labeled_stmt(
        &mut self,
        labeled: &ast::LabeledStmt,
        return_type: Option<&RustType>,
    ) -> Result<Stmt> {
        crate::transformer::statements::check_ts_internal_label_namespace(&labeled.label)?;
        let label_name = labeled.label.sym.to_string();
        match labeled.body.as_ref() {
            ast::Stmt::While(while_stmt) => {
                let condition = self.convert_expr(&while_stmt.test)?;
                let body = self.convert_block_or_stmt(&while_stmt.body, return_type)?;
                Ok(Stmt::While {
                    label: Some(label_name),
                    condition,
                    body,
                })
            }
            ast::Stmt::ForOf(for_of) => {
                let mut stmt = self.convert_for_of_stmt(for_of, return_type)?;
                if let Stmt::ForIn { ref mut label, .. } = stmt {
                    *label = Some(label_name);
                }
                Ok(stmt)
            }
            ast::Stmt::For(for_stmt) => {
                let mut stmt = self.convert_for_stmt(for_stmt, return_type)?;
                if let Stmt::ForIn { ref mut label, .. } = stmt {
                    *label = Some(label_name);
                }
                Ok(stmt)
            }
            ast::Stmt::DoWhile(do_while) => {
                let mut stmt =
                    self.convert_do_while_stmt(do_while, return_type, Some(&label_name))?;
                if let Stmt::Loop { ref mut label, .. } = stmt {
                    *label = Some(label_name);
                }
                Ok(stmt)
            }
            ast::Stmt::ForIn(for_in) => {
                let mut stmt = self.convert_for_in_stmt(for_in, return_type)?;
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
    pub(super) fn convert_while_stmt(
        &mut self,
        while_stmt: &ast::WhileStmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let body = self.convert_block_or_stmt(&while_stmt.body, return_type)?;

        if let Some(ca) = extract_conditional_assignment(&while_stmt.test) {
            return self.convert_while_with_conditional_assignment(&ca, body);
        }

        let condition = self.convert_expr(&while_stmt.test)?;
        Ok(vec![Stmt::While {
            label: None,
            condition,
            body,
        }])
    }

    /// Converts a `do...while` statement to `loop` with break condition at the end.
    ///
    /// When the body contains `continue` targeting this do-while, wraps the body in
    /// a labeled block (`'do_while: { ... }`) so that `continue` is rewritten to
    /// `break 'do_while`, correctly falling through to the condition check.
    /// Unlabeled `break` inside the block is also rewritten to target the outer loop.
    pub(super) fn convert_do_while_stmt(
        &mut self,
        do_while: &ast::DoWhileStmt,
        return_type: Option<&RustType>,
        loop_label: Option<&str>,
    ) -> Result<Stmt> {
        let mut body_stmts = match do_while.body.as_ref() {
            ast::Stmt::Block(block) => self.convert_stmt_list(&block.stmts, return_type)?,
            single => self.convert_stmt(single, return_type)?,
        };

        // Cat A: boolean context (do-while condition)
        let condition = self.convert_expr(&do_while.test)?;

        let needs_labeled_block = has_continue_targeting_do_while(&body_stmts, loop_label, 0);

        if needs_labeled_block {
            // The outer loop needs a label so that `break` inside the LabeledBlock
            // can target it (Rust E0695: unlabeled break inside labeled block).
            // I-154: use `__ts_` prefix namespace for internal labels (hygiene).
            // User labels starting with `__ts_` are rejected at
            // `check_ts_internal_label_namespace`, so collision with user code is
            // structurally impossible.
            let effective_loop_label = loop_label.unwrap_or("__ts_do_while_loop");

            rewrite_do_while_body(
                &mut body_stmts,
                "__ts_do_while",
                loop_label,
                effective_loop_label,
                0,
            );

            let break_check = Stmt::If {
                condition: Expr::UnaryOp {
                    op: UnOp::Not,
                    operand: Box::new(condition),
                },
                then_body: vec![Stmt::Break {
                    label: Some(effective_loop_label.to_string()),
                    value: None,
                }],
                else_body: None,
            };

            Ok(Stmt::Loop {
                label: Some(effective_loop_label.to_string()),
                body: vec![
                    Stmt::LabeledBlock {
                        label: "__ts_do_while".to_string(),
                        body: body_stmts,
                    },
                    break_check,
                ],
            })
        } else {
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
            body_stmts.push(break_check);
            Ok(Stmt::Loop {
                label: None,
                body: body_stmts,
            })
        }
    }

    /// Converts a C-style `for` statement to a `loop` when it doesn't match the simple counter pattern.
    pub(super) fn convert_for_stmt_as_loop(
        &mut self,
        for_stmt: &ast::ForStmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let mut result = Vec::new();

        // 1. Extract init → Stmt::Let { mutable: true, ... }
        match &for_stmt.init {
            Some(ast::VarDeclOrExpr::VarDecl(var_decl)) => {
                // Support multiple declarators: for (let i = 0, len = n; ...)
                for decl in &var_decl.decls {
                    let name = extract_pat_ident_name(&decl.name)
                        .map_err(|_| anyhow!("unsupported for loop: non-ident binding"))?;
                    let init_expr = decl
                        .init
                        .as_ref()
                        // Cat A: for-loop initializer
                        .map(|e| self.convert_expr(e))
                        .transpose()?;
                    result.push(Stmt::Let {
                        mutable: true,
                        name,
                        ty: None,
                        init: init_expr,
                    });
                }
            }
            Some(ast::VarDeclOrExpr::Expr(expr)) => {
                // Cat A: for-loop init expression
                let e = self.convert_expr(expr)?;
                result.push(Stmt::Expr(e));
            }
            None => {}
        }

        // 2. Build loop body
        let mut loop_body = Vec::new();

        // 2a. Condition → if !(condition) { break; }
        if let Some(test) = &for_stmt.test {
            // Cat A: boolean context (for-loop condition)
            let condition = self.convert_expr(test)?;
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
        let body_stmts = self.convert_block_or_stmt(&for_stmt.body, return_type)?;
        loop_body.extend(body_stmts);

        // 2c. Update expression
        if let Some(update) = &for_stmt.update {
            let update_stmt = self.convert_update_to_stmt(update)?;
            loop_body.push(update_stmt);
        }

        result.push(Stmt::Loop {
            label: None,
            body: loop_body,
        });

        Ok(result)
    }

    /// Converts an update expression (i++, i--) to an IR statement.
    fn convert_update_to_stmt(&mut self, expr: &ast::Expr) -> Result<Stmt> {
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
                // Cat A: for-loop update expression
                let e = self.convert_expr(&ast::Expr::Assign(assign.clone()))?;
                Ok(Stmt::Expr(e))
            }
            other => {
                // Cat A: for-loop update expression
                let e = self.convert_expr(other)?;
                Ok(Stmt::Expr(e))
            }
        }
    }

    /// Converts a nested function declaration to a closure `let` binding.
    pub(super) fn convert_nested_fn_decl(&mut self, fn_decl: &ast::FnDecl) -> Result<Stmt> {
        let name = fn_decl.ident.sym.to_string();
        let mut params = Vec::new();
        for p in &fn_decl.function.params {
            let param_name = extract_pat_ident_name(&p.pat)?;
            let ty = match &p.pat {
                ast::Pat::Ident(ident) => {
                    if let Some(ann) = ident.type_ann.as_ref() {
                        Some(convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            params.push(Param {
                name: param_name,
                ty,
            });
        }

        let return_type = if let Some(ann) = fn_decl.function.return_type.as_ref() {
            Some(convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?)
        } else {
            None
        }
        .and_then(|ty| {
            if matches!(ty, RustType::Unit) {
                None
            } else {
                Some(ty)
            }
        });

        // Sub-Transformer for nested function body.
        // TypeResolver handles parameter types via scope_stack.
        let body = match &fn_decl.function.body {
            Some(block) => self
                .spawn_nested_scope()
                .convert_stmt_list(&block.stmts, return_type.as_ref())?,
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
}

fn has_continue_targeting_do_while(stmts: &[Stmt], loop_label: Option<&str>, depth: usize) -> bool {
    for stmt in stmts {
        let found = match stmt {
            Stmt::Continue { label: None } if depth == 0 => true,
            Stmt::Continue { label: Some(l) } if loop_label.is_some_and(|ll| l == ll) => true,
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                has_continue_targeting_do_while(then_body, loop_label, depth)
                    || else_body
                        .as_ref()
                        .is_some_and(|eb| has_continue_targeting_do_while(eb, loop_label, depth))
            }
            Stmt::LabeledBlock { body, .. } => {
                has_continue_targeting_do_while(body, loop_label, depth)
            }
            Stmt::IfLet {
                then_body,
                else_body,
                ..
            } => {
                has_continue_targeting_do_while(then_body, loop_label, depth)
                    || else_body
                        .as_ref()
                        .is_some_and(|eb| has_continue_targeting_do_while(eb, loop_label, depth))
            }
            Stmt::Match { arms, .. } => arms
                .iter()
                .any(|arm| has_continue_targeting_do_while(&arm.body, loop_label, depth)),
            Stmt::Loop { body, .. }
            | Stmt::While { body, .. }
            | Stmt::ForIn { body, .. }
            | Stmt::WhileLet { body, .. } => {
                has_continue_targeting_do_while(body, loop_label, depth + 1)
            }
            _ => false,
        };
        if found {
            return true;
        }
    }
    false
}

/// Rewrites `continue` and `break` in a do-while body for correct semantics.
///
/// `continue` targeting this do-while → `break 'block_label` (falls through to condition).
/// Unlabeled `break` at depth 0 → `break 'loop_label` (avoids Rust E0695 inside labeled block).
fn rewrite_do_while_body(
    stmts: &mut [Stmt],
    block_label: &str,
    loop_label: Option<&str>,
    effective_loop_label: &str,
    depth: usize,
) {
    for stmt in stmts.iter_mut() {
        match stmt {
            // continue targeting this do-while → break 'block_label
            Stmt::Continue { label: None } if depth == 0 => {
                *stmt = Stmt::Break {
                    label: Some(block_label.to_string()),
                    value: None,
                };
            }
            Stmt::Continue { label: Some(l) } if loop_label.is_some_and(|ll| l == ll) => {
                *stmt = Stmt::Break {
                    label: Some(block_label.to_string()),
                    value: None,
                };
            }
            // unlabeled break at depth 0 → break 'loop_label (E0695)
            Stmt::Break { label: None, value } if depth == 0 => {
                *stmt = Stmt::Break {
                    label: Some(effective_loop_label.to_string()),
                    value: value.take(),
                };
            }
            // Recurse into control flow blocks (same loop depth)
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                rewrite_do_while_body(
                    then_body,
                    block_label,
                    loop_label,
                    effective_loop_label,
                    depth,
                );
                if let Some(else_body) = else_body {
                    rewrite_do_while_body(
                        else_body,
                        block_label,
                        loop_label,
                        effective_loop_label,
                        depth,
                    );
                }
            }
            Stmt::LabeledBlock { body, .. } => {
                rewrite_do_while_body(body, block_label, loop_label, effective_loop_label, depth);
            }
            Stmt::IfLet {
                then_body,
                else_body,
                ..
            } => {
                rewrite_do_while_body(
                    then_body,
                    block_label,
                    loop_label,
                    effective_loop_label,
                    depth,
                );
                if let Some(else_body) = else_body {
                    rewrite_do_while_body(
                        else_body,
                        block_label,
                        loop_label,
                        effective_loop_label,
                        depth,
                    );
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    rewrite_do_while_body(
                        &mut arm.body,
                        block_label,
                        loop_label,
                        effective_loop_label,
                        depth,
                    );
                }
            }
            // Nested loops increase depth
            Stmt::Loop { body, .. }
            | Stmt::While { body, .. }
            | Stmt::ForIn { body, .. }
            | Stmt::WhileLet { body, .. } => {
                rewrite_do_while_body(
                    body,
                    block_label,
                    loop_label,
                    effective_loop_label,
                    depth + 1,
                );
            }
            _ => {}
        }
    }
}
