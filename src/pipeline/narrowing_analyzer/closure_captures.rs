//! Closure-capture event collection for the
//! [`narrowing_analyzer`](super) module (I-169 T6-2 follow-up).
//!
//! Walks a function body to enumerate every `(outer ident, closure span)`
//! pair where an outer-declared variable is reassigned inside a callable
//! capture boundary (arrow / fn expr / nested fn decl / class member /
//! object method / getter / setter / static block / class prop init).
//!
//! # Scope-aware design
//!
//! - **Outer candidates first**: the analyzer pre-computes the candidate
//!   ident set as `params + body top-level let/const/var decls`
//!   ([`collect_outer_candidates`]). Inner-fn local vars (B10) and
//!   nested-block decls (B9 / I-167 scope) are excluded by construction —
//!   the over-collection bug from the T6-2 prototype is structurally
//!   prevented (P2 root fix).
//! - **Active-candidate shadow tracking at closure boundaries** (primary
//!   shadow mechanism): [`walk_closure_boundary`] pre-removes closure
//!   self-names, param bindings, and inner body top-level `let`/`const`/
//!   `var` decl names from a cloned `boundary_active`. Nested closures
//!   within that body then inherit the correctly shadowed set so a
//!   closure inside a scope that shadows an outer candidate does NOT
//!   emit a false-positive event (R-2 / matrix cell #25 fix).
//! - **In-block shadow tracking for fn / class decls** (secondary): when
//!   the walker encounters a `function foo()` or `class Foo` statement
//!   inside a block, [`walk_decl`] removes the decl's name from the
//!   active set so subsequent stmts in the same block see the shadow.
//!   `let` / `const` / `var` decls at non-outer-body levels are NOT
//!   removed here — the primary mechanism handles them at closure-body
//!   entry, and doing so at `walk_decl` would conflict with outer-body
//!   candidate preservation (where `let x = 5;` IS the candidate's own
//!   declaration, not a shadow of it). Block-scoped `let` shadow within
//!   a non-closure context remains an I-167 scope-out imprecision.
//! - **Per-event scope membership**: events emitted here flow into
//!   [`NarrowEvent::ClosureCapture`] with `enclosing_fn_body` set by
//!   `analyze_function` to the function body's span. The
//!   `is_var_closure_reassigned(name, position)` accessor filters by
//!   this span, ensuring multi-function scope isolation (P1 fix).
//!
//! # Why a separate module
//!
//! Splitting closure-capture detection out of `classifier.rs` keeps the
//! [`super::classifier`] module focused on per-ident reset classification
//! (the T3 / T4 contract) and brings `classifier.rs` back under the 1000-
//! line file-size convention. The two modules share a small interface:
//! `closure_captures` calls
//! [`super::classifier::classify_closure_body_for_outer_ident`] (which
//! internally handles closure-body shadow via its own private
//! `pat_binds_ident`) for the per-candidate invalidation check.

use std::collections::HashSet;

use swc_ecma_ast as ast;

use super::classifier::{classify_closure_body_for_outer_ident, ArrowOrFnBody};
use super::events::ResetCause;

/// `(var_name, closure_span)` pair returned by
/// [`collect_closure_capture_pairs_for_candidates`].
///
/// A pair is recorded whenever an outer-declared ident in the candidate set
/// is reassigned inside a callable boundary (arrow / fn expr / nested fn
/// decl / class member / object method / getter / setter / static block /
/// class prop init). Used by the Transformer (I-144 T6-2 follow-up) to
/// suppress narrow shadow-let emission for the captured ident so the
/// variable stays `Option<T>` and the closure body can still reassign it.
pub(super) type ClosureCapturePair = (String, swc_common::Span);

// -----------------------------------------------------------------------------
// Public entry: candidate enumeration + walker driver
// -----------------------------------------------------------------------------

