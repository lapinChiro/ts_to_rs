//! Control flow statement conversion.
//!
//! Converts if/while/for/do-while/labeled statements into IR representations.
//! Handles conditional assignments, narrowing guards, and `if let` generation.

use anyhow::Result;
use swc_ecma_ast as ast;

use super::helpers::{
    extract_conditional_assignment, generate_truthiness_condition, unwrap_parens,
    ConditionalAssignment,
};
use crate::ir::{BinOp, CallTarget, Expr, MatchArm, Pattern, PatternCtor, RustType, Stmt};
use crate::pipeline::synthetic_registry::SyntheticTypeKind;
use crate::transformer::expressions::patterns::extract_narrowing_guards;
use crate::transformer::helpers::truthy;
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

        // I-144 T6-3 (cell-i024): `if (!x) <exit>` on `Option<T>` emitted as a
        // consolidated match that combines the composite truthy predicate with
        // the non-null narrow materialization. Must run before the primitive
        // fallback because `!x` on Option<Union> has no valid predicate form.
        if else_body.is_none() {
            if let Some(stmts) =
                self.try_generate_option_truthy_complement_match(&if_stmt.test, &then_body)?
            {
                return Ok(stmts);
            }
        }

        let condition =
            if let Some(pred) = self.try_generate_primitive_truthy_condition(&if_stmt.test) {
                pred
            } else {
                self.convert_expr(&if_stmt.test)?
            };
        Ok(vec![Stmt::If {
            condition,
            then_body,
            else_body,
        }])
    }

    /// Emits a consolidated `match` for `if (!x) <early-exit>` on
    /// `Option<T>`.
    ///
    /// This is the T6-3 core for cell-i024. Replaces the naive fallback
    /// `if !x { <exit> }` (invalid Rust for `Option<T>`) with:
    ///
    /// ```text
    /// let x = match x {
    ///     <truthy Some arms>  => <rebound value>,
    ///     _ => { <exit body> }
    /// };
    /// ```
    ///
    /// The arm shape depends on `T`:
    /// - `T = F64 | String | Bool | integer`: single `Some(v) if <v truthy> => v`.
    /// - `T = Named` (synthetic union with primitive variants): one
    ///   `Some(Enum::Variant(v)) if <v truthy> => Enum::Variant(v)` per variant.
    /// - `T = Named` (non-synthetic or variant without primitive data): single
    ///   `Some(v) => v` (all `Some` values are JS-truthy for non-primitive
    ///   payloads: objects, arrays, functions).
    ///
    /// Returns `None` (fall through to existing emission) when:
    /// - `test` is not `!ident`.
    /// - `ident` has no resolved type or is not `Option<T>`.
    /// - `then_body` does not always exit (return/throw/break/continue).
    ///   The non-exit case is semantically a no-op narrow on failure, which
    ///   the existing Option if-let path handles correctly.
    fn try_generate_option_truthy_complement_match(
        &self,
        test: &ast::Expr,
        then_body: &[Stmt],
    ) -> Result<Option<Vec<Stmt>>> {
        let ast::Expr::Unary(unary) = unwrap_parens(test) else {
            return Ok(None);
        };
        if unary.op != ast::UnaryOp::Bang {
            return Ok(None);
        }
        let ast::Expr::Ident(ident) = unwrap_parens(unary.arg.as_ref()) else {
            return Ok(None);
        };
        let var_name = ident.sym.to_string();
        let Some(var_ty) = self.get_type_for_var(&var_name, ident.span) else {
            return Ok(None);
        };
        let RustType::Option(inner) = var_ty else {
            return Ok(None);
        };
        if !ir_body_always_exits(then_body) {
            return Ok(None);
        }

        let arms = self.build_option_truthy_match_arms(&var_name, inner, then_body)?;
        let Some(arms) = arms else {
            return Ok(None);
        };

        let match_expr = Expr::Match {
            expr: Box::new(Expr::Ident(var_name.clone())),
            arms,
        };
        Ok(Some(vec![Stmt::Let {
            mutable: false,
            name: var_name,
            ty: None,
            init: Some(match_expr),
        }]))
    }

    /// Builds match arms for the composite Option truthy complement emission.
    ///
    /// The final arm (`_ => <exit_body>`) is appended unconditionally by the
    /// caller. This method returns the positive (truthy) arms only.
    /// Returns `None` when the inner type is not supported (e.g. Vec, Fn,
    /// Tuple) — the caller falls back to the existing emission so we do not
    /// introduce a silent semantic change for cases the PRD does not cover.
    fn build_option_truthy_match_arms(
        &self,
        var_name: &str,
        inner: &RustType,
        exit_body: &[Stmt],
    ) -> Result<Option<Vec<MatchArm>>> {
        let exit_arm = MatchArm {
            patterns: vec![Pattern::Wildcard],
            guard: None,
            body: exit_body.to_vec(),
        };

        let positive_arms: Option<Vec<MatchArm>> = match inner {
            RustType::F64 | RustType::String | RustType::Bool | RustType::Primitive(_) => {
                let guard = truthy::truthy_predicate(var_name, inner);
                Some(vec![MatchArm {
                    patterns: vec![Pattern::some_binding(var_name)],
                    guard,
                    body: vec![Stmt::TailExpr(Expr::Ident(var_name.to_string()))],
                }])
            }
            RustType::Named { name, type_args } if type_args.is_empty() => {
                self.build_union_variant_truthy_arms(name)
            }
            _ => None,
        };

        let Some(mut arms) = positive_arms else {
            return Ok(None);
        };
        arms.push(exit_arm);
        Ok(Some(arms))
    }

    /// For a synthetic union enum `enum_name`, emits one arm per variant:
    ///
    /// `Some(Enum::Variant(v)) if <v truthy> => Enum::Variant(v)` for
    /// primitive payloads, or the same pattern without a guard for
    /// non-primitive (always-truthy) payloads.
    ///
    /// Returns `None` when the union is not registered as a synthetic
    /// `UnionEnum`, or when any variant lacks a `data` RustType. The outer
    /// variable name is not threaded here — the per-arm inner binding is
    /// arm-local (`__ts_union_inner`) and cannot collide with outer
    /// identifiers regardless of the caller's choice of `var_name`.
    fn build_union_variant_truthy_arms(&self, enum_name: &str) -> Option<Vec<MatchArm>> {
        let def = self.synthetic.get(enum_name)?;
        if def.kind != SyntheticTypeKind::UnionEnum {
            return None;
        }
        let crate::ir::Item::Enum { variants, .. } = &def.item else {
            return None;
        };
        // `__ts_` prefix follows the internal-var convention established in
        // I-154. Arm-local scope guarantees no collision with outer bindings
        // even when user code happens to use the same identifier.
        const INNER_BIND: &str = "__ts_union_inner";
        let enum_ref = crate::ir::UserTypeRef::new(enum_name.to_string());

        let mut arms = Vec::with_capacity(variants.len());
        for variant in variants {
            let variant_ty = variant.data.as_ref()?;
            let guard = if is_supported_variant_truthy_type(variant_ty) {
                truthy::truthy_predicate(INNER_BIND, variant_ty)
            } else {
                // Non-primitive variant payload (Named struct, Vec, Tuple, Fn,
                // etc.) — JS treats all object references as truthy. Emit the
                // arm without a guard so `Some(Union::V(v))` unconditionally
                // materializes the narrow.
                None
            };
            let pattern = Pattern::TupleStruct {
                ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
                fields: vec![Pattern::TupleStruct {
                    ctor: PatternCtor::UserEnumVariant {
                        enum_ty: enum_ref.clone(),
                        variant: variant.name.clone(),
                    },
                    fields: vec![Pattern::binding(INNER_BIND)],
                }],
            };
            let body_expr = Expr::FnCall {
                target: CallTarget::UserEnumVariantCtor {
                    enum_ty: enum_ref.clone(),
                    variant: variant.name.clone(),
                },
                args: vec![Expr::Ident(INNER_BIND.to_string())],
            };
            arms.push(MatchArm {
                patterns: vec![pattern],
                guard,
                body: vec![Stmt::TailExpr(body_expr)],
            });
        }
        Some(arms)
    }

    /// Emits a JS truthy/falsy predicate expression when the test is a bare
    /// identifier (or its negation) on a primitive RustType.
    ///
    /// This is the T6-3 E10 entry point for `if (x)` / `if (!x)` on
    /// `F64` / `String` / `Bool` / integer primitives where the naive
    /// `if x { ... }` Rust emission would fail type checking (`expected
    /// bool, found f64`). Option/Named types return `None` so the caller
    /// delegates to the existing `if let Some(..) = ..` narrow path.
    ///
    /// Paren wrapping is unwrapped structurally so `if ((x))` / `if (!(x))`
    /// exercise the same path as `if (x)` / `if (!x)`.
    fn try_generate_primitive_truthy_condition(&self, test: &ast::Expr) -> Option<Expr> {
        match unwrap_parens(test) {
            ast::Expr::Ident(ident) => {
                let ty = self.get_type_for_var(ident.sym.as_ref(), ident.span)?;
                crate::transformer::helpers::truthy::truthy_predicate(ident.sym.as_ref(), ty)
            }
            ast::Expr::Unary(u) if u.op == ast::UnaryOp::Bang => {
                if let ast::Expr::Ident(ident) = unwrap_parens(u.arg.as_ref()) {
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

/// Whether a variant's `data` RustType requires a JS-truthy guard
/// (`!= 0.0 && !.is_nan()`, `!.is_empty()`, etc.) in the composite match.
///
/// Primitives (`F64`, `String`, `Bool`, `Primitive(int)`) can be falsy at
/// runtime, so the per-variant arm needs a guard to exclude falsy values.
/// Non-primitive payloads (`Named` struct, `Vec`, `Tuple`, `Fn`, etc.) are
/// always truthy in JS (every object reference is truthy), so the arm emits
/// without a guard and matches any `Some(Enum::Variant(_))` of that variant.
fn is_supported_variant_truthy_type(ty: &RustType) -> bool {
    matches!(
        ty,
        RustType::F64 | RustType::String | RustType::Bool | RustType::Primitive(_)
    )
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
pub(super) fn ir_body_always_exits(body: &[Stmt]) -> bool {
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
