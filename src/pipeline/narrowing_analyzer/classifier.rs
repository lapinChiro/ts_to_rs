//! Per-ident reset-cause classification for the
//! [`narrowing_analyzer`](super) module.
//!
//! Free functions that walk the AST and report whether a given variable
//! `ident` is mutated (and how) by a statement, expression, or scope body.
//!
//! # Responsibilities
//!
//! - **Reset classification**: [`classify_reset_in_stmts`] /
//!   [`classify_reset_in_stmt`] / [`classify_reset_in_expr`] produce a
//!   [`ResetCause`] describing the first invalidating mutation (or the
//!   first preserving mutation, when no invalidation occurs).
//! - **Scope awareness**: [`classify_reset_in_stmts`] tracks block-level
//!   shadowing (`let x = ...`, `function x()`, `class x`) and `VarDecl`
//!   left-to-right evaluation so that inits preceding a shadow binding
//!   are still classified against the outer `ident`.
//! - **Closure-boundary descent**: [`classify_closure_body_for_outer_ident`]
//!   walks arrow / fn / class / object-method bodies and escalates any
//!   detected mutation to [`ResetCause::ClosureReassign`], respecting
//!   parameter shadowing and destructuring-pattern defaults.
//! - **Combinators**: [`merge_branches`] / [`merge_sequential`] compose
//!   per-branch or per-stmt causes into the final classification.
//!
//! All functions are `pub(super)` so the parent module's
//! [`analyze_function`](super::analyze_function) and the
//! [`super::closure_captures`] sub-module (which uses
//! [`classify_closure_body_for_outer_ident`] / [`ArrowOrFnBody`] for
//! per-candidate closure-body classification) can invoke them. No
//! analyzer state is threaded through — the classifier is **stateless**.
//!
//! # Module split (I-169 T6-2 follow-up)
//!
//! The closure-capture-pair collection logic that briefly lived here
//! (T6-2 prototype) was moved to [`super::closure_captures`] in the
//! I-169 follow-up. That module hosts the candidate-limited +
//! shadow-tracking walker that emits `NarrowEvent::ClosureCapture` events.
//! `classifier` retains the per-ident classification primitives that
//! `closure_captures` calls into.

use swc_ecma_ast as ast;

use super::events::ResetCause;

/// If `stmt` is `<ident> ??= <rhs>;` with a bare-identifier LHS, returns
/// `(name, span)`. Otherwise returns `None`.
///
/// Mirrors the extraction used by the legacy scanner. Matched explicitly here
/// so the analyzer has no dependency on `transformer/` internals.
pub(super) fn extract_nullish_assign_ident_stmt(
    stmt: &ast::Stmt,
) -> Option<(&str, swc_common::Span)> {
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

// --- Combinator helpers --------------------------------------------------

/// Combines two independent branch outcomes (e.g. `if` cons/alt, `switch`
/// cases). Returns the "worst" outcome: invalidating beats preserving, and
/// some beats none.
///
/// # Contract
///
/// - Exactly one side `Some`: that side's cause is returned.
/// - Both `None`: returns `None`.
/// - Both `Some` and at least one is invalidating: the **first** invalidating
///   side (left then right) is returned.
/// - Both `Some` and both preserving (or both non-invalidating causes):
///   the **left** side is returned — source-order determinism.
///
/// # Rationale
///
/// `if (A) { preserving } else { invalidating }` must yield an invalidating
/// result: static analysis cannot know which branch executes at runtime, so
/// the conservative guarantee is "at least one branch invalidates → the
/// combined result is invalidating".
///
/// When *both* branches are invalidating but with different causes (e.g.
/// `NullAssign` vs `DirectAssign`) the choice is arbitrary for the current
/// hint-selection use: both drive
/// [`super::events::EmissionHint::GetOrInsertWith`]. The source-order
/// preference provides determinism. Future consumers that record per-branch
/// `NarrowEvent::Reset` entries should emit both causes separately rather
/// than relying on this merge result — `merge_branches` is a **summary**
/// combinator, not a lossless union.
pub(super) fn merge_branches(a: Option<ResetCause>, b: Option<ResetCause>) -> Option<ResetCause> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        (Some(x), Some(y)) => {
            if x.invalidates_narrow() {
                Some(x)
            } else if y.invalidates_narrow() {
                Some(y)
            } else {
                // Both preserving — either is acceptable. Prefer `x` (source
                // order) for determinism.
                Some(x)
            }
        }
    }
}

/// Combines two sequential causes (this-then-that). Short-circuits on
/// invalidation (`a` invalidating → return `a`); otherwise surfaces the
/// first non-`None` cause.
pub(super) fn merge_sequential(a: Option<ResetCause>, b: Option<ResetCause>) -> Option<ResetCause> {
    if matches!(&a, Some(c) if c.invalidates_narrow()) {
        return a;
    }
    if matches!(&b, Some(c) if c.invalidates_narrow()) {
        return b;
    }
    a.or(b)
}

