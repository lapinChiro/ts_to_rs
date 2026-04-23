//! Binary and unary expression conversion.

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType, Stmt, UnOp};

use super::literals::{is_string_like, is_string_type};
use super::patterns::typeof_to_string;
use crate::transformer::helpers::coerce_default::{
    build_option_coerce_to_string, build_option_coerce_to_t,
};
use crate::transformer::helpers::peek_through::peek_through_type_assertions;
use crate::transformer::helpers::truthy::{
    falsy_predicate_for_expr, truthy_predicate_for_expr, try_constant_fold_bang, TempBinder,
};
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Converts a binary expression to IR, handling special patterns like
    /// nullish coalescing, typeof/undefined comparisons, and string concatenation.
    pub(crate) fn convert_bin_expr(
        &mut self,
        bin: &ast::BinExpr,
        expected: Option<&RustType>,
    ) -> Result<Expr> {
        // typeof x === "type" / typeof x !== "type" pattern
        if let Some(result) = self.try_convert_typeof_comparison(bin) {
            return Ok(result);
        }

        // x === undefined / x !== undefined pattern
        if let Some(result) = self.try_convert_undefined_comparison(bin) {
            return Ok(result);
        }

        // string literal enum comparison: d == "up" → d == Direction::Up
        if let Some(result) = self.try_convert_enum_string_comparison(bin) {
            return Ok(result);
        }

        // x instanceof ClassName pattern
        if bin.op == ast::BinaryOp::InstanceOf {
            return Ok(self.convert_instanceof(bin));
        }

        // "key" in obj pattern
        if bin.op == ast::BinaryOp::In {
            return Ok(self.convert_in_operator(bin));
        }

        // `x ?? y` emission (I-022):
        // - LHS Option + RHS non-Option  → `x.unwrap_or(y)` / `x.unwrap_or_else(|| y)`
        // - LHS Option + RHS Option      → `x.or(y)` / `x.or_else(|| y)` (chain case)
        // - LHS definitively non-Option  → short-circuit return LHS (TS: `??` is no-op)
        //
        // `is_option_left` combines the TS-inferred type with a structural IR check
        // via `produces_option_result`, catching `arr[i]` (emitted as `.get().cloned()`
        // via `resolve_bin_expr` LHS span propagation) and wrapped `Some(_)` literals.
        if bin.op == ast::BinaryOp::NullishCoalescing {
            // Cat A: ?? left operand — type is resolved separately for Option detection
            let left = self.convert_expr(&bin.left)?;
            let left_type = self.get_expr_type(&bin.left);
            let is_option_left = left_type.is_some_and(|ty| matches!(ty, RustType::Option(_)))
                || super::produces_option_result(&left);

            // Short-circuit: LHS is definitively non-Option (known static type + no
            // Option-producing IR shape). TS `??` with non-null LHS evaluates to LHS.
            if !is_option_left && left_type.is_some() {
                return Ok(left);
            }

            let right = self.convert_expr(&bin.right)?;
            let right_type = self.get_expr_type(&bin.right);
            let is_option_right = right_type.is_some_and(|ty| matches!(ty, RustType::Option(_)))
                || super::produces_option_result(&right);

            // RHS is also Option<T> (chain `a ?? b ?? c` inner case): preserve Option
            // via `.or()` / `.or_else()` so outer `??` can terminate with unwrap_or.
            if is_option_right {
                return Ok(crate::transformer::build_option_or_option(left, right));
            }
            return Ok(crate::transformer::build_option_unwrap_with_default(
                left, right,
            ));
        }

        // Cat A: binary operands — result type depends on operator, not context
        let left = self.convert_expr(&bin.left)?;
        let right = self.convert_expr(&bin.right)?;
        let op = convert_binary_op(bin.op)?;

        // String concatenation: wrap RHS in Ref(&) when LHS is string-like.
        // Priority: type inference → expected type → IR heuristic (is_string_like fallback).
        let is_string_context = if op == BinOp::Add {
            let left_type = self.get_expr_type(&bin.left);
            let type_inferred = left_type.is_some_and(is_string_type);
            type_inferred || matches!(expected, Some(RustType::String)) || is_string_like(&left)
        } else {
            false
        };

        // Mixed-type concatenation: one side is string, other is known non-string → format!
        // Handles: `42 + " px"` (f64 + &str) and `"val: " + x` (String + f64)
        if op == BinOp::Add && is_string_context {
            let left_type = self.get_expr_type(&bin.left);
            let right_type = self.get_expr_type(&bin.right);
            let left_is_string = left_type.is_some_and(is_string_type) || is_string_like(&left);
            let left_known_non_string = (left_type.is_some()
                && !left_type.is_some_and(is_string_type))
                && !is_string_like(&left);
            let right_known_non_string = (right_type.is_some()
                && !right_type.is_some_and(is_string_type))
                && !is_string_like(&right);

            if (left_known_non_string && !left_is_string)
                || (right_known_non_string && left_is_string)
            {
                // I-144 T6-2: coerce closure-reassigned Option<T> args to String
                // via the JS coerce_default table (`null` → `"null"`).
                let left = self.maybe_coerce_for_string_concat(&bin.left, left);
                let right = self.maybe_coerce_for_string_concat(&bin.right, right);
                return Ok(Expr::FormatMacro {
                    template: "{}{}".to_string(),
                    args: vec![left, right],
                });
            }
        }

        // I-144 T6-2: arithmetic context (`+`/`-`/`*`/`/`/`%`) — when an Ident
        // operand refers to a closure-reassigned `Option<T>` var, wrap with the
        // JS coerce_default value (`null` → `0.0` for `F64`) so post-stale
        // reads reproduce JS runtime semantics (`null + 1 = 1`).
        let (left, right) = if matches!(
            op,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
        ) && !is_string_context
        {
            (
                self.maybe_coerce_for_arith(&bin.left, left),
                self.maybe_coerce_for_arith(&bin.right, right),
            )
        } else {
            (left, right)
        };

        // In string concat context:
        // - LHS StringLit needs .to_string() (Rust: &str can't use + operator directly)
        // - LHS self.field needs .clone() (Rust: can't move out of &self)
        // - RHS non-literal needs & (Rust: String + &str)
        let left = if is_string_context && matches!(left, Expr::StringLit(_)) {
            Expr::MethodCall {
                object: Box::new(left),
                method: "to_string".to_string(),
                args: vec![],
            }
        } else if is_string_context
            && matches!(
                &left,
                Expr::FieldAccess { object, .. } if matches!(object.as_ref(), Expr::Ident(name) if name == "self")
            )
        {
            Expr::MethodCall {
                object: Box::new(left),
                method: "clone".to_string(),
                args: vec![],
            }
        } else {
            left
        };

        let right = if is_string_context && !matches!(right, Expr::StringLit(_)) {
            Expr::Ref(Box::new(right))
        } else {
            right
        };

        Ok(Expr::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        })
    }

    /// I-144 T6-2: if `ast_expr` is an `Ident` referring to a closure-reassigned
    /// `Option<T>` variable (narrow suppressed), wrap `ir_expr` with the
    /// JS coerce_default value via `unwrap_or` so an arithmetic read site
    /// reproduces JS runtime semantics. Otherwise returns `ir_expr` unchanged.
    fn maybe_coerce_for_arith(&self, ast_expr: &ast::Expr, ir_expr: Expr) -> Expr {
        let ast::Expr::Ident(id) = ast_expr else {
            return ir_expr;
        };
        if !self.is_var_closure_reassigned(id.sym.as_ref(), ast_expr.span().lo.0) {
            return ir_expr;
        }
        let Some(RustType::Option(inner)) = self.get_expr_type(ast_expr) else {
            return ir_expr;
        };
        build_option_coerce_to_t(ir_expr.clone(), inner).unwrap_or(ir_expr)
    }

    /// I-144 T6-2: same as [`maybe_coerce_for_arith`] but emits the string-context
    /// coerce shape (`x.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string())`).
    fn maybe_coerce_for_string_concat(&self, ast_expr: &ast::Expr, ir_expr: Expr) -> Expr {
        let ast::Expr::Ident(id) = ast_expr else {
            return ir_expr;
        };
        if !self.is_var_closure_reassigned(id.sym.as_ref(), ast_expr.span().lo.0) {
            return ir_expr;
        }
        let Some(RustType::Option(inner)) = self.get_expr_type(ast_expr) else {
            return ir_expr;
        };
        build_option_coerce_to_string(ir_expr.clone(), inner).unwrap_or(ir_expr)
    }

    /// Converts a unary expression (`!x`, `-x`, `typeof x`) to IR.
    pub(crate) fn convert_unary_expr(&mut self, unary: &ast::UnaryExpr) -> Result<Expr> {
        // typeof x → resolve based on FileTypeResolution
        if unary.op == ast::UnaryOp::TypeOf {
            let operand_type = self.get_expr_type(&unary.arg);
            return match operand_type {
                Some(RustType::Option(inner)) => {
                    // Option<T>: runtime branch — is_some() → typeof inner, else "undefined"
                    let operand = self.convert_expr(&unary.arg)?;
                    let inner_typeof = typeof_to_string(inner);
                    Ok(Expr::If {
                        condition: Box::new(Expr::MethodCall {
                            object: Box::new(operand),
                            method: "is_some".to_string(),
                            args: vec![],
                        }),
                        then_expr: Box::new(Expr::StringLit(inner_typeof.to_string())),
                        else_expr: Box::new(Expr::StringLit("undefined".to_string())),
                    })
                }
                Some(RustType::Any) => {
                    // Any type: runtime typeof via js_typeof helper
                    let operand = self.convert_expr(&unary.arg)?;
                    Ok(Expr::RuntimeTypeof {
                        operand: Box::new(operand),
                    })
                }
                Some(ty) => Ok(Expr::StringLit(typeof_to_string(ty).to_string())),
                None => {
                    // Type unresolvable: report as unsupported instead of silent "object"
                    Err(super::super::UnsupportedSyntaxError::new(
                        "typeof on unresolved type",
                        unary.span,
                    )
                    .into())
                }
            };
        }

        // Unary plus: +x → numeric conversion
        if unary.op == ast::UnaryOp::Plus {
            let operand_type = self.get_expr_type(&unary.arg);
            let operand = self.convert_expr(&unary.arg)?;
            return Ok(match operand_type {
                Some(RustType::F64) => operand, // already numeric, identity
                Some(RustType::String) => Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(operand),
                        method: "parse::<f64>".to_string(),
                        args: vec![],
                    }),
                    method: "unwrap".to_string(),
                    args: vec![],
                },
                _ => operand, // fallback: return as-is, let compiler catch type errors
            });
        }

        if unary.op == ast::UnaryOp::Bang {
            return self.convert_bang_expr(&unary.arg);
        }
        let ast::UnaryOp::Minus = unary.op else {
            return Err(anyhow!("unsupported unary operator: {:?}", unary.op));
        };
        // Cat A: unary operand — type depends on operator semantics
        let operand = self.convert_expr(&unary.arg)?;
        Ok(Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(operand),
        })
    }

    /// Emits JS `!<expr>` with type-aware falsy predicate dispatch (I-171 Layer 1).
    ///
    /// Pre-I-171 the Bang arm emitted a bare `!<operand_ir>`, which Rust rejects
    /// for non-`bool` operands (`F64` / `String` / `Option<T>` / `Vec<T>` / ...).
    /// Post-I-171 the emission is dispatched by the operand's effective type via
    /// the shared `falsy_predicate_for_expr` helper, so every TS `!<e>` lowers
    /// to a semantically equivalent Rust predicate regardless of operand shape.
    ///
    /// The dispatch layers:
    /// 1. `peek_through_type_assertions` strips runtime-no-op wrappers
    ///    (`Paren` / `TsAs` / `TsNonNull` / `TsTypeAssertion` / `TsConstAssertion`).
    /// 2. `try_constant_fold_bang` folds literals and always-truthy literal
    ///    shapes (`Arrow` / `Fn` / `Regex` / BigInt / Null / `undefined` ident).
    /// 3. Double negation `!!<e>` delegates to `truthy_predicate_for_expr`.
    ///    3b. De Morgan on `Bin(LogicalAnd/LogicalOr)` transforms `!(x && y)` /
    ///    `!(x || y)` at the AST layer so each operand is typed independently
    ///    (avoids emitting a raw Rust `x && y` on two `Option<T>` operands).
    ///    3c. Assign desugar: TS `x = rhs` returns the assigned value, but Rust's
    ///    `x = rhs` evaluates to `()`. Desugar to a block that captures rhs
    ///    into a tmp, performs the write, and feeds the tmp to the predicate.
    /// 4. General case uses `falsy_predicate_for_expr` with TempBinder for
    ///    side-effect-prone operands (non-Ident / non-Lit shapes).
    /// 5. Fallback (unresolved type / Any / TypeVar blocked) preserves the
    ///    pre-I-171 bare `!<operand>` emission so the compile error surface
    ///    stays explicit (structural follow-up tracked by I-050 / generic
    ///    bounds PRD).
    fn convert_bang_expr(&mut self, arg: &ast::Expr) -> Result<Expr> {
        let unwrapped = peek_through_type_assertions(arg);

        // Layer 2: AST-level const-fold for literals and literal-equivalent
        // shapes with no side effects (pure fold — does not evaluate operand).
        if let Some(folded) = try_constant_fold_bang(unwrapped) {
            return Ok(folded);
        }

        // Layer 3: double negation `!!<e>` → truthy_predicate_for_expr(e, ty).
        // Peek through the inner operand's wrappers before dispatch.
        if let ast::Expr::Unary(inner) = unwrapped {
            if inner.op == ast::UnaryOp::Bang {
                let inner_arg = peek_through_type_assertions(&inner.arg);

                // Layer 2 recursive const-fold: `!!<lit> = !(fold_bang(<lit>))`.
                // When the inner operand is a literal or literal-equivalent
                // shape recognised by `try_constant_fold_bang`, the double
                // negation reduces to the inverted fold result independently
                // of any TypeResolver context. This removes the Layer-5
                // fallback that previously emitted raw `!!<lit_ir>` for
                // untyped literal operands (Rust compile error for
                // non-`bool` literals such as `!!null` → `!!None`).
                if let Some(Expr::BoolLit(inner_fold)) = try_constant_fold_bang(inner_arg) {
                    return Ok(Expr::BoolLit(!inner_fold));
                }

                // Shapes where the AST-layer semantics diverge from what a
                // direct `truthy_predicate_for_expr(<ir>, <ty>)` emission can
                // represent in Rust:
                //
                // - `Assign` / compound arithmetic-assign: TS assign expressions
                //   evaluate to the assigned value, but the IR form
                //   `Expr::Assign { .. }` has Rust type `()`. A direct predicate
                //   (`<Assign>.is_nan()` / `<Assign> == 0.0`) fails to compile.
                // - `Bin(LogicalAnd / LogicalOr)`: TS `&&` / `||` returns a
                //   union type (first falsy / first truthy operand), but the
                //   IR form `Expr::BinaryOp { LogicalAnd / LogicalOr }` is
                //   Rust's bool-only `&&` / `||`, which rejects non-bool
                //   operands (`Option<T> && Option<U>` is a compile error).
                //
                // For these shapes we recurse through `convert_bang_expr` on
                // the inner operand — which routes Assign to Layer 3c and
                // LogicalAnd/Or to Layer 3b De Morgan — and invert the
                // resulting falsy emission with an outer `!`. This is
                // equivalent to the double-negation identity `!!x = !(!x)`
                // and composes cleanly with whichever layer handles `!x`.
                let needs_bang_recurse = matches!(inner_arg, ast::Expr::Assign(_))
                    || matches!(inner_arg, ast::Expr::Bin(b)
                        if matches!(b.op, ast::BinaryOp::LogicalAnd | ast::BinaryOp::LogicalOr));
                if needs_bang_recurse {
                    let inner_bang = self.convert_bang_expr(&inner.arg)?;
                    if let Expr::BoolLit(b) = &inner_bang {
                        return Ok(Expr::BoolLit(!*b));
                    }
                    return Ok(Expr::UnaryOp {
                        op: UnOp::Not,
                        operand: Box::new(inner_bang),
                    });
                }

                // Direct truthy predicate: Matrix B.1.19 ideal for regular
                // type-resolved operands (Ident / Member / OptChain / Call /
                // Cond / primitive Bin / etc.).
                let inner_ty = self.get_expr_type(inner_arg).cloned();
                let inner_ir = self.convert_expr(inner_arg)?;
                if let Some(ty) = inner_ty {
                    let mut binder = TempBinder::new();
                    if let Some(pred) =
                        truthy_predicate_for_expr(&inner_ir, &ty, self.synthetic, &mut binder)
                    {
                        return Ok(pred);
                    }
                }
                // Unresolved / Any / TypeVar non-literal: fall back to
                // nested `!!` emission (compile-error surface preserved for
                // I-050 / generic-bounds scope).
                return Ok(Expr::UnaryOp {
                    op: UnOp::Not,
                    operand: Box::new(Expr::UnaryOp {
                        op: UnOp::Not,
                        operand: Box::new(inner_ir),
                    }),
                });
            }
        }

        // Layer 3b: De Morgan on logical connectors (Matrix B.1.23 / B.1.24).
        if let ast::Expr::Bin(bin) = unwrapped {
            if let Some(folded) = self.convert_bang_logical(bin)? {
                return Ok(folded);
            }
        }

        // Layer 3c: Assign operand (`!(x = rhs)`).
        if let ast::Expr::Assign(assign_expr) = unwrapped {
            if let Some(folded) = self.convert_bang_assign(assign_expr)? {
                return Ok(folded);
            }
        }

        // Layer 4: general `!<e>` via falsy_predicate_for_expr dispatch.
        let operand_ty = self.get_expr_type(unwrapped).cloned();
        let operand_ir = self.convert_expr(unwrapped)?;
        if let Some(ty) = operand_ty {
            let mut binder = TempBinder::new();
            if let Some(pred) =
                falsy_predicate_for_expr(&operand_ir, &ty, self.synthetic, &mut binder)
            {
                return Ok(pred);
            }
        }

        // Layer 5: fallback preserves pre-I-171 emission so callers see an
        // explicit `!<non-bool>` compile error (structural fix tracked by
        // I-050 Any umbrella / generic-bounds PRD).
        Ok(Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(operand_ir),
        })
    }

    /// De Morgan transformation for `!(x && y)` / `!(x || y)` at the AST layer.
    ///
    /// Rust's `&&` / `||` are bool-only, so forwarding the inner
    /// `ast::Expr::Bin(LogicalAnd/LogicalOr)` to `convert_expr` before
    /// type-aware falsy dispatch would emit e.g. `x && y` on two
    /// `Option<T>` operands — a compile error. Transforming at the AST
    /// layer keeps the operands separate so each one is typed with
    /// `get_expr_type` and dispatched through `falsy_predicate_for_expr`.
    ///
    /// Returns `Ok(None)` for non-logical binary ops (caller falls through
    /// to the general dispatch).
    fn convert_bang_logical(&mut self, bin: &ast::BinExpr) -> Result<Option<Expr>> {
        let combinator = match bin.op {
            ast::BinaryOp::LogicalAnd => BinOp::LogicalOr,
            ast::BinaryOp::LogicalOr => BinOp::LogicalAnd,
            _ => return Ok(None),
        };
        let left_ir = self.convert_bang_expr(&bin.left)?;
        let right_ir = self.convert_bang_expr(&bin.right)?;
        Ok(Some(Expr::BinaryOp {
            left: Box::new(left_ir),
            op: combinator,
            right: Box::new(right_ir),
        }))
    }

    /// `!(x = rhs)` desugar: TS Assign evaluates to the assigned value, but
    /// Rust's `x = rhs` evaluates to `()`. Emit
    /// `{ let __ts_tmp: LhsType = <value>; x = __ts_tmp [.clone()]; <falsy(tmp, lhs_ty)> }`
    /// so the predicate reads the value after the side-effecting write.
    ///
    /// ## Type annotation = LHS type
    ///
    /// The `rhs` AST span carries the inferred type of the raw expression
    /// (e.g., `f64` for the literal `5`), but `convert_assign_expr` routes the
    /// RHS through `convert_expr_with_expected(... , lhs_ty)` which may wrap
    /// the IR in `Some(...)` to match an `Option<T>` LHS. Using the RHS span
    /// type for the tmp annotation therefore yields `let tmp: f64 = Some(5.0)`
    /// — a type mismatch. The correct annotation is the LHS storage type
    /// (the type that both the `value` IR and the assignment target share).
    ///
    /// ## Copy-aware assignment
    ///
    /// The predicate reads `tmp` after the assignment. For Copy types (`f64`,
    /// `bool`, `Primitive(int)`, `Option<T Copy>`, Copy tuples) `x = tmp`
    /// performs a Copy, leaving `tmp` available. For non-Copy types (`String`,
    /// `Option<String>`, `Vec<T>`, `Named` struct / union enum, `Tuple` with
    /// non-Copy components) the direct assignment would move `tmp`, making the
    /// subsequent predicate a borrow-after-move Rust error. We emit
    /// `x = tmp.clone()` in those cases so the original `tmp` remains owned
    /// for the predicate.
    ///
    /// ## Compound assigns
    ///
    /// The gate is IR-shape-based, not AST-op-based: Layer 3c fires iff
    /// `convert_assign_expr` emits an `Expr::Assign { target, value }` IR.
    ///
    /// - Plain `=` emits `Expr::Assign { target, rhs }` → desugar fires, tmp
    ///   captures the assigned value.
    /// - Arithmetic `+=`/`-=`/`*=`/`/=`/`%=` and bitwise compound emit
    ///   `Expr::Assign { target, BinaryOp(target, op, rhs) }` (plain-assign
    ///   shape with composite value) → desugar fires, tmp captures the new
    ///   LHS value which matches TS `!(x += v) = !<new x>`. This is required
    ///   because Rust `x = x + v` evaluates to `()`, so without the desugar
    ///   Layer 4 would emit `let tmp: T = (x = x + v)` — E0308 unit/T
    ///   mismatch.
    /// - `&&=` / `||=` / `??=` lower to non-Assign IR (`If` / `Block` /
    ///   `get_or_insert_with` etc.), so the `let Expr::Assign = assign_ir`
    ///   destructure below returns `None` and they fall through to Layer 4
    ///   (their conditional / lazy-RHS semantics are preserved by the
    ///   non-Assign path).
    fn convert_bang_assign(&mut self, assign_expr: &ast::AssignExpr) -> Result<Option<Expr>> {
        let lhs_ty = self.assign_target_type(&assign_expr.left);
        let Some(lhs_ty) = lhs_ty else {
            return Ok(None);
        };
        let assign_ir = self.convert_expr(&ast::Expr::Assign(assign_expr.clone()))?;
        let Expr::Assign { target, value } = assign_ir else {
            return Ok(None);
        };
        let mut binder = TempBinder::new();
        let tmp_name = binder.fresh("assign");
        let tmp_ident = Expr::Ident(tmp_name.clone());
        let Some(predicate) =
            falsy_predicate_for_expr(&tmp_ident, &lhs_ty, self.synthetic, &mut binder)
        else {
            return Ok(None);
        };
        // Preserve tmp for the predicate by cloning into the assignment when
        // the LHS storage type is not Copy.
        let assign_value = if lhs_ty.is_copy_type() {
            Expr::Ident(tmp_name.clone())
        } else {
            Expr::MethodCall {
                object: Box::new(Expr::Ident(tmp_name.clone())),
                method: "clone".to_string(),
                args: vec![],
            }
        };
        Ok(Some(Expr::Block(vec![
            Stmt::Let {
                mutable: false,
                name: tmp_name,
                ty: Some(lhs_ty),
                init: Some(*value),
            },
            Stmt::Expr(Expr::Assign {
                target,
                value: Box::new(assign_value),
            }),
            Stmt::TailExpr(predicate),
        ])))
    }

    /// Resolves the storage type of an assignment target.
    ///
    /// - `Ident` targets look up the variable's declared type via the current
    ///   variable scope (`get_type_for_var`).
    /// - `Member` targets consult `get_expr_type` on the synthesised Member
    ///   expression — TypeResolver records member types against the span of
    ///   the Member AST node.
    /// - Pattern / other compound targets return `None` (callers fall back to
    ///   the general dispatch path; pattern-based targets are handled by
    ///   downstream destructuring emission).
    fn assign_target_type(&self, target: &ast::AssignTarget) -> Option<RustType> {
        match target {
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
                self.get_type_for_var(&ident.id.sym, ident.id.span).cloned()
            }
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => self
                .get_expr_type(&ast::Expr::Member((*member).clone()))
                .cloned(),
            _ => None,
        }
    }
}

