//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{CallTarget, Expr, RustType};
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
// Re-export for tests that use `super::*`
#[cfg(test)]
use crate::ir::{ClosureBody, Param};
use crate::transformer::Transformer;

mod assignments;
mod binary;
mod calls;
mod data_literals;
mod functions;
mod literals;
pub(crate) mod member_access;
pub(crate) mod member_dispatch;
pub(crate) mod methods;
pub(crate) mod patterns;
mod type_resolution;
pub(crate) use binary::convert_binary_op;

// =============================================================================
// I-154 namespace reservation rule extension to value bindings (T7 source)
// =============================================================================

/// Binding name for postfix UpdateExpr / compound assign **old-value**
/// preservation in setter desugar / fallback FieldAccess block emissions.
///
/// Used in `obj.x++` desugar:
/// `{ let __ts_old = obj.x(); obj.set_x(__ts_old + 1.0); __ts_old }`
/// (B4 setter dispatch) and `{ let __ts_old = obj.x; obj.x = __ts_old + 1.0;
/// __ts_old }` (B1 field fallback)、Ident form `i++` も同 binding。
///
/// Single source of truth for the postfix old-value emission binding name —
/// references this constant from all emission sites (`assignments.rs`
/// `convert_update_expr` + `build_fallback_field_update_block`、
/// `member_dispatch/shared.rs` `build_setter_desugar_block`) to prevent silent
/// drift if the convention changes.
///
/// ## I-154 namespace reservation extension
///
/// The `__ts_` prefix namespace is reserved for ts_to_rs internal emission per
/// the [I-154 rule](crate::transformer::statements) (originally for labels:
/// `__ts_switch`, `__ts_try_block`, `__ts_do_while`, `__ts_do_while_loop`).
/// T7 (Iteration v11) extends the reservation from labels (statement-level) to
/// **value bindings** (expression-level) — user TS code containing identifiers
/// like `_old` / `_new` cannot collide with this emission inside the inner
/// block scope, ensuring hygiene without case-by-case collision check.
pub(super) const TS_OLD_BINDING: &str = "__ts_old";

/// Binding name for prefix UpdateExpr / compound assign **new-value**
/// preservation in setter desugar block emissions.
///
/// Used in `++obj.x` desugar:
/// `{ let __ts_new = obj.x() + 1.0; obj.set_x(__ts_new); __ts_new }`
/// (B4 setter dispatch、prefix yields the new value)、および compound assign
/// (`obj.x += v`) yields the new value (= prefix update と same shape with
/// `rhs` replacing `1.0`)。
///
/// Single source of truth for the prefix/compound new-value emission binding name
/// (see [`TS_OLD_BINDING`] for the I-154 namespace reservation extension rationale).
pub(super) const TS_NEW_BINDING: &str = "__ts_new";

/// Binding name for **side-effect-having receiver** preservation in setter
/// desugar IIFE block emissions (T8 INV-3 1-evaluate compliance).
///
/// Used when the receiver of `obj.x += v` / `obj.x++` carries side effects
/// (e.g., `getInstance().value += v`). The IIFE form binds the receiver to
/// `__ts_recv` (mutable for setter `&mut self` borrow) and reroutes both
/// getter call (`__ts_recv.x()`) and setter call (`__ts_recv.set_x(...)`)
/// through this binding, ensuring the source-side receiver expression is
/// evaluated **exactly once** per INV-3 (a) Property statement:
///
/// ```text
/// {
///   let mut __ts_recv = <receiver>;          // evaluated once
///   let __ts_new = __ts_recv.x() + v;
///   __ts_recv.set_x(__ts_new);
///   __ts_new
/// }
/// ```
///
/// Side-effect-free receivers (`Expr::Ident` / depth-bounded `Expr::FieldAccess`)
/// skip this binding and embed the receiver directly twice — Rust handles
/// `obj.x()` + `obj.set_x(...)` via auto-borrow without source-level repetition
/// changing semantics.
///
/// Single source of truth for the IIFE receiver binding name (see
/// [`TS_OLD_BINDING`] for the I-154 namespace reservation extension rationale).
pub(super) const TS_RECV_BINDING: &str = "__ts_recv";

