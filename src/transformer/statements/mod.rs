//! Statement conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC statement nodes into the IR [`Stmt`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, ClosureBody, Expr, MatchArm, MatchPattern, Param, RustType, Stmt, UnOp};
use crate::pipeline::type_converter::convert_ts_type;
use crate::transformer::expressions::patterns::extract_narrowing_guards;
use crate::transformer::Transformer;
use crate::transformer::TypeEnv;
use crate::transformer::{
    extract_pat_ident_name, extract_prop_name, single_declarator, TypePosition,
};

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
impl<'a> Transformer<'a> {
    /// Converts an SWC [`ast::Stmt`] into IR [`Stmt`]s.
    pub(crate) fn convert_stmt(
        &mut self,
        stmt: &ast::Stmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        match stmt {
            ast::Stmt::Return(ret) => {
                if let Some(stmts) = self.try_expand_spread_return(ret)? {
                    return Ok(stmts);
                }
                let expr = ret.arg.as_ref().map(|e| self.convert_expr(e)).transpose()?;
                Ok(vec![Stmt::Return(expr)])
            }
            ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
                if let Some(stmts) = self.try_expand_spread_var_decl(var_decl)? {
                    return Ok(stmts);
                }
                if let Some(expanded) = self.try_convert_object_destructuring(var_decl)? {
                    Ok(expanded)
                } else if let Some(expanded) = self.try_convert_array_destructuring(var_decl)? {
                    Ok(expanded)
                } else {
                    self.convert_var_decl(var_decl)
                }
            }
            ast::Stmt::If(if_stmt) => self.convert_if_stmt(if_stmt, return_type),
            ast::Stmt::Expr(expr_stmt) => {
                if let Some(stmts) = self.try_expand_spread_expr_stmt(expr_stmt)? {
                    return Ok(stmts);
                }
                let expr = self.convert_expr(&expr_stmt.expr)?;
                Ok(vec![Stmt::Expr(expr)])
            }
            ast::Stmt::Throw(throw_stmt) => Ok(vec![self.convert_throw_stmt(throw_stmt)?]),
            ast::Stmt::While(while_stmt) => self.convert_while_stmt(while_stmt, return_type),
            ast::Stmt::ForOf(for_of) => Ok(vec![self.convert_for_of_stmt(for_of, return_type)?]),
            ast::Stmt::For(for_stmt) => match self.convert_for_stmt(for_stmt, return_type) {
                Ok(s) => Ok(vec![s]),
                Err(_) => self.convert_for_stmt_as_loop(for_stmt, return_type),
            },
            ast::Stmt::Break(break_stmt) => {
                let label = break_stmt.label.as_ref().map(|l| l.sym.to_string());
                Ok(vec![Stmt::Break { label, value: None }])
            }
            ast::Stmt::Continue(cont_stmt) => {
                let label = cont_stmt.label.as_ref().map(|l| l.sym.to_string());
                Ok(vec![Stmt::Continue { label }])
            }
            ast::Stmt::Labeled(labeled_stmt) => {
                Ok(vec![self.convert_labeled_stmt(labeled_stmt, return_type)?])
            }
            ast::Stmt::DoWhile(do_while) => {
                Ok(vec![self.convert_do_while_stmt(do_while, return_type)?])
            }
            ast::Stmt::Try(try_stmt) => self.convert_try_stmt(try_stmt, return_type),
            ast::Stmt::Decl(ast::Decl::Fn(fn_decl)) => {
                Ok(vec![self.convert_nested_fn_decl(fn_decl)?])
            }
            ast::Stmt::Switch(switch_stmt) => self.convert_switch_stmt(switch_stmt, return_type),
            ast::Stmt::ForIn(for_in) => Ok(vec![self.convert_for_in_stmt(for_in, return_type)?]),
            ast::Stmt::Decl(ast::Decl::TsInterface(_) | ast::Decl::TsTypeAlias(_)) => Ok(vec![]),
            ast::Stmt::Empty(_) => Ok(vec![]),
            _ => Err(anyhow!("unsupported statement: {:?}", stmt)),
        }
    }
    /// Converts a variable declaration to IR `Stmt::Let` statements.
    fn convert_var_decl(&mut self, var_decl: &ast::VarDecl) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        for declarator in &var_decl.decls {
            let name = extract_pat_ident_name(&declarator.name)?;

            let ty = match &declarator.name {
                ast::Pat::Ident(ident) => {
                    let converted = if let Some(ann) = ident.type_ann.as_ref() {
                        Some(crate::pipeline::type_converter::convert_type_for_position(
                            &ann.type_ann,
                            TypePosition::Value,
                            self.synthetic,
                            self.reg(),
                        )?)
                    } else {
                        None
                    };
                    match converted {
                        Some(RustType::Any) => self.type_env.get(&name).cloned().or(converted),
                        other => other,
                    }
                }
                _ => None,
            };

            let mutable = if matches!(var_decl.kind, ast::VarDeclKind::Const) {
                ty.as_ref().is_some_and(is_object_type)
            } else {
                true
            };

            let init = declarator
                .init
                .as_ref()
                .map(|e| self.convert_expr(e))
                .transpose()?;

            stmts.push(Stmt::Let {
                mutable,
                name,
                ty,
                init,
            });
        }
        Ok(stmts)
    }
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