/// Normalizes a control-flow body (`if` cons / alt, loop body, labeled
/// body, `with` body) into a statement slice for uniform processing.
///
/// When the body is a `BlockStmt` (`if (c) { x ??= 10; }`) the slice is
/// the block's own statements. When the body is a single, brace-less stmt
/// (`if (c) x ??= 10;`) the slice is a one-element reference to the stmt
/// itself. This unification is what allows the parent module's
/// `analyze_stmt_list` hint finder to locate `??=` sites inside
/// brace-less bodies.
pub(super) fn body_as_stmt_list(body: &ast::Stmt) -> &[ast::Stmt] {
    if let ast::Stmt::Block(b) = body {
        &b.stmts
    } else {
        std::slice::from_ref(body)
    }
}

/// Classifies a sequence of statements in one block-level scope for
/// assignments to `ident`.
///
/// **Scope-aware**: if one of the statements declares `ident` at block level
/// (`let` / `const` / `var` / `function` / `class` decl, or a top-level
/// destructure binding), all statements following the declaration refer to
/// the fresh local binding and are not classified against the outer
/// narrow. For a multi-decl `VarDecl` (`let a = ..., x = ..., b = ...`)
/// every decl is evaluated **left-to-right**: earlier decls' init
/// expressions are classified against the outer binding, and shadowing
/// takes effect only after the specific decl that introduces the local
/// `ident`.
///
/// **Unreachable-aware**: after a statement that unconditionally exits the
/// enclosing scope (`return` / `throw` / `break` / `continue`, or a block
/// / `if` that collectively always exits), subsequent statements are
/// unreachable at runtime and classification halts so they are not
/// misrecorded as resets.
pub(super) fn classify_reset_in_stmts(stmts: &[ast::Stmt], ident: &str) -> Option<ResetCause> {
    let mut acc: Option<ResetCause> = None;
    for stmt in stmts {
        // Handle declarations inline — they may both evaluate outer-ident
        // side effects (in inits) and introduce a local shadow.
        match stmt {
            ast::Stmt::Decl(ast::Decl::Var(v)) => {
                // L-to-R: each decl's init runs against the current (outer)
                // binding, **then** the decl's name takes effect. Once the
                // target ident is shadowed by some decl in this VarDecl,
                // subsequent decls' inits reference the local binding.
                for d in &v.decls {
                    if let Some(init) = &d.init {
                        let cause = classify_reset_in_expr(init, ident);
                        acc = merge_sequential(acc, cause);
                        if matches!(&acc, Some(c) if c.invalidates_narrow()) {
                            return acc;
                        }
                    }
                    if pat_binds_ident(&d.name, ident) {
                        // Subsequent decls & stmts are local-scope. Stop.
                        return acc;
                    }
                }
                // No decl in this VarDecl shadows `ident` — continue to next stmt.
                continue;
            }
            ast::Stmt::Decl(ast::Decl::Fn(f)) if f.ident.sym.as_ref() == ident => {
                return acc; // fn decl name shadows target
            }
            ast::Stmt::Decl(ast::Decl::Class(c)) if c.ident.sym.as_ref() == ident => {
                return acc; // class decl name shadows target
            }
            _ => {}
        }

        // Normal classification.
        let cause = classify_reset_in_stmt(stmt, ident);
        acc = merge_sequential(acc, cause);
        if matches!(&acc, Some(c) if c.invalidates_narrow()) {
            return acc;
        }

        // Unreachable-code pruning: if the stmt always exits (return / throw
        // / break / continue / exhaustive if), subsequent stmts cannot run
        // and must not be misclassified.
        if crate::pipeline::narrowing_patterns::stmt_always_exits(stmt) {
            return acc;
        }
    }
    acc
}