impl<'a> Transformer<'a> {
    /// Converts an SWC [`ast::Expr`] into an IR [`Expr`].
    ///
    /// The expected type is read from `FileTypeResolution.expected_type()`.
    pub(crate) fn convert_expr(&mut self, expr: &ast::Expr) -> Result<Expr> {
        self.convert_expr_with_expected(expr, None)
    }

    /// Converts an expression with an explicit expected type override.
    ///
    /// Callers:
    /// - `convert_expr` (default: no override, reads TypeResolver expected_type)
    /// - `convert_stmt` Return arm (override = return_type when Any, I-050)
    /// - `convert_var_decl` (override = declared type when Any, I-050)
    /// - Internal recursion for Option<T> unwrap (avoids re-reading same span)
    pub(crate) fn convert_expr_with_expected(
        &mut self,
        expr: &ast::Expr,
        expected_override: Option<&RustType>,
    ) -> Result<Expr> {
        let expected = expected_override.or_else(|| {
            self.tctx
                .type_resolution
                .expected_type(Span::from_swc(expr.span()))
        });
        // Option<T> expected: unified wrapping logic.
        if let Some(RustType::Option(inner)) = expected {
            // null / undefined → None
            if matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
                || matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
            {
                return Ok(Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None));
            }
            // Literals → Some(convert(expr, inner_T))
            if matches!(expr, ast::Expr::Lit(_)) {
                let inner_result = self.convert_expr_with_expected(expr, Some(inner))?;
                return Ok(Expr::FnCall {
                    target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                    args: vec![inner_result],
                });
            }
            let expr_type = self.get_expr_type(expr);
            if matches!(expr_type, Some(RustType::Option(_))) {
                // Already produces Option — skip wrapping, fall through
            } else {
                let inner_result = self.convert_expr_with_expected(expr, Some(inner))?;
                if matches!(
                    &inner_result,
                    Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)
                ) || matches!(
                    &inner_result,
                    Expr::FnCall {
                        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                        ..
                    }
                ) {
                    return Ok(inner_result);
                }
                // Structural wrap-skip: the generated IR may itself be a call to
                // a Rust API that always returns `Option<T>`. In that case wrapping
                // would produce `Option<Option<T>>`. This runs after conversion so
                // it correctly detects remapped calls (e.g. TS `.find()` → the
                // `.iter().cloned().find(...)` chain terminated by `find`) even
                // when TypeResolver could not resolve the object type precisely
                // (e.g. in the no-builtins `transpile_collecting` path).
                if produces_option_result(&inner_result) {
                    return Ok(inner_result);
                }
                return Ok(Expr::FnCall {
                    target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                    args: vec![inner_result],
                });
            }
        }

        let result = match expr {
            ast::Expr::Ident(ident) => {
                let name = ident.sym.to_string();
                match name.as_str() {
                    "undefined" => Ok(Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)),
                    "NaN" => Ok(Expr::PrimitiveAssocConst {
                        ty: crate::ir::PrimitiveType::F64,
                        name: "NAN".to_string(),
                    }),
                    "Infinity" => Ok(Expr::PrimitiveAssocConst {
                        ty: crate::ir::PrimitiveType::F64,
                        name: "INFINITY".to_string(),
                    }),
                    _ => Ok(Expr::Ident(name)),
                }
            }
            ast::Expr::Lit(lit) => self.convert_lit(lit, expected),
            ast::Expr::Bin(bin) => self.convert_bin_expr(bin, expected),
            ast::Expr::Tpl(tpl) => self.convert_template_literal(tpl),
            ast::Expr::Paren(paren) => self.convert_expr(&paren.expr),
            ast::Expr::Member(member) => self.convert_member_expr(member),
            ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
            ast::Expr::Assign(assign) => self.convert_assign_expr(assign),
            ast::Expr::Update(up) => self.convert_update_expr(up),
            ast::Expr::Arrow(arrow) => self.convert_arrow_expr(arrow, false, &mut Vec::new()),
            ast::Expr::Fn(fn_expr) => self.convert_fn_expr(fn_expr),
            ast::Expr::Call(call) => self.convert_call_expr(call),
            ast::Expr::New(new_expr) => self.convert_new_expr(new_expr),
            ast::Expr::Array(array_lit) => self.convert_array_lit(array_lit, expected),
            ast::Expr::Object(obj_lit) => self.convert_object_lit(obj_lit, expected),
            ast::Expr::Cond(cond) => self.convert_cond_expr(cond),
            ast::Expr::Unary(unary) => self.convert_unary_expr(unary),
            ast::Expr::TsAs(ts_as) => {
                match convert_ts_type(&ts_as.type_ann, self.synthetic, self.reg()) {
                    Ok(target_ty) if matches!(target_ty, RustType::F64 | RustType::Bool) => {
                        let inner = self.convert_expr(&ts_as.expr)?;
                        Ok(Expr::Cast {
                            expr: Box::new(inner),
                            target: target_ty,
                        })
                    }
                    _ => self.convert_expr(&ts_as.expr),
                }
            }
            ast::Expr::OptChain(opt_chain) => self.convert_opt_chain_expr(opt_chain),
            ast::Expr::Await(await_expr) => {
                let inner = self.convert_expr(&await_expr.arg)?;
                Ok(Expr::Await(Box::new(inner)))
            }
            ast::Expr::TsNonNull(ts_non_null) => self.convert_expr(&ts_non_null.expr),
            _ => Err(anyhow!("unsupported expression: {:?}", expr)),
        }?;

        // Trait type coercion
        if let Some(expected) = expected {
            if self.needs_trait_box_coercion(expected, expr) {
                return Ok(Expr::FnCall {
                    // `Box::new(...)` is a std call, not a user type reference.
                    target: CallTarget::ExternalPath(vec!["Box".to_string(), "new".to_string()]),
                    args: vec![result],
                });
            }
        }

        // Any coercion: concrete literal → serde_json::Value::from(...)
        // When the caller explicitly requests Any (via expected_override) and the
        // source expression is a primitive literal, wrap in Value::from(). I-050-a.
        //
        // Only triggers on:
        // - expected_override = Some(Any) (explicit caller request from return/let-init)
        // - Expr is a Lit (Str/Num/Bool) — always safe to coerce
        //
        // Ident coercion is deferred to a future sub-PRD: TypeResolver's expr_type
        // for idents can diverge from the actual IR type (e.g., `input as string`
        // is typed as String by TypeResolver but remains Value in IR), causing
        // false-positive coercion.
        if let Some(RustType::Any) = expected_override {
            if matches!(
                expr,
                ast::Expr::Lit(ast::Lit::Str(_) | ast::Lit::Num(_) | ast::Lit::Bool(_))
            ) {
                return Ok(Expr::FnCall {
                    target: CallTarget::ExternalPath(vec![
                        "serde_json".to_string(),
                        "Value".to_string(),
                        "from".to_string(),
                    ]),
                    args: vec![result],
                });
            }
        }

        Ok(result)
    }

    /// Converts a template literal to `Expr::FormatMacro`.
    ///
    /// `` `Hello ${name}` `` becomes `format!("Hello {}", name)`.
    fn convert_template_literal(&mut self, tpl: &ast::Tpl) -> Result<Expr> {
        let mut template = String::new();
        let mut args = Vec::new();

        for (i, quasi) in tpl.quasis.iter().enumerate() {
            template.push_str(&quasi.raw);
            if i < tpl.exprs.len() {
                template.push_str("{}");
                let arg = self.convert_expr(&tpl.exprs[i])?;
                args.push(arg);
            }
        }

        Ok(Expr::FormatMacro { template, args })
    }

    /// Converts an SWC conditional (ternary) expression to `Expr::If` or `Expr::IfLet`.
    ///
    /// Supports compound `&&` guards (e.g., `typeof x === "string" && y !== null`)
    /// by generating nested `Expr::IfLet`.
    fn convert_cond_expr(&mut self, cond: &ast::CondExpr) -> Result<Expr> {
        let compound = patterns::extract_narrowing_guards(&cond.test);

        // Separate if-let guards from non-guard conditions
        let mut if_let_guards = Vec::new();
        let mut non_if_let_ast: Vec<&ast::Expr> = Vec::new();
        for (guard, ast_expr) in &compound.guards {
            if self.can_generate_if_let(guard) {
                if_let_guards.push(guard);
            } else {
                non_if_let_ast.push(*ast_expr);
            }
        }

        if !if_let_guards.is_empty() {
            let all_remaining: Vec<&ast::Expr> = non_if_let_ast
                .iter()
                .copied()
                .chain(compound.remaining.iter().copied())
                .collect();
            let remaining_condition = self.convert_and_combine_conditions(&all_remaining)?;

            let then_expr = self.convert_expr(&cond.cons)?;
            let else_expr = self.convert_expr(&cond.alt)?;

            // Build innermost expression: if there are remaining conditions,
            // wrap with a regular Expr::If
            let inner_expr = if let Some(remaining_cond) = remaining_condition {
                Expr::If {
                    condition: Box::new(remaining_cond),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr.clone()),
                }
            } else {
                then_expr
            };

            // Build nested IfLet from inside out
            let result = self.build_nested_expr_if_let(&if_let_guards, inner_expr, else_expr);
            return Ok(result);
        }

        // Single guard without compound
        if compound.guards.len() == 1 && compound.remaining.is_empty() {
            let guard = &compound.guards[0].0;
            if let Some((pattern, is_swap)) = self.resolve_if_let_pattern(guard) {
                let (matched_ast, unmatched_ast) = if is_swap {
                    (cond.alt.as_ref(), cond.cons.as_ref())
                } else {
                    (cond.cons.as_ref(), cond.alt.as_ref())
                };
                let matched_expr = self.convert_expr(matched_ast)?;
                let unmatched_expr = self.convert_expr(unmatched_ast)?;
                let expr_ir = Expr::Ident(guard.var_name().to_string());
                return Ok(Expr::IfLet {
                    pattern: Box::new(pattern),
                    expr: Box::new(expr_ir),
                    then_expr: Box::new(matched_expr),
                    else_expr: Box::new(unmatched_expr),
                });
            }
        }

        // Fallback: regular if expression
        let condition = self.convert_expr(&cond.test)?;
        let then_expr = self.convert_expr(&cond.cons)?;
        let else_expr = self.convert_expr(&cond.alt)?;
        Ok(Expr::If {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        })
    }

    /// Builds nested `Expr::IfLet` from a list of guards, innermost first.
    fn build_nested_expr_if_let(
        &self,
        guards: &[&patterns::NarrowingGuard],
        inner_expr: Expr,
        else_expr: Expr,
    ) -> Expr {
        let mut current = inner_expr;
        for guard in guards.iter().rev() {
            let (pattern, is_swap) = self.resolve_if_let_pattern(guard).unwrap();
            let expr_ir = Expr::Ident(guard.var_name().to_string());
            if is_swap {
                current = Expr::IfLet {
                    pattern: Box::new(pattern),
                    expr: Box::new(expr_ir),
                    then_expr: Box::new(else_expr.clone()),
                    else_expr: Box::new(current),
                };
            } else {
                current = Expr::IfLet {
                    pattern: Box::new(pattern),
                    expr: Box::new(expr_ir),
                    then_expr: Box::new(current),
                    else_expr: Box::new(else_expr.clone()),
                };
            }
        }
        current
    }

    /// Returns true when the expected type is a trait type (`Box<dyn Trait>`)
    /// and the source expression produces a concrete (non-Box) value that needs wrapping.
    fn needs_trait_box_coercion(&self, expected: &RustType, src_expr: &ast::Expr) -> bool {
        // I-387: `Box<dyn Trait>` は `StdCollection { Box, [DynTrait] }` で表現
        let trait_name = match expected {
            RustType::StdCollection {
                kind: crate::ir::StdCollectionKind::Box,
                args,
            } if args.len() == 1 && matches!(&args[0], RustType::DynTrait(_)) => {
                if let RustType::DynTrait(t) = &args[0] {
                    t.as_str()
                } else {
                    return false;
                }
            }
            RustType::Named { name, .. } if self.reg().is_trait_type(name) => name.as_str(),
            _ => return false,
        };

        let Some(expr_type) = self.get_expr_type(src_expr) else {
            return false;
        };
        if matches!(expr_type, RustType::Any) {
            return false;
        }

        // I-387: 既に `Box<dyn Trait>` にラップ済の式は再 wrap しない
        if matches!(
            &expr_type,
            RustType::StdCollection {
                kind: crate::ir::StdCollectionKind::Box,
                args,
            } if args.first().is_some_and(|a| matches!(a, RustType::DynTrait(t) if t == trait_name))
        ) {
            return false;
        }

        if let RustType::Named {
            name: expr_name,
            type_args: expr_args,
        } = expr_type
        {
            if expr_args.is_empty()
                && expr_name == trait_name
                && self.reg().is_trait_type(expr_name)
            {
                return false;
            }
        }

        true
    }
}