/// Represents a conditional assignment extracted from a condition expression.
///
/// Covers patterns like `if (x = expr)` and `if ((x = expr) > 0)`.
struct ConditionalAssignment<'a> {
    /// The variable name being assigned to
    var_name: String,
    /// The right-hand side of the assignment
    rhs: &'a ast::Expr,
    /// If the assignment was inside a comparison, the outer comparison details.
    /// `None` for bare assignments like `if (x = expr)`.
    outer_comparison: Option<OuterComparison<'a>>,
}

/// Details of a comparison expression wrapping a conditional assignment.
struct OuterComparison<'a> {
    /// The binary operator (e.g., `>`, `!==`)
    op: ast::BinaryOp,
    /// The other operand of the comparison (not the assignment side)
    other_operand: &'a ast::Expr,
    /// Whether the assignment was on the left side of the comparison
    assign_on_left: bool,
}

/// Extracts a conditional assignment from a condition expression, if present.
///
/// Recognizes:
/// - Bare assignment: `x = expr` (possibly wrapped in parens)
/// - Assignment inside comparison: `(x = expr) > 0`, `(x = expr) !== null`
fn extract_conditional_assignment(expr: &ast::Expr) -> Option<ConditionalAssignment<'_>> {
    // Unwrap parentheses
    let expr = unwrap_parens(expr);

    // Pattern 1: bare assignment `x = expr`
    if let ast::Expr::Assign(assign) = expr {
        if assign.op == ast::AssignOp::Assign {
            if let Some(var_name) = extract_assign_target_name(&assign.left) {
                return Some(ConditionalAssignment {
                    var_name,
                    rhs: &assign.right,
                    outer_comparison: None,
                });
            }
        }
    }

    // Pattern 2: comparison with assignment on one side: `(x = expr) > 0`
    if let ast::Expr::Bin(bin) = expr {
        if is_comparison_op(bin.op) {
            // Check left side for assignment
            if let Some(assign) = extract_assign_from_expr(&bin.left) {
                return Some(ConditionalAssignment {
                    var_name: assign.0,
                    rhs: assign.1,
                    outer_comparison: Some(OuterComparison {
                        op: bin.op,
                        other_operand: &bin.right,
                        assign_on_left: true,
                    }),
                });
            }
            // Check right side for assignment
            if let Some(assign) = extract_assign_from_expr(&bin.right) {
                return Some(ConditionalAssignment {
                    var_name: assign.0,
                    rhs: assign.1,
                    outer_comparison: Some(OuterComparison {
                        op: bin.op,
                        other_operand: &bin.left,
                        assign_on_left: false,
                    }),
                });
            }
        }
    }

    None
}

