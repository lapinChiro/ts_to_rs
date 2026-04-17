//! Structural conversion of `x ??= d` (TS `NullishAssign`) to IR (I-142).
//!
//! TS `x ??= d` is a compound operator with two observable effects:
//!
//! 1. **Assignment**: when `x` is `null`/`undefined`, `x` is set to `d`.
//! 2. **Narrowing**: after evaluation, `x` is narrowed from `T | null | undefined`
//!    to `T` in the enclosing flow.
//!
//! A naive translation to `x.get_or_insert_with(|| d)` drops the narrowing
//! (return expressions that expect `T` see `&mut T` or `Option<T>` and fail to
//! compile) and requires `x` to be `let mut`, even though the surrounding Rust
//! has no imperative mutation to trigger `mark_mutated_vars`.
//!
//! This module intercepts `x ??= d;` in statement context and rewrites it to a
//! Rust shadow-let (`let x = x.unwrap_or[_else](|| d);`), which preserves the
//! narrowing by rebinding `x` to the unwrapped type for the rest of the block.
//! A follow-up fusion pass collapses `let x = init; let x = x.unwrap_or[_else]
//! (|| d);` to a single `let x = init.unwrap_or[_else](|| d);` where safe,
//! eliminating the cosmetic double-let.
//!
//! Expression-context `??=` (inside call args / return value / conditions) is
//! handled separately in [`convert_assign_expr`] via `get_or_insert_with` with
//! deref-or-clone wrapping based on `is_copy_type`.

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType, Stmt};
use crate::transformer::{build_option_unwrap_with_default, Transformer, UnsupportedSyntaxError};

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
    /// Pre-check hook run by block-iteration sites before converting each
    /// statement (D-1).
    ///
    /// When `stmt` is a statement-level `x ??= d;` with an `Ident` LHS whose
    /// type triggers [`NullishAssignStrategy::ShadowLet`], the emitted Rust
    /// shadow-let (`let x = x.unwrap_or*(...)`) rebinds `x` to the narrowed
    /// inner type for the rest of the Rust lexical scope. If the surrounding
    /// TypeScript scope subsequently reassigns `x` (e.g., `x = null;` or
    /// `if (cond) { x = v; }`), TS re-widens the binding but the generated
    /// Rust cannot — the shadowed `let x: T` does not accept `None` or a
    /// union-typed value, producing a silent compile error.
    ///
    /// This pre-check scans `remaining_in_block` (the statements following
    /// `stmt` within the same block, including nested `if` / loop / switch
    /// bodies but *excluding* closure / nested-fn bodies, per
    /// `report/i142-step3-inv1-narrowing-reset.md`). If a re-assignment to the
    /// same ident is detected, the method returns an `Err` carrying
    /// `UnsupportedSyntaxError("nullish-assign with narrowing-reset (I-144)")`
    /// so the compile error becomes an explicit unsupported-syntax surface.
    ///
    /// This is an **interim** surface: the structural fix (control-flow graph
    /// analysis that can choose a `let mut` + `get_or_insert_with` emission
    /// path when a reset is detected) belongs to the I-144 narrowing analyzer
    /// umbrella. Until I-144 lands, surfacing rather than silently emitting
    /// broken Rust satisfies the Tier-1 priority in
    /// `conversion-correctness-priority.md`.
    ///
    /// Block-iteration callers (`convert_stmt_list`, switch-case handling,
    /// class-method bodies, fn-expression bodies) must invoke this method with
    /// the correct `remaining_in_block` slice. Single-stmt contexts (no
    /// following siblings) pass an empty slice, in which case this method is a
    /// no-op.
    pub(crate) fn pre_check_narrowing_reset(
        &self,
        stmt: &ast::Stmt,
        remaining_in_block: &[ast::Stmt],
    ) -> Result<()> {
        let Some((ident_name, span)) = extract_nullish_assign_ident_stmt(stmt) else {
            return Ok(());
        };
        let Some(lhs_type) = self.get_type_for_var(ident_name, span) else {
            return Ok(());
        };
        if pick_strategy(lhs_type) != NullishAssignStrategy::ShadowLet {
            return Ok(());
        }
        if has_narrowing_reset_in_stmts(remaining_in_block, ident_name) {
            return Err(UnsupportedSyntaxError::new(
                "nullish-assign with narrowing-reset (I-144)",
                span,
            )
            .into());
        }
        Ok(())
    }

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
        let ident = match &assign.left {
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(i)) => i,
            // Member / Index LHS (`obj.x ??= d`, `arr[i] ??= d`) are out of
            // scope for I-142. Return None so the normal expression path
            // surfaces the existing behavior (expression-context arm will
            // emit an UnsupportedSyntaxError).
            _ => return Ok(None),
        };

        let name = ident.id.sym.to_string();
        // Resolve LHS type before converting RHS so error cases short-circuit.
        let lhs_type = self
            .get_type_for_var(&name, ident.id.span)
            .cloned()
            .ok_or_else(|| {
                UnsupportedSyntaxError::new("nullish-assign on unresolved type", assign.span)
            })?;

        let stmts = match pick_strategy(&lhs_type) {
            // Cells #1–3, #13: Option<T> — shadow-let via unwrap_or /
            // unwrap_or_else. RHS is converted here (after the strategy
            // decision) so that blocked strategies can short-circuit without
            // paying for RHS conversion.
            NullishAssignStrategy::ShadowLet => {
                let right_ir = self.convert_expr(&assign.right)?;
                vec![Stmt::Let {
                    mutable: false,
                    name: name.clone(),
                    ty: None,
                    init: Some(build_option_unwrap_with_default(
                        Expr::Ident(name),
                        right_ir,
                    )),
                }]
            }
            // Cell #4: non-nullable `T` — `??=` is dead code at runtime. Emit
            // nothing; the prior `let` stays as written. TS treats this as
            // dead; Rust simply omits the no-op.
            NullishAssignStrategy::Identity => vec![],
            // Cell #5: Any — runtime null check + RHS coercion to
            // `serde_json::Value` requires the I-050 Any coercion umbrella.
            // Until I-050 lands, surface this cell as unsupported rather than
            // emit silently-broken Rust.
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

// -----------------------------------------------------------------------------
// D-1: Narrowing-reset scanner
//
// Detects whether a following sequence of TS statements contains a re-assignment
// to a given identifier that would invalidate the shadow-let emitted for
// `<ident> ??= <default>`. The scan descends into nested blocks (if/for/while/
// switch/try/block) but skips closures and nested-fn bodies, matching TS's
// flow-sensitive narrowing boundaries — see
// `report/i142-step3-inv1-narrowing-reset.md`.
// -----------------------------------------------------------------------------

/// If `stmt` is `<ident> ??= <rhs>;` with a bare-identifier LHS, returns
/// `Some((name, span))`. Otherwise returns `None`.
///
/// Member / index LHS shapes are out of `pre_check_narrowing_reset`'s scope
/// (they are surfaced as I-142-b / I-142-c unsupported elsewhere) and return
/// `None` here so the pre-check is a no-op for them.
fn extract_nullish_assign_ident_stmt(stmt: &ast::Stmt) -> Option<(&str, swc_common::Span)> {
    let ast::Stmt::Expr(expr_stmt) = stmt else {
        return None;
    };
    let ast::Expr::Assign(assign) = expr_stmt.expr.as_ref() else {
        return None;
    };
    if assign.op != ast::AssignOp::NullishAssign {
        return None;
    }
    let ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) = &assign.left else {
        return None;
    };
    Some((ident.id.sym.as_ref(), ident.id.span))
}

