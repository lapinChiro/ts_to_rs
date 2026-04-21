//! CFG-based narrowing analyzer (I-144).
//!
//! Provides a **scope-aware, branch-merging** classifier for narrowing-related
//! events so the Transformer can select between shadow-let and
//! `get_or_insert_with` (E1 vs E2a) at `??=` sites without false-positive
//! resets, without missing closure-captured reassigns, and without the
//! short-circuit branch-union bug that afflicts the legacy scanner.
//!
//! # Module layout
//!
//! The implementation is split by concern into private submodules:
//!
//! - **`events`** — data-type backbone ([`NarrowEvent`], [`NarrowTrigger`],
//!   [`PrimaryTrigger`], [`NullCheckKind`], [`ResetCause`], [`EmissionHint`]).
//!   No behavior.
//! - **`classifier`** — stateless functions that walk the AST and classify
//!   mutations, merge branch outcomes, and descend into closure /
//!   class / object boundaries.
//! - **`guards`** — narrow guard detection (`typeof` / `instanceof` /
//!   null check / truthy + early-return complement) producing
//!   [`NarrowEvent::Narrow`] through a [`NarrowTypeContext`] callback.
//!   Entry points [`detect_narrowing_guard`] /
//!   [`detect_early_return_narrowing`] are re-exported here.
//! - **`type_context`** — [`NarrowTypeContext`] trait the guard detector
//!   uses to read declared types, introspect synthetic enums, register
//!   complement sub-unions, and emit events.
//! - This file ([`analyze_function`], [`AnalysisResult`]) — the free
//!   function that discovers `??=` sites, dispatches to the classifier,
//!   and records per-site [`EmissionHint`]s.
//!
//! All event types and the guard-detection entry points are re-exported
//! here so downstream callers use the single
//! `use crate::pipeline::narrowing_analyzer::{NarrowEvent, ...};`
//! import site they always used.
//!
//! # Task-stage boundaries
//!
//! This module is built in stages aligned with the I-144 task list:
//!
//! - **T3 + T4**: Event type definitions + the `??=` emission-hint
//!   classifier. The classifier:
//!   * **Merges** branch outcomes (`if` cons/alt, `switch` cases,
//!     `try`/catch) so an invalidating reset in *any* branch is detected.
//!   * **Passes through** narrow-preserving causes (`+=`, `x++`, `??=` on
//!     narrow) regardless of enclosing loop/scope (PRD Sub-matrix 4 F4).
//!   * **Descends** into every callable capture boundary: arrow / fn expr /
//!     nested `function` decl / class (method / ctor / prop init / static
//!     block) / object method / getter / setter. All outer-ident mutations
//!     inside such bodies are classified as [`ResetCause::ClosureReassign`].
//!   * **Respects scope shadowing**: closure parameters, for-init `let`,
//!     `for-of` / `for-in` `let` / `const` heads, and block-level
//!     `let` / `const` / `var` / `function` / `class` declarations all
//!     shadow the target ident. Classification of the outer narrow stops at
//!     the shadow boundary.
//!
//! - **T5**: Narrow guard detection migrated from the (now deleted)
//!   `type_resolver/narrowing.rs` module into the `guards` submodule.
//!   The [`TypeResolver`](crate::pipeline::type_resolver::TypeResolver)
//!   pipeline calls [`detect_narrowing_guard`] /
//!   [`detect_early_return_narrowing`] directly via the
//!   [`NarrowTypeContext`] trait; events flow to
//!   `FileTypeResolution::narrow_events` through
//!   [`NarrowTypeContext::push_narrow_event`].
//!
//! - **T6**: The legacy `pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts`
//!   transformer scanner is retired. `Transformer::try_convert_nullish_assign_stmt`
//!   consults [`AnalysisResult::emission_hints`] (populated by
//!   [`TypeResolver::collect_emission_hints`](crate::pipeline::type_resolver::TypeResolver))
//!   to pick between [`EmissionHint::ShadowLet`] (E1) and
//!   [`EmissionHint::GetOrInsertWith`] (E2a) at each `??=` site.
//!
//! # Design reference
//!
//! I-144 PRD (closed 2026-04-21, archived in git history as
//! `backlog/I-144-control-flow-narrowing-analyzer.md`) and its settled
//! design decisions live in [`doc/handoff/design-decisions.md`] under
//! section "Control-flow narrowing analyzer (I-144)": Problem Space
//! matrix (Sub-matrix 1-5), Phase 3b closure reassign emission policy,
//! 2-channel architecture (NarrowEvent vs EmissionHint vs du_analysis),
//! coerce_default table.
//!
//! [`doc/handoff/design-decisions.md`]: ../../doc/handoff/design-decisions.md