/// Returns true when `expr` is structurally known to produce `Option<T>`.
///
/// Used by [`Transformer::convert_expr_with_expected`] as a secondary wrap-skip
/// heuristic and by [`Transformer::convert_bin_expr`] as a secondary `is_option`
/// signal for `??` operands: the expected type is `Option<T>`, but `TypeResolver`
/// may have failed to propagate an `Option<T>` type through chained method calls
/// (common in `transpile_collecting` without builtin signatures). Checking the
/// generated IR directly catches remapped calls such as `TS .find()` which always
/// emits an `.iter().cloned().find(...)` chain terminated by `Iterator::find`
/// (returns `Option<Self::Item>` by contract).
///
/// Keep the predicate narrow: only list Rust API method names that *unconditionally*
/// return `Option<T>` (by value, not `Option<&T>`) when invoked on any receiver
/// that reaches them. Methods such as `first`/`last` return `Option<&T>` and must
/// not be added here — their expected-type unification is different.
pub(crate) fn produces_option_result(expr: &Expr) -> bool {
    // Explicit Option constructors: `Some(x)` and `None`. Emitted by
    // `convert_expr_with_expected` when wrapping a non-Option expression into
    // an `Option<T>`-expected context; also emitted by user-written `Some(_)`
    // through `BuiltinVariant` dispatch. These are definitively Option-producing.
    match expr {
        Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            ..
        } => return true,
        Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None) => return true,
        _ => {}
    }
    let Expr::MethodCall {
        object,
        method,
        args,
        ..
    } = expr
    else {
        return false;
    };
    match method.as_str() {
        // `Iterator::find(predicate)` — target of TS `.find()` remapping and any
        // direct user call that falls through to the generic iterator API.
        "find" => args.len() == 1,
        // `Vec::pop()` — zero-arg method, always returns `Option<T>` by value.
        // TS `Array.pop()` maps through the passthrough arm to this signature.
        "pop" => args.is_empty(),
        // `<obj>.get(<idx>).cloned()` — Vec index read form emitted by
        // `build_safe_index_expr` (I-138). `Vec::get` returns `Option<&T>`, and
        // `Option::cloned` lifts it to `Option<T>` by value, matching the
        // `Option<T>` expected-type contract. Arity-gate `get` to a single
        // argument to avoid matching hypothetical multi-arg `get` methods
        // from unrelated APIs. Note: `HashMap::get(k)` returns `Option<&V>` as
        // well, but the transformer emits a different IR shape for HashMap
        // indexed access, so this `.get(i).cloned()` pattern is exclusively
        // the Vec index form at the current call site.
        "cloned" => {
            args.is_empty()
                && matches!(
                    object.as_ref(),
                    Expr::MethodCall { method: m, args: a, .. } if m == "get" && a.len() == 1
                )
        }
        // `<option-producing>.flatten()` — Vec<Option<T>> index form. `.get(i).cloned()`
        // on `Vec<Option<T>>` yields `Option<Option<T>>`, which `.flatten()` collapses
        // to `Option<T>`. Only recognize when the receiver is itself an Option-producing
        // IR (recursive check), ensuring the chain terminates in a known Option pattern.
        "flatten" => args.is_empty() && produces_option_result(object),
        // `Option::or(other)` / `Option::or_else(closure)` — Rust API contract:
        // receiver of type `Option<T>` returns `Option<T>`. Emitted by
        // `build_option_or_option` for the `??` chain case (I-022). Require the
        // receiver to be Option-producing recursively, mirroring the `.flatten()`
        // pattern — this prevents misidentifying unrelated `.or()` / `.or_else()`
        // method calls on non-Option types.
        "or" => args.len() == 1 && produces_option_result(object),
        "or_else" => {
            args.len() == 1
                && matches!(&args[0], Expr::Closure { .. })
                && produces_option_result(object)
        }
        _ => false,
    }
}