/// Unwraps nested parentheses from an expression.
fn unwrap_parens(expr: &ast::Expr) -> &ast::Expr {
    match expr {
        ast::Expr::Paren(p) => unwrap_parens(&p.expr),
        _ => expr,
    }
}

/// Extracts the variable name from an assignment target.
fn extract_assign_target_name(target: &ast::AssignTarget) -> Option<String> {
    match target {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
            Some(ident.id.sym.to_string())
        }
        _ => None,
    }
}

/// Extracts an assignment expression from a (possibly parenthesized) expression.
fn extract_assign_from_expr(expr: &ast::Expr) -> Option<(String, &ast::Expr)> {
    let expr = unwrap_parens(expr);
    if let ast::Expr::Assign(assign) = expr {
        if assign.op == ast::AssignOp::Assign {
            if let Some(name) = extract_assign_target_name(&assign.left) {
                return Some((name, &assign.right));
            }
        }
    }
    None
}

/// Returns true if the operator is a comparison (not logical).
fn is_comparison_op(op: ast::BinaryOp) -> bool {
    matches!(
        op,
        ast::BinaryOp::EqEq
            | ast::BinaryOp::NotEq
            | ast::BinaryOp::EqEqEq
            | ast::BinaryOp::NotEqEq
            | ast::BinaryOp::Lt
            | ast::BinaryOp::LtEq
            | ast::BinaryOp::Gt
            | ast::BinaryOp::GtEq
    )
}

/// Generates a truthiness check expression for a given type.
///
/// Returns `None` for Option types (which use `if let` / `while let` instead).
fn generate_truthiness_condition(var_name: &str, ty: &RustType) -> Expr {
    match ty {
        RustType::F64 => Expr::BinaryOp {
            left: Box::new(Expr::Ident(var_name.to_string())),
            op: BinOp::NotEq,
            right: Box::new(Expr::NumberLit(0.0)),
        },
        RustType::String => Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident(var_name.to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }),
        },
        RustType::Bool => Expr::Ident(var_name.to_string()),
        // Fallback for unknown types: use the variable as-is (may need manual fixing)
        _ => Expr::Ident(var_name.to_string()),
    }
}

impl<'a> Transformer<'a> {
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
            self.type_env.insert(
                ca.var_name.clone(),
                rhs_type.cloned().unwrap_or(RustType::Any),
            );
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
                self.type_env.insert(ca.var_name.clone(), ty.clone());
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
                self.type_env.insert(ca.var_name.clone(), ty.clone());
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
}

/// Generates a falsy check condition (the inverse of truthiness) for loop break.
fn generate_falsy_condition(var_name: &str, ty: &RustType) -> Expr {
    match ty {
        RustType::F64 => Expr::BinaryOp {
            left: Box::new(Expr::Ident(var_name.to_string())),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        },
        RustType::String => Expr::MethodCall {
            object: Box::new(Expr::Ident(var_name.to_string())),
            method: "is_empty".to_string(),
            args: vec![],
        },
        RustType::Bool => Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident(var_name.to_string())),
        },
        _ => Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident(var_name.to_string())),
        },
    }
}