mod classifier;
mod closure_captures;
mod events;
mod guards;
mod type_context;

use std::collections::HashMap;

use swc_ecma_ast as ast;

// Re-export the event-type backbone so downstream modules preserve their
// existing `use crate::pipeline::narrowing_analyzer::{NarrowEvent, ...};`
// import path.
pub use events::{
    EmissionHint, NarrowEvent, NarrowEventRef, NarrowTrigger, NullCheckKind, PrimaryTrigger,
    ResetCause,
};
pub use guards::{detect_early_return_narrowing, detect_narrowing_guard};
pub use type_context::NarrowTypeContext;

// -----------------------------------------------------------------------------
// `??=` emission-hint analysis (stateless)
// -----------------------------------------------------------------------------

/// Result of [`analyze_function`].
///
/// Carries `??=` emission hints (T6-1) and closure-capture events (T6-2).
/// Other narrow events from guard detection flow straight into
/// `FileTypeResolution::narrow_events` through
/// [`NarrowTypeContext::push_narrow_event`] and do not pass through this
/// struct.
#[derive(Debug, Default)]
pub struct AnalysisResult {
    /// Emission strategy hints keyed by `??=` statement start position
    /// (`stmt.span.lo.0`).
    ///
    /// Consumed by `Transformer::try_convert_nullish_assign_stmt` at T6.
    pub emission_hints: HashMap<u32, EmissionHint>,
    /// `NarrowEvent::ClosureCapture` events derived by walking the function
    /// body for closures that reassign outer idents (T6-2).
    ///
    /// Consumed by `Transformer` to suppress narrow shadow-let emission for
    /// captured idents and to coerce subsequent reads via the JS
    /// `coerce_default` table.
    pub closure_captures: Vec<NarrowEvent>,
}

/// Analyzes a function body, returning `??=` emission hints and closure
/// capture events.
///
/// `params` carries the function's parameter patterns so they participate
/// in the outer candidate set for closure-capture detection (I-169 P3
/// fix: param-declared vars are first-class candidates alongside body
/// top-level decls).
///
/// Stateless: consumers call this as a free function per function
/// body without instantiating any analyzer.
#[must_use]
pub fn analyze_function(body: &ast::BlockStmt, params: &[&ast::Pat]) -> AnalysisResult {
    let mut result = AnalysisResult::default();
    analyze_stmt_list(&body.stmts, &mut result);
    // I-169 T6-2 follow-up: collect closure-capture events with scope-aware
    // candidate enumeration + walker shadow tracking. `enclosing_fn_body`
    // is set to the function body's span so downstream
    // `is_var_closure_reassigned(name, position)` queries filter events by
    // function-scope membership (multi-fn isolation P1).
    let candidates = closure_captures::collect_outer_candidates(params, &body.stmts);
    let captured =
        closure_captures::collect_closure_capture_pairs_for_candidates(&body.stmts, &candidates);
    let enclosing = crate::pipeline::type_resolution::Span::from_swc(body.span);
    result.closure_captures = captured
        .into_iter()
        .map(|var_name| NarrowEvent::ClosureCapture {
            var_name,
            enclosing_fn_body: enclosing,
        })
        .collect();
    result
}