/// Returns `true` if any statement in `stmts` (or any of its transitively
/// reachable nested non-closure blocks) contains a re-assignment to `ident`.
fn has_narrowing_reset_in_stmts(stmts: &[ast::Stmt], ident: &str) -> bool {
    stmts.iter().any(|s| stmt_has_reset(s, ident))
}

/// Walks a single statement looking for re-assignments to `ident`.
fn stmt_has_reset(stmt: &ast::Stmt, ident: &str) -> bool {
    match stmt {
        ast::Stmt::Block(b) => has_narrowing_reset_in_stmts(&b.stmts, ident),
        ast::Stmt::Expr(e) => expr_has_reset(&e.expr, ident),
        ast::Stmt::If(if_stmt) => {
            expr_has_reset(&if_stmt.test, ident)
                || stmt_has_reset(&if_stmt.cons, ident)
                || if_stmt
                    .alt
                    .as_ref()
                    .is_some_and(|alt| stmt_has_reset(alt, ident))
        }
        ast::Stmt::While(w) => expr_has_reset(&w.test, ident) || stmt_has_reset(&w.body, ident),
        ast::Stmt::DoWhile(d) => stmt_has_reset(&d.body, ident) || expr_has_reset(&d.test, ident),
        ast::Stmt::For(f) => {
            let init_hit = f.init.as_ref().is_some_and(|init| match init {
                ast::VarDeclOrExpr::VarDecl(v) => vardecl_init_has_reset(v, ident),
                ast::VarDeclOrExpr::Expr(e) => expr_has_reset(e, ident),
            });
            init_hit
                || f.test.as_ref().is_some_and(|t| expr_has_reset(t, ident))
                || f.update.as_ref().is_some_and(|u| expr_has_reset(u, ident))
                || stmt_has_reset(&f.body, ident)
        }
        ast::Stmt::ForOf(fo) => {
            for_head_binds_ident(&fo.left, ident)
                || expr_has_reset(&fo.right, ident)
                || stmt_has_reset(&fo.body, ident)
        }
        ast::Stmt::ForIn(fi) => {
            for_head_binds_ident(&fi.left, ident)
                || expr_has_reset(&fi.right, ident)
                || stmt_has_reset(&fi.body, ident)
        }
        ast::Stmt::Switch(sw) => {
            expr_has_reset(&sw.discriminant, ident)
                || sw.cases.iter().any(|c| {
                    c.test.as_ref().is_some_and(|t| expr_has_reset(t, ident))
                        || has_narrowing_reset_in_stmts(&c.cons, ident)
                })
        }
        ast::Stmt::Try(t) => {
            has_narrowing_reset_in_stmts(&t.block.stmts, ident)
                || t.handler
                    .as_ref()
                    .is_some_and(|h| has_narrowing_reset_in_stmts(&h.body.stmts, ident))
                || t.finalizer
                    .as_ref()
                    .is_some_and(|f| has_narrowing_reset_in_stmts(&f.stmts, ident))
        }
        ast::Stmt::Labeled(l) => stmt_has_reset(&l.body, ident),
        ast::Stmt::Return(r) => r.arg.as_ref().is_some_and(|e| expr_has_reset(e, ident)),
        ast::Stmt::Throw(t) => expr_has_reset(&t.arg, ident),
        ast::Stmt::Decl(ast::Decl::Var(v)) => vardecl_init_has_reset(v, ident),
        // Function / class / TS-only decls don't reset outer bindings.
        ast::Stmt::Decl(_) => false,
        ast::Stmt::With(w) => expr_has_reset(&w.obj, ident) || stmt_has_reset(&w.body, ident),
        ast::Stmt::Break(_)
        | ast::Stmt::Continue(_)
        | ast::Stmt::Empty(_)
        | ast::Stmt::Debugger(_) => false,
    }
}