/// Classifies a single statement's assignments to `ident`.
///
/// # Caller contract
///
/// **Block-level shadowing is the caller's responsibility.** This function
/// descends into compound statements (blocks, loops, branches, etc.) but
/// does not itself check whether `stmt` introduces a local binding that
/// shadows the outer `ident`. That check lives in [`classify_reset_in_stmts`]
/// where the iteration order determines when shadow takes effect.
///
/// Consequently, when this function is invoked directly with a `Decl` stmt
/// (`Var` / `Fn` / `Class`), it assumes the caller has already verified
/// that the decl does **not** shadow `ident` — the function descends
/// unconditionally into the decl's body / init expressions and escalates
/// any detected cause via [`escalate_closure_reassign`].
fn classify_reset_in_stmt(stmt: &ast::Stmt, ident: &str) -> Option<ResetCause> {
    match stmt {
        ast::Stmt::Block(b) => classify_reset_in_stmts(&b.stmts, ident),
        ast::Stmt::Expr(e) => classify_reset_in_expr(&e.expr, ident),
        ast::Stmt::If(if_stmt) => {
            let test = classify_reset_in_expr(&if_stmt.test, ident);
            if matches!(&test, Some(c) if c.invalidates_narrow()) {
                return test;
            }
            let cons = classify_reset_in_stmt(&if_stmt.cons, ident);
            let alt = if_stmt
                .alt
                .as_ref()
                .and_then(|alt| classify_reset_in_stmt(alt, ident));
            merge_sequential(test, merge_branches(cons, alt))
        }
        ast::Stmt::While(w) => classify_loop_like(Some(&w.test), None, None, &w.body, ident),
        ast::Stmt::DoWhile(d) => classify_loop_like(Some(&d.test), None, None, &d.body, ident),
        ast::Stmt::For(f) => classify_for_stmt(f, ident),
        ast::Stmt::ForOf(fo) => classify_for_of_stmt(&fo.left, &fo.right, &fo.body, ident),
        ast::Stmt::ForIn(fi) => classify_for_of_stmt(&fi.left, &fi.right, &fi.body, ident),
        ast::Stmt::Switch(sw) => classify_switch_stmt(sw, ident),
        ast::Stmt::Try(t) => classify_try_stmt(t, ident),
        ast::Stmt::Labeled(l) => classify_reset_in_stmt(&l.body, ident),
        ast::Stmt::Return(r) => r
            .arg
            .as_ref()
            .and_then(|e| classify_reset_in_expr(e, ident)),
        ast::Stmt::Throw(t) => classify_reset_in_expr(&t.arg, ident),
        ast::Stmt::Decl(ast::Decl::Var(v)) => classify_reset_in_vardecl_init(v, ident),
        // Nested function decl: body is a closure-capture boundary.
        // The caller (`classify_reset_in_stmts`) has already verified this
        // decl does NOT shadow `ident` (otherwise it would have short-
        // circuited). Descend into the body and escalate any detected
        // cause to [`ResetCause::ClosureReassign`].
        ast::Stmt::Decl(ast::Decl::Fn(fn_decl)) => {
            let params: Vec<&ast::Pat> = fn_decl.function.params.iter().map(|p| &p.pat).collect();
            fn_decl.function.body.as_ref().and_then(|b| {
                classify_closure_body_for_outer_ident(
                    &params,
                    ArrowOrFnBody::Block(&b.stmts),
                    ident,
                )
            })
        }
        // Nested class decl: descend into members (each member body is a
        // closure-capture boundary). Caller has verified non-shadowing.
        ast::Stmt::Decl(ast::Decl::Class(class_decl)) => {
            classify_reset_in_class_body(&class_decl.class.body, ident)
        }
        ast::Stmt::Decl(_) => None,
        ast::Stmt::With(w) => merge_sequential(
            classify_reset_in_expr(&w.obj, ident),
            classify_reset_in_stmt(&w.body, ident),
        ),
        ast::Stmt::Break(_)
        | ast::Stmt::Continue(_)
        | ast::Stmt::Empty(_)
        | ast::Stmt::Debugger(_) => None,
    }
}

/// Classifies the parts of a loop statement (init / test / update / body)
/// for mutations to `ident`, with for-init shadowing support.
///
/// Used by `while` / `do-while` / `for`: `while` and `do-while` have no
/// init/update — callers pass `None` for those; `for` uses all four.
/// Regardless of loop kind, the parts are merged sequentially because
/// all are in the **same scope** as the outer (the loop body may run
/// zero-or-more times but any reset within it is observable).
fn classify_loop_like(
    test: Option<&ast::Expr>,
    init: Option<&ast::VarDeclOrExpr>,
    update: Option<&ast::Expr>,
    body: &ast::Stmt,
    ident: &str,
) -> Option<ResetCause> {
    let init_cause = init.and_then(|i| match i {
        ast::VarDeclOrExpr::VarDecl(v) => classify_reset_in_vardecl_init(v, ident),
        ast::VarDeclOrExpr::Expr(e) => classify_reset_in_expr(e, ident),
    });
    if matches!(&init_cause, Some(c) if c.invalidates_narrow()) {
        return init_cause;
    }
    // `for-init` can declare the target as a fresh block binding; if so,
    // test / update / body are all in the scope of the local shadow.
    if let Some(ast::VarDeclOrExpr::VarDecl(v)) = init {
        if v.decls.iter().any(|d| pat_binds_ident(&d.name, ident)) {
            return init_cause;
        }
    }
    let test_cause = test.and_then(|t| classify_reset_in_expr(t, ident));
    if matches!(&test_cause, Some(c) if c.invalidates_narrow()) {
        return merge_sequential(init_cause, test_cause);
    }
    let update_cause = update.and_then(|u| classify_reset_in_expr(u, ident));
    if matches!(&update_cause, Some(c) if c.invalidates_narrow()) {
        return merge_sequential(merge_sequential(init_cause, test_cause), update_cause);
    }
    let body_cause = classify_reset_in_stmt(body, ident);
    merge_sequential(
        merge_sequential(merge_sequential(init_cause, test_cause), update_cause),
        body_cause,
    )
}

