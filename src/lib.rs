//! ts_to_rs library: TypeScript → Rust transpiler.
//!
//! This library provides the core transformation pipeline:
//! TypeScript source → SWC AST → IR → Rust source.

pub mod directory;
pub mod generator;
pub mod ir;
pub mod parser;
pub mod registry;
pub mod transformer;

use anyhow::Result;

use crate::registry::{build_registry, TypeRegistry};

/// Transpiles TypeScript source code to Rust source code.
///
/// Chains the full pipeline: parse → build registry → transform → generate.
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile(ts_source: &str) -> Result<String> {
    let module = parser::parse_typescript(ts_source)?;
    let reg = build_registry(&module);
    let items = transformer::transform_module(&module, &reg)?;
    Ok(generator::generate(&items))
}

/// Transpiles TypeScript source code to Rust source code using a pre-built [`TypeRegistry`].
///
/// Used in directory mode where the registry is constructed from multiple files.
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile_with_registry(ts_source: &str, registry: &TypeRegistry) -> Result<String> {
    let module = parser::parse_typescript(ts_source)?;
    let mut reg = build_registry(&module);
    reg.merge(registry);
    let items = transformer::transform_module(&module, &reg)?;
    Ok(generator::generate(&items))
}
