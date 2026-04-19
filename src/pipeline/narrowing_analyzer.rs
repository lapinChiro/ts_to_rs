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
//! The implementation is split by concern into three private submodules:
//!
//! - **`events`** — data-type backbone ([`NarrowEvent`], [`NarrowTrigger`],
//!   [`PrimaryTrigger`], [`NullCheckKind`], [`ResetCause`], [`EmissionHint`],
//!   [`RcContext`]). No behavior.
//! - **`classifier`** — stateless functions that walk the AST and classify
//!   mutations, merge branch outcomes, and descend into closure /
//!   class / object boundaries.
//! - This file ([`NarrowingAnalyzer`], [`AnalysisResult`]) — the public
//!   API that orchestrates discovery of `??=` sites, dispatches to the
//!   classifier, and records per-site [`EmissionHint`]s.
//!
//! All event types are re-exported here so downstream callers use the
//! single `use crate::pipeline::narrowing_analyzer::{NarrowEvent, ...};`
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
//! - **T5**: `type_resolver/narrowing.rs::detect_narrowing_guard` delegates
//!   detection to this module.
//!
//! - **T6-T7**: `pre_check_narrowing_reset` / `has_narrowing_reset_in_stmts`
//!   are retired; call sites consult [`AnalysisResult::emission_hints`].
//!
//! # Design reference
//!
//! `backlog/I-144-control-flow-narrowing-analyzer.md` — Problem Space
//! matrix, Sub-matrix 1-5, Phase 3b closure reassign emission policy.

mod classifier;
mod events;

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::ir::RustType;

// Re-export the event-type backbone so downstream modules preserve their
// existing `use crate::pipeline::narrowing_analyzer::{NarrowEvent, ...};`
// import path.
pub use events::{
    EmissionHint, NarrowEvent, NarrowEventRef, NarrowTrigger, NullCheckKind, PrimaryTrigger,
    RcContext, ResetCause,
};

// -----------------------------------------------------------------------------
// Analyzer
// -----------------------------------------------------------------------------

/// Result of analyzing a function body.
#[derive(Debug, Default, Clone)]
pub struct AnalysisResult {
    /// Narrow events in source order.
    ///
    /// Populated progressively across T3-T6: the T3/T4 slice leaves this
    /// empty (hint-only); T5 adds `Narrow` events migrated from
    /// `detect_narrowing_guard`; T6 adds `Reset` / `ClosureCapture` events.
    pub events: Vec<NarrowEvent>,
    /// Emission strategy hints keyed by `??=` statement start position
    /// (`stmt.span.lo.0`).
    ///
    /// Consumed by `Transformer::try_convert_nullish_assign_stmt` at T6.
    pub emission_hints: HashMap<u32, EmissionHint>,
}

/// Classifies narrowing events and emission hints over a function body.
///
/// See the module-level documentation for the stage-by-stage buildout.
pub struct NarrowingAnalyzer {
    /// Declared / resolved types of variables visible to the analysis.
    ///
    /// Populated by T5 when the analyzer is wired into the TypeResolver
    /// pipeline and begins tracking per-function variable types. Currently
    /// consulted only by the type-aware trigger classification path
    /// (typeof / instanceof) which T5 will introduce.
    var_types: HashMap<String, RustType>,
}

impl NarrowingAnalyzer {
    /// Creates an analyzer with no known variable types.
    ///
    /// Suitable for the T3/T4 `??=` emission-hint slice, which is purely
    /// structural. Downstream stages (T5) will call [`Self::with_var_types`]
    /// to populate type information.
    #[must_use]
    pub fn new() -> Self {
        Self {
            var_types: HashMap::new(),
        }
    }

    /// Creates an analyzer seeded with variable type information.
    #[must_use]
    pub fn with_var_types(var_types: HashMap<String, RustType>) -> Self {
        Self { var_types }
    }

    /// Declared type of `name`, if seeded via [`Self::with_var_types`].
    #[must_use]
    pub fn var_type(&self, name: &str) -> Option<&RustType> {
        self.var_types.get(name)
    }

    /// Analyzes a function body, returning events + emission hints.
    #[must_use]
    pub fn analyze_function(&self, body: &ast::BlockStmt) -> AnalysisResult {
        let mut result = AnalysisResult::default();
        self.analyze_stmt_list(&body.stmts, &mut result);
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
    fn analyze_stmt_list(&self, stmts: &[ast::Stmt], result: &mut AnalysisResult) {
        for (i, stmt) in stmts.iter().enumerate() {
            self.recurse_into_nested_stmts(stmt, result);
            if let Some((ident_name, span)) = classifier::extract_nullish_assign_ident_stmt(stmt) {
                let hint = self.classify_nullish_assign(ident_name, &stmts[i + 1..]);
                result.emission_hints.insert(span.lo.0, hint);
            }
        }
    }

    /// Recurses into nested same-scope blocks so `??=` inside them also
    /// gets a hint.
    ///
    /// This method intentionally stops at **closure / function / class /
    /// var-decl boundaries**: those are separate scopes whose `??=` sites
    /// should be analyzed by a separate `analyze_function` invocation (the
    /// per-function contract). Only same-scope nested stmts are descended.
    fn recurse_into_nested_stmts(&self, stmt: &ast::Stmt, result: &mut AnalysisResult) {
        use classifier::body_as_stmt_list;
        match stmt {
            ast::Stmt::Block(block) => self.analyze_stmt_list(&block.stmts, result),
            ast::Stmt::If(if_stmt) => {
                self.analyze_stmt_list(body_as_stmt_list(&if_stmt.cons), result);
                if let Some(alt) = &if_stmt.alt {
                    self.analyze_stmt_list(body_as_stmt_list(alt), result);
                }
            }
            ast::Stmt::While(w) => self.analyze_stmt_list(body_as_stmt_list(&w.body), result),
            ast::Stmt::DoWhile(d) => self.analyze_stmt_list(body_as_stmt_list(&d.body), result),
            ast::Stmt::For(f) => self.analyze_stmt_list(body_as_stmt_list(&f.body), result),
            ast::Stmt::ForOf(fo) => self.analyze_stmt_list(body_as_stmt_list(&fo.body), result),
            ast::Stmt::ForIn(fi) => self.analyze_stmt_list(body_as_stmt_list(&fi.body), result),
            ast::Stmt::Switch(sw) => {
                for case in &sw.cases {
                    self.analyze_stmt_list(&case.cons, result);
                }
            }
            ast::Stmt::Try(t) => {
                self.analyze_stmt_list(&t.block.stmts, result);
                if let Some(handler) = &t.handler {
                    self.analyze_stmt_list(&handler.body.stmts, result);
                }
                if let Some(finalizer) = &t.finalizer {
                    self.analyze_stmt_list(&finalizer.stmts, result);
                }
            }
            ast::Stmt::Labeled(l) => self.analyze_stmt_list(body_as_stmt_list(&l.body), result),
            ast::Stmt::With(w) => self.analyze_stmt_list(body_as_stmt_list(&w.body), result),
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
    fn classify_nullish_assign(&self, ident: &str, remaining: &[ast::Stmt]) -> EmissionHint {
        match classifier::classify_reset_in_stmts(remaining, ident) {
            Some(cause) if cause.invalidates_narrow() => EmissionHint::GetOrInsertWith,
            _ => EmissionHint::ShadowLet,
        }
    }
}

impl Default for NarrowingAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
