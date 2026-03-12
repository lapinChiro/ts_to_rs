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
use serde::Serialize;

use crate::registry::{build_registry, TypeRegistry};
use crate::transformer::UnsupportedSyntaxError;

/// A report entry for an unsupported TypeScript syntax encountered during transformation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UnsupportedSyntax {
    /// The SWC AST node kind (e.g., `"ExportDefaultExpr"`, `"TsModuleDecl"`)
    pub kind: String,
    /// Source location as `"line:col"` (1-based)
    pub location: String,
}

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

/// Transpiles TypeScript source code, collecting unsupported syntax instead of aborting.
///
/// Returns the Rust source for the supported portions and a list of unsupported syntax entries.
///
/// # Errors
///
/// Returns an error if parsing fails. Unsupported syntax errors are collected, not propagated.
pub fn transpile_collecting(ts_source: &str) -> Result<(String, Vec<UnsupportedSyntax>)> {
    transpile_collecting_with_registry(ts_source, &TypeRegistry::new())
}

/// Like [`transpile_collecting`] but with a pre-built [`TypeRegistry`].
///
/// # Errors
///
/// Returns an error if parsing fails. Unsupported syntax errors are collected, not propagated.
pub fn transpile_collecting_with_registry(
    ts_source: &str,
    registry: &TypeRegistry,
) -> Result<(String, Vec<UnsupportedSyntax>)> {
    let module = parser::parse_typescript(ts_source)?;
    let mut reg = build_registry(&module);
    reg.merge(registry);
    let (items, raw_unsupported) = transformer::transform_module_collecting(&module, &reg)?;
    let output = generator::generate(&items);
    let unsupported = raw_unsupported
        .into_iter()
        .map(|raw| resolve_unsupported(ts_source, raw))
        .collect();
    Ok((output, unsupported))
}

/// Resolves an [`UnsupportedSyntaxError`] into an [`UnsupportedSyntax`] with line/col info.
fn resolve_unsupported(source: &str, raw: UnsupportedSyntaxError) -> UnsupportedSyntax {
    let (line, col) = byte_pos_to_line_col(source, raw.byte_pos);
    UnsupportedSyntax {
        kind: raw.kind,
        location: format!("{line}:{col}"),
    }
}

/// Converts a SWC `BytePos` (1-based byte position) to a 1-based (line, col) pair.
fn byte_pos_to_line_col(source: &str, byte_pos: u32) -> (usize, usize) {
    if byte_pos == 0 {
        return (1, 1);
    }
    let offset = (byte_pos as usize).saturating_sub(1);
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if ch == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_pos_to_line_col_first_byte() {
        assert_eq!(byte_pos_to_line_col("hello", 1), (1, 1));
    }

    #[test]
    fn test_byte_pos_to_line_col_second_line() {
        // "abc\ndef" — 'd' is at byte 4 (0-based), BytePos = 5
        assert_eq!(byte_pos_to_line_col("abc\ndef", 5), (2, 1));
    }

    #[test]
    fn test_byte_pos_to_line_col_mid_line() {
        // "abc\ndef" — 'e' is at byte 5 (0-based), BytePos = 6
        assert_eq!(byte_pos_to_line_col("abc\ndef", 6), (2, 2));
    }

    #[test]
    fn test_byte_pos_to_line_col_zero_returns_1_1() {
        assert_eq!(byte_pos_to_line_col("hello", 0), (1, 1));
    }

    #[test]
    fn test_transpile_collecting_all_supported_returns_empty_unsupported() {
        let source = "interface Foo { name: string; }";
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(!output.is_empty());
        assert!(unsupported.is_empty());
    }

    #[test]
    fn test_transpile_collecting_with_unsupported_collects_items() {
        let source = "export default 42;";
        let (_output, unsupported) = transpile_collecting(source).unwrap();
        assert_eq!(unsupported.len(), 1);
        assert_eq!(unsupported[0].kind, "ExportDefaultExpr");
        assert_eq!(unsupported[0].location, "1:1");
    }

    #[test]
    fn test_transpile_collecting_still_generates_supported_output() {
        let source = r#"
interface Foo { name: string; }
export default 42;
"#;
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(output.contains("struct Foo"));
        assert_eq!(unsupported.len(), 1);
    }

    #[test]
    fn test_transpile_default_mode_errors_on_unsupported() {
        let source = "export default 42;";
        let result = transpile(source);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unsupported syntax"));
    }
}