fn classify_for_stmt(f: &ast::ForStmt, ident: &str) -> Option<ResetCause> {
    classify_loop_like(
        f.test.as_deref(),
        f.init.as_ref(),
        f.update.as_deref(),
        &f.body,
        ident,
    )
}

fn classify_for_of_stmt(
    head: &ast::ForHead,
    right: &ast::Expr,
    body: &ast::Stmt,
    ident: &str,
) -> Option<ResetCause> {
    match for_head_effect(head, ident) {
        ForHeadEffect::RebindsOuter => Some(ResetCause::LoopBoundary),
        ForHeadEffect::ShadowsBody => {
            // fresh binding (let/const) — body sees local ident.
            // Only the RHS may still reference the outer.
            classify_reset_in_expr(right, ident)
        }
        ForHeadEffect::None => merge_sequential(
            classify_reset_in_expr(right, ident),
            classify_reset_in_stmt(body, ident),
        ),
    }
}

fn classify_switch_stmt(sw: &ast::SwitchStmt, ident: &str) -> Option<ResetCause> {
    let disc = classify_reset_in_expr(&sw.discriminant, ident);
    if matches!(&disc, Some(c) if c.invalidates_narrow()) {
        return disc;
    }
    let merged_cases = sw.cases.iter().fold(None, |acc, case| {
        let test_cause = case
            .test
            .as_ref()
            .and_then(|t| classify_reset_in_expr(t, ident));
        let body_cause = classify_reset_in_stmts(&case.cons, ident);
        let case_cause = merge_sequential(test_cause, body_cause);
        merge_branches(acc, case_cause)
    });
    merge_sequential(disc, merged_cases)
}

fn classify_try_stmt(t: &ast::TryStmt, ident: &str) -> Option<ResetCause> {
    // Try body and catch handler are alternative paths (normal exit or
    // exception → handler), so merge them as branches. The finalizer is
    // sequential — it runs after either branch.
    let block = classify_reset_in_stmts(&t.block.stmts, ident);
    let handler = t
        .handler
        .as_ref()
        .and_then(|h| classify_reset_in_stmts(&h.body.stmts, ident));
    let try_branches = merge_branches(block, handler);
    let finalizer = t
        .finalizer
        .as_ref()
        .and_then(|f| classify_reset_in_stmts(&f.stmts, ident));
    merge_sequential(try_branches, finalizer)
}

/// Classifies causes inside a class body. Each member (method / ctor /
/// prop init / static block / auto accessor) is an independent
/// closure-capture boundary.
fn classify_reset_in_class_body(members: &[ast::ClassMember], ident: &str) -> Option<ResetCause> {
    // Members are emitted/invoked independently; merge as branches so an
    // invalidating reassign in *any* member is detected.
    members.iter().fold(None, |acc, m| {
        merge_branches(acc, classify_class_member(m, ident))
    })
}

fn classify_class_member(member: &ast::ClassMember, ident: &str) -> Option<ResetCause> {
    match member {
        ast::ClassMember::Method(method) => classify_function_body_as_closure(
            &method.function.params,
            method.function.body.as_ref(),
            ident,
        ),
        ast::ClassMember::PrivateMethod(pm) => {
            classify_function_body_as_closure(&pm.function.params, pm.function.body.as_ref(), ident)
        }
        ast::ClassMember::Constructor(ctor) => {
            // Ctor params include TsParamProp variants; extract Pat lists.
            let params: Vec<&ast::Pat> = ctor
                .params
                .iter()
                .filter_map(|p| match p {
                    ast::ParamOrTsParamProp::Param(param) => Some(&param.pat),
                    ast::ParamOrTsParamProp::TsParamProp(_) => None,
                })
                .collect();
            ctor.body.as_ref().and_then(|b| {
                classify_closure_body_for_outer_ident(
                    &params,
                    ArrowOrFnBody::Block(&b.stmts),
                    ident,
                )
            })
        }
        ast::ClassMember::ClassProp(prop) => prop
            .value
            .as_deref()
            .and_then(|e| classify_reset_in_expr(e, ident))
            .map(escalate_closure_reassign),
        ast::ClassMember::PrivateProp(prop) => prop
            .value
            .as_deref()
            .and_then(|e| classify_reset_in_expr(e, ident))
            .map(escalate_closure_reassign),
        ast::ClassMember::StaticBlock(sb) => {
            // Static blocks are implicit IIFE-style closures capturing outer.
            classify_reset_in_stmts(&sb.body.stmts, ident).map(escalate_closure_reassign)
        }
        ast::ClassMember::AutoAccessor(acc) => acc
            .value
            .as_deref()
            .and_then(|e| classify_reset_in_expr(e, ident))
            .map(escalate_closure_reassign),
        ast::ClassMember::TsIndexSignature(_) | ast::ClassMember::Empty(_) => None,
    }
}