/// Enumerates the outer scope's candidate ident set: function parameter
/// names + body top-level `let`/`const`/`var` declaration names.
///
/// Block-level decls (`if (...) { let x; }`), nested-fn decls' inner vars,
/// and loop-init `let i` are intentionally excluded — they are either
/// I-167 scope (block-scoped narrow + closure) or scope-local-only
/// (loop iter) and outside this PRD's scope.
pub(super) fn collect_outer_candidates(
    params: &[&ast::Pat],
    stmts: &[ast::Stmt],
) -> HashSet<String> {
    let mut out = HashSet::new();
    for pat in params {
        collect_pat_idents(pat, &mut out);
    }
    collect_top_level_decl_idents(stmts, &mut out);
    out
}

/// Walks `stmts` and emits a [`ClosureCapturePair`] for every closure
/// boundary inside the body that reassigns one of the outer `candidates`,
/// respecting nested-scope shadowing.
///
/// The single public entry of this module. `analyze_function` calls it
/// after computing `candidates` via [`collect_outer_candidates`].
pub(super) fn collect_closure_capture_pairs_for_candidates(
    stmts: &[ast::Stmt],
    candidates: &HashSet<String>,
) -> Vec<ClosureCapturePair> {
    let mut out = Vec::new();
    let mut active = candidates.clone();
    walk_stmts(stmts, &mut active, &mut out);
    out
}

// -----------------------------------------------------------------------------
// Pat ident collection (Phase 8 detail)
// -----------------------------------------------------------------------------

/// Collects every binding ident name introduced by a parameter / variable
/// pattern, recursing through array, object (KeyValue / Assign /
/// Rest), rest, and `Pat::Assign` (default-value) patterns.
///
/// Used for outer candidate enumeration (params). For closure-body shadow
/// detection see [`remove_pat_idents`] — shadowing subtracts names from
/// the active set rather than adding them.
pub(super) fn collect_pat_idents(pat: &ast::Pat, out: &mut HashSet<String>) {
    match pat {
        ast::Pat::Ident(id) => {
            out.insert(id.id.sym.to_string());
        }
        ast::Pat::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                collect_pat_idents(elem, out);
            }
        }
        ast::Pat::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => collect_pat_idents(&kv.value, out),
                    ast::ObjectPatProp::Assign(a) => {
                        out.insert(a.key.sym.to_string());
                    }
                    ast::ObjectPatProp::Rest(r) => collect_pat_idents(&r.arg, out),
                }
            }
        }
        ast::Pat::Rest(r) => collect_pat_idents(&r.arg, out),
        ast::Pat::Assign(a) => collect_pat_idents(&a.left, out),
        // `Pat::Invalid` and `Pat::Expr` cannot introduce binding idents.
        _ => {}
    }
}

/// Collects names introduced by `let`/`const`/`var` declarations at the
/// **top level** of `stmts` only — does NOT descend into nested blocks
/// (if / loop / try / labeled / with / switch case bodies).
///
/// `function`/`class` decl names are NOT included: they are not reassign
/// targets for narrow purposes (I-169 scope), and shadow tracking handles
/// their name shadowing inside the walker.
fn collect_top_level_decl_idents(stmts: &[ast::Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        if let ast::Stmt::Decl(ast::Decl::Var(v)) = stmt {
            for d in &v.decls {
                collect_pat_idents(&d.name, out);
            }
        }
    }
}

/// Removes every binding name introduced by `pat` from `active`. Used by
/// the walker to apply shadow semantics: a declaration inside a nested
/// scope subtracts its names from the outer candidate set for the
/// duration of that scope.
fn remove_pat_idents(pat: &ast::Pat, active: &mut HashSet<String>) {
    match pat {
        ast::Pat::Ident(id) => {
            active.remove(id.id.sym.as_ref());
        }
        ast::Pat::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                remove_pat_idents(elem, active);
            }
        }
        ast::Pat::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => remove_pat_idents(&kv.value, active),
                    ast::ObjectPatProp::Assign(a) => {
                        active.remove(a.key.sym.as_ref());
                    }
                    ast::ObjectPatProp::Rest(r) => remove_pat_idents(&r.arg, active),
                }
            }
        }
        ast::Pat::Rest(r) => remove_pat_idents(&r.arg, active),
        ast::Pat::Assign(a) => remove_pat_idents(&a.left, active),
        _ => {}
    }
}

