//! Per-function-body invocation of the narrowing analyzer.
//!
//! Sits alongside the other [`TypeResolver`] submodules as a thin adapter
//! that runs [`crate::pipeline::narrowing_analyzer::analyze_function`] on
//! each function-like body the resolver visits (function / method /
//! constructor / arrow / function expression) and merges the returned
//! [`AnalysisResult`](crate::pipeline::narrowing_analyzer::AnalysisResult)
//! into [`FileTypeResolution`].
//!
//! Two pieces of analyzer output are merged here:
//!
//! - **`emission_hints`** (T6-1): per-`??=` site shadow-let-vs-`get_or_insert_with`
//!   selection, keyed by `stmt.span.lo.0`.
//! - **`closure_captures`** (T6-2): `NarrowEvent::ClosureCapture` events for
//!   outer idents reassigned by some inner closure body. Used by the
//!   Transformer to suppress narrow shadow-let emission and to apply the
//!   JS `coerce_default` table at narrow-stale read sites.
//!
//! Each function body is analyzed independently: the analyzer's own
//! recursion stops at closure / nested-function / class / var-decl
//! boundaries (see `narrowing_analyzer::recurse_into_nested_stmts`),
//! so every function-like boundary must be analyzed by its own call.
//! The resulting hint map is globally keyed by `stmt.span.lo.0`, which is
//! unique across the file, so merging from multiple calls never conflicts.
//!
//! See `backlog/I-144-control-flow-narrowing-analyzer.md` (T6-1 / T6-2) for
//! the pipeline integration rationale.

use swc_ecma_ast as ast;

use crate::pipeline::narrowing_analyzer;

use super::TypeResolver;

impl<'a> TypeResolver<'a> {
    /// Runs the narrowing analyzer on a function body and records its
    /// `??=` emission hints (T6-1) and closure-capture events (T6-2) into
    /// [`FileTypeResolution`].
    ///
    /// Called once per function-like body by the visitors before the body
    /// is walked statement-by-statement.
    ///
    /// [`FileTypeResolution`]:
    ///     crate::pipeline::type_resolution::FileTypeResolution
    pub(super) fn collect_emission_hints(&mut self, body: &ast::BlockStmt, params: &[&ast::Pat]) {
        let analysis = narrowing_analyzer::analyze_function(body, params);
        self.result.emission_hints.extend(analysis.emission_hints);
        // T6-2: closure-capture events flow through the same `narrow_events`
        // channel as guard-derived narrows. Consumers filter the variant they
        // care about via `NarrowEvent::var_name` / pattern matching.
        // I-169 T6-2 follow-up: `params` enables param-as-candidate detection
        // (P3) and `enclosing_fn_body` (set inside `analyze_function`)
        // enables multi-fn scope isolation (P1).
        self.result.narrow_events.extend(analysis.closure_captures);
    }
}