/// Descends into a `Function`'s body, treating the body as a closure that
/// captures the outer `ident`. Parameters of the function may shadow the
/// outer binding, in which case no classification is performed.
fn classify_function_body_as_closure(
    params: &[ast::Param],
    body: Option<&ast::BlockStmt>,
    ident: &str,
) -> Option<ResetCause> {
    let pats: Vec<&ast::Pat> = params.iter().map(|p| &p.pat).collect();
    body.and_then(|b| {
        classify_closure_body_for_outer_ident(&pats, ArrowOrFnBody::Block(&b.stmts), ident)
    })
}

fn classify_reset_in_vardecl_init(var_decl: &ast::VarDecl, ident: &str) -> Option<ResetCause> {
    var_decl.decls.iter().fold(None, |acc, d| {
        let init_cause = d
            .init
            .as_ref()
            .and_then(|e| classify_reset_in_expr(e, ident));
        merge_sequential(acc, init_cause)
    })
}

/// Effect of a `for-of` / `for-in` head on the outer `ident` binding.
enum ForHeadEffect {
    /// The head rebinds the outer `ident` at each iteration (`for (x of ..)`
    /// with no decl keyword, or `for (var x of ..)` with a clashing name).
    RebindsOuter,
    /// The head declares a fresh block-scoped binding that shadows the outer
    /// `ident` inside the body (`for (let x of ..)` / `for (const x of ..)`).
    ShadowsBody,
    /// The head does not mention `ident` at all.
    None,
}

fn for_head_effect(head: &ast::ForHead, ident: &str) -> ForHeadEffect {
    match head {
        ast::ForHead::VarDecl(v) => {
            let binds = v.decls.iter().any(|d| pat_binds_ident(&d.name, ident));
            if !binds {
                return ForHeadEffect::None;
            }
            match v.kind {
                // `var` is function-scoped; if it binds the same name as
                // our target at an enclosing function level, the binding
                // collapses — treat it as rebinding the outer.
                ast::VarDeclKind::Var => ForHeadEffect::RebindsOuter,
                ast::VarDeclKind::Let | ast::VarDeclKind::Const => ForHeadEffect::ShadowsBody,
            }
        }
        ast::ForHead::Pat(pat) => {
            if pat_binds_ident(pat, ident) {
                ForHeadEffect::RebindsOuter
            } else {
                ForHeadEffect::None
            }
        }
        ast::ForHead::UsingDecl(_) => ForHeadEffect::None,
    }
}

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

