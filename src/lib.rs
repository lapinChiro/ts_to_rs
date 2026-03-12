//! ts_to_rs library: TypeScript → Rust transpiler.
//!
//! This library provides the core transformation pipeline:
//! TypeScript source → SWC AST → IR → Rust source.

pub mod directory;
pub mod generator;
pub mod ir;
pub mod parser;
pub mod transformer;

use anyhow::Result;

/// Transpiles TypeScript source code to Rust source code.
///
/// Chains the full pipeline: parse → transform → generate.
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile(ts_source: &str) -> Result<String> {
    let module = parser::parse_typescript(ts_source)?;
    let items = transformer::transform_module(&module)?;
    Ok(generator::generate(&items))
}