/// Walks a statement list, producing per-`??=` emission hints.
///
/// For each `??=` on an Ident LHS, scans the following siblings in the
/// same scope and classifies the first invalidating reset (if any) to
/// pick between [`EmissionHint::ShadowLet`] and
/// [`EmissionHint::GetOrInsertWith`].
///
/// Recurses through nested control-flow blocks (`if` / loop / switch /
/// try / labeled / with) so `??=` inside them also receives a hint.
/// Bodies are normalized via
/// [`classifier::body_as_stmt_list`] so brace-less single-stmt bodies
/// (`if (flag) x ??= 10;`) are handled uniformly with braced bodies.
fn analyze_stmt_list(stmts: &[ast::Stmt], result: &mut AnalysisResult) {
    for (i, stmt) in stmts.iter().enumerate() {
        recurse_into_nested_stmts(stmt, result);
        if let Some((ident_name, span)) = classifier::extract_nullish_assign_ident_stmt(stmt) {
            let hint = classify_nullish_assign(ident_name, &stmts[i + 1..]);
            result.emission_hints.insert(span.lo.0, hint);
        }
    }
}

/// Recurses into nested same-scope blocks so `??=` inside them also
/// gets a hint.
///
/// This function intentionally stops at **closure / function / class /
/// var-decl boundaries**: those are separate scopes whose `??=` sites
/// should be analyzed by a separate [`analyze_function`] invocation
/// (the per-function contract). Only same-scope nested stmts are descended.
fn recurse_into_nested_stmts(stmt: &ast::Stmt, result: &mut AnalysisResult) {
    use classifier::body_as_stmt_list;
    match stmt {
        ast::Stmt::Block(block) => analyze_stmt_list(&block.stmts, result),
        ast::Stmt::If(if_stmt) => {
            analyze_stmt_list(body_as_stmt_list(&if_stmt.cons), result);
            if let Some(alt) = &if_stmt.alt {
                analyze_stmt_list(body_as_stmt_list(alt), result);
            }
        }
        ast::Stmt::While(w) => analyze_stmt_list(body_as_stmt_list(&w.body), result),
        ast::Stmt::DoWhile(d) => analyze_stmt_list(body_as_stmt_list(&d.body), result),
        ast::Stmt::For(f) => analyze_stmt_list(body_as_stmt_list(&f.body), result),
        ast::Stmt::ForOf(fo) => analyze_stmt_list(body_as_stmt_list(&fo.body), result),
        ast::Stmt::ForIn(fi) => analyze_stmt_list(body_as_stmt_list(&fi.body), result),
        ast::Stmt::Switch(sw) => {
            for case in &sw.cases {
                analyze_stmt_list(&case.cons, result);
            }
        }
        ast::Stmt::Try(t) => {
            analyze_stmt_list(&t.block.stmts, result);
            if let Some(handler) = &t.handler {
                analyze_stmt_list(&handler.body.stmts, result);
            }
            if let Some(finalizer) = &t.finalizer {
                analyze_stmt_list(&finalizer.stmts, result);
            }
        }
        ast::Stmt::Labeled(l) => analyze_stmt_list(body_as_stmt_list(&l.body), result),
        ast::Stmt::With(w) => analyze_stmt_list(body_as_stmt_list(&w.body), result),
        _ => {}
    }
}

/// Selects the emission hint for a `??=` on `ident` given the statements
/// that follow it in the same block.
///
/// Preserves shadow-let when no invalidating reset follows; otherwise
/// falls back to `get_or_insert_with` so `x` remains `Option<T>` and can
/// still accept re-nullification, closure reassign, or other mutation
/// that would break a shadow-let binding.
fn classify_nullish_assign(ident: &str, remaining: &[ast::Stmt]) -> EmissionHint {
    match classifier::classify_reset_in_stmts(remaining, ident) {
        Some(cause) if cause.invalidates_narrow() => EmissionHint::GetOrInsertWith,
        _ => EmissionHint::ShadowLet,
    }
}

#[cfg(test)]
mod tests;