#[cfg(test)]
mod produces_option_result_tests {
    use super::*;
    use crate::ir::{BinOp, ClosureBody, Param};

    fn ident(n: &str) -> Expr {
        Expr::Ident(n.to_string())
    }

    fn mc(obj: Expr, method: &str, args: Vec<Expr>) -> Expr {
        Expr::MethodCall {
            object: Box::new(obj),
            method: method.to_string(),
            args,
        }
    }

    #[test]
    fn test_find_method_call_is_option() {
        // arr.find(|x| *x > 0)
        let closure = Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: None,
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Deref(Box::new(ident("x")))),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            })),
        };
        assert!(produces_option_result(&mc(
            ident("arr"),
            "find",
            vec![closure]
        )));
    }

    #[test]
    fn test_pop_zero_args_is_option() {
        assert!(produces_option_result(&mc(ident("v"), "pop", vec![])));
    }

    #[test]
    fn test_pop_with_args_is_not_option() {
        // Not a Vec::pop() — some custom one-arg method named `pop`. Unknown return.
        assert!(!produces_option_result(&mc(
            ident("v"),
            "pop",
            vec![Expr::NumberLit(0.0)]
        )));
    }

    #[test]
    fn test_find_wrong_arity_is_not_option() {
        // `find` with zero args is not `Iterator::find`.
        assert!(!produces_option_result(&mc(ident("arr"), "find", vec![])));
    }

    #[test]
    fn test_non_method_call_is_not_option() {
        assert!(!produces_option_result(&ident("x")));
        assert!(!produces_option_result(&Expr::NumberLit(0.0)));
    }

    #[test]
    fn test_first_and_last_are_not_option() {
        // Vec::first/last return Option<&T>, not Option<T> — must NOT be flagged
        // because unifying `Option<&T>` with an expected `Option<T>` is a different
        // shape and would skip wrap incorrectly.
        assert!(!produces_option_result(&mc(ident("v"), "first", vec![])));
        assert!(!produces_option_result(&mc(ident("v"), "last", vec![])));
    }

    // ── I-138: `.get(idx).cloned()` Vec index read pattern ──

    #[test]
    fn test_get_cloned_is_option_producing() {
        // arr.get(0).cloned() — Vec index read emitted by build_safe_index_expr.
        // Returns Option<T> by value (cloned lifts &T → T), so treating as Option-producing
        // prevents outer Some wrap from convert_expr_with_expected.
        let inner = mc(ident("arr"), "get", vec![Expr::IntLit(0)]);
        let outer = mc(inner, "cloned", vec![]);
        assert!(produces_option_result(&outer));
    }

    #[test]
    fn test_cloned_alone_is_not_option() {
        // x.cloned() with non-.get() receiver — cloned() on Iterator returns
        // Iterator<Item = T>, not Option<T>. Must not be flagged.
        assert!(!produces_option_result(&mc(ident("x"), "cloned", vec![])));
    }

    #[test]
    fn test_get_without_cloned_is_not_option() {
        // map.get(key) on HashMap returns Option<&V>, not Option<V> by value.
        // build_safe_index_expr never emits bare .get() (always followed by .cloned()),
        // so bare .get() must not be flagged.
        let expr = mc(ident("map"), "get", vec![ident("key")]);
        assert!(!produces_option_result(&expr));
    }

    #[test]
    fn test_get_cloned_with_multi_args_is_not_option() {
        // .get(a, b).cloned() — hypothetical multi-arg get (doesn't exist on Vec/HashMap).
        // Guard the arity check to avoid false positives on unrelated 2-arg methods named `get`.
        let inner = mc(
            ident("custom"),
            "get",
            vec![Expr::IntLit(0), Expr::IntLit(1)],
        );
        let outer = mc(inner, "cloned", vec![]);
        assert!(!produces_option_result(&outer));
    }

    #[test]
    fn test_get_cloned_flatten_is_option_producing() {
        // arr.get(0).cloned().flatten() — Vec<Option<T>> index read form (I-138).
        // .flatten() collapses Option<Option<T>> to Option<T>; receiver must be
        // Option-producing itself (recursive check).
        let get = mc(ident("arr"), "get", vec![Expr::IntLit(0)]);
        let cloned = mc(get, "cloned", vec![]);
        let flatten = mc(cloned, "flatten", vec![]);
        assert!(produces_option_result(&flatten));
    }

    #[test]
    fn test_flatten_on_non_option_is_not_option() {
        // x.flatten() where x is a bare Ident — receiver is not Option-producing,
        // so the chain terminates outside the known pattern. Must not be flagged
        // to avoid false-positive wrap skips on unrelated `flatten` method calls.
        let flatten = mc(ident("x"), "flatten", vec![]);
        assert!(!produces_option_result(&flatten));
    }

    #[test]
    fn test_flatten_with_args_is_not_option() {
        // .flatten(arg) — zero-arg arity gate. Hypothetical overloads should not
        // be flagged.
        let get = mc(ident("arr"), "get", vec![Expr::IntLit(0)]);
        let cloned = mc(get, "cloned", vec![]);
        let flatten = mc(cloned, "flatten", vec![ident("arg")]);
        assert!(!produces_option_result(&flatten));
    }

    // ── I-022: Option constructors + `.or()` / `.or_else()` chain ──

    #[test]
    fn test_some_variant_is_option_producing() {
        // Some(x) — emitted by convert_expr_with_expected when wrapping a non-Option
        // expression into an Option<T> expected context. Definitively Option.
        let some = Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            args: vec![ident("x")],
        };
        assert!(produces_option_result(&some));
    }

    #[test]
    fn test_none_value_is_option_producing() {
        // None literal — emitted for undefined / null in Option<T> expected context.
        let none = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None);
        assert!(produces_option_result(&none));
    }

    #[test]
    fn test_ok_variant_is_not_option_producing() {
        // Ok(x) / Err(x) return Result<T, E>, not Option<T>. Must not be flagged.
        let ok = Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
            args: vec![ident("x")],
        };
        assert!(!produces_option_result(&ok));
        let err = Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
            args: vec![ident("x")],
        };
        assert!(!produces_option_result(&err));
    }

    #[test]
    fn test_or_with_option_receiver_is_option() {
        // Some(x).or(y) — receiver is Option-producing, .or returns Option<T>.
        let some = Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            args: vec![ident("x")],
        };
        let expr = mc(some, "or", vec![ident("y")]);
        assert!(produces_option_result(&expr));
    }

    #[test]
    fn test_or_with_non_option_receiver_is_not_option() {
        // Ident.or(x) — receiver is bare Ident, not Option-producing. Prevents
        // false-positive on unrelated `.or()` methods on non-Option types.
        let expr = mc(ident("obj"), "or", vec![ident("y")]);
        assert!(!produces_option_result(&expr));
    }

    #[test]
    fn test_or_wrong_arity_is_not_option() {
        // Some(x).or() — zero-arg. Rust Option::or takes one arg. Not flagged.
        let some = Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            args: vec![ident("x")],
        };
        let expr = mc(some, "or", vec![]);
        assert!(!produces_option_result(&expr));
    }

    #[test]
    fn test_or_else_with_closure_and_option_receiver_is_option() {
        // Some(x).or_else(|| y) — closure arg + Option-producing receiver.
        let some = Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            args: vec![ident("x")],
        };
        let closure = Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(ident("y"))),
        };
        let expr = mc(some, "or_else", vec![closure]);
        assert!(produces_option_result(&expr));
    }

    #[test]
    fn test_or_else_without_closure_is_not_option() {
        // Some(x).or_else(y) — non-closure arg. Not Rust Option::or_else contract.
        let some = Expr::FnCall {
            target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
            args: vec![ident("x")],
        };
        let expr = mc(some, "or_else", vec![ident("y")]);
        assert!(!produces_option_result(&expr));
    }

    #[test]
    fn test_or_else_with_non_option_receiver_is_not_option() {
        // Ident.or_else(|| y) — bare Ident receiver. Not flagged.
        let closure = Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(ident("y"))),
        };
        let expr = mc(ident("obj"), "or_else", vec![closure]);
        assert!(!produces_option_result(&expr));
    }

    #[test]
    fn test_or_else_chained_with_cloned_get_receiver_is_option() {
        // arr.get(0).cloned().or_else(|| y) — the real NC chain shape. Receiver
        // is `.get().cloned()` (Option-producing). Must be flagged.
        let get = mc(ident("arr"), "get", vec![Expr::IntLit(0)]);
        let cloned = mc(get, "cloned", vec![]);
        let closure = Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(ident("y"))),
        };
        let expr = mc(cloned, "or_else", vec![closure]);
        assert!(produces_option_result(&expr));
    }
}

#[cfg(test)]
mod tests;