// -----------------------------------------------------------------------------
// Shadow-tracking walker (R-2 fix)
// -----------------------------------------------------------------------------

/// Walks a stmt list, threading `active` through block-level scope
/// transitions.
///
/// The `active` set is **block-scoped**: declarations made within `stmts`
/// shadow outer candidates from their declaration point onward, and the
/// outer state is restored on block exit. Compound stmts (if / loop /
/// switch / try / labeled / with) recursively walk their bodies in their
/// own block scope without leaking inner shadows back to the caller.
fn walk_stmts(
    stmts: &[ast::Stmt],
    active: &mut HashSet<String>,
    out: &mut Vec<ClosureCapturePair>,
) {
    let saved = active.clone();
    for stmt in stmts {
        walk_stmt(stmt, active, out);
    }
    *active = saved;
}

fn walk_stmt(stmt: &ast::Stmt, active: &mut HashSet<String>, out: &mut Vec<ClosureCapturePair>) {
    match stmt {
        ast::Stmt::Block(b) => walk_stmts(&b.stmts, active, out),
        ast::Stmt::Expr(e) => walk_expr(&e.expr, active, out),
        ast::Stmt::If(if_stmt) => {
            walk_expr(&if_stmt.test, active, out);
            // `cons` and `alt` are evaluated in mutually exclusive branches;
            // each gets its own block-scoped walk so neither leaks shadows.
            let saved = active.clone();
            walk_stmt(&if_stmt.cons, active, out);
            *active = saved.clone();
            if let Some(alt) = &if_stmt.alt {
                walk_stmt(alt, active, out);
                *active = saved;
            }
        }
        ast::Stmt::While(w) => {
            walk_expr(&w.test, active, out);
            let saved = active.clone();
            walk_stmt(&w.body, active, out);
            *active = saved;
        }
        ast::Stmt::DoWhile(d) => {
            let saved = active.clone();
            walk_stmt(&d.body, active, out);
            *active = saved;
            walk_expr(&d.test, active, out);
        }
        ast::Stmt::For(f) => {
            let saved = active.clone();
            if let Some(init) = &f.init {
                match init {
                    ast::VarDeclOrExpr::VarDecl(v) => {
                        for d in &v.decls {
                            if let Some(e) = &d.init {
                                walk_expr(e, active, out);
                            }
                            // for-init `let` shadows the body scope.
                            remove_pat_idents(&d.name, active);
                        }
                    }
                    ast::VarDeclOrExpr::Expr(e) => walk_expr(e, active, out),
                }
            }
            if let Some(test) = &f.test {
                walk_expr(test, active, out);
            }
            if let Some(update) = &f.update {
                walk_expr(update, active, out);
            }
            walk_stmt(&f.body, active, out);
            *active = saved;
        }
        ast::Stmt::ForOf(fo) => {
            let saved = active.clone();
            walk_expr(&fo.right, active, out);
            apply_for_head_shadow(&fo.left, active);
            walk_stmt(&fo.body, active, out);
            *active = saved;
        }
        ast::Stmt::ForIn(fi) => {
            let saved = active.clone();
            walk_expr(&fi.right, active, out);
            apply_for_head_shadow(&fi.left, active);
            walk_stmt(&fi.body, active, out);
            *active = saved;
        }
        ast::Stmt::Switch(sw) => {
            walk_expr(&sw.discriminant, active, out);
            for case in &sw.cases {
                if let Some(test) = &case.test {
                    walk_expr(test, active, out);
                }
                let saved = active.clone();
                walk_stmts(&case.cons, active, out);
                *active = saved;
            }
        }
        ast::Stmt::Try(t) => {
            walk_stmts(&t.block.stmts, active, out);
            if let Some(handler) = &t.handler {
                let saved = active.clone();
                if let Some(param) = &handler.param {
                    remove_pat_idents(param, active);
                }
                walk_stmts(&handler.body.stmts, active, out);
                *active = saved;
            }
            if let Some(finalizer) = &t.finalizer {
                walk_stmts(&finalizer.stmts, active, out);
            }
        }
        ast::Stmt::Labeled(l) => walk_stmt(&l.body, active, out),
        ast::Stmt::With(w) => {
            walk_expr(&w.obj, active, out);
            walk_stmt(&w.body, active, out);
        }
        ast::Stmt::Return(r) => {
            if let Some(arg) = &r.arg {
                walk_expr(arg, active, out);
            }
        }
        ast::Stmt::Throw(t) => walk_expr(&t.arg, active, out),
        ast::Stmt::Decl(decl) => walk_decl(decl, active, out),
        ast::Stmt::Break(_)
        | ast::Stmt::Continue(_)
        | ast::Stmt::Empty(_)
        | ast::Stmt::Debugger(_) => {}
    }
}

