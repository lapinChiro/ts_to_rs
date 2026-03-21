//! Core data types for the unified transformation pipeline.

use std::path::{Path, PathBuf};

use crate::ir::RustType;
use crate::registry::TypeRegistry;
use crate::UnsupportedSyntax;

/// A collection of parsed TypeScript files.
///
/// Shared as immutable data across all pipeline passes.
/// Files are parsed once in Pass 0 and reused in subsequent passes.
pub struct ParsedFiles {
    /// The parsed files.
    pub files: Vec<ParsedFile>,
}

/// A single parsed TypeScript file.
pub struct ParsedFile {
    /// The original file path.
    pub path: PathBuf,
    /// The original TypeScript source text (used for error position resolution).
    pub source: String,
    /// The parsed SWC AST.
    pub module: swc_ecma_ast::Module,
}

/// Resolves an import specifier to a file path.
///
/// Different resolution strategies (Node.js, Bundler, Deno) implement this trait.
/// The resolver only handles file-system-level path resolution; it does not
/// understand TypeScript or Rust module semantics.
pub trait ModuleResolver {
    /// Resolves an import specifier to a file path.
    ///
    /// - `from_file`: the file containing the import statement
    /// - `specifier`: the import path (e.g., `"./foo"`, `"../bar"`, `"lodash"`)
    ///
    /// Returns `Some(path)` if resolved to a known file, `None` for external packages.
    fn resolve(&self, from_file: &Path, specifier: &str) -> Option<PathBuf>;
}

/// A module resolver that resolves nothing.
///
/// Used in single-file mode where cross-module imports cannot be resolved.
pub struct NullModuleResolver;

impl ModuleResolver for NullModuleResolver {
    fn resolve(&self, _from_file: &Path, _specifier: &str) -> Option<PathBuf> {
        None
    }
}

/// Input to the unified transpilation pipeline.
pub struct TranspileInput {
    /// TypeScript source files to transpile: `(file_path, source_text)`.
    pub files: Vec<(PathBuf, String)>,
    /// Optional pre-built type registry (e.g., built-in types).
    pub builtin_types: Option<TypeRegistry>,
    /// Module resolver for import path resolution.
    pub module_resolver: Box<dyn ModuleResolver>,
}

/// Output of the unified transpilation pipeline.
pub struct TranspileOutput {
    /// Per-file transpilation results.
    pub files: Vec<FileOutput>,
    // module_graph: ModuleGraph — will be added in P8
    // synthetic_types: SyntheticTypeRegistry — will be added in P8
}

/// The result of type resolution for an expression or variable.
///
/// `Known` indicates the type was successfully resolved.
/// `Unknown` indicates the resolver could not determine the type,
/// and the Transformer should apply fallback heuristics.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedType {
    /// The type was successfully resolved.
    Known(RustType),
    /// The type could not be determined.
    Unknown,
}

/// Transpilation result for a single file.
pub struct FileOutput {
    /// Output file path (`.rs` extension).
    pub path: PathBuf,
    /// Generated Rust source code.
    pub rust_source: String,
    /// Unsupported syntax entries encountered during transformation.
    pub unsupported: Vec<UnsupportedSyntax>,
}