impl<'a> Transformer<'a> {
    /// Converts an if statement to an IR `Stmt::If`.
    fn convert_if_stmt(
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
    /// Converts AST expressions and combines them with `&&`.
    fn convert_and_combine_conditions(&mut self, exprs: &[&ast::Expr]) -> Result<Option<Expr>> {
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
}

/// Builds nested `if let` statements from inside out.
impl<'a> Transformer<'a> {
    fn build_nested_if_let(
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

    fn can_generate_if_let(
        &self,
        guard: &crate::transformer::expressions::patterns::NarrowingGuard,
    ) -> bool {
        self.resolve_if_let_pattern(guard).is_some()
    }

    fn generate_if_let(
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

// resolve_typeof_to_enum_variant and resolve_instanceof_to_enum_variant
// are defined in patterns.rs and accessed via NarrowingGuard::if_let_pattern.

impl<'a> Transformer<'a> {
    /// Converts a C-style `for` statement to `Stmt::ForIn` if it matches the simple counter pattern.
    fn convert_for_stmt(
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
    fn convert_for_of_stmt(
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
    fn convert_for_in_stmt(
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
    fn convert_labeled_stmt(
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
    fn convert_while_stmt(
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
    /// Expands a `try` statement into primitive IR statements.
    fn convert_try_stmt(
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
                    name: "Ok".to_string(),
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
            result.extend(try_body);
        }

        Ok(result)
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
    matches!(expr, Expr::FnCall { name, .. } if name == "Err")
}

impl<'a> Transformer<'a> {
    /// Converts a `throw` statement into `return Err(...)`.
    fn convert_throw_stmt(&mut self, throw_stmt: &ast::ThrowStmt) -> Result<Stmt> {
        let err_arg = self.extract_error_message(&throw_stmt.arg);
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
    fn extract_error_message(&mut self, expr: &ast::Expr) -> Expr {
        match expr {
            ast::Expr::New(new_expr) => {
                if let Some(args) = &new_expr.args {
                    if let Some(first) = args.first() {
                        if let Ok(e) = self.convert_expr(&first.expr) {
                            return e;
                        }
                    }
                }
                Expr::StringLit("unknown error".to_string())
            }
            other => self
                .convert_expr(other)
                .unwrap_or_else(|_| Expr::StringLit("unknown error".to_string())),
        }
    }
    /// Converts a list of SWC statements into IR statements.
    pub(crate) fn convert_stmt_list(
        &mut self,
        stmts: &[ast::Stmt],
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let mut result = Vec::new();
        for stmt in stmts {
            let converted = self.convert_stmt(stmt, return_type)?;
            for s in &converted {
                match s {
                    Stmt::Let {
                        name, ty: Some(ty), ..
                    } => {
                        self.type_env.insert(name.clone(), ty.clone());
                    }
                    Stmt::Let {
                        name,
                        ty: None,
                        init: Some(init),
                        ..
                    } => {
                        if let Some(fn_type) = infer_fn_type_from_closure(&Some(init.clone())) {
                            self.type_env.insert(name.clone(), fn_type);
                        } else if let Some(resolved) = extract_var_decl_init(stmt, name)
                            .and_then(|ast_init| self.get_expr_type(ast_init).cloned())
                        {
                            self.type_env.insert(name.clone(), resolved);
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
}

/// Extracts the init expression from a VarDecl AST statement
/// when the declarator has a simple identifier matching `expected_name`.
///
/// Returns `None` for destructuring patterns (array/object) because a single
/// AST VarDecl expands into multiple IR `Stmt::Let`, and the init expression
/// does not correspond to any individual destructured variable.
fn extract_var_decl_init<'a>(stmt: &'a ast::Stmt, expected_name: &str) -> Option<&'a ast::Expr> {
    if let ast::Stmt::Decl(ast::Decl::Var(var_decl)) = stmt {
        let decl = var_decl.decls.first()?;
        // Only match simple identifiers, not destructuring patterns
        if let ast::Pat::Ident(ident) = &decl.name {
            if ident.sym.as_ref() == expected_name {
                return decl.init.as_deref();
            }
        }
    }
    None
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
impl<'a> Transformer<'a> {
    /// Converts spread array elements to IR expressions.
    fn convert_spread_segments(&mut self, array_lit: &ast::ArrayLit) -> Result<Vec<(bool, Expr)>> {
        array_lit
            .elems
            .iter()
            .filter_map(|e| e.as_ref())
            .map(|elem| {
                let expr = self.convert_expr(&elem.expr)?;
                Ok((elem.spread.is_some(), expr))
            })
            .collect()
    }
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
impl<'a> Transformer<'a> {
    /// Detects `let x = [...arr, 1]` and expands to IR statements.
    fn try_expand_spread_var_decl(&mut self, var_decl: &ast::VarDecl) -> Result<Option<Vec<Stmt>>> {
        let (pat, array_lit) = match extract_spread_array_init(var_decl) {
            Some(v) => v,
            None => return Ok(None),
        };
        let name = extract_pat_ident_name(pat)?;
        let ty = match pat {
            ast::Pat::Ident(ident) => {
                if let Some(ann) = ident.type_ann.as_ref() {
                    Some(convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?)
                } else {
                    None
                }
            }
            _ => None,
        };

        let segments = self.convert_spread_segments(array_lit)?;

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

    /// Detects `return [...arr, 1]` and expands to IR statements.
    fn try_expand_spread_return(&mut self, ret: &ast::ReturnStmt) -> Result<Option<Vec<Stmt>>> {
        let arg = match &ret.arg {
            Some(arg) => arg,
            None => return Ok(None),
        };
        let array_lit = match arg.as_ref() {
            ast::Expr::Array(a) if has_spread_elements(a) => a,
            _ => return Ok(None),
        };

        let segments = self.convert_spread_segments(array_lit)?;

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

    /// Detects `[...arr, 1]` as a bare expression statement and expands.
    fn try_expand_spread_expr_stmt(
        &mut self,
        expr_stmt: &ast::ExprStmt,
    ) -> Result<Option<Vec<Stmt>>> {
        let array_lit = match expr_stmt.expr.as_ref() {
            ast::Expr::Array(a) if has_spread_elements(a) => a,
            _ => return Ok(None),
        };

        let segments = self.convert_spread_segments(array_lit)?;

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
    fn try_convert_object_destructuring(
        &mut self,
        var_decl: &ast::VarDecl,
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
        let source_expr = self.convert_expr(source)?;

        let mutable = !matches!(var_decl.kind, ast::VarDeclKind::Const);
        let source_type = self.get_expr_type(source);
        let mut stmts = Vec::new();

        self.expand_object_pat_props(
            &obj_pat.props,
            &source_expr,
            mutable,
            &mut stmts,
            source_type,
        )?;

        Ok(Some(stmts))
    }
    /// Recursively expands object destructuring pattern properties into `let` statements.
    fn expand_object_pat_props(
        &mut self,
        props: &[ast::ObjectPatProp],
        source_expr: &Expr,
        mutable: bool,
        stmts: &mut Vec<Stmt>,
        source_type: Option<&RustType>,
    ) -> Result<()> {
        for prop in props {
            match prop {
                ast::ObjectPatProp::Assign(assign) => {
                    let field_name = assign.key.sym.to_string();
                    let field_access = Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
                        field: field_name.clone(),
                    };
                    let init_expr = if let Some(default_expr) = &assign.value {
                        let default_ir = self.convert_expr(default_expr)?;
                        match &default_ir {
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
                        ast::Pat::Object(inner_pat) => {
                            self.expand_object_pat_props(
                                &inner_pat.props,
                                &nested_source,
                                mutable,
                                stmts,
                                None,
                            )?;
                        }
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
                    let explicit_fields: Vec<String> = props
                        .iter()
                        .filter_map(|p| match p {
                            ast::ObjectPatProp::Assign(a) => Some(a.key.sym.to_string()),
                            ast::ObjectPatProp::KeyValue(kv) => extract_prop_name(&kv.key).ok(),
                            _ => None,
                        })
                        .collect();

                    let type_name = source_type.and_then(|ty| match ty {
                        RustType::Named { name, .. } => Some(name.as_str()),
                        _ => None,
                    });
                    if let Some(crate::registry::TypeDef::Struct { fields, .. }) =
                        type_name.and_then(|n| self.reg().get(n))
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
                }
            }
        }

        Ok(())
    }
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
impl<'a> Transformer<'a> {
    /// Converts a `switch` statement to IR match or if-else chain.
    fn convert_switch_stmt(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        if let Some(result) = self.try_convert_discriminated_union_switch(switch, return_type)? {
            return Ok(result);
        }

        if let Some(result) = self.try_convert_typeof_switch(switch, return_type)? {
            return Ok(result);
        }

        if let Some(result) = self.try_convert_string_enum_switch(switch, return_type)? {
            return Ok(result);
        }

        let mut discriminant = self.convert_expr(&switch.discriminant)?;

        let has_string_cases = switch
            .cases
            .iter()
            .any(|case| matches!(case.test.as_deref(), Some(ast::Expr::Lit(ast::Lit::Str(_)))));
        if has_string_cases {
            discriminant = Expr::MethodCall {
                object: Box::new(discriminant),
                method: "as_str".to_string(),
                args: vec![],
            };
        }

        let case_count = switch.cases.len();
        let has_code_fallthrough = switch.cases.iter().enumerate().any(|(i, case)| {
            let is_last = i == case_count - 1;
            let has_body = !case.cons.is_empty();
            let is_terminated = is_case_terminated(&case.cons);
            has_body && !is_terminated && !is_last
        });

        if has_code_fallthrough {
            self.convert_switch_fallthrough(switch, &discriminant, return_type)
        } else {
            self.convert_switch_clean_match(switch, discriminant, return_type)
        }
    }
}

/// `switch (typeof x)` を enum match に変換する。
///
/// `switch (typeof x) { case "string": ... case "number": ... }` →
/// `match x { Enum::String(x) => { ... }, Enum::F64(x) => { ... } }`
///
/// 各 case body 内では destructured なフィールドが type_env に登録される。
impl<'a> Transformer<'a> {
    /// `switch (typeof x)` を enum match に変換する。
    fn try_convert_typeof_switch(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Option<Vec<Stmt>>> {
        let typeof_var = match switch.discriminant.as_ref() {
            ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::TypeOf => {
                if let ast::Expr::Ident(ident) = unary.arg.as_ref() {
                    Some(ident.sym.to_string())
                } else {
                    None
                }
            }
            _ => None,
        };
        let var_name = match typeof_var {
            Some(name) => name,
            None => return Ok(None),
        };

        let var_type = match self.type_env.get(&var_name) {
            Some(ty) => ty.clone(),
            None => return Ok(None),
        };
        let enum_name = match &var_type {
            RustType::Named { name, type_args } if type_args.is_empty() => name.clone(),
            _ => return Ok(None),
        };
        if !matches!(
            self.reg().get(&enum_name),
            Some(crate::registry::TypeDef::Enum { .. })
        ) {
            return Ok(None);
        }

        // Bail out if any case has code fall-through (non-empty body without terminator
        // followed by another case). typeof switch with fall-through is unusual and the
        // regular switch conversion handles it correctly.
        let case_count = switch.cases.len();
        let has_code_fallthrough = switch.cases.iter().enumerate().any(|(i, case)| {
            let is_last = i == case_count - 1;
            let has_body = !case.cons.is_empty();
            let is_terminated = is_case_terminated(&case.cons);
            has_body && !is_terminated && !is_last
        });
        if has_code_fallthrough {
            return Ok(None);
        }

        // Build match arms from cases.
        // Pending entries accumulate during empty-body fall-through (case "string": case "number": ...)
        // and are flushed when a non-empty body is encountered — each pending entry generates
        // a separate arm with the same body, because Rust `|` patterns cannot bind different types.
        let mut arms: Vec<MatchArm> = Vec::new();
        let mut pending: Vec<MatchPattern> = Vec::new();

        for case in &switch.cases {
            if let Some(test) = &case.test {
                // Extract typeof string from the case: case "string", case "number", etc.
                let typeof_str = match test.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                    _ => return Ok(None), // Non-string case — bail out
                };

                // Resolve to enum variant
                let variant = self.resolve_typeof_to_enum_variant(&var_type, &typeof_str);
                let pattern = match variant {
                    Some((ref ename, ref vname)) => {
                        MatchPattern::Literal(Expr::Ident(format!("{ename}::{vname}({var_name})")))
                    }
                    None => {
                        // typeof string doesn't match any variant — skip this conversion
                        return Ok(None);
                    }
                };

                // Accumulate pattern (will be flushed when we hit a non-empty body)
                pending.push(pattern);

                if case.cons.is_empty() {
                    continue;
                }

                // Non-empty body: flush all pending patterns as separate arms with this body.
                for pat in pending.drain(..) {
                    let body: Vec<Stmt> = case
                        .cons
                        .iter()
                        .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                        .map(|s| self.convert_stmt(s, return_type))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect();

                    arms.push(MatchArm {
                        patterns: vec![pat],
                        guard: None,
                        body,
                    });
                }
            } else {
                // default case — also flush any pending patterns
                for pat in pending.drain(..) {
                    let body: Vec<Stmt> = case
                        .cons
                        .iter()
                        .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                        .map(|s| self.convert_stmt(s, return_type))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect();
                    arms.push(MatchArm {
                        patterns: vec![pat],
                        guard: None,
                        body,
                    });
                }
                let body: Vec<Stmt> = case
                    .cons
                    .iter()
                    .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                    .map(|s| self.convert_stmt(s, return_type))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect();

                arms.push(MatchArm {
                    patterns: vec![MatchPattern::Wildcard],
                    guard: None,
                    body,
                });
            }
        }

        // Flush any remaining pending patterns (trailing empty-body cases that fell off
        // the end of the switch). Generate arms with empty body to preserve match exhaustiveness.
        for pat in pending.drain(..) {
            arms.push(MatchArm {
                patterns: vec![pat],
                guard: None,
                body: vec![],
            });
        }

        Ok(Some(vec![Stmt::Match {
            expr: Expr::Ident(var_name),
            arms,
        }]))
    }
}

/// discriminated union の tag フィールドに対する switch を enum match に変換する。
impl<'a> Transformer<'a> {
    /// discriminated union の tag フィールドに対する switch を enum match に変換する。
    fn try_convert_discriminated_union_switch(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Option<Vec<Stmt>>> {
        use crate::registry::TypeDef;
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
        let obj_type = self.get_expr_type(&member.obj);
        let enum_name = match obj_type {
            Some(RustType::Named { name, .. }) => name.clone(),
            _ => return Ok(None),
        };

        // Check if this is a discriminated union and the field is the tag
        let (string_values, variant_fields) = match self.reg().get(&enum_name) {
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
        // Cat A: receiver object
        let object = self.convert_expr(&member.obj)?;
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
                        if needed_fields.contains(fname)
                            && !field_types.iter().any(|(n, _)| n == fname)
                        {
                            field_types.push((fname.clone(), ftype.clone()));
                        }
                    }
                }
            }

            // Push scope with bound fields, convert body, pop scope
            self.type_env.push_scope();
            for (fname, ftype) in &field_types {
                self.type_env.insert(fname.clone(), ftype.clone());
            }

            let body = case
                .cons
                .iter()
                .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                .map(|s| self.convert_stmt(s, return_type))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            self.type_env.pop_scope();

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
    match expr {
        Expr::IntLit(_) | Expr::NumberLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) => true,
        // Enum variant paths (e.g., Direction::Up) are valid match patterns
        Expr::Ident(name) => name.contains("::"),
        _ => false,
    }
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

/// Converts a switch on a string enum (non-tagged) into a match with enum variant patterns.
///
/// Detects `switch (dir) { case "up": ... }` where `dir` is typed as a string enum like
/// `Direction`, and resolves `"up"` → `Direction::Up` using the enum's `string_values` map.
impl<'a> Transformer<'a> {
    fn try_convert_string_enum_switch(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Option<Vec<Stmt>>> {
        use crate::registry::TypeDef;

        // Resolve the discriminant's type
        let disc_type = self.get_expr_type(&switch.discriminant);
        let enum_name = match disc_type {
            Some(RustType::Named { name, .. }) => name.clone(),
            _ => return Ok(None),
        };

        // Check if this is a string enum (non-tagged, with string_values)
        let string_values = match self.reg().get(&enum_name) {
            Some(TypeDef::Enum {
                tag_field: None,
                string_values,
                ..
            }) if !string_values.is_empty() => string_values,
            _ => return Ok(None),
        };

        // Convert discriminant
        let discriminant = self.convert_expr(&switch.discriminant)?;

        let mut arms: Vec<MatchArm> = Vec::new();
        let mut pending_patterns: Vec<MatchPattern> = Vec::new();

        for case in &switch.cases {
            if let Some(test) = &case.test {
                // Extract string literal from case
                let str_value = match test.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                    _ => return Ok(None), // Non-string case → fallback to normal switch
                };

                if let Some(variant_name) = string_values.get(&str_value) {
                    let path = format!("{enum_name}::{variant_name}");
                    pending_patterns.push(MatchPattern::Literal(Expr::Ident(path)));
                } else {
                    return Ok(None); // Unknown variant → fallback
                }
            }

            // Empty body = fall-through, accumulate patterns
            if case.cons.is_empty() {
                continue;
            }

            // Default case
            if case.test.is_none() {
                pending_patterns.push(MatchPattern::Wildcard);
            }

            let body = case
                .cons
                .iter()
                .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                .map(|s| self.convert_stmt(s, return_type))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            arms.push(MatchArm {
                patterns: std::mem::take(&mut pending_patterns),
                guard: None,
                body,
            });
        }

        Ok(Some(vec![Stmt::Match {
            expr: discriminant,
            arms,
        }]))
    }
}

/// Converts a switch with no code fall-through into a clean `Stmt::Match`.
impl<'a> Transformer<'a> {
    fn convert_switch_clean_match(
        &mut self,
        switch: &ast::SwitchStmt,
        discriminant: Expr,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let mut arms: Vec<MatchArm> = Vec::new();
        let mut pending_patterns: Vec<MatchPattern> = Vec::new();
        let mut pending_exprs: Vec<Expr> = Vec::new();

        // Build expected type from discriminant type for case value conversion.
        // Only propagate for enum types (Named types registered in registry), not primitives.

        for case in &switch.cases {
            if let Some(test) = &case.test {
                let pattern = self.convert_expr(test)?;
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
                .map(|s| self.convert_stmt(s, return_type))
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
    fn convert_switch_fallthrough(
        &mut self,
        switch: &ast::SwitchStmt,
        discriminant: &Expr,
        return_type: Option<&RustType>,
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
                .map(|s| self.convert_stmt(s, return_type))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            if let Some(test) = &case.test {
                // case val: ...
                // Only propagate enum types to case values, not primitives
                let test_expr = self.convert_expr(test)?;
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
    fn convert_do_while_stmt(
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
    fn try_convert_array_destructuring(
        &mut self,
        var_decl: &ast::VarDecl,
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
        // Cat A: destructuring source
        let source_expr = self.convert_expr(source)?;

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

    fn convert_for_stmt_as_loop(
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

    fn convert_block_or_stmt(
        &mut self,
        stmt: &ast::Stmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        match stmt {
            ast::Stmt::Block(block) => self.convert_stmt_list(&block.stmts, return_type),
            other => self.convert_stmt(other, return_type),
        }
    }

    fn convert_nested_fn_decl(&mut self, fn_decl: &ast::FnDecl) -> Result<Stmt> {
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

        let mut fn_type_env = TypeEnv::new();
        for param in &params {
            if let Some(ty) = &param.ty {
                fn_type_env.insert(param.name.clone(), ty.clone());
            }
        }

        // F-3b #8: Use sub-Transformer with local fn_type_env
        let body = match &fn_decl.function.body {
            Some(block) => Transformer {
                tctx: self.tctx,
                type_env: fn_type_env,
                synthetic: &mut *self.synthetic,
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

#[cfg(test)]
mod tests;