fn classify_reset_in_expr(expr: &ast::Expr, ident: &str) -> Option<ResetCause> {
    match expr {
        // Assignments to the ident — classified by assign-op + RHS shape.
        ast::Expr::Assign(assign) => {
            let lhs_is_ident = matches!(
                &assign.left,
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(id))
                    if id.id.sym.as_ref() == ident
            );
            if lhs_is_ident {
                // Classify the RHS (it may itself reassign or read ident
                // in a side-effecting way), but let the assignment's own
                // cause dominate.
                let rhs_cause = classify_reset_in_expr(&assign.right, ident);
                let op_cause = classify_assign_op(assign.op, &assign.right);
                return Some(match rhs_cause {
                    Some(c) if c.invalidates_narrow() && !op_cause.invalidates_narrow() => c,
                    _ => op_cause,
                });
            }
            merge_sequential(
                classify_reset_in_expr(&assign.right, ident),
                // LHS of a non-ident assign (e.g. member) may reference ident
                classify_reset_in_assign_lhs(&assign.left, ident),
            )
        }
        ast::Expr::Update(up) => {
            if matches!(up.arg.as_ref(), ast::Expr::Ident(id) if id.sym.as_ref() == ident) {
                Some(ResetCause::UpdateExpr)
            } else {
                classify_reset_in_expr(&up.arg, ident)
            }
        }
        // Arrow / Fn / Class — closure-capture boundaries. Descend with
        // parameter-level shadow detection.
        ast::Expr::Arrow(arrow) => {
            let params: Vec<&ast::Pat> = arrow.params.iter().collect();
            let body = match arrow.body.as_ref() {
                ast::BlockStmtOrExpr::BlockStmt(b) => ArrowOrFnBody::Block(&b.stmts),
                ast::BlockStmtOrExpr::Expr(e) => ArrowOrFnBody::Expr(e),
            };
            classify_closure_body_for_outer_ident(&params, body, ident)
        }
        ast::Expr::Fn(fn_expr) => {
            let params: Vec<&ast::Pat> = fn_expr.function.params.iter().map(|p| &p.pat).collect();
            // The fn expression's optional self-name shadows only inside the
            // body if it matches; handle that case.
            if let Some(fn_id) = &fn_expr.ident {
                if fn_id.sym.as_ref() == ident {
                    return None;
                }
            }
            fn_expr.function.body.as_ref().and_then(|b| {
                classify_closure_body_for_outer_ident(
                    &params,
                    ArrowOrFnBody::Block(&b.stmts),
                    ident,
                )
            })
        }
        ast::Expr::Class(class_expr) => {
            if let Some(class_id) = &class_expr.ident {
                if class_id.sym.as_ref() == ident {
                    // Named class expression where the class name shadows —
                    // the body references the class name as local, so any
                    // use is local-scope. However other members may still
                    // reference outer ident when NOT through the class
                    // name. A fully correct analysis would continue with a
                    // reduced scope; for simplicity, skip the whole class.
                    return None;
                }
            }
            classify_reset_in_class_body(&class_expr.class.body, ident)
        }
        ast::Expr::Bin(b) => merge_sequential(
            classify_reset_in_expr(&b.left, ident),
            classify_reset_in_expr(&b.right, ident),
        ),
        ast::Expr::Unary(u) => classify_reset_in_expr(&u.arg, ident),
        ast::Expr::Cond(c) => {
            let test = classify_reset_in_expr(&c.test, ident);
            if matches!(&test, Some(cause) if cause.invalidates_narrow()) {
                return test;
            }
            let cons = classify_reset_in_expr(&c.cons, ident);
            let alt = classify_reset_in_expr(&c.alt, ident);
            merge_sequential(test, merge_branches(cons, alt))
        }
        ast::Expr::Paren(p) => classify_reset_in_expr(&p.expr, ident),
        ast::Expr::Seq(s) => s.exprs.iter().fold(None, |acc, e| {
            merge_sequential(acc, classify_reset_in_expr(e, ident))
        }),
        ast::Expr::Call(c) => {
            let callee_hit = match &c.callee {
                ast::Callee::Expr(e) => classify_reset_in_expr(e, ident),
                ast::Callee::Super(_) | ast::Callee::Import(_) => None,
            };
            c.args.iter().fold(callee_hit, |acc, a| {
                merge_sequential(acc, classify_reset_in_expr(&a.expr, ident))
            })
        }
        ast::Expr::New(n) => {
            let callee_hit = classify_reset_in_expr(&n.callee, ident);
            match n.args.as_ref() {
                Some(args) => args.iter().fold(callee_hit, |acc, a| {
                    merge_sequential(acc, classify_reset_in_expr(&a.expr, ident))
                }),
                None => callee_hit,
            }
        }
        ast::Expr::Member(m) => merge_sequential(
            classify_reset_in_expr(&m.obj, ident),
            match &m.prop {
                ast::MemberProp::Computed(c) => classify_reset_in_expr(&c.expr, ident),
                ast::MemberProp::Ident(_) | ast::MemberProp::PrivateName(_) => None,
            },
        ),
        ast::Expr::SuperProp(sp) => match &sp.prop {
            ast::SuperProp::Computed(c) => classify_reset_in_expr(&c.expr, ident),
            ast::SuperProp::Ident(_) => None,
        },
        ast::Expr::Array(arr) => arr.elems.iter().fold(None, |acc, e| {
            let c = e
                .as_ref()
                .and_then(|eos| classify_reset_in_expr(&eos.expr, ident));
            merge_sequential(acc, c)
        }),
        ast::Expr::Object(obj) => obj.props.iter().fold(None, |acc, p| {
            let c = match p {
                ast::PropOrSpread::Spread(s) => classify_reset_in_expr(&s.expr, ident),
                ast::PropOrSpread::Prop(prop) => classify_reset_in_prop(prop, ident),
            };
            merge_sequential(acc, c)
        }),
        ast::Expr::Tpl(t) => t.exprs.iter().fold(None, |acc, e| {
            merge_sequential(acc, classify_reset_in_expr(e, ident))
        }),
        ast::Expr::TaggedTpl(tt) => {
            let tag_cause = classify_reset_in_expr(&tt.tag, ident);
            tt.tpl.exprs.iter().fold(tag_cause, |acc, e| {
                merge_sequential(acc, classify_reset_in_expr(e, ident))
            })
        }
        ast::Expr::Await(a) => classify_reset_in_expr(&a.arg, ident),
        ast::Expr::Yield(y) => y
            .arg
            .as_ref()
            .and_then(|a| classify_reset_in_expr(a, ident)),
        ast::Expr::OptChain(oc) => match &*oc.base {
            ast::OptChainBase::Member(m) => merge_sequential(
                classify_reset_in_expr(&m.obj, ident),
                match &m.prop {
                    ast::MemberProp::Computed(c) => classify_reset_in_expr(&c.expr, ident),
                    _ => None,
                },
            ),
            ast::OptChainBase::Call(c) => {
                let callee = classify_reset_in_expr(&c.callee, ident);
                c.args.iter().fold(callee, |acc, a| {
                    merge_sequential(acc, classify_reset_in_expr(&a.expr, ident))
                })
            }
        },
        // TS wrappers — peek through.
        ast::Expr::TsAs(a) => classify_reset_in_expr(&a.expr, ident),
        ast::Expr::TsTypeAssertion(a) => classify_reset_in_expr(&a.expr, ident),
        ast::Expr::TsNonNull(a) => classify_reset_in_expr(&a.expr, ident),
        ast::Expr::TsConstAssertion(a) => classify_reset_in_expr(&a.expr, ident),
        ast::Expr::TsSatisfies(a) => classify_reset_in_expr(&a.expr, ident),
        ast::Expr::TsInstantiation(a) => classify_reset_in_expr(&a.expr, ident),
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
        | ast::Expr::PrivateName(_) => None,
    }
}

