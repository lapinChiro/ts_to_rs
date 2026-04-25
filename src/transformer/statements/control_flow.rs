//! Control flow statement conversion.
//!
//! Converts if/while/for/do-while/labeled statements into IR representations.
//! Handles conditional assignments, narrowing guards, and `if let` generation.

use anyhow::Result;
use swc_ecma_ast as ast;

use super::helpers::{
    extract_conditional_assignment, generate_truthiness_condition, ConditionalAssignment,
};
use crate::ir::{BinOp, Expr, MatchArm, Pattern, RustType, Stmt};
use crate::transformer::expressions::patterns::extract_narrowing_guards;
use crate::transformer::helpers::peek_through::peek_through_type_assertions;
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

                if let Some(stmts) = self.try_generate_narrowing_match(
                    guard,
                    &then_body,
                    &else_body,
                    if_stmt.span.lo.0,
                )? {
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

        // I-171 Layer 2: `if (!x) <body> [else <else_body>]` on `Option<T>`
        // routes to a consolidated match.
        //
        // Three lowering forms are selected from the `(else_body present,
        // then-exits, else-exits)` triple of the source `if`:
        //
        // 1. `else_body.is_none()` && then exits: T6-3 (I-144) early-return
        //    form `let x = match x { Some(x) if truthy => x, _ => { exit } };`
        //    — threads narrow into post-if scope via outer let rebinding.
        //
        // 2. `else_body.is_some()` && then exits && else does **not** exit:
        //    T5 deep-fix form `let x = match x { Some(x) if truthy =>
        //    { else_body; x }, _ => { exit } };` — post-if is reachable only
        //    via the truthy else branch (then exits), so the narrow must
        //    materialise post-match. Wraps in let and tail-emits the
        //    narrowed value after running the user-written else_body.
        //
        // 3. `else_body.is_some()` && (else exits OR neither exits): T5 bare
        //    `match x { Some(x) if truthy => { else_body }, _ => { then_body } }`
        //    — narrow scoped to the `Some(x)` arm only; post-match `x` stays
        //    `Option<T>` because either post-if is unreachable (both exit) or
        //    the falsy then-branch can also fall through (no useful narrow).
        //
        // 4. `else_body.is_none()` && body non-exit: returns None and falls
        //    through to the predicate-form emission below (Matrix C-4).
        if let Some(stmts) = self.try_generate_option_truthy_complement_match(
            &if_stmt.test,
            &then_body,
            else_body.as_deref(),
            if_stmt.span.lo.0,
        )? {
            return Ok(stmts);
        }

        let condition =
            if let Some(pred) = self.try_generate_primitive_truthy_condition(&if_stmt.test) {
                pred
            } else {
                self.convert_expr(&if_stmt.test)?
            };

        // I-171 Layer 2 const-fold dead-code elimination (Matrix C-7..C-10 / C-24).
        // T4 `try_constant_fold_bang` lowers `!<lit>` / `!<always-truthy>` to
        // `Expr::BoolLit(b)`. Wrapping this in `if true { ... }` /
        // `if false { ... } else { ... }` would emit redundant Rust that the
        // compiler keeps but which fails the PRD's "ideal output" criterion
        // (and, for the `if true` form, leaves the trailing tail expression
        // looking unreachable to readers). Inline the live branch and discard
        // the dead one so the post-fold IR matches a hand-written equivalent.
        if let Expr::BoolLit(b) = &condition {
            return Ok(if *b {
                then_body
            } else {
                else_body.unwrap_or_default()
            });
        }

        Ok(vec![Stmt::If {
            condition,
            then_body,
            else_body,
        }])
    }

    // `try_generate_option_truthy_complement_match` and its arm-builder
    // helpers (`build_option_truthy_match_arms`, `build_union_variant_truthy_arms`)
    // along with the [`super::option_truthy_complement::OptionTruthyShape`]
    // enum live in [`super::option_truthy_complement`] to keep this file
    // under the per-file line budget. The method is still on
    // `impl<'a> Transformer<'a>` (Rust impl blocks may be split across
    // modules within a crate), so callers in this file invoke it as
    // `self.try_generate_option_truthy_complement_match(...)` unchanged.

    /// Tries to generate a primitive truthy / falsy predicate for the simple
    /// `if (x)` and `if (!x)` shapes.
    ///
    /// Both the outer test and the Bang inner operand are peeled with
    /// [`peek_through_type_assertions`] so syntactic wrappers
    /// (`Paren` / `TsAs` / `TsNonNull` / `TsTypeAssertion` / `TsConstAssertion`)
    /// transparently dispatch to the same primitive emission as the bare
    /// identifier (Matrix C-11 / C-12 / C-13 — `if (x as T)`, `if (!(x!))`,
    /// `if ((x))` etc.). Without peek-through these forms would fall through
    /// to the generic `convert_expr` path and emit non-bool primitive
    /// conditions in places that require a `bool`.
    fn try_generate_primitive_truthy_condition(&self, test: &ast::Expr) -> Option<Expr> {
        match peek_through_type_assertions(test) {
            ast::Expr::Ident(ident) => {
                let ty = self.get_type_for_var(ident.sym.as_ref(), ident.span)?;
                crate::transformer::helpers::truthy::truthy_predicate(ident.sym.as_ref(), ty)
            }
            ast::Expr::Unary(u) if u.op == ast::UnaryOp::Bang => {
                if let ast::Expr::Ident(ident) = peek_through_type_assertions(u.arg.as_ref()) {
                    let ty = self.get_type_for_var(ident.sym.as_ref(), ident.span)?;
                    crate::transformer::helpers::truthy::falsy_predicate(ident.sym.as_ref(), ty)
                } else {
                    None
                }
            }
            _ => None,
        }
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
                pattern: Pattern::some_binding(&ca.var_name),
                expr: rhs_ir,
                then_body,
                else_body,
            }]),
            Some(ty) => {
                let condition = generate_truthiness_condition(&ca.var_name, ty, self.synthetic);
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
        guard_position: u32,
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

        let complement_is_none = complement_pattern.is_none_unit();

        if is_early_return && !complement_is_none {
            // Early return: `let var = match var { Pos(v) => { exit }, Comp(v) => v };`
            let positive_arm = MatchArm {
                patterns: vec![positive_pattern.clone()],
                guard: None,
                body: positive_body,
            };
            let complement_arm = MatchArm {
                patterns: vec![complement_pattern.clone()],
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

        if complement_is_none && is_early_return && is_swap {
            // Option early return with === null: `let var = match var { None => { exit }, Some(v) => v };`
            // Only when is_swap (=== null): the null handler exits, and Some(v) extracts the value.
            // Truthy `if (x) { return; }` has is_swap=false: the Some arm exits, None has no value.
            //
            // T6-2 closure-reassign suppression (I-144 Sub-matrix 5 RC1/RC6 stale):
            // when an inner closure reassigns `var`, the outer narrow shadow-let
            // would bind `var` to a local `T` while the closure body needs to
            // reassign through `Option<T>` (E0308). Emit `if var.is_none()
            // { exit }` instead so `var` stays `Option<T>` and subsequent
            // T-expected reads coerce via `helpers::coerce_default`.
            if self.is_var_closure_reassigned(&var_name, guard_position) {
                let condition = Expr::MethodCall {
                    object: Box::new(Expr::Ident(var_name.clone())),
                    method: "is_none".to_string(),
                    args: vec![],
                };
                return Ok(Some(vec![Stmt::If {
                    condition,
                    then_body: complement_body,
                    else_body: None,
                }]));
            }
            let none_arm = MatchArm {
                patterns: vec![Pattern::none()],
                guard: None,
                body: complement_body,
            };
            let some_arm = MatchArm {
                patterns: vec![Pattern::some_binding(&var_name)],
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

        // Option early return WITH non-exit else (=== null swap form):
        //   `let var = match var { None => { then_body }, Some(v) => { else_body; v } };`
        // Symmetric with the bare-`!x` case handled by
        // `OptionTruthyShape::EarlyReturnFromExitWithElse` (T5 deep-fix):
        // post-if `var` must materialise as the narrow `T` because TS reaches
        // post-if only via the truthy `Some` branch (then exits). Without
        // this branch the emission falls through to the bare-`if let` form
        // which scopes the narrow inside the `if let` block, leaving post-if
        // `var` as `Option<T>` and breaking type-driven coercions like
        // `return var;` against an `Option<T>` return type
        // (TypeResolver expects narrow `T`, IR provides `Option<T>` →
        // `Some(var)` wrap → `Option<Option<T>>` mismatch).
        let then_exits = !then_body.is_empty() && ir_body_always_exits(then_body);
        let else_exits = else_body.as_ref().is_some_and(|b| ir_body_always_exits(b));
        if complement_is_none && is_swap && else_body.is_some() && then_exits && !else_exits {
            // T6-2 closure-reassign suppression: same rationale as the
            // bare-early-return form above. When the inner closure
            // reassigns `var`, the outer narrow shadow-let would bind
            // `var` to a local `T` while the closure body needs to
            // reassign through `Option<T>`. Fall back to the bare
            // `if var.is_none() { exit }` shape so `var` stays `Option<T>`.
            if self.is_var_closure_reassigned(&var_name, guard_position) {
                let condition = Expr::MethodCall {
                    object: Box::new(Expr::Ident(var_name.clone())),
                    method: "is_none".to_string(),
                    args: vec![],
                };
                let mut combined = vec![Stmt::If {
                    condition,
                    then_body: complement_body.clone(),
                    else_body: None,
                }];
                combined.extend(positive_body);
                return Ok(Some(combined));
            }
            let none_arm = MatchArm {
                patterns: vec![Pattern::none()],
                guard: None,
                body: complement_body,
            };
            let mut some_body = positive_body;
            some_body.push(Stmt::TailExpr(Expr::Ident(var_name.clone())));
            let some_arm = MatchArm {
                patterns: vec![Pattern::some_binding(&var_name)],
                guard: None,
                body: some_body,
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

        if else_body.is_some() && !complement_is_none {
            // Else block pattern: `match var { Pos(v) => { then }, Comp(v) => { else } }`
            let positive_arm = MatchArm {
                patterns: vec![positive_pattern],
                guard: None,
                body: positive_body,
            };
            let complement_arm = MatchArm {
                patterns: vec![complement_pattern],
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
///
/// Nested control-flow (`If` / `IfLet` / `Match`) is transitively
/// "always-exit" when every branch is always-exit. Empty `Match.arms` cannot
/// be always-exit because the match produces no statements; the non-empty
/// requirement is enforced explicitly.
pub(crate) fn ir_body_always_exits(body: &[Stmt]) -> bool {
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
        Stmt::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|arm| ir_body_always_exits(&arm.body))
        }
        _ => false,
    })
}
