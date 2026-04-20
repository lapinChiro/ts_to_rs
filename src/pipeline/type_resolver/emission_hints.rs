//! Per-function-body invocation of the narrowing analyzer's `??=` emission
//! hint pass.
//!
//! Sits alongside the other [`TypeResolver`] submodules as a thin adapter
//! that runs [`crate::pipeline::narrowing_analyzer::analyze_function`] on
//! each function-like body the resolver visits (function / method /
//! constructor / arrow / function expression) and merges the returned
//! [`AnalysisResult::emission_hints`](crate::pipeline::narrowing_analyzer::AnalysisResult)
//! into [`FileTypeResolution::emission_hints`].
//!
//! Each function body is analyzed independently: the analyzer's own
//! recursion stops at closure / nested-function / class / var-decl
//! boundaries (see `narrowing_analyzer::recurse_into_nested_stmts`),
//! so every function-like boundary must be analyzed by its own call.
//! The resulting hint map is globally keyed by `stmt.span.lo.0`, which is
//! unique across the file, so merging from multiple calls never conflicts.
//!
//! See `backlog/I-144-control-flow-narrowing-analyzer.md` (T6-1) for the
//! pipeline integration rationale.

use swc_ecma_ast as ast;

use crate::pipeline::narrowing_analyzer;

use super::TypeResolver;

impl<'a> TypeResolver<'a> {
    /// Runs the narrowing analyzer on a function body and records its
    /// `??=` emission hints into [`FileTypeResolution::emission_hints`].
    ///
    /// Called once per function-like body by the visitors before the body
    /// is walked statement-by-statement.
    ///
    /// [`FileTypeResolution::emission_hints`]:
    ///     crate::pipeline::type_resolution::FileTypeResolution::emission_hints
    pub(super) fn collect_emission_hints(&mut self, body: &ast::BlockStmt) {
        let analysis = narrowing_analyzer::analyze_function(body);
        self.result.emission_hints.extend(analysis.emission_hints);
    }
}
