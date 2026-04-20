//! Structural conversion of `x ??= d` (TS `NullishAssign`) to IR (I-142,
//! extended at I-144 T6-1 for CFG-analyzer-driven emission dispatch).
//!
//! TS `x ??= d` is a compound operator with two observable effects:
//!
//! 1. **Assignment**: when `x` is `null`/`undefined`, `x` is set to `d`.
//! 2. **Narrowing**: after evaluation, `x` is narrowed from `T | null | undefined`
//!    to `T` in the enclosing flow.
//!
//! Mapping this to a single Rust pattern is impossible because the narrowing
//! effect must be preserved **only when** the narrow survives subsequent
//! control flow. This module therefore dispatches between two emissions
//! based on the [`EmissionHint`] produced by
//! [`crate::pipeline::narrowing_analyzer::analyze_function`] (see I-144 T6-1):
//!
//! - **E1 shadow-let** (`let x = x.unwrap_or[_else](|| d);`): selected when
//!   the narrow survives — no later reset, no null reassign, no closure
//!   reassign, no loop-boundary rebind. Rebinds `x` to `T` so return
//!   expressions and subsequent arithmetic can treat `x` as unwrapped.
//!   A follow-up fusion pass collapses
//!   `let x = init; let x = x.unwrap_or[_else](|| d);` to a single
//!   `let x = init.unwrap_or[_else](|| d);` where safe.
//! - **E2a `get_or_insert_with`** (`x.get_or_insert_with(|| d);`): selected
//!   when the analyzer detects a narrow-invalidating sibling ([`ResetCause`]
//!   `invalidates_narrow() == true`). Keeps `x` as `Option<T>` so the later
//!   `x = None` / closure reassign / loop-boundary rebind typechecks.
//!
//! Absent analyzer output (fallback) the default is E1 shadow-let, matching
//! the pre-analyzer emission.
//!
//! Expression-context `??=` (inside call args / return value / conditions) is
//! handled separately in [`convert_assign_expr`] via `get_or_insert_with`
//! with deref-or-clone wrapping based on `is_copy_type`; it does **not**
//! consult the analyzer because the expression value must always be an
//! unwrapped `T`.
//!
//! [`EmissionHint`]: crate::pipeline::narrowing_analyzer::EmissionHint
//! [`ResetCause`]: crate::pipeline::narrowing_analyzer::ResetCause

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType, Stmt};
use crate::pipeline::narrowing_analyzer::EmissionHint;
use crate::transformer::{
    build_option_get_or_insert_with, build_option_unwrap_with_default, Transformer,
    UnsupportedSyntaxError,
};

/// Emission strategy for `x ??= d` derived from the LHS type.
///
/// The strategy is the single source of truth for `??=` dispatch — both the
/// statement-context path ([`Transformer::try_convert_nullish_assign_stmt`])
/// and the expression-context path (`convert_assign_expr::NullishAssign` arm)
/// select their emission through [`pick_strategy`] so that the Problem Space
/// matrix is encoded in exactly one place.
///
/// See `backlog/I-142-nullish-assign-shadow-let.md` for the cell-by-cell
/// mapping from LHS type to strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NullishAssignStrategy {
    /// Cells #1–3, #7, #8, #13: `Option<T>` — shadow-let (stmt) /
    /// `get_or_insert_with` (expr).
    ShadowLet,
    /// Cells #4, #6, #10: non-nullable `T` — `??=` is dead code at runtime;
    /// stmt emits nothing, expr emits the target identity (with `.clone()`
    /// when `T: !Copy`).
    Identity,
    /// Cells #5, #9: `Any` (= `serde_json::Value`) — requires runtime null
    /// check + RHS coercion to `serde_json::Value`. The RHS coercion belongs
    /// to the **I-050 Any coercion umbrella** PRD; until I-050 lands, surface
    /// these cells as unsupported rather than emitting silently-broken Rust.
    BlockedByI050,
}

