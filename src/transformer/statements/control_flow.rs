//! Control flow statement conversion.
//!
//! Converts if/while/for/do-while/labeled statements into IR representations.
//! Handles conditional assignments, narrowing guards, and `if let` generation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use super::helpers::{
    extract_conditional_assignment, generate_falsy_condition, generate_truthiness_condition,
    ConditionalAssignment,
};
use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType, Stmt, UnOp};
use crate::pipeline::type_converter::convert_ts_type;
use crate::transformer::expressions::patterns::extract_narrowing_guards;
use crate::transformer::{extract_pat_ident_name, single_declarator, Transformer};

impl<'a> Transformer<'a> {
    /// Converts an if statement to an IR `Stmt::If`.
    pub(super) fn convert_if_stmt(
        &mut self,
        if_stmt: &ast::IfStmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        if let Some(ca) = extract_conditional_assignment(&if_stmt.test) {
            let then_body = self.convert_block_or_stmt(&if_stmt.cons, return_type)?;
            let else_body = if_stmt
                .alt
                .as_ref()
                .map(|alt| self.convert_block_or_stmt(alt, return_type))
                .transpose()?;
            return self.convert_if_with_conditional_assignment(&ca, then_body, else_body);
        }

        let compound = extract_narrowing_guards(&if_stmt.test);

        let (if_let_guards, non_if_let_ast): (Vec<_>, Vec<_>) = {
            let mut if_let = Vec::new();
            let mut non_if_let = Vec::new();
            for (guard, ast_expr) in &compound.guards {
                if self.can_generate_if_let(guard) {
                    if_let.push(guard);
                } else {
                    non_if_let.push(*ast_expr);
                }
            }
            (if_let, non_if_let)
        };

        if !if_let_guards.is_empty() {
            let then_body = self.convert_block_or_stmt(&if_stmt.cons, return_type)?;

            let all_remaining: Vec<&ast::Expr> = non_if_let_ast
                .iter()
                .copied()
                .chain(compound.remaining.iter().copied())
                .collect();
            let remaining_condition = self.convert_and_combine_conditions(&all_remaining)?;

            let else_body = if_stmt
                .alt
                .as_ref()
                .map(|alt| self.convert_block_or_stmt(alt, return_type))
                .transpose()?;

            let inner_body = if let Some(cond) = remaining_condition {
                vec![Stmt::If {
                    condition: cond,
                    then_body,
                    else_body: else_body.clone(),
                }]
            } else {
                then_body
            };

            let stmt = self.build_nested_if_let(&if_let_guards, inner_body, else_body);
            return Ok(vec![stmt]);
        }

        let guard = if compound.guards.len() == 1 && compound.remaining.is_empty() {
            Some(&compound.guards[0].0)
        } else {
            None
        };

        let then_body = self.convert_block_or_stmt(&if_stmt.cons, return_type)?;

        let else_body = if let Some(alt) = &if_stmt.alt {
            Some(self.convert_block_or_stmt(alt, return_type)?)
        } else {
            None
        };

        if let Some(guard) = guard {
            if self.can_generate_if_let(guard) {
                return Ok(vec![self.generate_if_let(guard, then_body, else_body)]);
            }
        }

        let condition = self.convert_expr(&if_stmt.test)?;
        Ok(vec![Stmt::If {
            condition,
            then_body,
            else_body,
        }])
    }

    /// Converts an `if` statement with a conditional assignment.
    fn convert_if_with_conditional_assignment(
        &mut self,
        ca: &ConditionalAssignment<'_>,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    ) -> Result<Vec<Stmt>> {
        let rhs_type = self.get_expr_type(ca.rhs);
        let rhs_ir = self.convert_expr(ca.rhs)?;

        if let Some(outer) = &ca.outer_comparison {
            let other = self.convert_expr(outer.other_operand)?;
            let ir_op = crate::transformer::expressions::convert_binary_op(outer.op)?;
            let condition = if outer.assign_on_left {
                Expr::BinaryOp {
                    left: Box::new(Expr::Ident(ca.var_name.clone())),
                    op: ir_op,
                    right: Box::new(other),
                }
            } else {
                Expr::BinaryOp {
                    left: Box::new(other),
                    op: ir_op,
                    right: Box::new(Expr::Ident(ca.var_name.clone())),
                }
            };
            let let_stmt = Stmt::Let {
                mutable: false,
                name: ca.var_name.clone(),
                ty: rhs_type.cloned(),
                init: Some(rhs_ir),
            };
            return Ok(vec![
                let_stmt,
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                },
            ]);
        }