/// Returns true if any initializer in `var_decl` mentions a reassignment to
/// `ident` (variable declarations themselves shadow rather than reset the outer
/// binding — TS scoping — so the declaration itself is not a reset).
fn vardecl_init_has_reset(var_decl: &ast::VarDecl, ident: &str) -> bool {
    var_decl
        .decls
        .iter()
        .any(|d| d.init.as_ref().is_some_and(|e| expr_has_reset(e, ident)))
}

/// Returns true if a `for-of` / `for-in` loop head binds the *outer* `ident`
/// (not a fresh `let`/`const`/`var` binding). `for (x of arr)` on an
/// already-declared outer `x` reassigns `x` on every iteration and therefore
/// counts as a narrowing-reset.
fn for_head_binds_ident(head: &ast::ForHead, ident: &str) -> bool {
    match head {
        // `for (let x of arr)` / `for (const x of arr)` / `for (var x of arr)` —
        // these introduce *new* bindings at the loop scope, so they do not reset
        // the outer `ident`. Exception: `var` is function-scoped; if it binds
        // to the same name as our shadow it does reset the outer. Treat `var x`
        // of an already-shadowed ident as reset.
        ast::ForHead::VarDecl(v) => {
            v.kind == ast::VarDeclKind::Var
                && v.decls.iter().any(|d| {
                    matches!(
                        &d.name,
                        ast::Pat::Ident(id) if id.id.sym.as_ref() == ident
                    )
                })
        }
        // `for (x of arr)` — no declaration keyword, so this reassigns the
        // outer `x`. That's a reset if the outer `x` is our shadowed ident.
        ast::ForHead::Pat(pat) => match pat.as_ref() {
            ast::Pat::Ident(id) => id.id.sym.as_ref() == ident,
            // Destructuring heads never bind the exact `ident` name alone;
            // conservatively walk them.
            ast::Pat::Array(_) | ast::Pat::Object(_) | ast::Pat::Rest(_) => {
                pat_binds_ident(pat, ident)
            }
            ast::Pat::Assign(a) => {
                pat_binds_ident(&a.left, ident) || expr_has_reset(&a.right, ident)
            }
            _ => false,
        },
        ast::ForHead::UsingDecl(_) => false,
    }
}

