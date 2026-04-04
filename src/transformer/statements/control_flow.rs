//! Control flow statement conversion.
//!
//! Converts if/while/for/do-while/labeled statements into IR representations.
//! Handles conditional assignments, narrowing guards, and `if let` generation.

use anyhow::Result;
use swc_ecma_ast as ast;

use super::helpers::{
    extract_conditional_assignment, generate_truthiness_condition, ConditionalAssignment,
};
use crate::ir::{BinOp, Expr, MatchArm, MatchPattern, RustType, Stmt};
use crate::transformer::expressions::patterns::extract_narrowing_guards;
use crate::transformer::Transformer;

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

        // Single-guard complement match: for single narrowing guards with a resolvable
        // complement, generate `match` instead of `if let` so both arms bind the variable
        // to the correct narrowed type. This must be checked BEFORE the compound path
        // because the compound path also handles single guards.
        if compound.guards.len() == 1 && compound.remaining.is_empty() {
            let guard = &compound.guards[0].0;
            if self.can_generate_if_let(guard) {
                let then_body = self.convert_block_or_stmt(&if_stmt.cons, return_type)?;
                let else_body = if_stmt
                    .alt
                    .as_ref()
                    .map(|alt| self.convert_block_or_stmt(alt, return_type))
                    .transpose()?;

                if let Some(stmts) =
                    self.try_generate_narrowing_match(guard, &then_body, &else_body)?
                {
                    return Ok(stmts);
                }
                return Ok(vec![self.generate_if_let(guard, then_body, else_body)]);
            }
        }

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

        let then_body = self.convert_block_or_stmt(&if_stmt.cons, return_type)?;

        let else_body = if let Some(alt) = &if_stmt.alt {
            Some(self.convert_block_or_stmt(alt, return_type)?)
        } else {
            None
        };

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

    /// Converts AST expressions and combines them with `&&`.
    pub(crate) fn convert_and_combine_conditions(
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

    /// Attempts to generate a `match` statement with complement narrowing.
    ///
    /// When a narrowing guard has a resolvable complement pattern, generates
    /// `match var { PositivePattern(var) => then, ComplementPattern(var) => else }`
    /// instead of `if let`. This ensures both arms bind the variable to the
    /// correct narrowed type.
    ///
    /// For early return patterns (no else, then always exits), generates
    /// `let var = match var { Positive(v) => { exit_body }, Complement(v) => v };`
    /// so subsequent code uses the narrowed variable.
    fn try_generate_narrowing_match(
        &self,
        guard: &crate::transformer::expressions::patterns::NarrowingGuard,
        then_body: &[Stmt],
        else_body: &Option<Vec<Stmt>>,
    ) -> Result<Option<Vec<Stmt>>> {
        let complement_pattern = match self.resolve_complement_pattern(guard) {
            Some(p) => p,
            None => return Ok(None),
        };
        let Some((positive_pattern, is_swap)) = self.resolve_if_let_pattern(guard) else {
            return Ok(None);
        };
        let var_name = guard.var_name().to_string();
        let expr = Expr::Ident(var_name.clone());

        // Determine which body goes to which arm based on is_swap
        let (positive_body, complement_body) = if is_swap {
            (else_body.clone().unwrap_or_default(), then_body.to_vec())
        } else {
            (then_body.to_vec(), else_body.clone().unwrap_or_default())
        };

        let is_early_return =
            else_body.is_none() && !then_body.is_empty() && ir_body_always_exits(then_body);

        if is_early_return && complement_pattern != "None" {
            // Early return: `let var = match var { Pos(v) => { exit }, Comp(v) => v };`
            let positive_arm = MatchArm {
                patterns: vec![MatchPattern::Verbatim(positive_pattern.clone())],
                guard: None,
                body: positive_body,
            };
            let complement_arm = MatchArm {
                patterns: vec![MatchPattern::Verbatim(complement_pattern.clone())],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::Ident(var_name.clone()))],
            };
            return Ok(Some(vec![Stmt::Let {
                mutable: false,
                name: var_name,
                ty: None,
                init: Some(Expr::Match {
                    expr: Box::new(expr),
                    arms: vec![positive_arm, complement_arm],
                }),
            }]));
        }

        if complement_pattern == "None" && is_early_return && is_swap {
            // Option early return with === null: `let var = match var { None => { exit }, Some(v) => v };`
            // Only when is_swap (=== null): the null handler exits, and Some(v) extracts the value.
            // Truthy `if (x) { return; }` has is_swap=false: the Some arm exits, None has no value.
            let none_arm = MatchArm {
                patterns: vec![MatchPattern::Verbatim("None".to_string())],
                guard: None,
                body: complement_body,
            };
            let some_arm = MatchArm {
                patterns: vec![MatchPattern::Verbatim(format!("Some({})", var_name))],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::Ident(var_name.clone()))],
            };
            return Ok(Some(vec![Stmt::Let {
                mutable: false,
                name: var_name,
                ty: None,
                init: Some(Expr::Match {
                    expr: Box::new(expr),
                    arms: vec![none_arm, some_arm],
                }),
            }]));
        }

        if else_body.is_some() && complement_pattern != "None" {
            // Else block pattern: `match var { Pos(v) => { then }, Comp(v) => { else } }`
            let positive_arm = MatchArm {
                patterns: vec![MatchPattern::Verbatim(positive_pattern.clone())],
                guard: None,
                body: positive_body,
            };
            let complement_arm = MatchArm {
                patterns: vec![MatchPattern::Verbatim(complement_pattern.clone())],
                guard: None,
                body: complement_body,
            };
            return Ok(Some(vec![Stmt::Match {
                expr,
                arms: vec![positive_arm, complement_arm],
            }]));
        }

        Ok(None)
    }

    pub(crate) fn can_generate_if_let(
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
}

/// Returns `true` if an IR statement body always exits its enclosing scope.
///
/// Checks for return, break, continue. Throw is converted to `Stmt::Return(Err(...))`
/// by `convert_throw_stmt`, so it's automatically covered by the `Return` check.
fn ir_body_always_exits(body: &[Stmt]) -> bool {
    body.last().is_some_and(|stmt| match stmt {
        Stmt::Return { .. } => true,
        Stmt::Break { .. } | Stmt::Continue { .. } => true,
        Stmt::If {
            then_body,
            else_body: Some(else_body),
            ..
        } => ir_body_always_exits(then_body) && ir_body_always_exits(else_body),
        Stmt::IfLet {
            then_body,
            else_body: Some(else_body),
            ..
        } => ir_body_always_exits(then_body) && ir_body_always_exits(else_body),
        _ => false,
    })
}