/// Applies the binding effect of a `for-of` / `for-in` head to the active
/// candidate set: `let` / `const` / `var` head names shadow inside the
/// loop body; bare `Pat` heads (no decl keyword, e.g. `for (x of arr)`)
/// rebind the outer ident — for the purposes of capture detection we
/// remove the candidate from `active` so closures inside the body don't
/// emit events for the iter-rebind reassign.
fn apply_for_head_shadow(head: &ast::ForHead, active: &mut HashSet<String>) {
    match head {
        ast::ForHead::VarDecl(v) => {
            for d in &v.decls {
                remove_pat_idents(&d.name, active);
            }
        }
        ast::ForHead::Pat(pat) => remove_pat_idents(pat, active),
        ast::ForHead::UsingDecl(_) => {}
    }
}

fn walk_decl(decl: &ast::Decl, active: &mut HashSet<String>, out: &mut Vec<ClosureCapturePair>) {
    match decl {
        ast::Decl::Var(v) => {
            // We **do not** remove VarDecl names from `active` here.
            //
            // Rationale: at the outer function body's top level, VarDecl
            // names are the outer candidates themselves — removing would
            // silence every subsequent closure event for them. All
            // legitimate shadow removal happens at **closure boundary
            // entry** (see [`walk_closure_boundary`]) which pre-computes
            // the boundary-scoped active set by subtracting params +
            // inner-body top-level decl names. Nested closures within a
            // closure body thereby inherit a correctly shadowed active
            // set.
            //
            // Block-level shadow within a non-closure context (e.g.
            // `if (cond) { let x; ... }`) is intentionally unhandled —
            // matrix cell #19 / B9 is scoped out to I-167. Attempting to
            // shadow at `walk_decl` indiscriminately would conflict with
            // the outer-body-top-level case above; a precise fix requires
            // per-binding identity (I-165 scope). The current walker
            // emits conservatively (may over-suppress inside block shadow)
            // which is runtime-correct.
            for d in &v.decls {
                if let Some(init) = &d.init {
                    walk_expr(init, active, out);
                }
            }
        }
        ast::Decl::Fn(fn_decl) => {
            // Fn decl name shadows the outer ident from this point onward
            // in the enclosing block (hoisted to the block top, but for our
            // purposes adding it to `active` removal here suffices since
            // any closure that references the name happens after the decl
            // textually in source-order walks).
            active.remove(fn_decl.ident.sym.as_ref());
            // Inner fn body is a separate function scope; analyzed by its
            // own `analyze_function` call. From the outer walk's perspective
            // it is still a closure boundary that may reassign outer
            // candidates — visit it here through `walk_closure_boundary`.
            if let Some(body) = &fn_decl.function.body {
                let params: Vec<&ast::Pat> =
                    fn_decl.function.params.iter().map(|p| &p.pat).collect();
                walk_closure_boundary(
                    fn_decl.function.span,
                    &params,
                    ArrowOrFnBody::Block(&body.stmts),
                    Some(fn_decl.ident.sym.as_ref()),
                    active,
                    out,
                );
            }
        }
        ast::Decl::Class(class_decl) => {
            active.remove(class_decl.ident.sym.as_ref());
            walk_class_body(
                &class_decl.class.body,
                Some(class_decl.ident.sym.as_ref()),
                active,
                out,
            );
        }
        // TS-only or import/export decls cannot introduce closure boundaries
        // that capture outer narrow vars.
        _ => {}
    }
}