/// Picks the `??=` emission strategy from the LHS type.
///
/// Pure function — no side effects, no Transformer state. This keeps the
/// strategy table auditable in one place and lets both the stmt and expr
/// contexts dispatch identically.
///
/// The match is **exhaustive** by design (no `_` fallback): adding a new
/// `RustType` variant forces a compile error here, ensuring the strategy table
/// stays auditable. See `report/i142-step3-inv3-pick-strategy-variants.md` for
/// the per-variant semantic analysis.
pub(crate) fn pick_strategy(lhs_type: &RustType) -> NullishAssignStrategy {
    use NullishAssignStrategy::{BlockedByI050, Identity, ShadowLet};
    match lhs_type {
        // Only nullable variant — shadow-let (stmt) / get_or_insert_with (expr).
        RustType::Option(_) => ShadowLet,
        // Runtime null check + Value coercion requires the I-050 umbrella.
        RustType::Any => BlockedByI050,
        // All remaining variants are non-nullable in Rust; `??=` is dead code
        // at runtime for these. Listed exhaustively so a new variant forces
        // re-evaluation of its `??=` semantics.
        RustType::Unit
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Vec(_)
        | RustType::Fn { .. }
        | RustType::Result { .. }
        | RustType::Tuple(_)
        | RustType::Never
        | RustType::Named { .. }
        | RustType::TypeVar { .. }
        | RustType::Primitive(_)
        | RustType::StdCollection { .. }
        | RustType::Ref(_)
        | RustType::DynTrait(_)
        | RustType::QSelf { .. } => Identity,
    }
}