/// Classifies an assignment target (LHS of `=` / `+=` / ... ) for side-
/// effecting references to `ident`. The target itself is not a reassign of
/// `ident` when it is not a bare Ident (handled by the caller); this
/// function looks for ident references inside computed member keys etc.
fn classify_reset_in_assign_lhs(target: &ast::AssignTarget, ident: &str) -> Option<ResetCause> {
    match target {
        ast::AssignTarget::Simple(simple) => match simple {
            ast::SimpleAssignTarget::Ident(_) => None,
            ast::SimpleAssignTarget::Member(m) => merge_sequential(
                classify_reset_in_expr(&m.obj, ident),
                match &m.prop {
                    ast::MemberProp::Computed(c) => classify_reset_in_expr(&c.expr, ident),
                    ast::MemberProp::Ident(_) | ast::MemberProp::PrivateName(_) => None,
                },
            ),
            ast::SimpleAssignTarget::SuperProp(sp) => match &sp.prop {
                ast::SuperProp::Computed(c) => classify_reset_in_expr(&c.expr, ident),
                ast::SuperProp::Ident(_) => None,
            },
            ast::SimpleAssignTarget::Paren(p) => classify_reset_in_expr(&p.expr, ident),
            ast::SimpleAssignTarget::OptChain(o) => match &*o.base {
                ast::OptChainBase::Member(m) => merge_sequential(
                    classify_reset_in_expr(&m.obj, ident),
                    match &m.prop {
                        ast::MemberProp::Computed(c) => classify_reset_in_expr(&c.expr, ident),
                        _ => None,
                    },
                ),
                ast::OptChainBase::Call(c) => {
                    let callee = classify_reset_in_expr(&c.callee, ident);
                    c.args.iter().fold(callee, |acc, a| {
                        merge_sequential(acc, classify_reset_in_expr(&a.expr, ident))
                    })
                }
            },
            ast::SimpleAssignTarget::TsAs(a) => classify_reset_in_expr(&a.expr, ident),
            ast::SimpleAssignTarget::TsSatisfies(s) => classify_reset_in_expr(&s.expr, ident),
            ast::SimpleAssignTarget::TsNonNull(n) => classify_reset_in_expr(&n.expr, ident),
            ast::SimpleAssignTarget::TsTypeAssertion(a) => classify_reset_in_expr(&a.expr, ident),
            ast::SimpleAssignTarget::TsInstantiation(i) => classify_reset_in_expr(&i.expr, ident),
            ast::SimpleAssignTarget::Invalid(_) => None,
        },
        ast::AssignTarget::Pat(_) => None,
    }
}