fn walk_class_body(
    members: &[ast::ClassMember],
    class_self_name: Option<&str>,
    active: &HashSet<String>,
    out: &mut Vec<ClosureCapturePair>,
) {
    for member in members {
        match member {
            ast::ClassMember::Method(m) => {
                if let Some(body) = &m.function.body {
                    let params: Vec<&ast::Pat> = m.function.params.iter().map(|p| &p.pat).collect();
                    walk_closure_boundary(
                        m.function.span,
                        &params,
                        ArrowOrFnBody::Block(&body.stmts),
                        class_self_name,
                        active,
                        out,
                    );
                }
            }
            ast::ClassMember::PrivateMethod(m) => {
                if let Some(body) = &m.function.body {
                    let params: Vec<&ast::Pat> = m.function.params.iter().map(|p| &p.pat).collect();
                    walk_closure_boundary(
                        m.function.span,
                        &params,
                        ArrowOrFnBody::Block(&body.stmts),
                        class_self_name,
                        active,
                        out,
                    );
                }
            }
            ast::ClassMember::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    let params: Vec<&ast::Pat> = ctor
                        .params
                        .iter()
                        .filter_map(|p| match p {
                            ast::ParamOrTsParamProp::Param(param) => Some(&param.pat),
                            ast::ParamOrTsParamProp::TsParamProp(_) => None,
                        })
                        .collect();
                    walk_closure_boundary(
                        ctor.span,
                        &params,
                        ArrowOrFnBody::Block(&body.stmts),
                        class_self_name,
                        active,
                        out,
                    );
                }
            }
            ast::ClassMember::ClassProp(prop) => {
                // Class prop init is evaluated per-instance inside the
                // constructor: it is a closure-capture boundary (matrix
                // cell #14 / A9). Wrap as an Expr-bodied boundary.
                if let Some(value) = &prop.value {
                    walk_closure_boundary(
                        prop.span,
                        &[],
                        ArrowOrFnBody::Expr(value),
                        class_self_name,
                        active,
                        out,
                    );
                }
            }
            ast::ClassMember::PrivateProp(prop) => {
                if let Some(value) = &prop.value {
                    walk_closure_boundary(
                        prop.span,
                        &[],
                        ArrowOrFnBody::Expr(value),
                        class_self_name,
                        active,
                        out,
                    );
                }
            }
            ast::ClassMember::StaticBlock(sb) => {
                walk_closure_boundary(
                    sb.span,
                    &[],
                    ArrowOrFnBody::Block(&sb.body.stmts),
                    class_self_name,
                    active,
                    out,
                );
            }
            ast::ClassMember::AutoAccessor(acc) => {
                if let Some(value) = &acc.value {
                    walk_closure_boundary(
                        acc.span,
                        &[],
                        ArrowOrFnBody::Expr(value),
                        class_self_name,
                        active,
                        out,
                    );
                }
            }
            ast::ClassMember::TsIndexSignature(_) | ast::ClassMember::Empty(_) => {}
        }
    }
}