        match rhs_type {
            Some(RustType::Option(_)) => Ok(vec![Stmt::IfLet {
                pattern: format!("Some({})", ca.var_name),
                expr: rhs_ir,
                then_body,
                else_body,
            }]),
            Some(ty) => {
                let condition = generate_truthiness_condition(&ca.var_name, ty);
                let let_stmt = Stmt::Let {
                    mutable: false,
                    name: ca.var_name.clone(),
                    ty: rhs_type.cloned(),
                    init: Some(rhs_ir),
                };
                Ok(vec![
                    let_stmt,
                    Stmt::If {
                        condition,
                        then_body,
                        else_body,
                    },
                ])
            }
            None => {
                let let_stmt = Stmt::Let {
                    mutable: false,
                    name: ca.var_name.clone(),
                    ty: None,
                    init: Some(rhs_ir),
                };
                Ok(vec![
                    let_stmt,
                    Stmt::If {
                        condition: Expr::Ident(ca.var_name.clone()),
                        then_body,
                        else_body,
                    },
                ])
            }
        }
    }

    /// Converts a `while` statement with a conditional assignment.
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
                pattern: format!("Some({})", ca.var_name),
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

    /// Converts AST expressions and combines them with `&&`.
    pub(super) fn convert_and_combine_conditions(
        &mut self,
        exprs: &[&ast::Expr],
    ) -> Result<Option<Expr>> {
        if exprs.is_empty() {
            return Ok(None);
        }
        let mut parts: Vec<Expr> = Vec::new();
        for ast_expr in exprs {
            parts.push(self.convert_expr(ast_expr)?);
        }
        let combined = parts
            .into_iter()
            .reduce(|left, right| Expr::BinaryOp {
                left: Box::new(left),
                op: BinOp::LogicalAnd,
                right: Box::new(right),
            })
            .unwrap();
        Ok(Some(combined))
    }

    /// Builds nested `if let` statements from inside out.
    pub(super) fn build_nested_if_let(
        &self,
        guards: &[&crate::transformer::expressions::patterns::NarrowingGuard],
        inner_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    ) -> Stmt {
        let mut current_body = inner_body;
        for guard in guards.iter().rev() {
            let stmt = self.generate_if_let(guard, current_body, else_body.clone());
            current_body = vec![stmt];
        }
        current_body.into_iter().next().unwrap()
    }

    pub(super) fn can_generate_if_let(
        &self,
        guard: &crate::transformer::expressions::patterns::NarrowingGuard,
    ) -> bool {
        self.resolve_if_let_pattern(guard).is_some()
    }

    pub(super) fn generate_if_let(
        &self,
        guard: &crate::transformer::expressions::patterns::NarrowingGuard,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    ) -> Stmt {
        let (pattern, is_swap) = self.resolve_if_let_pattern(guard).unwrap();
        let expr = Expr::Ident(guard.var_name().to_string());
        if is_swap {
            Stmt::IfLet {
                pattern,
                expr,
                then_body: else_body.unwrap_or_default(),
                else_body: Some(then_body),
            }
        } else {
            Stmt::IfLet {
                pattern,
                expr,
                then_body,
                else_body,
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
    pub(super) fn convert_labeled_stmt(
        &mut self,
        labeled: &ast::LabeledStmt,
        return_type: Option<&RustType>,
    ) -> Result<Stmt> {
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
    pub(super) fn convert_do_while_stmt(
        &mut self,
        do_while: &ast::DoWhileStmt,
        return_type: Option<&RustType>,
    ) -> Result<Stmt> {
        let body_stmts = match do_while.body.as_ref() {
            ast::Stmt::Block(block) => self.convert_stmt_list(&block.stmts, return_type)?,
            single => self.convert_stmt(single, return_type)?,
        };

        // Cat A: boolean context (do-while condition)
        let condition = self.convert_expr(&do_while.test)?;
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
            Some(block) => Transformer {
                tctx: self.tctx,
                synthetic: &mut *self.synthetic,
                mut_method_names: self.mut_method_names.clone(),
            }
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