/// Body of an arrow / fn / method — either a block of statements or (only
/// for arrows) a single expression.
///
/// Both variants wrap immutable references so the enum is `Copy`; callers
/// pass by value without worrying about consumption.
#[derive(Copy, Clone)]
pub(super) enum ArrowOrFnBody<'a> {
    Block(&'a [ast::Stmt]),
    Expr(&'a ast::Expr),
}

/// Classifies reassignments of an **outer** ident inside a closure body,
/// respecting the closure's parameters as shadowing bindings and
/// classifying parameter default expressions against the outer binding.
///
/// If any parameter pattern binds the target ident, the outer narrow is
/// not reachable from inside the closure and `None` is returned.
///
/// Parameter default expressions (`(p = <expr>) => body`) are evaluated
/// each time the closure is invoked with `undefined` for that slot. If
/// `<expr>` reassigns the outer ident, the outer narrow is invalidated
/// when the closure runs. Defaults are classified **before** the body so
/// the resulting cause is escalated to
/// [`ResetCause::ClosureReassign`] together with any body-detected cause.
pub(super) fn classify_closure_body_for_outer_ident(
    params: &[&ast::Pat],
    body: ArrowOrFnBody<'_>,
    ident: &str,
) -> Option<ResetCause> {
    if params.iter().any(|p| pat_binds_ident(p, ident)) {
        return None;
    }
    let defaults_cause = params.iter().fold(None, |acc, p| {
        merge_sequential(acc, classify_pat_defaults(p, ident))
    });
    let body_cause = match body {
        ArrowOrFnBody::Block(stmts) => classify_reset_in_stmts(stmts, ident),
        ArrowOrFnBody::Expr(e) => classify_reset_in_expr(e, ident),
    };
    merge_sequential(defaults_cause, body_cause).map(escalate_closure_reassign)
}

/// Classifies default-value expressions nested inside a destructuring
/// parameter pattern for side effects on the outer `ident`.
///
/// Walks `Pat::Assign` (`= <default>`) nodes and object / array / rest
/// patterns, returning the merged cause of all default-value expressions
/// that mutate `ident`. Destructuring without defaults contributes no
/// cause. Binding names (`Pat::Ident`) are leaves — shadowing has already
/// been checked by [`classify_closure_body_for_outer_ident`].
fn classify_pat_defaults(pat: &ast::Pat, ident: &str) -> Option<ResetCause> {
    match pat {
        ast::Pat::Ident(_) => None,
        ast::Pat::Assign(a) => merge_sequential(
            classify_pat_defaults(&a.left, ident),
            classify_reset_in_expr(&a.right, ident),
        ),
        ast::Pat::Array(arr) => arr.elems.iter().fold(None, |acc, e| {
            let c = e.as_ref().and_then(|p| classify_pat_defaults(p, ident));
            merge_sequential(acc, c)
        }),
        ast::Pat::Object(obj) => obj.props.iter().fold(None, |acc, p| {
            let c = match p {
                ast::ObjectPatProp::KeyValue(kv) => classify_pat_defaults(&kv.value, ident),
                ast::ObjectPatProp::Assign(a) => a
                    .value
                    .as_deref()
                    .and_then(|v| classify_reset_in_expr(v, ident)),
                ast::ObjectPatProp::Rest(r) => classify_pat_defaults(&r.arg, ident),
            };
            merge_sequential(acc, c)
        }),
        ast::Pat::Rest(r) => classify_pat_defaults(&r.arg, ident),
        _ => None,
    }
}

/// Maps a cause observed inside a callable-capture boundary body to
/// [`ResetCause::ClosureReassign`]. Preserving causes are escalated as well
/// because shadow-let cannot be re-mutated across the boundary even for
/// narrow-preserving ops.
fn escalate_closure_reassign(_cause: ResetCause) -> ResetCause {
    ResetCause::ClosureReassign
}

fn classify_assign_op(op: ast::AssignOp, rhs: &ast::Expr) -> ResetCause {
    use ast::AssignOp;
    match op {
        AssignOp::Assign => {
            if crate::pipeline::narrowing_patterns::is_null_or_undefined(rhs) {
                ResetCause::NullAssign
            } else {
                ResetCause::DirectAssign
            }
        }
        // Nullish-assign on an already-narrowed ident is a runtime no-op:
        // predicate elides, narrow is preserved.
        AssignOp::NullishAssign => ResetCause::NullishAssignOnNarrow,
        // Logical compound — RHS type drives the new value, so narrow is
        // re-evaluated.
        AssignOp::AndAssign | AssignOp::OrAssign => ResetCause::CompoundLogical,
        // Arithmetic / bitwise compound — numeric, narrow-preserving.
        AssignOp::AddAssign
        | AssignOp::SubAssign
        | AssignOp::MulAssign
        | AssignOp::DivAssign
        | AssignOp::ModAssign
        | AssignOp::BitAndAssign
        | AssignOp::BitOrAssign
        | AssignOp::BitXorAssign
        | AssignOp::LShiftAssign
        | AssignOp::RShiftAssign
        | AssignOp::ZeroFillRShiftAssign
        | AssignOp::ExpAssign => ResetCause::CompoundArith,
    }
}

fn classify_reset_in_prop(prop: &ast::Prop, ident: &str) -> Option<ResetCause> {
    match prop {
        ast::Prop::Shorthand(_) => None,
        ast::Prop::KeyValue(kv) => merge_sequential(
            classify_reset_in_prop_name(&kv.key, ident),
            classify_reset_in_expr(&kv.value, ident),
        ),
        ast::Prop::Assign(a) => classify_reset_in_expr(&a.value, ident),
        ast::Prop::Method(mp) => {
            classify_function_body_as_closure(&mp.function.params, mp.function.body.as_ref(), ident)
        }
        ast::Prop::Getter(g) => g
            .body
            .as_ref()
            .and_then(|b| classify_reset_in_stmts(&b.stmts, ident))
            .map(escalate_closure_reassign),
        ast::Prop::Setter(s) => {
            // Setter has a single param pat that may shadow.
            let param = [&*s.param];
            s.body.as_ref().and_then(|b| {
                classify_closure_body_for_outer_ident(&param, ArrowOrFnBody::Block(&b.stmts), ident)
            })
        }
    }
}

fn classify_reset_in_prop_name(name: &ast::PropName, ident: &str) -> Option<ResetCause> {
    match name {
        ast::PropName::Computed(c) => classify_reset_in_expr(&c.expr, ident),
        _ => None,
    }
}