impl<'a> Transformer<'a> {
    /// Intercepts `x ??= d;` at statement level and produces a shadow-let
    /// rewrite that preserves TS's narrowing semantics.
    ///
    /// Returns:
    /// - `Ok(Some(stmts))` when the statement was handled structurally
    ///   (subset: `Ident` LHS with a resolved type).
    /// - `Ok(None)` when the expression is not a `NullishAssign` statement or
    ///   the LHS is a shape (member / index) reserved for a separate PRD —
    ///   the caller falls through to the normal expression-conversion path.
    /// - `Err(_)` when the LHS type cannot be resolved or the strategy is
    ///   blocked by a dependent PRD (I-050 for `Any`) — surfaced through
    ///   `transform_module_collecting` as an unsupported-syntax entry.
    pub(crate) fn try_convert_nullish_assign_stmt(
        &mut self,
        expr: &ast::Expr,
    ) -> Result<Option<Vec<Stmt>>> {
        let ast::Expr::Assign(assign) = expr else {
            return Ok(None);
        };
        if assign.op != ast::AssignOp::NullishAssign {
            return Ok(None);
        }
        // Dispatch on LHS shape: Ident uses shadow-let, Member uses direct
        // field/index mutation (I-142-b/c).
        match &assign.left {
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
                let name = ident.id.sym.to_string();
                let lhs_type = self
                    .get_type_for_var(&name, ident.id.span)
                    .cloned()
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "nullish-assign on unresolved type",
                            assign.span,
                        )
                    })?;

                let stmts = match pick_strategy(&lhs_type) {
                    NullishAssignStrategy::ShadowLet => {
                        let right_ir = self.convert_expr(&assign.right)?;
                        // I-144 T6-1: dispatch on narrowing analyzer's hint.
                        // `GetOrInsertWith` preserves the outer `Option<T>` so a
                        // later narrow-invalidating mutation (null reassign, loop
                        // boundary, closure reassign) still typechecks.
                        // Absent analyzer output (pre-analyzer paths or when the
                        // stmt is not a same-scope `??=`) the default is E1
                        // shadow-let, which matches the pre-analyzer emission.
                        match self.get_emission_hint(ident.id.span.lo.0) {
                            Some(EmissionHint::GetOrInsertWith) => {
                                vec![Stmt::Expr(build_option_get_or_insert_with(
                                    Expr::Ident(name),
                                    right_ir,
                                ))]
                            }
                            _ => vec![Stmt::Let {
                                mutable: false,
                                name: name.clone(),
                                ty: None,
                                init: Some(build_option_unwrap_with_default(
                                    Expr::Ident(name),
                                    right_ir,
                                )),
                            }],
                        }
                    }
                    NullishAssignStrategy::Identity => vec![],
                    NullishAssignStrategy::BlockedByI050 => {
                        return Err(UnsupportedSyntaxError::new(
                            "nullish-assign on Any LHS (I-050 Any coercion umbrella)",
                            assign.span,
                        )
                        .into());
                    }
                };
                Ok(Some(stmts))
            }
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => {
                // I-142-b/c: FieldAccess / Index LHS.
                // Resolve field type via TypeResolver (recorded at member span).
                let member_expr = ast::Expr::Member(member.clone());
                let lhs_type = self.get_expr_type(&member_expr).cloned().ok_or_else(|| {
                    UnsupportedSyntaxError::new(
                        "nullish-assign on unresolved member type",
                        assign.span,
                    )
                })?;
                let strategy = pick_strategy(&lhs_type);
                match strategy {
                    NullishAssignStrategy::ShadowLet => {
                        let target = self.convert_member_expr_for_write(member)?;
                        let right_ir = self.convert_expr(&assign.right)?;

                        // Index on HashMap → entry().or_insert_with(|| d)
                        let stmts = if let Expr::Index {
                            object: ref container,
                            index: ref key,
                        } = target
                        {
                            let closure = Expr::Closure {
                                params: vec![],
                                return_type: None,
                                body: crate::ir::ClosureBody::Expr(Box::new(right_ir)),
                            };
                            let key_for_entry = Expr::MethodCall {
                                object: Box::new(*key.clone()),
                                method: "clone".to_string(),
                                args: vec![],
                            };
                            vec![Stmt::Expr(Expr::MethodCall {
                                object: Box::new(Expr::MethodCall {
                                    object: container.clone(),
                                    method: "entry".to_string(),
                                    args: vec![key_for_entry],
                                }),
                                method: "or_insert_with".to_string(),
                                args: vec![closure],
                            })]
                        } else {
                            // FieldAccess → if target.is_none() { target = Some(d); }
                            vec![Stmt::If {
                                condition: Expr::MethodCall {
                                    object: Box::new(target.clone()),
                                    method: "is_none".to_string(),
                                    args: vec![],
                                },
                                then_body: vec![Stmt::Expr(Expr::Assign {
                                    target: Box::new(target),
                                    value: Box::new(Expr::FnCall {
                                        target: crate::ir::CallTarget::BuiltinVariant(
                                            crate::ir::BuiltinVariant::Some,
                                        ),
                                        args: vec![right_ir],
                                    }),
                                })],
                                else_body: None,
                            }]
                        };
                        Ok(Some(stmts))
                    }
                    NullishAssignStrategy::Identity => Ok(Some(vec![])),
                    NullishAssignStrategy::BlockedByI050 => Err(UnsupportedSyntaxError::new(
                        "nullish-assign on Any LHS (I-050 Any coercion umbrella)",
                        assign.span,
                    )
                    .into()),
                }
            }
            // Other targets (SuperProp, etc.) — fall through to expression path
            _ => Ok(None),
        }
    }
}

/// Fuses consecutive `let x = init; let x = x.unwrap_or[_else](|| d);` pairs
/// into a single `let x = init.unwrap_or[_else](|| d);`.
///
/// Cosmetic-only: the unfused form compiles and executes identically. Fusion
/// avoids the redundant-binding warning and reads more naturally when the
/// initializer and the `??=` target are adjacent.
///
/// Safety conditions (all must hold, else the pair is left unfused):
///
/// 1. Both statements are immutable `let`s with the **same name**.
/// 2. The second `let`'s init is a single-arg `MethodCall` whose object is
///    `Ident(<name>)` and whose method is `unwrap_or` or `unwrap_or_else`.
///    This is the exact shape emitted by [`try_convert_nullish_assign_stmt`]
///    for `Option<T>` — other shapes indicate unrelated code and must not be
///    folded.
/// 3. The first `let`'s init is **not itself** the same shadow-let shape
///    (object `Ident(<name>)` + `unwrap_or[_else]`). This guard prevents
///    fusing chained `??=` statements (degenerate TS like `x ??= 0; x ??= 5;`)
///    into `x.unwrap_or(0).unwrap_or(5)`, which would not type-check because
///    the inner `unwrap_or` returns `T`, not `Option<T>`.
///
/// On fuse, the type annotation is dropped (`ty: None`) — Rust infers the
/// unwrapped type from the RHS, which differs from the outer `let`'s annotation
/// (e.g., `Option<T>` → `T`).
pub(super) fn fuse_nullish_assign_shadow_lets(stmts: &mut Vec<Stmt>) {
    let mut i = 0;
    while i + 1 < stmts.len() {
        if is_fusable_pair(&stmts[i], &stmts[i + 1]) {
            fuse_pair_at(stmts, i);
            // Skip past the fused let to avoid re-fusing chained shadow-lets
            // (see safety condition 3).
            i += 1;
        } else {
            i += 1;
        }
    }
}

