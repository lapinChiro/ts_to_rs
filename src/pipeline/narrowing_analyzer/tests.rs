//! Unit tests for [`NarrowingAnalyzer`].
//!
//! Coverage targets the Problem Space matrix cells enumerated in
//! `backlog/I-144-control-flow-narrowing-analyzer.md`:
//!
//! - Sub-matrix 2 (LHS × Reset cause): one test per reset class × mutation
//!   shape, verifying the classifier distinguishes narrow-preserving resets
//!   (arithmetic / update / `??=`-on-narrow / pass-by-mutation / method call
//!   on x) from true resets (direct / null assign / logical compound /
//!   closure reassign / loop boundary).
//! - Sub-matrix 3/5 (`??=` emission strategy): the hint output at each
//!   `??=` site follows the matrix — `ShadowLet` when the narrow survives
//!   the remaining block, `GetOrInsertWith` when a following mutation
//!   invalidates it.
//! - Traversal partitions: nested blocks, nested `if` / loop / switch /
//!   try-catch-finally bodies, closures (arrow / fn / nested fn decl /
//!   class method / ctor / prop init / static block / object method /
//!   getter / setter), closure-local shadowing vs outer reassign.
//! - Branch-merge semantics: `if` cons/alt, `switch` multi-case, try/catch
//!   alternative paths — an invalidating reset in *any* branch is detected.

use swc_ecma_ast as ast;

use super::*;
use crate::parser::parse_typescript;

/// Parses a TypeScript source snippet and runs the analyzer against the body
/// of the **first** function declaration at the top level.
fn analyze_first_fn(source: &str) -> AnalysisResult {
    let module = parse_typescript(source).expect("fixture must parse");
    let fn_body = find_first_fn_body(&module).expect("fixture must declare a function");
    NarrowingAnalyzer::new().analyze_function(fn_body)
}

fn find_first_fn_body(module: &ast::Module) -> Option<&ast::BlockStmt> {
    for item in &module.body {
        if let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = item {
            return fn_decl.function.body.as_ref();
        }
    }
    None
}

/// Returns the single emission hint produced by the analyzer, panicking if
/// exactly one `??=` hint is not present.
fn single_hint(result: &AnalysisResult) -> EmissionHint {
    assert_eq!(
        result.emission_hints.len(),
        1,
        "expected exactly one `??=` hint, got {:?}",
        result.emission_hints
    );
    *result.emission_hints.values().next().unwrap()
}

/// Parses a single-function fixture, runs the analyzer, and asserts that
/// the **single** produced emission hint matches `expected`.
///
/// Eliminates the 3-line boilerplate pattern that appeared in the vast
/// majority of hint tests:
///
/// ```ignore
/// let r = analyze_first_fn(source);
/// assert_eq!(single_hint(&r), expected);
/// ```
fn assert_hint(source: &str, expected: EmissionHint) {
    let r = analyze_first_fn(source);
    assert_eq!(
        single_hint(&r),
        expected,
        "fixture:\n{source}\nhints: {:?}",
        r.emission_hints
    );
}

/// Parses a single-function fixture, runs the analyzer, and asserts that
/// **no** emission hints are produced (empty map).
fn assert_no_hint(source: &str) {
    let r = analyze_first_fn(source);
    assert!(
        r.emission_hints.is_empty(),
        "expected no `??=` hints; fixture:\n{source}\nhints: {:?}",
        r.emission_hints
    );
}

// -----------------------------------------------------------------------------
// Test module groupings (split for per-file cohesion and line-count compliance)
// -----------------------------------------------------------------------------

mod closures;
mod hints_flat;
mod hints_nested;
mod scope_and_exprs;
mod types_and_combinators;
