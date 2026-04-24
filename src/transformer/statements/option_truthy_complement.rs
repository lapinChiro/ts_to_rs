//! `if (!x) <body> [else <else_body>]` lowering on `Option<T>` (I-171 Layer 2).
//!
//! Extracts the consolidated-match emission paths from
//! [`super::control_flow`] so the latter stays under the file-line budget.
//! See [`Transformer::try_generate_option_truthy_complement_match`] for the
//! full lowering specification.

use anyhow::Result;
use swc_ecma_ast as ast;

use super::control_flow::ir_body_always_exits;
use crate::ir::{CallTarget, Expr, MatchArm, Pattern, PatternCtor, RustType, Stmt};
use crate::pipeline::synthetic_registry::SyntheticTypeKind;
use crate::transformer::helpers::peek_through::peek_through_type_assertions;
use crate::transformer::helpers::truthy;
use crate::transformer::Transformer;

/// Selects which `if (!x) ... [else ...]` lowering shape
/// [`Transformer::try_generate_option_truthy_complement_match`] should
/// produce.
///
/// Selection key: the `(else_body present, then-exits, else-exits)` triple
/// of the source `if`.
///
/// 1. **EarlyReturn** (T6-3, I-144 cell-i024) — `else_body` absent and
///    `then_body` always exits. Lowers to
///    `let x = match x { Some(x) if truthy => x, _ => { exit_body } };` —
///    threads the narrow into post-if scope via outer let rebinding.
///
/// 2. **EarlyReturnFromExitWithElse** (T5 deep-fix, Matrix C-5 sub-case) —
///    `else_body` present, `then_body` always exits, `else_body` does
///    *not* always exit. Post-if is reachable only via the truthy else
///    branch, so the narrow must materialise post-match. Lowers to
///    `let x = match x { Some(x) if truthy => { else_body; x }, _ =>
///    { exit_body } };` — runs the user else_body then tail-emits the
///    narrowed value to feed the outer let.
///
/// 3. **ElseBranch** (T5, Matrix C-5 general) — `else_body` present and
///    not the case (2) shape (either both branches exit, or then does
///    not exit). Lowers to
///    `match x { Some(x) if truthy => { else_body }, _ => { then_body } }`
///    — narrow scoped to the `Some(x)` arm only; post-match `x` stays
///    `Option<T>` because either post-if is unreachable (both exit) or
///    the falsy then-branch can also fall through (no useful narrow).
pub(super) enum OptionTruthyShape {
    EarlyReturn {
        exit_body: Vec<Stmt>,
    },
    EarlyReturnFromExitWithElse {
        else_body: Vec<Stmt>,
        exit_body: Vec<Stmt>,
    },
    ElseBranch {
        positive_body: Vec<Stmt>,
        wildcard_body: Vec<Stmt>,
    },
}