/// Walks a pattern looking for a bare `Ident(ident)` binding.
fn pat_binds_ident(pat: &ast::Pat, ident: &str) -> bool {
    match pat {
        ast::Pat::Ident(id) => id.id.sym.as_ref() == ident,
        ast::Pat::Array(arr) => arr
            .elems
            .iter()
            .any(|e| e.as_ref().is_some_and(|p| pat_binds_ident(p, ident))),
        ast::Pat::Object(obj) => obj.props.iter().any(|p| match p {
            ast::ObjectPatProp::KeyValue(kv) => pat_binds_ident(&kv.value, ident),
            ast::ObjectPatProp::Assign(a) => a.key.sym.as_ref() == ident,
            ast::ObjectPatProp::Rest(r) => pat_binds_ident(&r.arg, ident),
        }),
        ast::Pat::Rest(r) => pat_binds_ident(&r.arg, ident),
        ast::Pat::Assign(a) => pat_binds_ident(&a.left, ident),
        _ => false,
    }
}

/// Walks an expression looking for `<ident> = ...`, `<ident> ??=|+=|-=|... ...`,
/// or `<ident>++` / `<ident>--`.
///
/// Closures (`Arrow`, `Fn`, `Class`) are **not** descended into — mutations
/// inside them don't affect the outer scope's narrow per TS's control-flow
/// analysis (confirmed in `inv1-narrowing-reset.md` cases 03 and 05).
fn expr_has_reset(expr: &ast::Expr, ident: &str) -> bool {
    match expr {
        // Assignment to the ident (any op — `=`, `+=`, `??=`, `||=`, ...).
        ast::Expr::Assign(assign) => {
            let lhs_hit = matches!(
                &assign.left,
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(id))
                    if id.id.sym.as_ref() == ident
            );
            lhs_hit || expr_has_reset(&assign.right, ident)
        }
        // `x++` / `x--` — reassigns `x` to `x ± 1`, narrow may survive for `f64`
        // but we conservatively surface reset because the post-shadow type may
        // not match (e.g., if the inner `T` is non-numeric, `++` is a type error
        // anyway). Conservative surface is preferred over silent compile error.
        ast::Expr::Update(up) => {
            matches!(up.arg.as_ref(), ast::Expr::Ident(id) if id.sym.as_ref() == ident)
        }
        // Closure boundaries — do not descend.
        ast::Expr::Arrow(_) | ast::Expr::Fn(_) | ast::Expr::Class(_) => false,
        ast::Expr::Bin(b) => expr_has_reset(&b.left, ident) || expr_has_reset(&b.right, ident),
        ast::Expr::Unary(u) => expr_has_reset(&u.arg, ident),
        ast::Expr::Cond(c) => {
            expr_has_reset(&c.test, ident)
                || expr_has_reset(&c.cons, ident)
                || expr_has_reset(&c.alt, ident)
        }
        ast::Expr::Paren(p) => expr_has_reset(&p.expr, ident),
        ast::Expr::Seq(s) => s.exprs.iter().any(|e| expr_has_reset(e, ident)),
        ast::Expr::Call(c) => {
            let callee_hit = match &c.callee {
                ast::Callee::Expr(e) => expr_has_reset(e, ident),
                ast::Callee::Super(_) | ast::Callee::Import(_) => false,
            };
            callee_hit || c.args.iter().any(|a| expr_has_reset(&a.expr, ident))
        }
        ast::Expr::New(n) => {
            expr_has_reset(&n.callee, ident)
                || n.args
                    .as_ref()
                    .is_some_and(|args| args.iter().any(|a| expr_has_reset(&a.expr, ident)))
        }
        ast::Expr::Member(m) => {
            expr_has_reset(&m.obj, ident)
                || match &m.prop {
                    ast::MemberProp::Computed(c) => expr_has_reset(&c.expr, ident),
                    ast::MemberProp::Ident(_) | ast::MemberProp::PrivateName(_) => false,
                }
        }
        ast::Expr::SuperProp(sp) => match &sp.prop {
            ast::SuperProp::Computed(c) => expr_has_reset(&c.expr, ident),
            ast::SuperProp::Ident(_) => false,
        },
        ast::Expr::Array(arr) => arr.elems.iter().any(|e| {
            e.as_ref()
                .is_some_and(|eos| expr_has_reset(&eos.expr, ident))
        }),
        ast::Expr::Object(obj) => obj.props.iter().any(|p| match p {
            ast::PropOrSpread::Spread(s) => expr_has_reset(&s.expr, ident),
            ast::PropOrSpread::Prop(prop) => prop_has_reset(prop, ident),
        }),
        ast::Expr::Tpl(t) => t.exprs.iter().any(|e| expr_has_reset(e, ident)),
        ast::Expr::TaggedTpl(tt) => {
            expr_has_reset(&tt.tag, ident) || tt.tpl.exprs.iter().any(|e| expr_has_reset(e, ident))
        }
        ast::Expr::Await(a) => expr_has_reset(&a.arg, ident),
        ast::Expr::Yield(y) => y.arg.as_ref().is_some_and(|a| expr_has_reset(a, ident)),
        ast::Expr::OptChain(oc) => match &*oc.base {
            ast::OptChainBase::Member(m) => {
                expr_has_reset(&m.obj, ident)
                    || match &m.prop {
                        ast::MemberProp::Computed(c) => expr_has_reset(&c.expr, ident),
                        _ => false,
                    }
            }
            ast::OptChainBase::Call(c) => {
                expr_has_reset(&c.callee, ident)
                    || c.args.iter().any(|a| expr_has_reset(&a.expr, ident))
            }
        },
        // TS wrapper variants — peek through to inner.
        ast::Expr::TsAs(a) => expr_has_reset(&a.expr, ident),
        ast::Expr::TsTypeAssertion(a) => expr_has_reset(&a.expr, ident),
        ast::Expr::TsNonNull(a) => expr_has_reset(&a.expr, ident),
        ast::Expr::TsConstAssertion(a) => expr_has_reset(&a.expr, ident),
        ast::Expr::TsSatisfies(a) => expr_has_reset(&a.expr, ident),
        ast::Expr::TsInstantiation(a) => expr_has_reset(&a.expr, ident),
        // Leaf / non-assign variants.
        ast::Expr::Ident(_)
        | ast::Expr::This(_)
        | ast::Expr::Lit(_)
        | ast::Expr::MetaProp(_)
        | ast::Expr::Invalid(_)
        | ast::Expr::JSXMember(_)
        | ast::Expr::JSXNamespacedName(_)
        | ast::Expr::JSXEmpty(_)
        | ast::Expr::JSXElement(_)
        | ast::Expr::JSXFragment(_)
        | ast::Expr::PrivateName(_) => false,
    }
}

/// Dispatches `ast::Prop` variants for `expr_has_reset` traversal of
/// object-literal properties.
fn prop_has_reset(prop: &ast::Prop, ident: &str) -> bool {
    match prop {
        ast::Prop::Shorthand(id) => {
            // `{ x }` shorthand is a **read** of `x`, not a reassignment. Don't
            // treat it as a reset. This branch exists so we don't accidentally
            // recurse into an `Ident` below.
            let _ = id;
            false
        }
        ast::Prop::KeyValue(kv) => {
            // Computed key can contain an assignment expression.
            let key_hit = match &kv.key {
                ast::PropName::Computed(c) => expr_has_reset(&c.expr, ident),
                _ => false,
            };
            key_hit || expr_has_reset(&kv.value, ident)
        }
        ast::Prop::Assign(a) => expr_has_reset(&a.value, ident),
        ast::Prop::Getter(_) | ast::Prop::Setter(_) | ast::Prop::Method(_) => {
            // Getter/setter/method bodies are closure-like; skip.
            false
        }
    }
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