/// Checks the three safety conditions above.
fn is_fusable_pair(first: &Stmt, second: &Stmt) -> bool {
    let Stmt::Let {
        mutable: false,
        name: shadow_name,
        init: Some(shadow_init),
        ..
    } = second
    else {
        return false;
    };
    if !is_shadow_let_output(shadow_init, shadow_name) {
        return false;
    }
    let Stmt::Let {
        mutable: false,
        name: first_name,
        init: Some(first_init),
        ..
    } = first
    else {
        return false;
    };
    first_name == shadow_name && !is_shadow_let_output(first_init, first_name)
}

/// Returns true iff `init` has the exact shape emitted for a shadow-let of `name`:
/// `Ident(name).unwrap_or(arg)` or `Ident(name).unwrap_or_else(closure)`.
fn is_shadow_let_output(init: &Expr, name: &str) -> bool {
    let Expr::MethodCall {
        object,
        method,
        args,
    } = init
    else {
        return false;
    };
    if !matches!(method.as_str(), "unwrap_or" | "unwrap_or_else") {
        return false;
    }
    if args.len() != 1 {
        return false;
    }
    matches!(object.as_ref(), Expr::Ident(obj_name) if obj_name == name)
}

/// Replaces the pair at `(i, i+1)` with a single fused `let`. Callers must have
/// already verified [`is_fusable_pair`].
fn fuse_pair_at(stmts: &mut Vec<Stmt>, i: usize) {
    // Consume the first `let` and its init.
    let first = stmts.remove(i);
    let Stmt::Let {
        init: Some(first_init),
        ..
    } = first
    else {
        unreachable!("is_fusable_pair guaranteed Stmt::Let with Some(init)")
    };
    // Rewrite the shadow-let at (now) position `i`: replace its method-call
    // object with the consumed first-let's init, producing `init.unwrap_or*(d)`.
    // Drop the type annotation — the unwrapped type is inferable from the RHS
    // and differs from whatever the outer `let` carried (usually `Option<T>`).
    let Stmt::Let {
        ty: shadow_ty,
        init: Some(Expr::MethodCall { object, .. }),
        ..
    } = &mut stmts[i]
    else {
        unreachable!("is_fusable_pair guaranteed shadow-let shape")
    };
    **object = first_init;
    *shadow_ty = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{PrimitiveIntKind, StdCollectionKind, TraitRef};
    use crate::ir::ClosureBody;
    use crate::ir::RustType;

    // -------------------------------------------------------------------------
    // D-4: `pick_strategy` table tests — `RustType` variant 網羅
    //
    // `pick_strategy` is the single source of truth for `??=` dispatch (encoded
    // as an exhaustive match). These tests lock in the strategy for every
    // `RustType` variant and its sub-variants (PrimitiveIntKind,
    // StdCollectionKind). When a new `RustType` variant is added, the match
    // itself catches the gap at compile time; these tests complement that by
    // locking in the semantic expectation (Identity vs ShadowLet vs
    // BlockedByI050).
    //
    // See `report/i142-step3-inv3-pick-strategy-variants.md` for the per-variant
    // semantic analysis that drives these expectations.
    // -------------------------------------------------------------------------

    #[test]
    fn pick_strategy_option_maps_to_shadow_let() {
        assert_eq!(
            pick_strategy(&RustType::Option(Box::new(RustType::F64))),
            NullishAssignStrategy::ShadowLet
        );
        // Nested Option<Option<T>> must still pick ShadowLet; only the outer
        // layer drives the strategy (the inner layer becomes the narrowed type
        // after `unwrap_or*`).
        assert_eq!(
            pick_strategy(&RustType::Option(Box::new(RustType::Option(Box::new(
                RustType::String
            ))))),
            NullishAssignStrategy::ShadowLet
        );
        // Option<String> (non-Copy inner) is the same strategy; the Copy/!Copy
        // distinction is applied downstream in the emission layer, not here.
        assert_eq!(
            pick_strategy(&RustType::Option(Box::new(RustType::String))),
            NullishAssignStrategy::ShadowLet
        );
    }

    #[test]
    fn pick_strategy_any_maps_to_blocked_by_i050() {
        // Cells #5 / #9 — blocked until the I-050 Any coercion umbrella lands.
        assert_eq!(
            pick_strategy(&RustType::Any),
            NullishAssignStrategy::BlockedByI050
        );
    }

    #[test]
    fn pick_strategy_primitive_kinds_all_map_to_identity() {
        // Every `PrimitiveIntKind` variant represents a non-nullable Rust
        // integer / f32 — `??=` is dead code at runtime. Enumerating every
        // kind prevents a new `PrimitiveIntKind` variant from silently
        // inheriting `Identity` without semantic review.
        for kind in [
            PrimitiveIntKind::Usize,
            PrimitiveIntKind::Isize,
            PrimitiveIntKind::I8,
            PrimitiveIntKind::I16,
            PrimitiveIntKind::I32,
            PrimitiveIntKind::I64,
            PrimitiveIntKind::I128,
            PrimitiveIntKind::U8,
            PrimitiveIntKind::U16,
            PrimitiveIntKind::U32,
            PrimitiveIntKind::U64,
            PrimitiveIntKind::U128,
            PrimitiveIntKind::F32,
        ] {
            assert_eq!(
                pick_strategy(&RustType::Primitive(kind)),
                NullishAssignStrategy::Identity,
                "Primitive({kind:?}) must map to Identity"
            );
        }
    }

    #[test]
    fn pick_strategy_std_collection_kinds_all_map_to_identity() {
        // Every `StdCollectionKind` variant is non-nullable (Box, HashMap,
        // RefCell, Mutex, ...). Enumerate all kinds so future additions force
        // re-evaluation here.
        for kind in [
            StdCollectionKind::Box,
            StdCollectionKind::HashMap,
            StdCollectionKind::BTreeMap,
            StdCollectionKind::HashSet,
            StdCollectionKind::BTreeSet,
            StdCollectionKind::VecDeque,
            StdCollectionKind::Rc,
            StdCollectionKind::Arc,
            StdCollectionKind::Mutex,
            StdCollectionKind::RwLock,
            StdCollectionKind::RefCell,
            StdCollectionKind::Cell,
        ] {
            assert_eq!(
                pick_strategy(&RustType::StdCollection { kind, args: vec![] }),
                NullishAssignStrategy::Identity,
                "StdCollection({kind:?}) must map to Identity"
            );
        }
    }

    #[test]
    fn pick_strategy_all_non_nullable_main_variants_map_to_identity() {
        // Main `RustType` variants (except Option / Any) must all map to
        // Identity. If a new variant is added to RustType, the exhaustive
        // match in `pick_strategy` forces a compile error — this test then
        // locks in the expected strategy.
        for (name, ty) in [
            ("Unit", RustType::Unit),
            ("String", RustType::String),
            ("F64", RustType::F64),
            ("Bool", RustType::Bool),
            ("Vec<f64>", RustType::Vec(Box::new(RustType::F64))),
            (
                "Fn() -> ()",
                RustType::Fn {
                    params: vec![],
                    return_type: Box::new(RustType::Unit),
                },
            ),
            (
                "Result<f64, String>",
                RustType::Result {
                    ok: Box::new(RustType::F64),
                    err: Box::new(RustType::String),
                },
            ),
            (
                "Tuple<(f64, bool)>",
                RustType::Tuple(vec![RustType::F64, RustType::Bool]),
            ),
            ("Never", RustType::Never),
            (
                "Named Foo",
                RustType::Named {
                    name: "Foo".into(),
                    type_args: vec![],
                },
            ),
            ("TypeVar T", RustType::TypeVar { name: "T".into() }),
            ("Ref<f64>", RustType::Ref(Box::new(RustType::F64))),
            ("DynTrait Greeter", RustType::DynTrait("Greeter".into())),
            (
                "QSelf <T as Trait>::Item",
                RustType::QSelf {
                    qself: Box::new(RustType::TypeVar { name: "T".into() }),
                    trait_ref: TraitRef {
                        name: "Trait".into(),
                        type_args: vec![],
                    },
                    item: "Item".into(),
                },
            ),
        ] {
            assert_eq!(
                pick_strategy(&ty),
                NullishAssignStrategy::Identity,
                "{name} must map to Identity"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Existing: `fuse_nullish_assign_shadow_lets` unit tests.
    // -------------------------------------------------------------------------

    // Helper: build a shadow-let `let <name> = <name>.unwrap_or(default);`
    fn shadow_let_unwrap_or(name: &str, default: Expr) -> Stmt {
        Stmt::Let {
            mutable: false,
            name: name.to_string(),
            ty: None,
            init: Some(Expr::MethodCall {
                object: Box::new(Expr::Ident(name.to_string())),
                method: "unwrap_or".to_string(),
                args: vec![default],
            }),
        }
    }

    // Helper: build a shadow-let `let <name> = <name>.unwrap_or_else(|| body);`
    fn shadow_let_unwrap_or_else(name: &str, body: Expr) -> Stmt {
        Stmt::Let {
            mutable: false,
            name: name.to_string(),
            ty: None,
            init: Some(Expr::MethodCall {
                object: Box::new(Expr::Ident(name.to_string())),
                method: "unwrap_or_else".to_string(),
                args: vec![Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(body)),
                }],
            }),
        }
    }

    fn plain_let(name: &str, init: Expr) -> Stmt {
        Stmt::Let {
            mutable: false,
            name: name.to_string(),
            ty: None,
            init: Some(init),
        }
    }

    #[test]
    fn fuse_folds_plain_let_followed_by_shadow_let() {
        // let val = x; let val = val.unwrap_or(0);
        // → let val = x.unwrap_or(0);
        let mut stmts = vec![
            plain_let("val", Expr::Ident("x".to_string())),
            shadow_let_unwrap_or("val", Expr::NumberLit(0.0)),
        ];
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(stmts.len(), 1);
        assert_eq!(
            stmts[0],
            Stmt::Let {
                mutable: false,
                name: "val".to_string(),
                ty: None,
                init: Some(Expr::MethodCall {
                    object: Box::new(Expr::Ident("x".to_string())),
                    method: "unwrap_or".to_string(),
                    args: vec![Expr::NumberLit(0.0)],
                }),
            }
        );
    }

    #[test]
    fn fuse_folds_unwrap_or_else_closure_variant() {
        // let name = n; let name = name.unwrap_or_else(|| "def".to_string());
        let mut stmts = vec![
            plain_let("name", Expr::Ident("n".to_string())),
            shadow_let_unwrap_or_else("name", Expr::StringLit("def".to_string())),
        ];
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Let {
                init: Some(Expr::MethodCall { object, method, .. }),
                ..
            } => {
                assert_eq!(method, "unwrap_or_else");
                assert!(matches!(object.as_ref(), Expr::Ident(n) if n == "n"));
            }
            other => panic!("expected fused Let, got {other:?}"),
        }
    }

    #[test]
    fn fuse_drops_type_annotation() {
        // let val: Option<f64> = x; let val = val.unwrap_or(0);
        // → let val = x.unwrap_or(0); (ty: None so Rust infers f64)
        let mut stmts = vec![
            Stmt::Let {
                mutable: false,
                name: "val".to_string(),
                ty: Some(RustType::Option(Box::new(RustType::F64))),
                init: Some(Expr::Ident("x".to_string())),
            },
            shadow_let_unwrap_or("val", Expr::NumberLit(0.0)),
        ];
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(stmts.len(), 1);
        if let Stmt::Let { ty, .. } = &stmts[0] {
            assert!(
                ty.is_none(),
                "fusion must drop outer Option<T> ty annotation"
            );
        } else {
            panic!("expected Stmt::Let");
        }
    }

    #[test]
    fn fuse_skips_when_names_differ() {
        let mut stmts = vec![
            plain_let("a", Expr::Ident("x".to_string())),
            shadow_let_unwrap_or("b", Expr::NumberLit(0.0)),
        ];
        let before = stmts.clone();
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(stmts, before, "different-name lets must not fuse");
    }

    #[test]
    fn fuse_skips_when_intervening_stmt() {
        // let val = x; x = 5; let val = val.unwrap_or(0);
        // → unchanged (non-consecutive)
        let mut stmts = vec![
            plain_let("val", Expr::Ident("x".to_string())),
            Stmt::Expr(Expr::Assign {
                target: Box::new(Expr::Ident("x".to_string())),
                value: Box::new(Expr::NumberLit(5.0)),
            }),
            shadow_let_unwrap_or("val", Expr::NumberLit(0.0)),
        ];
        let before = stmts.clone();
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(stmts, before, "intervening statement must block fusion");
    }

    #[test]
    fn fuse_skips_when_first_is_shadow_let_output() {
        // let val = val.unwrap_or(0); let val = val.unwrap_or(5);
        // The first let is already a shadow-let output. Fusing would produce
        // `val.unwrap_or(0).unwrap_or(5)`, which does not type-check because
        // `unwrap_or(0)` returns `T`, not `Option<T>`.
        let mut stmts = vec![
            shadow_let_unwrap_or("val", Expr::NumberLit(0.0)),
            shadow_let_unwrap_or("val", Expr::NumberLit(5.0)),
        ];
        let before = stmts.clone();
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(
            stmts, before,
            "chained shadow-lets must not fuse (would produce ill-typed .unwrap_or.unwrap_or chain)"
        );
    }

    #[test]
    fn fuse_does_not_touch_mutable_first_let() {
        // let mut val = x; let val = val.unwrap_or(0);
        // The first is `let mut`, distinct from a plain `let`; safety requires
        // the consumed let to match shadow-let expectations.
        let mut stmts = vec![
            Stmt::Let {
                mutable: true,
                name: "val".to_string(),
                ty: None,
                init: Some(Expr::Ident("x".to_string())),
            },
            shadow_let_unwrap_or("val", Expr::NumberLit(0.0)),
        ];
        let before = stmts.clone();
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(stmts, before, "mutable first let must not be fused");
    }

    #[test]
    fn fuse_folds_at_most_one_pair_per_variable_chain() {
        // let val = x; let val = val.unwrap_or(0); let other = val.clone();
        // Only the first pair fuses; `other` is unrelated.
        let mut stmts = vec![
            plain_let("val", Expr::Ident("x".to_string())),
            shadow_let_unwrap_or("val", Expr::NumberLit(0.0)),
            plain_let(
                "other",
                Expr::MethodCall {
                    object: Box::new(Expr::Ident("val".to_string())),
                    method: "clone".to_string(),
                    args: vec![],
                },
            ),
        ];
        fuse_nullish_assign_shadow_lets(&mut stmts);
        assert_eq!(stmts.len(), 2);
        // stmts[0] = fused let val = x.unwrap_or(0)
        // stmts[1] = let other = val.clone() (untouched)
        assert!(matches!(
            &stmts[0],
            Stmt::Let { name, .. } if name == "val"
        ));
        assert!(matches!(
            &stmts[1],
            Stmt::Let { name, .. } if name == "other"
        ));
    }
}