impl<'a> Transformer<'a> {
    /// Emits a consolidated `match` for `if (!x) <body> [else <else_body>]`
    /// on `Option<T>`. See [`OptionTruthyShape`] for the three lowering
    /// shapes and their selection criteria.
    ///
    /// The truthy-arm shape depends on `T`:
    /// - `T = F64 | String | Bool | integer`: single `Some(v) if <v truthy> => ...`.
    /// - `T = Named` (synthetic union with primitive variants): one
    ///   `Some(Enum::Variant(v)) if <v truthy> => ...` per variant.
    /// - `T = Named` (non-synthetic or variant without primitive data): single
    ///   `Some(v) => v` (all `Some` values are JS-truthy for non-primitive
    ///   payloads: objects, arrays, functions).
    ///
    /// `peek_through_type_assertions` is applied to both the outer test
    /// and the inner Bang operand so `!(x as T)`, `!(x!)`, `!(<T>x)`,
    /// `!(x as const)` — and parenthesized variants thereof — all route
    /// through this path (Matrix C-11/C-12/C-13). Bang itself is observable
    /// and must not be peeled, so the inner peek-through stops at the
    /// next non-wrapper.
    ///
    /// Returns `None` (caller falls back to predicate-form emission) when:
    /// - `test` is not `!<peeked-ident>`.
    /// - `ident` has no resolved type or is not `Option<T>`.
    /// - `else_body` is absent **and** `then_body` does not always exit
    ///   — predicate form (Matrix C-4) is the ideal shape.
    pub(super) fn try_generate_option_truthy_complement_match(
        &self,
        test: &ast::Expr,
        then_body: &[Stmt],
        else_body: Option<&[Stmt]>,
        if_stmt_position: u32,
    ) -> Result<Option<Vec<Stmt>>> {
        let ast::Expr::Unary(unary) = peek_through_type_assertions(test) else {
            return Ok(None);
        };
        if unary.op != ast::UnaryOp::Bang {
            return Ok(None);
        }
        let ast::Expr::Ident(ident) = peek_through_type_assertions(unary.arg.as_ref()) else {
            return Ok(None);
        };
        let var_name = ident.sym.to_string();
        let Some(var_ty) = self.get_type_for_var(&var_name, ident.span) else {
            return Ok(None);
        };
        let RustType::Option(inner) = var_ty else {
            return Ok(None);
        };

        // T6-2 closure-reassign suppression (parity with
        // [`Transformer::try_generate_narrowing_match`]'s Option swap branch):
        // when an inner closure reassigns `var_name`, materialising the
        // narrow via `let var = match var { Some(x) => x, _ => exit };`
        // would shadow the outer `Option<T>` binding with an immutable
        // `T`, breaking subsequent `var = None` (or any `Option<T>`-shaped
        // reassignment) in the closure body. Returning `None` here lets
        // the caller fall through to Layer 1's predicate form
        // (`if <falsy(var)> { ... }`), which leaves `var` bound to the
        // outer `Option<T>` so closure-reassign continues to compile.
        if self.is_var_closure_reassigned(&var_name, if_stmt_position) {
            return Ok(None);
        }

        let then_exits = ir_body_always_exits(then_body);
        let else_exits = else_body.is_some_and(ir_body_always_exits);
        let shape = match (else_body, then_exits, else_exits) {
            (None, true, _) => OptionTruthyShape::EarlyReturn {
                exit_body: then_body.to_vec(),
            },
            (Some(else_stmts), true, false) => OptionTruthyShape::EarlyReturnFromExitWithElse {
                else_body: else_stmts.to_vec(),
                exit_body: then_body.to_vec(),
            },
            (Some(else_stmts), _, _) => OptionTruthyShape::ElseBranch {
                positive_body: else_stmts.to_vec(),
                wildcard_body: then_body.to_vec(),
            },
            (None, false, _) => return Ok(None),
        };

        let arms = self.build_option_truthy_match_arms(&var_name, inner, &shape)?;
        let Some(arms) = arms else {
            return Ok(None);
        };

        let expr = Expr::Match {
            expr: Box::new(Expr::Ident(var_name.clone())),
            arms,
        };
        Ok(Some(match shape {
            OptionTruthyShape::EarlyReturn { .. }
            | OptionTruthyShape::EarlyReturnFromExitWithElse { .. } => vec![Stmt::Let {
                mutable: false,
                name: var_name,
                ty: None,
                init: Some(expr),
            }],
            OptionTruthyShape::ElseBranch { .. } => {
                let Expr::Match { expr, arms } = expr else {
                    unreachable!("just constructed an Expr::Match above");
                };
                vec![Stmt::Match { expr: *expr, arms }]
            }
        }))
    }