fn walk_expr(expr: &ast::Expr, active: &mut HashSet<String>, out: &mut Vec<ClosureCapturePair>) {
    match expr {
        ast::Expr::Arrow(arrow) => {
            let params: Vec<&ast::Pat> = arrow.params.iter().collect();
            let body = match arrow.body.as_ref() {
                ast::BlockStmtOrExpr::BlockStmt(b) => ArrowOrFnBody::Block(&b.stmts),
                ast::BlockStmtOrExpr::Expr(e) => ArrowOrFnBody::Expr(e),
            };
            walk_closure_boundary(arrow.span, &params, body, None, active, out);
        }
        ast::Expr::Fn(fn_expr) => {
            if let Some(body) = &fn_expr.function.body {
                let params: Vec<&ast::Pat> =
                    fn_expr.function.params.iter().map(|p| &p.pat).collect();
                let self_name = fn_expr.ident.as_ref().map(|i| i.sym.as_ref());
                walk_closure_boundary(
                    fn_expr.function.span,
                    &params,
                    ArrowOrFnBody::Block(&body.stmts),
                    self_name,
                    active,
                    out,
                );
            }
        }
        ast::Expr::Class(class_expr) => {
            let self_name = class_expr.ident.as_ref().map(|i| i.sym.as_ref());
            walk_class_body(&class_expr.class.body, self_name, active, out);
        }
        ast::Expr::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ast::PropOrSpread::Spread(s) => walk_expr(&s.expr, active, out),
                    ast::PropOrSpread::Prop(p) => match p.as_ref() {
                        ast::Prop::KeyValue(kv) => walk_expr(&kv.value, active, out),
                        ast::Prop::Assign(a) => walk_expr(&a.value, active, out),
                        ast::Prop::Method(mp) => {
                            if let Some(body) = &mp.function.body {
                                let params: Vec<&ast::Pat> =
                                    mp.function.params.iter().map(|p| &p.pat).collect();
                                walk_closure_boundary(
                                    mp.function.span,
                                    &params,
                                    ArrowOrFnBody::Block(&body.stmts),
                                    None,
                                    active,
                                    out,
                                );
                            }
                        }
                        ast::Prop::Getter(g) => {
                            if let Some(body) = &g.body {
                                walk_closure_boundary(
                                    g.span,
                                    &[],
                                    ArrowOrFnBody::Block(&body.stmts),
                                    None,
                                    active,
                                    out,
                                );
                            }
                        }
                        ast::Prop::Setter(s) => {
                            if let Some(body) = &s.body {
                                let pats: [&ast::Pat; 1] = [&*s.param];
                                walk_closure_boundary(
                                    s.span,
                                    &pats,
                                    ArrowOrFnBody::Block(&body.stmts),
                                    None,
                                    active,
                                    out,
                                );
                            }
                        }
                        ast::Prop::Shorthand(_) => {}
                    },
                }
            }
        }
        ast::Expr::Assign(a) => walk_expr(&a.right, active, out),
        ast::Expr::Update(u) => walk_expr(&u.arg, active, out),
        ast::Expr::Bin(b) => {
            walk_expr(&b.left, active, out);
            walk_expr(&b.right, active, out);
        }
        ast::Expr::Unary(u) => walk_expr(&u.arg, active, out),
        ast::Expr::Cond(c) => {
            walk_expr(&c.test, active, out);
            walk_expr(&c.cons, active, out);
            walk_expr(&c.alt, active, out);
        }
        ast::Expr::Paren(p) => walk_expr(&p.expr, active, out),
        ast::Expr::Seq(s) => {
            for e in &s.exprs {
                walk_expr(e, active, out);
            }
        }
        ast::Expr::Call(c) => {
            if let ast::Callee::Expr(e) = &c.callee {
                walk_expr(e, active, out);
            }
            for a in &c.args {
                walk_expr(&a.expr, active, out);
            }
        }
        ast::Expr::New(n) => {
            walk_expr(&n.callee, active, out);
            if let Some(args) = &n.args {
                for a in args {
                    walk_expr(&a.expr, active, out);
                }
            }
        }
        ast::Expr::Member(m) => {
            walk_expr(&m.obj, active, out);
            if let ast::MemberProp::Computed(c) = &m.prop {
                walk_expr(&c.expr, active, out);
            }
        }
        ast::Expr::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                walk_expr(&elem.expr, active, out);
            }
        }
        ast::Expr::Tpl(t) => {
            for e in &t.exprs {
                walk_expr(e, active, out);
            }
        }
        ast::Expr::TaggedTpl(tt) => {
            walk_expr(&tt.tag, active, out);
            for e in &tt.tpl.exprs {
                walk_expr(e, active, out);
            }
        }
        ast::Expr::Await(a) => walk_expr(&a.arg, active, out),
        ast::Expr::Yield(y) => {
            if let Some(arg) = &y.arg {
                walk_expr(arg, active, out);
            }
        }
        ast::Expr::OptChain(oc) => match &*oc.base {
            ast::OptChainBase::Member(m) => {
                walk_expr(&m.obj, active, out);
                if let ast::MemberProp::Computed(c) = &m.prop {
                    walk_expr(&c.expr, active, out);
                }
            }
            ast::OptChainBase::Call(c) => {
                walk_expr(&c.callee, active, out);
                for a in &c.args {
                    walk_expr(&a.expr, active, out);
                }
            }
        },
        ast::Expr::TsAs(a) => walk_expr(&a.expr, active, out),
        ast::Expr::TsTypeAssertion(a) => walk_expr(&a.expr, active, out),
        ast::Expr::TsNonNull(a) => walk_expr(&a.expr, active, out),
        ast::Expr::TsConstAssertion(a) => walk_expr(&a.expr, active, out),
        ast::Expr::TsSatisfies(a) => walk_expr(&a.expr, active, out),
        ast::Expr::TsInstantiation(a) => walk_expr(&a.expr, active, out),
        ast::Expr::Ident(_)
        | ast::Expr::This(_)
        | ast::Expr::Lit(_)
        | ast::Expr::SuperProp(_)
        | ast::Expr::MetaProp(_)
        | ast::Expr::Invalid(_)
        | ast::Expr::JSXMember(_)
        | ast::Expr::JSXNamespacedName(_)
        | ast::Expr::JSXEmpty(_)
        | ast::Expr::JSXElement(_)
        | ast::Expr::JSXFragment(_)
        | ast::Expr::PrivateName(_) => {}
    }
}