/// Converts an SWC binary operator to an IR [`BinOp`].
pub(crate) fn convert_binary_op(op: ast::BinaryOp) -> Result<BinOp> {
    match op {
        ast::BinaryOp::Add => Ok(BinOp::Add),
        ast::BinaryOp::Sub => Ok(BinOp::Sub),
        ast::BinaryOp::Mul => Ok(BinOp::Mul),
        ast::BinaryOp::Div => Ok(BinOp::Div),
        ast::BinaryOp::Mod => Ok(BinOp::Mod),
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq => Ok(BinOp::Eq),
        ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq => Ok(BinOp::NotEq),
        ast::BinaryOp::Lt => Ok(BinOp::Lt),
        ast::BinaryOp::LtEq => Ok(BinOp::LtEq),
        ast::BinaryOp::Gt => Ok(BinOp::Gt),
        ast::BinaryOp::GtEq => Ok(BinOp::GtEq),
        ast::BinaryOp::LogicalAnd => Ok(BinOp::LogicalAnd),
        ast::BinaryOp::LogicalOr => Ok(BinOp::LogicalOr),
        ast::BinaryOp::BitAnd => Ok(BinOp::BitAnd),
        ast::BinaryOp::BitOr => Ok(BinOp::BitOr),
        ast::BinaryOp::BitXor => Ok(BinOp::BitXor),
        ast::BinaryOp::LShift => Ok(BinOp::Shl),
        ast::BinaryOp::RShift => Ok(BinOp::Shr),
        ast::BinaryOp::ZeroFillRShift => Ok(BinOp::UShr),
        _ => Err(anyhow!("unsupported binary operator: {:?}", op)),
    }
}