    /// Builds the per-`inner`-type arms for the consolidated match.
    ///
    /// Three inner-type families are recognised, each with a distinct truthy
    /// arm shape:
    ///
    /// 1. **Primitive** (`F64` / `String` / `Bool` / `Primitive(int)`):
    ///    single `Some(x) if <truthy(x)> => <body>` arm. Inner value can be
    ///    JS-falsy (0 / "" / NaN / false), so the truthy guard filters
    ///    falsy `Some` values into the wildcard arm.
    ///
    /// 2. **Synthetic union enum** (`Named` non-empty registered as
    ///    [`SyntheticTypeKind::UnionEnum`]): per-variant
    ///    `Some(Enum::V(__ts_union_inner)) [if truthy] => <body>` arms,
    ///    one per variant. Each variant's truthiness is checked against
    ///    its own payload (primitive variants get a guard, object-payload
    ///    variants emit guard-less).
    ///
    /// 3. **Always-truthy** (`Named other` non-synthetic / `Vec` / `Fn` /
    ///    `Tuple` / `StdCollection` / `DynTrait` / `Ref`): single
    ///    `Some(x) => <body>` arm WITHOUT a truthy guard — JS treats every
    ///    object reference as truthy, so any `Some` is unconditionally
    ///    truthy. The narrow materialises directly via the `Some(x)` shadow.
    ///
    /// Body composition is performed inside each arm to avoid allocating
    /// bodies that the chosen arm shape does not consume.
    ///
    /// Returns `None` only when the inner type is fundamentally unsupported
    /// (e.g. `Any` / `TypeVar` / `Never` / `Unit` — handled by separate
    /// PRDs); the caller falls back to predicate-form emission for those.
    fn build_option_truthy_match_arms(
        &self,
        var_name: &str,
        inner: &RustType,
        shape: &OptionTruthyShape,
    ) -> Result<Option<Vec<MatchArm>>> {
        let positive_arms: Option<Vec<MatchArm>> = match inner {
            RustType::F64 | RustType::String | RustType::Bool | RustType::Primitive(_) => {
                let guard = truthy::truthy_predicate(var_name, inner);
                Some(vec![MatchArm {
                    patterns: vec![Pattern::some_binding(var_name)],
                    guard,
                    body: build_some_arm_body(var_name, shape),
                }])
            }
            RustType::Named { name, type_args } if type_args.is_empty() => {
                // Try synthetic-union per-variant emission first; if the
                // Named is not a registered synthetic union, fall through
                // to the always-truthy single-arm path below (interface /
                // class / non-synthetic enum types are JS-truthy when
                // `Some`, identical to Vec / Fn / etc. semantics).
                if let Some(arms) = self.build_union_variant_truthy_arms(name, var_name, shape) {
                    Some(arms)
                } else {
                    Some(vec![MatchArm {
                        patterns: vec![Pattern::some_binding(var_name)],
                        guard: None,
                        body: build_some_arm_body(var_name, shape),
                    }])
                }
            }
            inner if truthy::is_always_truthy_type(inner, self.synthetic) => {
                // Vec / Fn / Tuple / StdCollection / DynTrait / Ref — JS
                // always-truthy, no payload-truthy filter needed.
                Some(vec![MatchArm {
                    patterns: vec![Pattern::some_binding(var_name)],
                    guard: None,
                    body: build_some_arm_body(var_name, shape),
                }])
            }
            _ => None,
        };

        let Some(mut arms) = positive_arms else {
            return Ok(None);
        };
        let wildcard_body: Vec<Stmt> = match shape {
            OptionTruthyShape::EarlyReturn { exit_body }
            | OptionTruthyShape::EarlyReturnFromExitWithElse { exit_body, .. } => exit_body.clone(),
            OptionTruthyShape::ElseBranch { wildcard_body, .. } => wildcard_body.clone(),
        };
        arms.push(MatchArm {
            patterns: vec![Pattern::Wildcard],
            guard: None,
            body: wildcard_body,
        });
        Ok(Some(arms))
    }