/// Crosses a closure boundary (arrow / fn / method / ctor / static block /
/// getter / setter): computes the boundary-shadow-aware
/// `boundary_active`, classifies each remaining candidate against the
/// closure body for an invalidating reassign, emits matching pairs, and
/// recursively walks the body with `boundary_active` so nested closures
/// inherit the corrected scope.
///
/// `self_name` carries the closure's own name (fn expr / class self) when
/// applicable so it is excluded from `boundary_active` along with params
/// and body top-level decls.
fn walk_closure_boundary(
    closure_span: swc_common::Span,
    params: &[&ast::Pat],
    body: ArrowOrFnBody<'_>,
    self_name: Option<&str>,
    outer_active: &HashSet<String>,
    out: &mut Vec<ClosureCapturePair>,
) {
    let mut boundary_active = outer_active.clone();
    if let Some(name) = self_name {
        boundary_active.remove(name);
    }
    for pat in params {
        remove_pat_idents(pat, &mut boundary_active);
    }
    // Inner body top-level decls (`let` / `const` / `var` / `function` /
    // `class`) shadow outer candidates inside the closure. We pre-remove
    // all of them here so the classify step below sees the correct set.
    // The subsequent `walk_stmts` recursion into the body re-applies
    // fn/class-decl removals through [`walk_decl`] (block-scoped) but
    // those are no-ops on an already-shadowed `boundary_active`.
    if let ArrowOrFnBody::Block(stmts) = body {
        for stmt in stmts {
            match stmt {
                ast::Stmt::Decl(ast::Decl::Var(v)) => {
                    for d in &v.decls {
                        remove_pat_idents(&d.name, &mut boundary_active);
                    }
                }
                ast::Stmt::Decl(ast::Decl::Fn(fn_decl)) => {
                    boundary_active.remove(fn_decl.ident.sym.as_ref());
                }
                ast::Stmt::Decl(ast::Decl::Class(class_decl)) => {
                    boundary_active.remove(class_decl.ident.sym.as_ref());
                }
                _ => {}
            }
        }
    }

    // Emit pairs for the candidates still active after boundary shadow.
    for ident in &boundary_active {
        if matches!(
            classify_closure_body_for_outer_ident(params, body, ident),
            Some(ResetCause::ClosureReassign)
        ) {
            out.push((ident.clone(), closure_span));
        }
    }

    // Recurse into the body so nested closures get their own pairs with
    // correctly inherited active set.
    match body {
        ArrowOrFnBody::Block(stmts) => walk_stmts(stmts, &mut boundary_active, out),
        ArrowOrFnBody::Expr(e) => walk_expr(e, &mut boundary_active, out),
    }
}
