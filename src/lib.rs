//! ts_to_rs library: TypeScript → Rust transpiler.
//!
//! This library provides the core transformation pipeline:
//! TypeScript source → SWC AST → IR → Rust source.

pub mod directory;
pub mod external_types;
pub mod generator;
pub mod ir;
pub mod parser;
pub mod pipeline;
pub mod registry;
pub mod transformer;

use anyhow::Result;
use serde::Serialize;

use crate::pipeline::module_resolver::TrivialResolver;
use crate::pipeline::TranspileInput;
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
/// Uses the unified pipeline. Any unsupported syntax causes an error.
///
/// # Errors
///
/// Returns an error if parsing or transformation fails, or if unsupported syntax is encountered.
pub fn transpile(ts_source: &str) -> Result<String> {
    let output = run_single_file_pipeline(ts_source, None)?;
    let file = extract_single_output(output)?;
    if let Some(first) = file.unsupported.first() {
        anyhow::bail!(
            "unsupported syntax: {} at byte {}",
            first.kind,
            first.byte_pos
        );
    }
    Ok(file.rust_source)
}

/// Transpiles TypeScript source code, collecting unsupported syntax instead of aborting.
///
/// Returns the Rust source for the supported portions and a list of unsupported syntax entries.
///
/// # Errors
///
/// Returns an error if parsing fails. Unsupported syntax errors are collected, not propagated.
pub fn transpile_collecting(ts_source: &str) -> Result<(String, Vec<UnsupportedSyntax>)> {
    let output = run_single_file_pipeline(ts_source, None)?;
    let file = extract_single_output(output)?;
    let unsupported = file
        .unsupported
        .into_iter()
        .map(|raw| resolve_unsupported(ts_source, raw))
        .collect();
    Ok((file.rust_source, unsupported))
}

/// Resolves an [`UnsupportedSyntaxError`] into an [`UnsupportedSyntax`] with line/col info.
pub fn resolve_unsupported(source: &str, raw: UnsupportedSyntaxError) -> UnsupportedSyntax {
    let (line, col) = byte_pos_to_line_col(source, raw.byte_pos);
    UnsupportedSyntax {
        kind: raw.kind,
        location: format!("{line}:{col}"),
    }
}

/// Runs `rustfmt` on the given files. Prints a warning and continues if `rustfmt` is not available.
pub fn run_rustfmt(paths: &[std::path::PathBuf]) {
    if paths.is_empty() {
        return;
    }
    let result = std::process::Command::new("rustfmt").args(paths).status();
    match result {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("Warning: rustfmt exited with status {status}; output may not be formatted");
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Warning: rustfmt not found; output will not be formatted");
        }
        Err(e) => {
            eprintln!("Warning: failed to run rustfmt: {e}; output may not be formatted");
        }
    }
}

/// Converts a SWC `BytePos` (1-based byte position) to a 1-based (line, col) pair.
pub fn byte_pos_to_line_col(source: &str, byte_pos: u32) -> (usize, usize) {
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

// ===== Internal helpers =====

/// Runs the unified pipeline for a single source file.
fn run_single_file_pipeline(
    ts_source: &str,
    builtin_types: Option<registry::TypeRegistry>,
) -> Result<pipeline::TranspileOutput> {
    let input = TranspileInput {
        files: vec![(std::path::PathBuf::from("input.ts"), ts_source.to_string())],
        builtin_types,
        module_resolver: Box::new(TrivialResolver),
    };
    pipeline::transpile_pipeline(input)
}

/// Extracts the single file output from a pipeline result.
fn extract_single_output(output: pipeline::TranspileOutput) -> Result<pipeline::FileOutput> {
    output
        .files
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("pipeline returned no output files"))
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

    #[test]
    fn test_transpile_collecting_transformer_internal_error_collected() {
        let source = "function foo() { const x = tag`hello`; }";
        let result = transpile_collecting(source);
        assert!(result.is_ok(), "should not be a fatal error: {result:?}");
        let (_output, unsupported) = result.unwrap();
        assert!(
            !unsupported.is_empty(),
            "should report the function as unsupported"
        );
    }

    #[test]
    fn test_transpile_collecting_mixed_supported_and_internal_error() {
        let source = r#"
interface Foo { name: string; }
function bar() { const x = tag`hello`; }
"#;
        let result = transpile_collecting(source);
        assert!(result.is_ok(), "should not be a fatal error: {result:?}");
        let (output, unsupported) = result.unwrap();
        assert!(
            output.contains("struct Foo"),
            "convertible items should still appear in output"
        );
        assert!(
            !unsupported.is_empty(),
            "unconvertible function should be in unsupported list"
        );
    }

    #[test]
    fn test_transpile_collecting_default_param_converts_successfully() {
        let source = "function foo(x: number = 0) { return x; }";
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn foo"),
            "function with default param should be converted, got: {output}"
        );
        assert!(
            unsupported.is_empty(),
            "literal default param should not be unsupported"
        );
    }

    #[test]
    fn test_transpile_collecting_non_nullable_union_param_generates_enum() {
        let source = "export function foo(x: string | number): void { }";
        let (output, _unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn foo"),
            "function should be converted, got: {output}"
        );
        assert!(
            output.contains("x: StringOrF64"),
            "non-nullable union param should reference generated enum, got: {output}"
        );
        assert!(
            output.contains("enum StringOrF64"),
            "generated enum should be in output, got: {output}"
        );
    }

    #[test]
    fn test_transpile_collecting_mixed_param_types_union_generates_enum() {
        let source = "export function bar(a: string, b: string | number): void { }";
        let (output, _unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn bar"),
            "function should be converted, got: {output}"
        );
        assert!(
            output.contains("a: String"),
            "supported param should have normal type, got: {output}"
        );
        assert!(
            output.contains("b: StringOrF64"),
            "non-nullable union param should reference generated enum, got: {output}"
        );
    }

    #[test]
    fn test_transpile_collecting_non_nullable_union_return_type_generates_enum() {
        let source = "export function baz(x: number): string | number { return x; }";
        let (output, _unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn baz"),
            "function should be converted, got: {output}"
        );
        assert!(
            output.contains("-> StringOrF64"),
            "non-nullable union return type should reference generated enum, got: {output}"
        );
    }

    #[test]
    fn test_transpile_collecting_non_nullable_union_no_unsupported_report() {
        let source = "export function foo(x: string | number): void { }";
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(output.contains("fn foo"), "function should be converted");
        assert!(
            unsupported.is_empty(),
            "no unsupported items should be reported, got: {unsupported:?}"
        );
    }

    #[test]
    fn test_transpile_default_non_nullable_union_param_succeeds() {
        let source = "function foo(x: string | number): void { }";
        let result = transpile(source);
        assert!(
            result.is_ok(),
            "non-nullable union param should succeed, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_transpile_collecting_parse_error_still_returns_err() {
        let source = "function {{{";
        let result = transpile_collecting(source);
        assert!(result.is_err(), "parse errors should still propagate");
    }

    #[test]
    fn test_transpile_arrow_untyped_param_falls_back_to_any() {
        let source = "export const f = (c) => c;";
        let result = transpile(source);
        assert!(result.is_ok(), "untyped arrow param should fallback to Any");
        let output = result.unwrap();
        assert!(
            output.contains("serde_json::Value"),
            "untyped param should be serde_json::Value, got: {output}"
        );
    }
}