    /// For a synthetic union enum `enum_name`, emits one arm per variant.
    /// `var_name` is the outer-scope variable so each arm body can shadow
    /// it (`let <var_name> = Enum::Variant(v); ...`) before running the
    /// user-written else_body. Arm-local `__ts_union_inner` holds the
    /// variant payload during the truthy-guard evaluation.
    fn build_union_variant_truthy_arms(
        &self,
        enum_name: &str,
        var_name: &str,
        shape: &OptionTruthyShape,
    ) -> Option<Vec<MatchArm>> {
        let def = self.synthetic.get(enum_name)?;
        if def.kind != SyntheticTypeKind::UnionEnum {
            return None;
        }
        let crate::ir::Item::Enum { variants, .. } = &def.item else {
            return None;
        };
        const INNER_BIND: &str = "__ts_union_inner";
        let enum_ref = crate::ir::UserTypeRef::new(enum_name.to_string());

        let mut arms = Vec::with_capacity(variants.len());
        for variant in variants {
            let variant_ty = variant.data.as_ref()?;
            let guard = if is_supported_variant_truthy_type(variant_ty) {
                truthy::truthy_predicate(INNER_BIND, variant_ty)
            } else {
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
            let variant_ctor = Expr::FnCall {
                target: CallTarget::UserEnumVariantCtor {
                    enum_ty: enum_ref.clone(),
                    variant: variant.name.clone(),
                },
                args: vec![Expr::Ident(INNER_BIND.to_string())],
            };
            let body = match shape {
                OptionTruthyShape::EarlyReturn { .. } => {
                    vec![Stmt::TailExpr(variant_ctor)]
                }
                OptionTruthyShape::EarlyReturnFromExitWithElse { else_body, .. } => {
                    let mut stmts = Vec::with_capacity(else_body.len() + 2);
                    stmts.push(Stmt::Let {
                        mutable: false,
                        name: var_name.to_string(),
                        ty: None,
                        init: Some(variant_ctor),
                    });
                    stmts.extend(else_body.iter().cloned());
                    stmts.push(Stmt::TailExpr(Expr::Ident(var_name.to_string())));
                    stmts
                }
                OptionTruthyShape::ElseBranch { positive_body, .. } => {
                    let mut stmts = Vec::with_capacity(positive_body.len() + 1);
                    stmts.push(Stmt::Let {
                        mutable: false,
                        name: var_name.to_string(),
                        ty: None,
                        init: Some(variant_ctor),
                    });
                    stmts.extend(positive_body.iter().cloned());
                    stmts
                }
            };
            arms.push(MatchArm {
                patterns: vec![pattern],
                guard,
                body,
            });
        }
        Some(arms)
    }
}

/// Whether a synthetic-union variant payload requires a JS-truthy guard
/// in the consolidated match. Primitives (`F64` / `String` / `Bool` /
/// `Primitive(int)`) can be falsy at runtime; non-primitive payloads
/// (struct, Vec, Tuple, Fn, ...) are always JS-truthy and emit guard-less.
fn is_supported_variant_truthy_type(ty: &RustType) -> bool {
    matches!(
        ty,
        RustType::F64 | RustType::String | RustType::Bool | RustType::Primitive(_)
    )
}

/// Builds the `Some(x)`-arm body for the three lowering shapes.
///
/// - **EarlyReturn**: just `[TailExpr(Ident(var_name))]` — outer let
///   rebinds to the narrow value.
/// - **EarlyReturnFromExitWithElse**: `[<else_body>..., TailExpr(Ident(var_name))]`
///   — runs user else_body then tail-emits narrow value.
/// - **ElseBranch**: just the user-written `positive_body` verbatim —
///   narrow scoped to arm via `Some(x)` shadow.
///
/// Shared by the primitive-arm and always-truthy-arm paths in
/// [`Transformer::build_option_truthy_match_arms`]; the synthetic-union
/// path embeds variant-reconstruction logic and computes its own bodies
/// in [`Transformer::build_union_variant_truthy_arms`].
fn build_some_arm_body(var_name: &str, shape: &OptionTruthyShape) -> Vec<Stmt> {
    match shape {
        OptionTruthyShape::EarlyReturn { .. } => {
            vec![Stmt::TailExpr(Expr::Ident(var_name.to_string()))]
        }
        OptionTruthyShape::EarlyReturnFromExitWithElse { else_body, .. } => {
            let mut stmts = else_body.clone();
            stmts.push(Stmt::TailExpr(Expr::Ident(var_name.to_string())));
            stmts
        }
        OptionTruthyShape::ElseBranch { positive_body, .. } => positive_body.clone(),
    }
}
